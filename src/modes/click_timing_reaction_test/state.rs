use std::time::Instant;

use chrono::Local;
use rand::RngExt;

use super::timing::MotionPlan;
use super::types::{
    AppScreen, Configurables, FalseClickAction, InitialVelocityMode, RoundOutcome, RoundResult,
    RoundState, RunData, StopBehavior, TargetDirection,
};

pub struct AppState {
    pub screen: AppScreen,
    pub config: Configurables,
    pub round_state: RoundState,
    pub current_round: usize,
    pub attempt_results: Vec<RoundResult>,
    pub wait_start: Option<Instant>,
    pub movement_start: Option<Instant>,
    pub appearance_wait_ms: f64,
    pub plan: Option<MotionPlan>,
    pub pending_result: Option<RoundResult>,
    pub last_result: Option<RoundResult>,
    pub history_means: Vec<(chrono::NaiveDateTime, f64)>,
}

impl AppState {
    pub const fn new(
        config: Configurables,
        history_means: Vec<(chrono::NaiveDateTime, f64)>,
    ) -> Self {
        Self {
            screen: AppScreen::Start,
            config,
            round_state: RoundState::Waiting,
            current_round: 0,
            attempt_results: Vec::new(),
            wait_start: None,
            movement_start: None,
            appearance_wait_ms: 0.0,
            plan: None,
            pending_result: None,
            last_result: None,
            history_means,
        }
    }

    pub fn start_new_round(&mut self) {
        self.start_new_round_at(Instant::now());
    }

    pub fn start_new_round_at(&mut self, now: Instant) {
        let mut rng = rand::rng();
        let min_wait = self.config.min_wait_ms.min(self.config.max_wait_ms);
        let max_wait = self.config.min_wait_ms.max(self.config.max_wait_ms);
        let appearance_wait_ms = rng.random_range(min_wait..=max_wait) as f64;
        let direction = choose_direction(&self.config, &mut rng);
        let stop_behavior = if self.config.stop_on_crosshair || rng.random_range(0..2) == 0 {
            StopBehavior::StopOnCrosshair
        } else {
            StopBehavior::ContinueToOppositeWall
        };
        let initial_velocity_fraction = match self.config.initial_velocity {
            InitialVelocityMode::Full => 1.0,
            InitialVelocityMode::Zero => 0.0,
            InitialVelocityMode::Mixed => rng.random_range(0.0..=1.0),
        };
        let plan = MotionPlan::new(
            &self.config,
            direction,
            stop_behavior,
            initial_velocity_fraction,
        );
        self.start_new_round_with_plan_at(now, appearance_wait_ms, plan);
    }

    pub const fn start_new_round_with_plan_at(
        &mut self,
        now: Instant,
        appearance_wait_ms: f64,
        plan: MotionPlan,
    ) {
        self.round_state = RoundState::Waiting;
        self.wait_start = Some(now);
        self.movement_start = None;
        self.appearance_wait_ms = appearance_wait_ms.max(0.0);
        self.plan = Some(plan);
        self.pending_result = None;
        self.last_result = None;
    }

    pub fn update_at(&mut self, now: Instant, clicked: bool) -> Option<RunData> {
        if self.round_state == RoundState::Waiting {
            let waiting_elapsed = self.wait_start.map_or(0.0, |start| elapsed_ms(now, start));
            if waiting_elapsed >= self.appearance_wait_ms {
                self.round_state = RoundState::Moving;
                self.movement_start = Some(now);
            } else if clicked {
                self.show_result(self.make_result(RoundOutcome::TooSoon, None));
                return None;
            }
        }

        match self.round_state {
            RoundState::Waiting => None,
            RoundState::Moving => {
                let movement_start = self
                    .movement_start
                    .expect("moving state must have a movement start");
                let movement_elapsed = elapsed_ms(now, movement_start);
                let plan = self.plan.as_ref().expect("moving state must have a plan");
                if clicked {
                    let outcome = plan.classify_click(
                        movement_elapsed,
                        self.config.target_visual_radius,
                        self.config.target_clickable_radius,
                    );
                    let click_offset = Some(movement_elapsed - plan.center_crossing_ms());
                    self.show_result(self.make_result(outcome, click_offset));
                } else if movement_elapsed
                    >= plan.miss_deadline_ms(self.config.notify_miss_after_ms)
                {
                    self.show_result(self.make_result(RoundOutcome::TooLateNoClick, None));
                }
                None
            }
            RoundState::ResultShowing => {
                if clicked {
                    self.acknowledge_result(now)
                } else {
                    None
                }
            }
        }
    }

    pub fn movement_elapsed_ms(&self, now: Instant) -> Option<f64> {
        self.movement_start.map(|start| elapsed_ms(now, start))
    }

    pub fn acknowledge_result(&mut self, now: Instant) -> Option<RunData> {
        let result = self
            .pending_result
            .take()
            .expect("result screen must have a pending result");
        if result.outcome.is_hit() {
            self.current_round += 1;
            if self.current_round >= self.config.round_count.max(1) {
                Some(self.finish_run())
            } else {
                self.start_new_round_at(now);
                None
            }
        } else {
            match self.config.false_click_action {
                FalseClickAction::RetryRound => {
                    self.start_new_round_at(now);
                    None
                }
                FalseClickAction::EndRun => Some(self.finish_run()),
            }
        }
    }

    pub fn restart_run(&mut self) {
        self.attempt_results.clear();
        self.current_round = 0;
        self.round_state = RoundState::Waiting;
        self.wait_start = None;
        self.movement_start = None;
        self.appearance_wait_ms = 0.0;
        self.plan = None;
        self.pending_result = None;
        self.last_result = None;
        self.start_new_round();
        self.screen = AppScreen::Round;
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
        self.round_state = RoundState::Waiting;
        self.attempt_results.clear();
        self.current_round = 0;
        self.wait_start = None;
        self.movement_start = None;
        self.appearance_wait_ms = 0.0;
        self.plan = None;
        self.pending_result = None;
        self.last_result = None;
    }

    fn make_result(&self, outcome: RoundOutcome, click_offset_ms: Option<f64>) -> RoundResult {
        let plan = self.plan.as_ref().expect("active round must have a plan");
        RoundResult {
            round_number: self.current_round + 1,
            attempt_number: self.attempt_results.len() + 1,
            direction: plan.direction,
            appearance_wait_ms: self.appearance_wait_ms,
            initial_velocity_fraction: plan.initial_velocity_fraction,
            stop_behavior: plan.stop_behavior,
            center_crossing_ms: plan.center_crossing_ms(),
            click_offset_ms,
            outcome,
        }
    }

    fn show_result(&mut self, result: RoundResult) {
        self.attempt_results.push(result.clone());
        self.last_result = Some(result.clone());
        self.pending_result = Some(result);
        self.round_state = RoundState::ResultShowing;
    }

    fn finish_run(&mut self) -> RunData {
        let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S%.3f").to_string();
        let run_data = RunData {
            timestamp,
            config: self.config.clone(),
            attempts: self.attempt_results.clone(),
        };
        self.screen = AppScreen::End;
        run_data
    }
}

fn choose_direction(config: &Configurables, rng: &mut impl rand::Rng) -> TargetDirection {
    match (config.target_direction_left, config.target_direction_right) {
        (true, false) => TargetDirection::FromLeft,
        (false, true) => TargetDirection::FromRight,
        (true, true) => {
            if rng.random_range(0..2) == 0 {
                TargetDirection::FromLeft
            } else {
                TargetDirection::FromRight
            }
        }
        (false, false) => TargetDirection::FromLeft,
    }
}

fn elapsed_ms(now: Instant, start: Instant) -> f64 {
    now.duration_since(start).as_secs_f64() * 1000.0
}

pub fn compute_mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

pub fn compute_median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let middle = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        f64::midpoint(sorted[middle - 1], sorted[middle])
    } else {
        sorted[middle]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::click_timing_reaction_test::timing::MotionPlan;

    fn state() -> AppState {
        AppState::new(Configurables::default(), Vec::new())
    }

    fn plan(config: &Configurables) -> MotionPlan {
        MotionPlan::new(
            config,
            TargetDirection::FromLeft,
            StopBehavior::ContinueToOppositeWall,
            1.0,
        )
    }

    #[test]
    fn waiting_click_is_too_soon() {
        let mut state = state();
        let now = Instant::now();
        state.start_new_round_with_plan_at(now, 1000.0, plan(&state.config));
        state.update_at(now + std::time::Duration::from_millis(100), true);
        assert_eq!(
            state.last_result.as_ref().unwrap().outcome,
            RoundOutcome::TooSoon
        );
    }

    #[test]
    fn click_at_center_is_a_hit() {
        let mut state = state();
        state.config.acceleration_duration_ms = 0;
        state.config.deceleration_duration_ms = 0;
        state.config.full_speed_velocity = 100.0;
        state.config.left_wall_distance = 100.0;
        state.config.right_wall_distance = 100.0;
        let now = Instant::now();
        let motion = plan(&state.config);
        let center_ms = motion.center_crossing_ms();
        state.start_new_round_with_plan_at(now, 0.0, motion);
        state.update_at(now, false);
        state.update_at(
            now + std::time::Duration::from_secs_f64(center_ms / 1000.0),
            true,
        );
        assert_eq!(
            state.last_result.as_ref().unwrap().outcome,
            RoundOutcome::Hit
        );
    }

    #[test]
    fn miss_deadline_creates_no_click_failure() {
        let mut state = state();
        state.config.acceleration_duration_ms = 0;
        state.config.deceleration_duration_ms = 0;
        state.config.full_speed_velocity = 100.0;
        state.config.left_wall_distance = 100.0;
        state.config.right_wall_distance = 100.0;
        let now = Instant::now();
        let motion = plan(&state.config);
        let deadline = motion.miss_deadline_ms(state.config.notify_miss_after_ms);
        state.start_new_round_with_plan_at(now, 0.0, motion);
        state.update_at(now, false);
        state.update_at(
            now + std::time::Duration::from_secs_f64(deadline / 1000.0),
            false,
        );
        assert_eq!(
            state.last_result.as_ref().unwrap().outcome,
            RoundOutcome::TooLateNoClick
        );
    }

    #[test]
    fn retry_preserves_failure_as_an_attempt_and_keeps_round_number() {
        let mut state = state();
        let now = Instant::now();
        state.start_new_round_with_plan_at(now, 0.0, plan(&state.config));
        state.update_at(now, true);
        state.update_at(now + std::time::Duration::from_millis(1), true);
        assert_eq!(state.attempt_results.len(), 1);
        assert_eq!(state.attempt_results[0].attempt_number, 1);
        assert_eq!(state.attempt_results[0].round_number, 1);
        assert_eq!(state.current_round, 0);
        assert_eq!(state.round_state, RoundState::Waiting);
    }

    #[test]
    fn end_run_persists_failure() {
        let mut state = state();
        state.config.false_click_action = FalseClickAction::EndRun;
        let now = Instant::now();
        state.start_new_round_with_plan_at(now, 0.0, plan(&state.config));
        state.update_at(now, true);
        let run = state.update_at(now + std::time::Duration::from_millis(1), true);
        assert!(run.is_some());
        assert_eq!(run.unwrap().attempts[0].outcome, RoundOutcome::TooSoon);
        assert_eq!(state.screen, AppScreen::End);
    }

    #[test]
    fn retry_then_hit_records_both_attempts() {
        let mut state = state();
        state.config.round_count = 1;
        state.config.acceleration_duration_ms = 0;
        state.config.deceleration_duration_ms = 0;
        state.config.full_speed_velocity = 100.0;
        state.config.left_wall_distance = 100.0;
        state.config.right_wall_distance = 100.0;
        let now = Instant::now();
        let motion = plan(&state.config);
        state.start_new_round_with_plan_at(now, 0.0, motion);
        state.update_at(now, true);
        let retry_time = now + std::time::Duration::from_millis(1);
        state.update_at(retry_time, true);

        let second_motion = plan(&state.config);
        let center_ms = second_motion.center_crossing_ms();
        state.start_new_round_with_plan_at(retry_time, 0.0, second_motion);
        state.update_at(retry_time, false);
        let hit_time = retry_time + std::time::Duration::from_secs_f64(center_ms / 1000.0);
        state.update_at(hit_time, true);
        let run = state
            .update_at(hit_time + std::time::Duration::from_millis(1), true)
            .unwrap();

        assert_eq!(run.attempts.len(), 2);
        assert_eq!(run.attempts[0].outcome, RoundOutcome::TooSoon);
        assert_eq!(run.attempts[1].outcome, RoundOutcome::Hit);
        assert_eq!(run.attempts[1].attempt_number, 2);
        assert_eq!(run.attempts[1].round_number, 1);
    }

    #[test]
    fn computes_mean_and_median() {
        assert_eq!(compute_mean(&[]), 0.0);
        assert_eq!(compute_mean(&[100.0, 200.0, 300.0]), 200.0);
        assert_eq!(compute_median(&[300.0, 100.0, 200.0]), 200.0);
        assert_eq!(compute_median(&[400.0, 100.0, 300.0, 200.0]), 250.0);
    }
}
