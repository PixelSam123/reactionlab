use std::collections::{HashMap, HashSet};
use std::sync::mpsc::TryRecvError;
use std::time::Instant;

use chrono::Local;
use eframe::egui::ColorImage;
use rand::RngExt;

use super::extraction::{VideoSegment, extract_segment};
use super::preload::{PreloadEvent, PreloadHandle, VideoPlan, spawn_preload};
use super::types::{
    AppScreen, FalseClickAction, GroupConfig, RoundOutcome, RoundResult, RunData, RunSettings,
    VideoConfig,
};

/// In-flight pre-extraction of every video in the selected group.
pub struct PreloadState {
    pub handle: PreloadHandle,
    /// Videos planned for extraction; indexed like the worker's plan indices.
    pub pool: Vec<VideoConfig>,
    pub total: usize,
    pub processed: usize,
    /// Plan indices whose segments decoded successfully.
    succeeded: HashSet<usize>,
    /// (video path, ffmpeg error) for videos excluded from the round pool.
    failures: Vec<(String, String)>,
}

impl PreloadState {
    pub fn progress_fraction(&self) -> f32 {
        if self.total == 0 {
            return 1.0;
        }
        (self.processed as f32 / self.total as f32).clamp(0.0, 1.0)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Phase {
    PreWait,
    Waiting,
    PlayingToClick,
    ClickPending,
    PostClick,
    Result,
}

pub struct AppState {
    pub screen: AppScreen,
    pub config: GroupConfig,
    pub run_settings: RunSettings,
    pub selected_group: Option<usize>,
    pub current_round: usize,
    pub attempt_results: Vec<RoundResult>,
    pub phase: Phase,
    pub phase_start: Option<Instant>,
    pub appearance_wait_ms: f64,
    pub current_video: Option<VideoConfig>,
    pub pre_wait_seg: Option<VideoSegment>,
    pub to_click_seg: Option<VideoSegment>,
    pub post_click_seg: Option<VideoSegment>,
    pub click_point_ms: f64,
    pub click_point_start: Option<Instant>,
    pub pending_result: Option<RoundResult>,
    pub last_result: Option<RoundResult>,
    pub last_error: Option<String>,
    pub segment_cache: HashMap<String, VideoSegment>,
    /// Videos available for random round selection during the current run.
    /// `None` until preloading finishes; falls back to validated group videos.
    pub round_pool: Option<Vec<VideoConfig>>,
    pub preload: Option<PreloadState>,
    pub history_means: Vec<(chrono::NaiveDateTime, f64)>,
    display_idx: usize,
}

impl AppState {
    pub fn new(config: GroupConfig, history_means: Vec<(chrono::NaiveDateTime, f64)>) -> Self {
        Self {
            screen: AppScreen::Start,
            config,
            run_settings: RunSettings::default(),
            selected_group: None,
            current_round: 0,
            attempt_results: Vec::new(),
            phase: Phase::Result,
            phase_start: None,
            appearance_wait_ms: 0.0,
            current_video: None,
            pre_wait_seg: None,
            to_click_seg: None,
            post_click_seg: None,
            click_point_ms: 0.0,
            click_point_start: None,
            pending_result: None,
            last_result: None,
            last_error: None,
            segment_cache: HashMap::new(),
            round_pool: None,
            preload: None,
            history_means,
            display_idx: 0,
        }
    }

    pub fn go_to_start(&mut self) {
        self.reset_round_data();
        self.screen = AppScreen::Start;
    }

    pub fn reset_to_start(&mut self) {
        self.reset_round_data();
        self.screen = AppScreen::Start;
    }

    fn reset_round_data(&mut self) {
        if let Some(preload) = self.preload.take() {
            preload.handle.request_cancel();
        }
        self.current_round = 0;
        self.attempt_results.clear();
        self.phase = Phase::Result;
        self.phase_start = None;
        self.appearance_wait_ms = 0.0;
        self.current_video = None;
        self.pre_wait_seg = None;
        self.to_click_seg = None;
        self.post_click_seg = None;
        self.click_point_ms = 0.0;
        self.click_point_start = None;
        self.pending_result = None;
        self.last_result = None;
        self.last_error = None;
        self.round_pool = None;
    }

    /// Begin a run on the group at `group_index` by pre-extracting every
    /// valid video's segments; the first round starts once loading completes.
    pub fn start_run(&mut self, group_index: usize) {
        self.reset_round_data();
        self.selected_group = Some(group_index);
        let Some(group) = self.config.groups.get(group_index) else {
            self.last_error = Some("Selected group is missing".to_string());
            self.screen = AppScreen::Start;
            return;
        };
        let pool: Vec<VideoConfig> = group.usable_videos().cloned().collect();
        if pool.is_empty() {
            self.last_error = Some("Selected group has no usable videos".to_string());
            self.screen = AppScreen::Start;
            return;
        }
        let plans: Vec<VideoPlan> = pool
            .iter()
            .enumerate()
            .map(|(index, video)| VideoPlan {
                index,
                video: video.clone(),
            })
            .collect();
        let total = pool.len();
        self.preload = Some(PreloadState {
            handle: spawn_preload(plans),
            pool,
            total,
            processed: 0,
            succeeded: HashSet::new(),
            failures: Vec::new(),
        });
        self.screen = AppScreen::Preparing;
    }

    /// Drain finished preload work into the segment cache, advancing the
    /// progress counters; starts the run when every planned video is done.
    pub fn poll_preload(&mut self) {
        if self.preload.is_none() {
            return;
        }
        let mut events = Vec::new();
        let mut disconnected = false;
        if let Some(preload) = self.preload.as_mut() {
            loop {
                match preload.handle.events.try_recv() {
                    Ok(event) => events.push(event),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
        }
        for event in events {
            match event {
                PreloadEvent::Segments { index, items } => {
                    for (key, segment) in items {
                        self.segment_cache.insert(key, segment);
                    }
                    if let Some(preload) = self.preload.as_mut() {
                        preload.succeeded.insert(index);
                    }
                }
                PreloadEvent::VideoFailed { index, error } => {
                    if let Some(preload) = self.preload.as_mut()
                        && let Some(video) = preload.pool.get(index)
                    {
                        preload.failures.push((video.path.clone(), error));
                    }
                }
                PreloadEvent::VideoDone => {
                    if let Some(preload) = self.preload.as_mut() {
                        preload.processed += 1;
                    }
                }
            }
        }
        let finished = disconnected
            || self
                .preload
                .as_ref()
                .is_some_and(|preload| preload.processed >= preload.total);
        if finished {
            self.finish_preload();
        }
    }

    /// Stop any running preload job and return to the Start screen.
    pub fn cancel_preload(&mut self) {
        if let Some(preload) = self.preload.as_ref() {
            preload.handle.request_cancel();
        }
        self.go_to_start();
    }

    fn finish_preload(&mut self) {
        let Some(preload) = self.preload.take() else {
            return;
        };
        let mut pool = Vec::new();
        for (index, video) in preload.pool.iter().enumerate() {
            if preload.succeeded.contains(&index) {
                pool.push(video.clone());
            }
        }
        if pool.is_empty() {
            let detail = preload
                .failures
                .iter()
                .map(|(path, error)| format!("{path}: {error}"))
                .collect::<Vec<_>>()
                .join("\n");
            self.last_error = Some(if detail.is_empty() {
                "No videos could be prepared.".to_string()
            } else {
                format!("No videos could be prepared.\n{detail}")
            });
            self.screen = AppScreen::Start;
            return;
        }
        self.round_pool = Some(pool);
        self.screen = AppScreen::Round;
        if let Err(error) = self.start_new_round() {
            self.last_error = Some(error);
            self.screen = AppScreen::Start;
        }
    }

    pub fn start_new_round(&mut self) -> Result<(), String> {
        let group_index = self.selected_group.ok_or("No group selected")?;
        let group = self
            .config
            .groups
            .get(group_index)
            .ok_or("Selected group is missing")?;
        // Prefer the run's preloaded pool (decode failures already removed);
        // fall back to validated group videos when no preload happened.
        let usable: Vec<VideoConfig> = match self.round_pool.as_ref() {
            Some(pool) => pool.clone(),
            None => group.usable_videos().cloned().collect(),
        };
        if usable.is_empty() {
            return Err("Selected group has no usable videos".to_string());
        }
        let mut rng = rand::rng();
        let video = usable[rng.random_range(0..usable.len())].clone();
        let fps = video.effective_fps();
        let bounds = video.segments_ms();

        self.pre_wait_seg = match bounds.pre_wait {
            Some((start, end)) => Some(self.ensure_segment(&video.path, start, end, fps)?),
            None => None,
        };
        self.to_click_seg =
            Some(self.ensure_segment(&video.path, bounds.to_click.0, bounds.to_click.1, fps)?);
        self.post_click_seg = match bounds.post_click {
            Some((start, end)) => Some(self.ensure_segment(&video.path, start, end, fps)?),
            None => None,
        };
        self.current_video = Some(video);
        self.click_point_ms = self
            .to_click_seg
            .as_ref()
            .map_or(0.0, |segment| segment.duration_ms);

        let min_wait = self
            .run_settings
            .min_wait_ms
            .min(self.run_settings.max_wait_ms) as f64;
        let max_wait = self
            .run_settings
            .min_wait_ms
            .max(self.run_settings.max_wait_ms) as f64;
        self.appearance_wait_ms = rng.random_range(min_wait..=max_wait);

        self.display_idx = 0;
        let now = Instant::now();
        if self.pre_wait_seg.is_some() {
            self.set_phase(Phase::PreWait, now);
        } else {
            self.set_phase(Phase::Waiting, now);
        }
        Ok(())
    }

    fn ensure_segment(
        &mut self,
        path: &str,
        start_ms: f64,
        end_ms: f64,
        fps: f64,
    ) -> Result<VideoSegment, String> {
        let key = segment_key(path, start_ms, end_ms);
        if let Some(segment) = self.segment_cache.get(&key) {
            return Ok(segment.clone());
        }
        let segment = extract_segment(path, start_ms, end_ms, fps)?;
        self.segment_cache.insert(key, segment.clone());
        Ok(segment)
    }

    pub fn update_at(&mut self, now: Instant, clicked: bool) -> Option<RunData> {
        if self.screen != AppScreen::Round {
            return None;
        }
        match self.phase {
            Phase::PreWait => {
                let Some(segment) = self.pre_wait_seg.as_ref() else {
                    self.set_phase(Phase::Waiting, now);
                    return None;
                };
                let elapsed = phase_elapsed(now, self.phase_start);
                self.display_idx = frame_for_elapsed(segment, elapsed);
                if clicked {
                    self.record_result(RoundOutcome::TooSoon, None);
                    return None;
                }
                if elapsed >= segment.duration_ms {
                    self.display_idx = segment.frames.len() - 1;
                    self.set_phase(Phase::Waiting, now);
                }
            }
            Phase::Waiting => {
                self.display_idx = 0;
                let elapsed = phase_elapsed(now, self.phase_start);
                if clicked {
                    self.record_result(RoundOutcome::TooSoon, None);
                    return None;
                }
                if elapsed >= self.appearance_wait_ms {
                    self.set_phase(Phase::PlayingToClick, now);
                }
            }
            Phase::PlayingToClick => {
                let segment = self.to_click_seg.as_ref()?;
                let elapsed = phase_elapsed(now, self.phase_start);
                if clicked {
                    if elapsed >= self.click_point_ms {
                        self.record_hit(elapsed - self.click_point_ms);
                    } else {
                        self.record_result(RoundOutcome::TooSoon, None);
                    }
                    return None;
                }
                if elapsed >= self.click_point_ms {
                    self.display_idx = segment.frames.len() - 1;
                    self.click_point_start = Some(now);
                    self.set_phase(Phase::ClickPending, now);
                } else {
                    self.display_idx = frame_for_elapsed(segment, elapsed);
                }
            }
            Phase::ClickPending => {
                self.display_idx = self
                    .to_click_seg
                    .as_ref()
                    .map_or(0, |segment| segment.frames.len() - 1);
                let elapsed = phase_elapsed(now, self.click_point_start);
                if clicked {
                    self.record_hit(elapsed);
                    return None;
                }
                if elapsed >= self.run_settings.notify_miss_after_ms as f64 {
                    self.record_result(RoundOutcome::TooLateNoClick, None);
                }
            }
            Phase::PostClick => {
                let Some(segment) = self.post_click_seg.as_ref() else {
                    self.set_phase(Phase::Result, now);
                    return None;
                };
                let elapsed = phase_elapsed(now, self.phase_start);
                self.display_idx = frame_for_elapsed(segment, elapsed);
                if elapsed >= segment.duration_ms {
                    self.display_idx = segment.frames.len() - 1;
                    self.set_phase(Phase::Result, now);
                }
            }
            Phase::Result => {
                if clicked {
                    return self.acknowledge(now);
                }
            }
        }
        None
    }

    pub fn current_frame(&self) -> Option<&ColorImage> {
        let (segment, idx) = match self.phase {
            Phase::PreWait => (self.pre_wait_seg.as_ref()?, self.display_idx),
            Phase::Waiting => (self.to_click_seg.as_ref()?, 0),
            Phase::PlayingToClick | Phase::ClickPending => {
                (self.to_click_seg.as_ref()?, self.display_idx)
            }
            Phase::PostClick => (self.post_click_seg.as_ref()?, self.display_idx),
            Phase::Result => {
                let segment = self.to_click_seg.as_ref()?;
                return segment.frames.last();
            }
        };
        segment.frames.get(idx)
    }

    fn record_hit(&mut self, offset_ms: f64) {
        self.record_result(RoundOutcome::Hit, Some(offset_ms));
        if self.post_click_seg.is_some() {
            let now = Instant::now();
            self.display_idx = 0;
            self.set_phase(Phase::PostClick, now);
        }
    }

    fn record_result(&mut self, outcome: RoundOutcome, click_offset_ms: Option<f64>) {
        let group_index = self.selected_group.expect("active round has a group");
        let group_name = self.config.groups[group_index].name.clone();
        let video_path = self
            .current_video
            .as_ref()
            .map_or_else(String::new, |video| video.path.clone());
        let result = RoundResult {
            round_number: self.current_round + 1,
            attempt_number: self.attempt_results.len() + 1,
            group_name,
            video_path,
            appearance_wait_ms: self.appearance_wait_ms,
            click_offset_ms,
            outcome,
        };
        self.attempt_results.push(result.clone());
        self.last_result = Some(result.clone());
        self.pending_result = Some(result);
        self.set_phase(Phase::Result, Instant::now());
    }

    fn acknowledge(&mut self, _now: Instant) -> Option<RunData> {
        let result = self.pending_result.take()?;
        if result.outcome.is_hit() {
            self.current_round += 1;
            if self.current_round >= self.run_settings.round_count.max(1) {
                return Some(self.finish_run());
            }
            if let Err(error) = self.start_new_round() {
                self.last_error = Some(error);
                self.screen = AppScreen::Start;
            }
            return None;
        }
        match self.run_settings.false_click_action {
            FalseClickAction::RetryRound => {
                if let Err(error) = self.start_new_round() {
                    self.last_error = Some(error);
                    self.screen = AppScreen::Start;
                }
                None
            }
            FalseClickAction::EndRun => Some(self.finish_run()),
        }
    }

    fn finish_run(&mut self) -> RunData {
        let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S%.3f").to_string();
        let run_data = RunData {
            timestamp,
            settings: self.run_settings,
            attempts: self.attempt_results.clone(),
        };
        self.screen = AppScreen::End;
        run_data
    }

    fn set_phase(&mut self, phase: Phase, now: Instant) {
        self.phase = phase;
        self.phase_start = Some(now);
    }
}

fn phase_elapsed(now: Instant, start: Option<Instant>) -> f64 {
    start.map_or(0.0, |start| {
        now.duration_since(start).as_secs_f64() * 1000.0
    })
}

/// Cache key for one decoded segment. Shared by the preload worker and
/// [`AppState::ensure_segment`] so preloaded work is always a cache hit.
pub fn segment_key(path: &str, start_ms: f64, end_ms: f64) -> String {
    format!(
        "{path}|{start_ms:.3}|{end_ms:.3}|{}",
        super::extraction::MAX_FRAME_WIDTH
    )
}

fn frame_for_elapsed(segment: &VideoSegment, elapsed_ms: f64) -> usize {
    if segment.frames.is_empty() {
        return 0;
    }
    let idx = (elapsed_ms / 1000.0 * segment.fps).floor() as usize;
    idx.min(segment.frames.len() - 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::video_click_timing_test::types::{
        GroupConfig, TimestampInput, VideoGroup, VideoTimeStamp,
    };
    use std::process::Command;

    #[test]
    fn segment_keys_are_deterministic_and_distinct() {
        let first = segment_key("a.mp4", 1000.0, 2000.0);
        assert_eq!(first, segment_key("a.mp4", 1000.0, 2000.0));
        assert_ne!(first, segment_key("b.mp4", 1000.0, 2000.0));
        assert_ne!(first, segment_key("a.mp4", 1001.0, 2000.0));
        assert_ne!(first, segment_key("a.mp4", 1000.0, 2001.0));
    }

    #[test]
    fn preload_progress_is_clamped() {
        let mut preload = PreloadState {
            handle: PreloadHandle {
                events: std::sync::mpsc::channel().1,
                cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            },
            pool: Vec::new(),
            total: 4,
            processed: 2,
            succeeded: HashSet::new(),
            failures: Vec::new(),
        };
        assert_eq!(preload.progress_fraction(), 0.5);
        preload.processed = 9;
        assert_eq!(preload.progress_fraction(), 1.0);
        preload.total = 0;
        assert_eq!(preload.progress_fraction(), 1.0);
    }

    /// Full pipeline check against a synthetic clip, skipped automatically
    /// when ffmpeg is not installed on the machine running the tests.
    #[test]
    fn preloading_a_group_leads_to_an_instant_first_round() {
        if Command::new("ffprobe").arg("-version").output().is_err() {
            return;
        }
        let target = std::env::temp_dir().join(format!(
            "reactionlab_preload_test_{}.mp4",
            std::process::id()
        ));
        let target_path = target.to_string_lossy().into_owned();
        let created = ffmpeg_sidecar::command::FfmpegCommand::new()
            .format("lavfi")
            .input("testsrc=size=320x240:rate=30:duration=1")
            .overwrite()
            .output(&target_path)
            .spawn();
        let Ok(mut child) = created else { return };
        if let Ok(iter) = child.iter() {
            iter.for_each(|_| {});
        } else {
            let _ = std::fs::remove_file(&target);
            return;
        }

        let mut video = VideoConfig::default();
        video.path = target_path.clone();
        video.click_point = TimestampInput {
            time: VideoTimeStamp {
                milliseconds: 500,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut config = GroupConfig::default();
        config.groups.push(VideoGroup {
            name: "test".to_string(),
            videos: vec![video],
        });
        let mut state = AppState::new(config, Vec::new());

        state.start_run(0);
        assert_eq!(state.screen, AppScreen::Preparing);

        let deadline = Instant::now() + std::time::Duration::from_secs(30);
        while state.screen == AppScreen::Preparing && Instant::now() < deadline {
            state.poll_preload();
            if state.screen == AppScreen::Preparing {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        let _ = std::fs::remove_file(&target);

        assert_eq!(state.screen, AppScreen::Round);
        let pool = state
            .round_pool
            .as_ref()
            .expect("preloaded pool should be set");
        assert_eq!(pool.len(), 1);
        assert!(state.preload.is_none());
        assert!(state.segment_cache.len() >= 1);
        assert_eq!(state.phase, Phase::Waiting);

        // A follow-up round must resolve entirely from the cache.
        let cached_before = state.segment_cache.len();
        state.start_new_round().expect("cached rounds always start");
        assert_eq!(state.segment_cache.len(), cached_before);
    }
}
