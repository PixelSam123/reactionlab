use std::time::Instant;

use chrono::Local;
use rand::RngExt;

use super::types::{AppScreen, Configurables, FalseClickAction, RoundResult, RoundState, RunData};

pub struct AppState {
    pub screen: AppScreen,
    pub config: Configurables,
    pub round_state: RoundState,
    pub current_round: usize,
    pub round_results: Vec<RoundResult>,
    pub wait_start: Option<Instant>,
    pub react_start: Option<Instant>,
    pub random_wait_ms: f64,
    pub last_reaction_ms: Option<f64>,
    pub history_means: Vec<(chrono::NaiveDateTime, f64)>,
}

impl AppState {
    pub fn new(config: Configurables, history_means: Vec<(chrono::NaiveDateTime, f64)>) -> Self {
        Self {
            screen: AppScreen::Start,
            config,
            round_state: RoundState::Waiting,
            current_round: 0,
            round_results: Vec::new(),
            wait_start: None,
            react_start: None,
            random_wait_ms: 0.0,
            last_reaction_ms: None,
            history_means,
        }
    }

    pub fn start_new_round(&mut self) {
        self.start_new_round_at(Instant::now());
    }

    pub fn start_new_round_at(&mut self, now: Instant) {
        let mut rng = rand::rng();
        self.random_wait_ms =
            rng.random_range(self.config.min_wait_ms as f64..=self.config.max_wait_ms as f64);
        self.round_state = RoundState::Waiting;
        self.wait_start = Some(now);
        self.react_start = None;
        self.last_reaction_ms = None;
    }

    pub fn update_waiting(&mut self) {
        self.update_waiting_at(Instant::now());
    }

    pub fn update_waiting_at(&mut self, now: Instant) {
        if self.round_state == RoundState::Waiting
            && let Some(start) = self.wait_start
            && now.duration_since(start).as_secs_f64() * 1000.0 >= self.random_wait_ms
        {
            self.round_state = RoundState::Reacting;
            self.react_start = Some(now);
        }
    }

    pub fn handle_click(&mut self) -> Option<RunData> {
        self.handle_click_at(Instant::now())
    }

    pub fn handle_click_at(&mut self, now: Instant) -> Option<RunData> {
        match self.round_state {
            RoundState::Waiting => {
                self.round_state = RoundState::TooSoon;
                None
            }
            RoundState::Reacting => {
                let react_start = self
                    .react_start
                    .expect("reacting state must have a reaction start");
                let reaction = now.duration_since(react_start).as_secs_f64() * 1000.0;
                self.last_reaction_ms = Some(reaction);
                self.round_results.push(RoundResult {
                    wait_time_ms: self.random_wait_ms,
                    reaction_time_ms: reaction,
                });
                self.round_state = RoundState::ResultShowing;
                None
            }
            RoundState::ResultShowing => {
                self.current_round += 1;
                if self.current_round >= self.config.round_count {
                    Some(self.finish_run())
                } else {
                    self.start_new_round_at(now);
                    None
                }
            }
            RoundState::TooSoon => match self.config.false_click_action {
                FalseClickAction::RetryRound => {
                    self.start_new_round_at(now);
                    None
                }
                FalseClickAction::EndRun => Some(self.finish_run()),
            },
        }
    }

    pub fn restart_run(&mut self) {
        self.round_results.clear();
        self.current_round = 0;
        self.round_state = RoundState::Waiting;
        self.wait_start = None;
        self.react_start = None;
        self.random_wait_ms = 0.0;
        self.last_reaction_ms = None;
        self.start_new_round();
        self.screen = AppScreen::Round;
    }

    pub fn go_to_start(&mut self) {
        self.screen = AppScreen::Start;
        self.round_results.clear();
        self.current_round = 0;
    }

    pub fn reset_to_start(&mut self) {
        self.screen = AppScreen::Start;
        self.round_state = RoundState::Waiting;
        self.round_results.clear();
        self.current_round = 0;
        self.wait_start = None;
        self.react_start = None;
        self.random_wait_ms = 0.0;
        self.last_reaction_ms = None;
    }

    fn finish_run(&mut self) -> RunData {
        let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S%.3f").to_string();
        let run_data = RunData {
            timestamp,
            config: self.config.clone(),
            rounds: self.round_results.clone(),
        };
        self.screen = AppScreen::End;
        run_data
    }
}

pub fn compute_mean(times: &[f64]) -> f64 {
    if times.is_empty() {
        return 0.0;
    }
    times.iter().sum::<f64>() / times.len() as f64
}

pub fn compute_median(times: &[f64]) -> f64 {
    if times.is_empty() {
        return 0.0;
    }
    let mut sorted: Vec<f64> = times.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = sorted.len();
    if n.is_multiple_of(2) {
        f64::midpoint(sorted[n / 2 - 1], sorted[n / 2])
    } else {
        sorted[n / 2]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_mean_and_median() {
        assert_eq!(compute_mean(&[]), 0.0);
        assert_eq!(compute_mean(&[100.0, 200.0, 300.0]), 200.0);
        assert_eq!(compute_median(&[]), 0.0);
        assert_eq!(compute_median(&[300.0, 100.0, 200.0]), 200.0);
        assert_eq!(compute_median(&[400.0, 100.0, 300.0, 200.0]), 250.0);
    }

    #[test]
    fn waiting_round_becomes_reacting_after_delay() {
        let config = Configurables {
            min_wait_ms: 100,
            max_wait_ms: 100,
            ..Default::default()
        };
        let mut state = AppState::new(config, Vec::new());
        let start = Instant::now();
        state.random_wait_ms = 100.0;
        state.wait_start = Some(start);
        state.update_waiting_at(start + std::time::Duration::from_millis(99));
        assert!(matches!(state.round_state, RoundState::Waiting));
        state.update_waiting_at(start + std::time::Duration::from_millis(100));
        assert!(matches!(state.round_state, RoundState::Reacting));
    }

    #[test]
    fn reacting_click_records_result() {
        let mut state = AppState::new(Configurables::default(), Vec::new());
        let start = Instant::now();
        state.random_wait_ms = 250.0;
        state.round_state = RoundState::Reacting;
        state.react_start = Some(start);
        state.handle_click_at(start + std::time::Duration::from_millis(125));
        assert!(matches!(state.round_state, RoundState::ResultShowing));
        assert_eq!(state.round_results.len(), 1);
        assert_eq!(state.round_results[0].reaction_time_ms, 125.0);
    }

    #[test]
    fn too_soon_retry_starts_a_new_round() {
        let mut state = AppState::new(Configurables::default(), Vec::new());
        state.round_state = RoundState::TooSoon;
        state.handle_click_at(Instant::now());
        assert!(matches!(state.round_state, RoundState::Waiting));
        assert!(state.wait_start.is_some());
    }

    #[test]
    fn too_soon_end_finishes_run() {
        let config = Configurables {
            false_click_action: FalseClickAction::EndRun,
            ..Default::default()
        };
        let mut state = AppState::new(config, Vec::new());
        state.round_state = RoundState::TooSoon;
        let result = state.handle_click_at(Instant::now());
        assert!(result.is_some());
        assert!(matches!(state.screen, AppScreen::End));
    }

    #[test]
    fn restart_clears_previous_results_and_enters_round_screen() {
        let mut state = AppState::new(Configurables::default(), Vec::new());
        state.current_round = 3;
        state.round_results.push(RoundResult {
            wait_time_ms: 250.0,
            reaction_time_ms: 125.0,
        });
        state.restart_run();
        assert!(state.round_results.is_empty());
        assert_eq!(state.current_round, 0);
        assert!(matches!(state.screen, AppScreen::Round));
        assert!(matches!(state.round_state, RoundState::Waiting));
    }

    #[test]
    fn reset_to_start_clears_active_round_state() {
        let mut state = AppState::new(Configurables::default(), Vec::new());
        state.screen = AppScreen::Round;
        state.round_state = RoundState::Reacting;
        state.current_round = 2;
        state.round_results.push(RoundResult {
            wait_time_ms: 250.0,
            reaction_time_ms: 125.0,
        });
        state.wait_start = Some(Instant::now());
        state.react_start = Some(Instant::now());
        state.random_wait_ms = 250.0;
        state.last_reaction_ms = Some(125.0);

        state.reset_to_start();

        assert!(matches!(state.screen, AppScreen::Start));
        assert!(matches!(state.round_state, RoundState::Waiting));
        assert_eq!(state.current_round, 0);
        assert!(state.round_results.is_empty());
        assert!(state.wait_start.is_none());
        assert!(state.react_start.is_none());
        assert_eq!(state.random_wait_ms, 0.0);
        assert!(state.last_reaction_ms.is_none());
    }

    #[test]
    fn completing_last_result_finishes_run() {
        let config = Configurables {
            round_count: 1,
            ..Default::default()
        };
        let mut state = AppState::new(config, Vec::new());
        state.round_state = RoundState::ResultShowing;
        state.current_round = 0;
        state.round_results.push(RoundResult {
            wait_time_ms: 250.0,
            reaction_time_ms: 125.0,
        });
        let result = state.handle_click_at(Instant::now());
        assert!(result.is_some());
        assert!(matches!(state.screen, AppScreen::End));
    }
}
