use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;

use super::extraction::{extract_segment, VideoSegment};
use super::state::segment_key;
use super::types::VideoConfig;

/// One video queued for pre-extraction when a run starts.
pub struct VideoPlan {
    pub index: usize,
    pub video: VideoConfig,
}

/// Messages sent from the preload worker back to the UI thread.
pub enum PreloadEvent {
    /// Decoded segments ready to be inserted into the segment cache.
    Segments {
        index: usize,
        items: Vec<(String, VideoSegment)>,
    },
    /// A video failed to decode; it is excluded from the round pool.
    VideoFailed { index: usize, error: String },
    /// A video finished processing, successfully or not.
    VideoDone,
}

/// Handle to a running preload job.
pub struct PreloadHandle {
    pub events: Receiver<PreloadEvent>,
    pub cancel: Arc<AtomicBool>,
}

impl PreloadHandle {
    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// Spawn a worker that decodes every segment of every planned video.
///
/// Videos are processed sequentially so the progress counter advances in
/// video-sized steps. Duplicate videos share cache keys and are only decoded
/// once. Failed videos emit [`PreloadEvent::VideoFailed`] and processing
/// continues with the rest of the group.
pub fn spawn_preload(plans: Vec<VideoPlan>) -> PreloadHandle {
    let (sender, receiver) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = Arc::clone(&cancel);
    thread::spawn(move || run_worker(plans, sender, worker_cancel));
    PreloadHandle {
        events: receiver,
        cancel,
    }
}

fn run_worker(plans: Vec<VideoPlan>, sender: Sender<PreloadEvent>, cancel: Arc<AtomicBool>) {
    let mut seen_keys: HashSet<String> = HashSet::new();
    for plan in plans {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        let VideoPlan { index, video } = plan;
        let fps = video.effective_fps();
        let mut items = Vec::new();
        let mut failure: Option<String> = None;
        for (start_ms, end_ms) in planned_ranges(&video) {
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            let key = segment_key(&video.path, start_ms, end_ms);
            if !seen_keys.insert(key.clone()) {
                continue;
            }
            match extract_segment(&video.path, start_ms, end_ms, fps) {
                Ok(segment) => items.push((key, segment)),
                Err(error) => {
                    failure = Some(error);
                    break;
                }
            }
        }
        if let Some(error) = failure {
            let _ = sender.send(PreloadEvent::VideoFailed { index, error });
        } else if !items.is_empty() {
            let _ = sender.send(PreloadEvent::Segments { index, items });
        }
        let _ = sender.send(PreloadEvent::VideoDone);
    }
}

/// All segment ranges that must exist in the cache before this video can be
/// played in a round.
fn planned_ranges(video: &VideoConfig) -> Vec<(f64, f64)> {
    let bounds = video.segments_ms();
    let mut ranges = Vec::new();
    if let Some(range) = bounds.pre_wait {
        ranges.push(range);
    }
    ranges.push(bounds.to_click);
    if let Some(range) = bounds.post_click {
        ranges.push(range);
    }
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::video_click_timing_test::types::{TimestampInput, VideoTimeStamp};

    fn time_input(seconds: u64) -> TimestampInput {
        TimestampInput {
            time: VideoTimeStamp {
                seconds,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn sample_video(click_seconds: u64) -> VideoConfig {
        let mut video = VideoConfig::default();
        video.has_pre_wait = true;
        video.pre_wait_start = time_input(1);
        video.pre_wait_end = time_input(2);
        video.click_point = time_input(click_seconds);
        video.has_post_click = true;
        video.post_click_end = time_input(click_seconds + 1);
        video
    }

    fn keys_of(video: &VideoConfig) -> Vec<String> {
        planned_ranges(video)
            .iter()
            .map(|(start, end)| segment_key("path", *start, *end))
            .collect()
    }

    #[test]
    fn identical_videos_share_all_segment_keys() {
        assert_eq!(keys_of(&sample_video(5)), keys_of(&sample_video(5)));
    }

    #[test]
    fn distinct_videos_differ_somewhere() {
        assert_ne!(keys_of(&sample_video(5)), keys_of(&sample_video(6)));
    }

    #[test]
    fn planned_ranges_cover_every_enabled_segment() {
        let video = sample_video(5);
        let ranges = planned_ranges(&video);
        assert_eq!(ranges.len(), 3);
        assert_eq!(ranges[0], (1000.0, 2000.0));
        assert_eq!(ranges[1], (2000.0, 5000.0));
        assert_eq!(ranges[2], (5000.0, 6000.0));

        let mut minimal = VideoConfig::default();
        minimal.click_point = time_input(3);
        assert_eq!(planned_ranges(&minimal).len(), 1);
    }
}
