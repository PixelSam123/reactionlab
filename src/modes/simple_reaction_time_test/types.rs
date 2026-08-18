use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Configurables {
    #[serde(default = "default_wait_color")]
    pub wait_color: [u8; 3],
    #[serde(default = "default_react_color")]
    pub react_color: [u8; 3],
    #[serde(default = "default_min_wait")]
    pub min_wait_ms: u64,
    #[serde(default = "default_max_wait")]
    pub max_wait_ms: u64,
    #[serde(default = "default_round_count")]
    pub round_count: usize,
    #[serde(default)]
    pub false_click_action: FalseClickAction,
}

const fn default_wait_color() -> [u8; 3] {
    [255, 0, 0]
}
const fn default_react_color() -> [u8; 3] {
    [0, 255, 0]
}
const fn default_min_wait() -> u64 {
    250
}
const fn default_max_wait() -> u64 {
    10000
}
const fn default_round_count() -> usize {
    5
}

impl Default for Configurables {
    fn default() -> Self {
        Self {
            wait_color: default_wait_color(),
            react_color: default_react_color(),
            min_wait_ms: default_min_wait(),
            max_wait_ms: default_max_wait(),
            round_count: default_round_count(),
            false_click_action: FalseClickAction::default(),
        }
    }
}

impl Configurables {
    pub const fn wait_color_egui(&self) -> Color32 {
        Color32::from_rgb(self.wait_color[0], self.wait_color[1], self.wait_color[2])
    }
    pub const fn react_color_egui(&self) -> Color32 {
        Color32::from_rgb(
            self.react_color[0],
            self.react_color[1],
            self.react_color[2],
        )
    }
    pub const fn set_wait_color(&mut self, c: Color32) {
        self.wait_color = [c.r(), c.g(), c.b()];
    }
    pub const fn set_react_color(&mut self, c: Color32) {
        self.react_color = [c.r(), c.g(), c.b()];
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
pub enum FalseClickAction {
    #[serde(rename = "retry_round")]
    #[default]
    RetryRound,
    #[serde(rename = "end_run")]
    EndRun,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct RoundResult {
    pub wait_time_ms: f64,
    pub reaction_time_ms: f64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct RunData {
    pub timestamp: String,
    pub config: Configurables,
    pub rounds: Vec<RoundResult>,
}

#[derive(Clone)]
pub struct RunFileInfo {
    pub filename: String,
    pub display_name: String,
}

#[derive(PartialEq, Eq)]
pub enum AppScreen {
    Start,
    Round,
    End,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum RoundState {
    Waiting,
    Reacting,
    ResultShowing,
    TooSoon,
}
