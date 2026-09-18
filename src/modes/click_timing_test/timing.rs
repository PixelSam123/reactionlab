use super::types::{Configurables, RoundOutcome, StopBehavior, TargetDirection};

#[derive(Clone)]
pub struct MotionPlan {
    pub direction: TargetDirection,
    pub stop_behavior: StopBehavior,
    pub initial_velocity_fraction: f64,
    source_offset: f64,
    movement_sign: f64,
    center_progress: f64,
    path_distance: f64,
    full_speed: f64,
    initial_velocity: f64,
    acceleration_duration_ms: f64,
    acceleration_linearity: f64,
    deceleration_duration_ms: f64,
    deceleration_linearity: f64,
    acceleration_distance: f64,
    plateau_duration_ms: f64,
}

impl MotionPlan {
    pub fn new(
        config: &Configurables,
        direction: TargetDirection,
        stop_behavior: StopBehavior,
        initial_velocity_fraction: f64,
    ) -> Self {
        let left = f64::from(config.left_wall_distance.max(1.0));
        let right = f64::from(config.right_wall_distance.max(1.0));
        let center_progress = match direction {
            TargetDirection::FromLeft => left,
            TargetDirection::FromRight => right,
        };
        let (source_offset, movement_sign) = match direction {
            TargetDirection::FromLeft => (-left, 1.0),
            TargetDirection::FromRight => (right, -1.0),
        };
        let path_distance = match stop_behavior {
            StopBehavior::StopOnCrosshair => center_progress,
            StopBehavior::ContinueToOppositeWall => left + right,
        };
        let full_speed = f64::from(config.full_speed_velocity.max(0.001));
        let initial_velocity_fraction = initial_velocity_fraction.clamp(0.0, 1.0);
        let initial_velocity = full_speed * initial_velocity_fraction;
        let acceleration_linearity = f64::from(config.acceleration_linearity.max(0.001));
        let deceleration_linearity = f64::from(config.deceleration_linearity.max(0.001));
        let requested_acceleration_ms = config.acceleration_duration_ms as f64;
        let requested_deceleration_ms = config.deceleration_duration_ms as f64;

        let requested_acceleration_distance = acceleration_distance(
            initial_velocity,
            full_speed,
            requested_acceleration_ms,
            acceleration_linearity,
        );
        let requested_deceleration_distance = deceleration_distance(
            full_speed,
            requested_deceleration_ms,
            deceleration_linearity,
        );
        let requested_phases_distance =
            requested_acceleration_distance + requested_deceleration_distance;
        let phase_scale = if requested_phases_distance > path_distance {
            path_distance / requested_phases_distance
        } else {
            1.0
        };
        let acceleration_duration_ms = requested_acceleration_ms * phase_scale;
        let deceleration_duration_ms = requested_deceleration_ms * phase_scale;
        let acceleration_distance = acceleration_distance(
            initial_velocity,
            full_speed,
            acceleration_duration_ms,
            acceleration_linearity,
        );
        let deceleration_distance =
            deceleration_distance(full_speed, deceleration_duration_ms, deceleration_linearity);
        let plateau_distance =
            (path_distance - acceleration_distance - deceleration_distance).max(0.0);
        let plateau_duration_ms = plateau_distance / full_speed * 1000.0;

        Self {
            direction,
            stop_behavior,
            initial_velocity_fraction,
            source_offset,
            movement_sign,
            center_progress,
            path_distance,
            full_speed,
            initial_velocity,
            acceleration_duration_ms,
            acceleration_linearity,
            deceleration_duration_ms,
            deceleration_linearity,
            acceleration_distance,
            plateau_duration_ms,
        }
    }

    pub fn total_duration_ms(&self) -> f64 {
        self.acceleration_duration_ms + self.plateau_duration_ms + self.deceleration_duration_ms
    }

    pub fn center_crossing_ms(&self) -> f64 {
        self.time_for_progress(self.center_progress)
    }

    pub fn miss_deadline_ms(&self, notify_miss_after_ms: u64) -> f64 {
        self.center_crossing_ms() + notify_miss_after_ms as f64
    }

    pub fn progress_at(&self, elapsed_ms: f64) -> f64 {
        let elapsed_ms = elapsed_ms.max(0.0);
        if elapsed_ms <= self.acceleration_duration_ms {
            return acceleration_position(
                self.initial_velocity,
                self.full_speed,
                elapsed_ms,
                self.acceleration_duration_ms,
                self.acceleration_linearity,
            )
            .min(self.acceleration_distance);
        }

        let after_acceleration = elapsed_ms - self.acceleration_duration_ms;
        if after_acceleration <= self.plateau_duration_ms {
            return (self.acceleration_distance + self.full_speed * after_acceleration / 1000.0)
                .min(self.path_distance);
        }

        let after_plateau = after_acceleration - self.plateau_duration_ms;
        (self.acceleration_distance
            + self.plateau_duration_ms / 1000.0 * self.full_speed
            + deceleration_position(
                self.full_speed,
                after_plateau,
                self.deceleration_duration_ms,
                self.deceleration_linearity,
            ))
        .min(self.path_distance)
    }

    pub fn x_at(&self, center_x: f32, elapsed_ms: f64) -> f32 {
        (f64::from(center_x)
            + self.source_offset
            + self.movement_sign * self.progress_at(elapsed_ms)) as f32
    }

    pub fn distance_from_crosshair(&self, elapsed_ms: f64) -> f64 {
        (self.progress_at(elapsed_ms) - self.center_progress).abs()
    }

    pub fn is_before_crosshair(&self, elapsed_ms: f64) -> bool {
        self.progress_at(elapsed_ms) < self.center_progress
    }

    pub fn time_for_progress(&self, target: f64) -> f64 {
        let target = target.clamp(0.0, self.path_distance);
        let mut low = 0.0;
        let mut high = self.total_duration_ms();
        for _ in 0..64 {
            let middle = f64::midpoint(low, high);
            if self.progress_at(middle) < target {
                low = middle;
            } else {
                high = middle;
            }
        }
        f64::midpoint(low, high)
    }

    pub fn classify_click(
        &self,
        elapsed_ms: f64,
        visual_radius: f32,
        clickable_radius: f32,
    ) -> RoundOutcome {
        let distance = self.distance_from_crosshair(elapsed_ms);
        let visual_radius = f64::from(visual_radius.max(0.0));
        let clickable_radius = f64::from(clickable_radius.max(0.0));
        if distance <= clickable_radius {
            RoundOutcome::Hit
        } else if distance <= visual_radius {
            if self.is_before_crosshair(elapsed_ms) {
                RoundOutcome::AlmostThereTooSoon
            } else {
                RoundOutcome::AlmostThereTooLate
            }
        } else if self.is_before_crosshair(elapsed_ms) {
            RoundOutcome::TooSoon
        } else {
            RoundOutcome::TooLateClick
        }
    }
}

fn acceleration_distance(v0: f64, full_speed: f64, duration_ms: f64, linearity: f64) -> f64 {
    let duration_seconds = duration_ms / 1000.0;
    duration_seconds * (v0 + (full_speed - v0) / (linearity + 1.0))
}

fn deceleration_distance(full_speed: f64, duration_ms: f64, linearity: f64) -> f64 {
    duration_ms / 1000.0 * full_speed * linearity / (linearity + 1.0)
}

fn acceleration_position(
    v0: f64,
    full_speed: f64,
    elapsed_ms: f64,
    duration_ms: f64,
    linearity: f64,
) -> f64 {
    if duration_ms <= 0.0 {
        return 0.0;
    }
    let progress = (elapsed_ms / duration_ms).clamp(0.0, 1.0);
    let elapsed_seconds = elapsed_ms / 1000.0;
    elapsed_seconds * v0
        + duration_ms / 1000.0 * (full_speed - v0) * progress.powf(linearity + 1.0)
            / (linearity + 1.0)
}

fn deceleration_position(
    full_speed: f64,
    elapsed_ms: f64,
    duration_ms: f64,
    linearity: f64,
) -> f64 {
    if duration_ms <= 0.0 {
        return 0.0;
    }
    let progress = (elapsed_ms / duration_ms).clamp(0.0, 1.0);
    full_speed * duration_ms / 1000.0
        * (progress - progress.powf(linearity + 1.0) / (linearity + 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(stop_behavior: StopBehavior) -> MotionPlan {
        let config = Configurables {
            left_wall_distance: 100.0,
            right_wall_distance: 100.0,
            full_speed_velocity: 100.0,
            acceleration_duration_ms: 0,
            deceleration_duration_ms: 0,
            ..Default::default()
        };
        MotionPlan::new(&config, TargetDirection::FromLeft, stop_behavior, 1.0)
    }

    #[test]
    fn linear_motion_reaches_center_and_endpoint() {
        let motion = plan(StopBehavior::ContinueToOppositeWall);
        assert!((motion.progress_at(1000.0) - 100.0).abs() < 0.01);
        assert!((motion.progress_at(2000.0) - 200.0).abs() < 0.01);
        assert!((motion.center_crossing_ms() - 1000.0).abs() < 0.01);
    }

    #[test]
    fn phase_durations_are_scaled_when_distance_is_short() {
        let config = Configurables {
            left_wall_distance: 10.0,
            right_wall_distance: 10.0,
            full_speed_velocity: 100.0,
            acceleration_duration_ms: 1000,
            deceleration_duration_ms: 1000,
            ..Default::default()
        };
        let motion = MotionPlan::new(
            &config,
            TargetDirection::FromLeft,
            StopBehavior::StopOnCrosshair,
            0.0,
        );
        assert!((motion.progress_at(motion.total_duration_ms()) - 10.0).abs() < 0.01);
    }

    #[test]
    fn click_outcomes_use_visual_and_clickable_radii() {
        let motion = plan(StopBehavior::ContinueToOppositeWall);
        let center = motion.center_crossing_ms();
        assert_eq!(
            motion.classify_click(center - 200.0, 10.0, 5.0),
            RoundOutcome::TooSoon
        );
        assert_eq!(
            motion.classify_click(center - 60.0, 10.0, 5.0),
            RoundOutcome::AlmostThereTooSoon
        );
        assert_eq!(motion.classify_click(center, 10.0, 5.0), RoundOutcome::Hit);
        assert_eq!(
            motion.classify_click(center + 60.0, 10.0, 5.0),
            RoundOutcome::AlmostThereTooLate
        );
        assert_eq!(
            motion.classify_click(center + 200.0, 10.0, 5.0),
            RoundOutcome::TooLateClick
        );
    }
}
