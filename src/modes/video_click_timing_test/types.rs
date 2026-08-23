use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum MeasurementUnit {
    #[default]
    Time,
    Frame,
}

impl MeasurementUnit {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Time => "Time",
            Self::Frame => "Frame",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FalseClickAction {
    #[default]
    RetryRound,
    EndRun,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RoundOutcome {
    Hit,
    TooSoon,
    TooLateNoClick,
}

impl RoundOutcome {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Hit => "Hit",
            Self::TooSoon => "Too soon",
            Self::TooLateNoClick => "Too late, no click",
        }
    }

    pub const fn is_hit(self) -> bool {
        matches!(self, Self::Hit)
    }
}

/// A time-of-day style timestamp: minutes, seconds, milliseconds.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub struct VideoTimeStamp {
    #[serde(default)]
    pub minutes: u64,
    #[serde(default)]
    pub seconds: u64,
    #[serde(default)]
    pub milliseconds: u64,
}

impl VideoTimeStamp {
    pub const fn to_ms(self) -> f64 {
        ((self.minutes * 60 + self.seconds) * 1000) as f64 + self.milliseconds as f64
    }
}

/// A single point in a video, editable either as a clock time or a frame number.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub struct TimestampInput {
    #[serde(default)]
    pub time: VideoTimeStamp,
    #[serde(default)]
    pub frame: u64,
}

impl TimestampInput {
    pub fn to_ms(self, unit: MeasurementUnit, fps: f64) -> f64 {
        match unit {
            MeasurementUnit::Time => self.time.to_ms(),
            MeasurementUnit::Frame => {
                let fps = if fps > 0.0 { fps } else { 30.0 };
                self.frame as f64 / fps * 1000.0
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
pub struct VideoConfig {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub unit: MeasurementUnit,
    /// Auto-detected frames-per-second; required when the unit is Frame.
    #[serde(default)]
    pub fps: Option<f64>,

    #[serde(default)]
    pub has_pre_wait: bool,
    #[serde(default)]
    pub pre_wait_start: TimestampInput,
    #[serde(default)]
    pub pre_wait_end: TimestampInput,

    #[serde(default)]
    pub click_point: TimestampInput,

    #[serde(default)]
    pub has_post_click: bool,
    /// Post-click playback resumes from the click point and runs until here.
    #[serde(default)]
    pub post_click_end: TimestampInput,
}

impl VideoConfig {
    pub fn effective_fps(&self) -> f64 {
        self.fps.unwrap_or(30.0)
    }

    /// Resolve the three segment boundaries (ms) for this video. The post-click
    /// segment always starts at the click point, where playback was paused.
    pub fn segments_ms(&self) -> SegmentBoundaries {
        let fps = self.effective_fps();
        let unit = self.unit;
        let pre_wait_start = self.pre_wait_start.to_ms(unit, fps);
        let pre_wait_end = self.pre_wait_end.to_ms(unit, fps);
        let click_point = self.click_point.to_ms(unit, fps);
        let post_click_end = self.post_click_end.to_ms(unit, fps);
        let to_click_start = if self.has_pre_wait { pre_wait_end } else { 0.0 };
        SegmentBoundaries {
            pre_wait: self
                .has_pre_wait
                .then(|| (pre_wait_start.max(0.0), pre_wait_end.max(0.0))),
            to_click: (to_click_start.max(0.0), click_point.max(0.0)),
            post_click: self
                .has_post_click
                .then(|| (click_point.max(0.0), post_click_end.max(0.0))),
        }
    }

    /// Human-readable problems preventing this video from being used in a run.
    ///
    /// Note that a pre-wait whose start equals its end is valid: it denotes a
    /// custom starting point and is played as a single frame.
    pub fn validation_messages(&self) -> Vec<String> {
        let mut messages = Vec::new();
        if self.path.is_empty() {
            messages.push("Set a video path.".to_string());
        } else if !std::path::Path::new(&self.path).exists() {
            messages.push("Video file not found.".to_string());
        }
        if let MeasurementUnit::Frame = self.unit
            && self.fps.is_none()
        {
            messages.push(
                "Frame unit requires a known FPS; click Probe after setting a path.".to_string(),
            );
        }

        let fps = self.effective_fps();
        let unit = self.unit;
        let pre_wait_start = self.pre_wait_start.to_ms(unit, fps);
        let pre_wait_end = self.pre_wait_end.to_ms(unit, fps);
        let click_point = self.click_point.to_ms(unit, fps);
        let post_click_end = self.post_click_end.to_ms(unit, fps);

        if self.has_pre_wait && pre_wait_end < pre_wait_start {
            messages.push("Pre-wait end cannot be before pre-wait start.".to_string());
        }
        let main_start = if self.has_pre_wait { pre_wait_end } else { 0.0 };
        if click_point <= main_start {
            messages.push("Click point must come after the playback start.".to_string());
        }
        if self.has_post_click && post_click_end <= click_point {
            messages.push("Post-click end must come after the click point.".to_string());
        }
        messages
    }
}

pub struct SegmentBoundaries {
    pub pre_wait: Option<(f64, f64)>,
    pub to_click: (f64, f64),
    pub post_click: Option<(f64, f64)>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
pub struct VideoGroup {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub videos: Vec<VideoConfig>,
}

impl VideoGroup {
    /// Videos that are fully configured and may be picked for a round.
    pub fn usable_videos(&self) -> impl Iterator<Item = &VideoConfig> {
        self.videos
            .iter()
            .filter(|video| video.validation_messages().is_empty())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
pub struct GroupConfig {
    #[serde(default)]
    pub groups: Vec<VideoGroup>,
}

impl GroupConfig {
    pub fn has_usable_videos(&self) -> bool {
        self.groups
            .iter()
            .any(|group| group.usable_videos().next().is_some())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub struct RunSettings {
    #[serde(default = "default_min_wait")]
    pub min_wait_ms: u64,
    #[serde(default = "default_max_wait")]
    pub max_wait_ms: u64,
    #[serde(default = "default_round_count")]
    pub round_count: usize,
    #[serde(default)]
    pub false_click_action: FalseClickAction,
    #[serde(default = "default_notify_miss_after")]
    pub notify_miss_after_ms: u64,
}

const fn default_min_wait() -> u64 {
    500
}
const fn default_max_wait() -> u64 {
    5000
}
const fn default_round_count() -> usize {
    5
}
const fn default_notify_miss_after() -> u64 {
    200
}

impl Default for RunSettings {
    fn default() -> Self {
        Self {
            min_wait_ms: default_min_wait(),
            max_wait_ms: default_max_wait(),
            round_count: default_round_count(),
            false_click_action: FalseClickAction::default(),
            notify_miss_after_ms: default_notify_miss_after(),
        }
    }
}

impl RunSettings {
    pub fn validation_messages(&self) -> Vec<String> {
        let mut messages = Vec::new();
        if self.min_wait_ms > self.max_wait_ms {
            messages.push("Minimum wait cannot exceed maximum wait.".to_string());
        }
        if self.round_count == 0 {
            messages.push("Round count must be at least 1.".to_string());
        }
        messages
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct RoundResult {
    #[serde(default)]
    pub round_number: usize,
    #[serde(default)]
    pub attempt_number: usize,
    pub group_name: String,
    pub video_path: String,
    pub appearance_wait_ms: f64,
    pub click_offset_ms: Option<f64>,
    pub outcome: RoundOutcome,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct RunData {
    pub timestamp: String,
    pub settings: RunSettings,
    pub attempts: Vec<RoundResult>,
}

#[derive(Clone)]
pub struct RunFileInfo {
    pub filename: String,
    pub display_name: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum AppScreen {
    Start,
    SelectGroup,
    /// Pre-extracting every video in the selected group before round one.
    Preparing,
    Round,
    End,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_timestamp_converts_to_ms() {
        let stamp = VideoTimeStamp {
            minutes: 1,
            seconds: 35,
            milliseconds: 500,
        };
        assert_eq!(stamp.to_ms(), 95500.0);
    }

    #[test]
    fn frame_timestamp_converts_to_ms() {
        let input = TimestampInput {
            frame: 60,
            ..Default::default()
        };
        assert_eq!(input.to_ms(MeasurementUnit::Frame, 30.0), 2000.0);
    }

    #[test]
    fn segments_resolve_with_pre_wait() {
        let mut video = VideoConfig::default();
        video.has_pre_wait = true;
        video.pre_wait_start = TimestampInput {
            time: VideoTimeStamp {
                seconds: 1,
                ..Default::default()
            },
            ..Default::default()
        };
        video.pre_wait_end = TimestampInput {
            time: VideoTimeStamp {
                seconds: 2,
                ..Default::default()
            },
            ..Default::default()
        };
        video.click_point = TimestampInput {
            time: VideoTimeStamp {
                seconds: 5,
                ..Default::default()
            },
            ..Default::default()
        };
        let segments = video.segments_ms();
        assert_eq!(segments.pre_wait, Some((1000.0, 2000.0)));
        assert_eq!(segments.to_click, (2000.0, 5000.0));
        assert!(segments.post_click.is_none());
    }

    #[test]
    fn segments_without_pre_wait_start_at_zero() {
        let mut video = VideoConfig::default();
        video.click_point = TimestampInput {
            time: VideoTimeStamp {
                seconds: 3,
                ..Default::default()
            },
            ..Default::default()
        };
        let segments = video.segments_ms();
        assert!(segments.pre_wait.is_none());
        assert_eq!(segments.to_click, (0.0, 3000.0));
    }

    #[test]
    fn segments_resolve_frames_using_fps() {
        let mut video = VideoConfig::default();
        video.unit = MeasurementUnit::Frame;
        video.fps = Some(60.0);
        video.click_point = TimestampInput {
            frame: 120,
            ..Default::default()
        };
        let segments = video.segments_ms();
        assert_eq!(segments.to_click, (0.0, 2000.0));
    }

    #[test]
    fn post_click_starts_at_click_point() {
        let mut video = valid_video();
        video.has_post_click = true;
        video.click_point = time_input(2, 0);
        video.post_click_end = time_input(3, 500);
        let segments = video.segments_ms();
        assert_eq!(segments.post_click, Some((2000.0, 3500.0)));
    }

    #[test]
    fn degenerate_pre_wait_denotes_a_custom_start_point() {
        let mut video = valid_video();
        video.has_pre_wait = true;
        video.pre_wait_start = time_input(16, 0);
        video.pre_wait_end = time_input(16, 0);
        video.click_point = time_input(18, 500);

        assert!(video.validation_messages().is_empty());

        let segments = video.segments_ms();
        assert_eq!(segments.pre_wait, Some((16000.0, 16000.0)));
        assert_eq!(segments.to_click, (16000.0, 18500.0));
    }

    #[test]
    fn pre_wait_end_before_start_is_rejected() {
        let mut video = valid_video();
        video.has_pre_wait = true;
        video.pre_wait_start = time_input(16, 0);
        video.pre_wait_end = time_input(15, 0);
        video.click_point = time_input(18, 0);
        assert!(
            video
                .validation_messages()
                .iter()
                .any(|message| message.contains("Pre-wait end"))
        );
    }

    #[test]
    fn click_point_not_after_playback_start_is_rejected() {
        let mut video = valid_video();
        video.click_point = time_input(0, 0);
        assert!(
            video
                .validation_messages()
                .iter()
                .any(|message| message.contains("Click point"))
        );

        video.has_pre_wait = true;
        video.pre_wait_start = time_input(5, 0);
        video.pre_wait_end = time_input(5, 0);
        video.click_point = time_input(4, 999);
        assert!(
            video
                .validation_messages()
                .iter()
                .any(|message| message.contains("Click point"))
        );
    }

    #[test]
    fn post_click_end_not_after_click_point_is_rejected() {
        let mut video = valid_video();
        video.has_post_click = true;
        video.click_point = time_input(2, 0);
        video.post_click_end = time_input(2, 0);
        assert!(
            video
                .validation_messages()
                .iter()
                .any(|message| message.contains("Post-click end"))
        );
    }

    #[test]
    fn frame_unit_without_fps_is_rejected() {
        let mut video = valid_video();
        video.unit = MeasurementUnit::Frame;
        video.fps = None;
        video.click_point = time_input(2, 0);
        assert!(
            video
                .validation_messages()
                .iter()
                .any(|message| message.contains("FPS"))
        );
    }

    #[test]
    fn missing_path_is_rejected() {
        let video = VideoConfig::default();
        assert!(
            video
                .validation_messages()
                .iter()
                .any(|message| message.contains("path"))
        );
    }

    fn valid_video() -> VideoConfig {
        let mut video = VideoConfig::default();
        // The test executable is guaranteed to exist on disk.
        video.path = std::env::current_exe()
            .expect("tests always run from an existing executable")
            .to_string_lossy()
            .into_owned();
        video.click_point = time_input(2, 0);
        video
    }

    fn time_input(seconds: u64, milliseconds: u64) -> TimestampInput {
        TimestampInput {
            time: VideoTimeStamp {
                seconds,
                milliseconds,
                ..Default::default()
            },
            ..Default::default()
        }
    }
}
