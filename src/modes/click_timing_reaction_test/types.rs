use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

#[allow(clippy::struct_excessive_bools)]
#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct Configurables {
    #[serde(default)]
    pub crosshair_style: CrosshairStyle,
    #[serde(default = "default_crosshair_outline_thickness")]
    pub crosshair_outline_thickness: f32,
    #[serde(default = "default_crosshair_color")]
    pub crosshair_color: [u8; 4],
    #[serde(default = "default_crosshair_outline_color")]
    pub crosshair_outline_color: [u8; 4],
    #[serde(default = "default_regular_length")]
    pub regular_length: f32,
    #[serde(default = "default_regular_thickness")]
    pub regular_thickness: f32,
    #[serde(default = "default_regular_center_gap")]
    pub regular_center_gap: f32,
    #[serde(default)]
    pub regular_t_style: bool,
    #[serde(default)]
    pub regular_center_square_dot: bool,
    #[serde(default = "default_plus_length")]
    pub plus_length: f32,
    #[serde(default = "default_plus_thickness")]
    pub plus_thickness: f32,
    #[serde(default = "default_dot_diameter")]
    pub dot_diameter: f32,

    #[serde(default = "default_left_wall_distance")]
    pub left_wall_distance: f32,
    #[serde(default = "default_right_wall_distance")]
    pub right_wall_distance: f32,

    #[serde(default = "default_target_visual_radius")]
    pub target_visual_radius: f32,
    #[serde(default = "default_target_clickable_radius")]
    pub target_clickable_radius: f32,
    #[serde(default = "default_target_color")]
    pub target_color: [u8; 4],
    #[serde(default = "default_stop_on_crosshair")]
    pub stop_on_crosshair: bool,
    #[serde(default = "default_full_speed_velocity")]
    pub full_speed_velocity: f32,
    #[serde(default)]
    pub initial_velocity: InitialVelocityMode,
    #[serde(default = "default_acceleration_duration")]
    pub acceleration_duration_ms: u64,
    #[serde(default = "default_acceleration_linearity")]
    pub acceleration_linearity: f32,
    #[serde(default = "default_deceleration_duration")]
    pub deceleration_duration_ms: u64,
    #[serde(default = "default_deceleration_linearity")]
    pub deceleration_linearity: f32,
    #[serde(default = "default_notify_miss_after")]
    pub notify_miss_after_ms: u64,

    #[serde(default = "default_target_direction_left")]
    pub target_direction_left: bool,
    #[serde(default = "default_target_direction_right")]
    pub target_direction_right: bool,

    #[serde(default = "default_min_wait")]
    pub min_wait_ms: u64,
    #[serde(default = "default_max_wait")]
    pub max_wait_ms: u64,
    #[serde(default = "default_round_count")]
    pub round_count: usize,
    #[serde(default)]
    pub false_click_action: FalseClickAction,
}

const fn default_crosshair_outline_thickness() -> f32 {
    0.0
}

const fn default_crosshair_color() -> [u8; 4] {
    [255, 210, 40, 255]
}

const fn default_crosshair_outline_color() -> [u8; 4] {
    [0, 0, 0, 255]
}

const fn default_regular_length() -> f32 {
    6.0
}

const fn default_regular_thickness() -> f32 {
    2.0
}

const fn default_regular_center_gap() -> f32 {
    2.0
}

const fn default_plus_length() -> f32 {
    40.0
}

const fn default_plus_thickness() -> f32 {
    4.0
}

const fn default_dot_diameter() -> f32 {
    8.0
}

const fn default_left_wall_distance() -> f32 {
    100.0
}

const fn default_right_wall_distance() -> f32 {
    100.0
}

const fn default_target_visual_radius() -> f32 {
    8.0
}

const fn default_target_clickable_radius() -> f32 {
    6.0
}

const fn default_target_color() -> [u8; 4] {
    [200, 200, 255, 255]
}

const fn default_stop_on_crosshair() -> bool {
    false
}

const fn default_full_speed_velocity() -> f32 {
    400.0
}

const fn default_acceleration_duration() -> u64 {
    500
}

const fn default_acceleration_linearity() -> f32 {
    1.0
}

const fn default_deceleration_duration() -> u64 {
    125
}

const fn default_deceleration_linearity() -> f32 {
    1.0
}

const fn default_notify_miss_after() -> u64 {
    200
}

const fn default_target_direction_left() -> bool {
    true
}

const fn default_target_direction_right() -> bool {
    true
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

impl Default for Configurables {
    fn default() -> Self {
        Self {
            crosshair_style: CrosshairStyle::default(),
            crosshair_outline_thickness: default_crosshair_outline_thickness(),
            crosshair_color: default_crosshair_color(),
            crosshair_outline_color: default_crosshair_outline_color(),
            regular_length: default_regular_length(),
            regular_thickness: default_regular_thickness(),
            regular_center_gap: default_regular_center_gap(),
            regular_t_style: false,
            regular_center_square_dot: false,
            plus_length: default_plus_length(),
            plus_thickness: default_plus_thickness(),
            dot_diameter: default_dot_diameter(),
            left_wall_distance: default_left_wall_distance(),
            right_wall_distance: default_right_wall_distance(),
            target_visual_radius: default_target_visual_radius(),
            target_clickable_radius: default_target_clickable_radius(),
            target_color: default_target_color(),
            stop_on_crosshair: default_stop_on_crosshair(),
            full_speed_velocity: default_full_speed_velocity(),
            initial_velocity: InitialVelocityMode::default(),
            acceleration_duration_ms: default_acceleration_duration(),
            acceleration_linearity: default_acceleration_linearity(),
            deceleration_duration_ms: default_deceleration_duration(),
            deceleration_linearity: default_deceleration_linearity(),
            notify_miss_after_ms: default_notify_miss_after(),
            target_direction_left: default_target_direction_left(),
            target_direction_right: default_target_direction_right(),
            min_wait_ms: default_min_wait(),
            max_wait_ms: default_max_wait(),
            round_count: default_round_count(),
            false_click_action: FalseClickAction::default(),
        }
    }
}

impl Configurables {
    pub fn crosshair_color_egui(&self) -> Color32 {
        Color32::from_rgba_unmultiplied(
            self.crosshair_color[0],
            self.crosshair_color[1],
            self.crosshair_color[2],
            self.crosshair_color[3],
        )
    }

    pub fn crosshair_outline_color_egui(&self) -> Color32 {
        Color32::from_rgba_unmultiplied(
            self.crosshair_outline_color[0],
            self.crosshair_outline_color[1],
            self.crosshair_outline_color[2],
            self.crosshair_outline_color[3],
        )
    }

    pub fn target_color_egui(&self) -> Color32 {
        Color32::from_rgba_unmultiplied(
            self.target_color[0],
            self.target_color[1],
            self.target_color[2],
            self.target_color[3],
        )
    }

    pub const fn set_crosshair_color(&mut self, color: Color32) {
        self.crosshair_color = [color.r(), color.g(), color.b(), color.a()];
    }

    pub const fn set_crosshair_outline_color(&mut self, color: Color32) {
        self.crosshair_outline_color = [color.r(), color.g(), color.b(), color.a()];
    }

    pub const fn set_target_color(&mut self, color: Color32) {
        self.target_color = [color.r(), color.g(), color.b(), color.a()];
    }

    pub fn validation_messages(&self) -> Vec<String> {
        let mut messages = Vec::new();
        if !self.target_direction_left && !self.target_direction_right {
            messages.push("Select at least one target direction.".to_string());
        }
        if self.target_clickable_radius > self.target_visual_radius {
            messages.push("Clickable radius cannot exceed visual radius.".to_string());
        }
        if self.min_wait_ms > self.max_wait_ms {
            messages.push("Minimum wait cannot exceed maximum wait.".to_string());
        }
        if self.full_speed_velocity <= 0.0 || !self.full_speed_velocity.is_finite() {
            messages.push("Full speed velocity must be positive.".to_string());
        }
        if self.acceleration_linearity <= 0.0 || !self.acceleration_linearity.is_finite() {
            messages.push("Acceleration linearity must be positive.".to_string());
        }
        if self.deceleration_linearity <= 0.0 || !self.deceleration_linearity.is_finite() {
            messages.push("Deceleration linearity must be positive.".to_string());
        }
        messages
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CrosshairStyle {
    #[default]
    Regular,
    Plus,
    CircularDot,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum InitialVelocityMode {
    #[default]
    Full,
    Mixed,
    Zero,
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
pub enum TargetDirection {
    FromLeft,
    FromRight,
}

impl TargetDirection {
    pub const fn label(self) -> &'static str {
        match self {
            Self::FromLeft => "Left",
            Self::FromRight => "Right",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StopBehavior {
    StopOnCrosshair,
    ContinueToOppositeWall,
}

impl StopBehavior {
    pub const fn label(self) -> &'static str {
        match self {
            Self::StopOnCrosshair => "Stop on crosshair",
            Self::ContinueToOppositeWall => "Continue to opposite wall",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RoundOutcome {
    Hit,
    TooSoon,
    AlmostThereTooSoon,
    AlmostThereTooLate,
    TooLateClick,
    TooLateNoClick,
}

impl RoundOutcome {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Hit => "Hit",
            Self::TooSoon => "Too soon",
            Self::AlmostThereTooSoon => "Almost there, too soon",
            Self::AlmostThereTooLate => "Almost there, too late",
            Self::TooLateClick => "Too late click",
            Self::TooLateNoClick => "Too late, no click",
        }
    }

    pub const fn is_hit(self) -> bool {
        matches!(self, Self::Hit)
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct RoundResult {
    #[serde(default)]
    pub round_number: usize,
    #[serde(default)]
    pub attempt_number: usize,
    pub direction: TargetDirection,
    pub appearance_wait_ms: f64,
    pub initial_velocity_fraction: f64,
    pub stop_behavior: StopBehavior,
    pub center_crossing_ms: f64,
    pub click_offset_ms: Option<f64>,
    pub outcome: RoundOutcome,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct RunData {
    pub timestamp: String,
    pub config: Configurables,
    #[serde(alias = "rounds")]
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
    Round,
    End,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum RoundState {
    Waiting,
    Moving,
    ResultShowing,
}
