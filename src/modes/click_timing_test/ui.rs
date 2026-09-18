#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap
)]

use std::time::Instant;

use chrono::{Datelike, Local, NaiveDate};
use eframe::egui::{self, CentralPanel, Color32, Frame, Painter, Pos2, Rect, Stroke};
use egui_plot::{AxisHints, Bar, BarChart, HoverPosition, Line, Plot, PlotPoints};

use super::state::{AppState, compute_mean, compute_median};
use super::storage;
use super::timing::MotionPlan;
use super::types::{
    AppScreen, Configurables, CrosshairStyle, FalseClickAction, InitialVelocityMode, RoundOutcome,
    RoundResult, RoundState, RunData, RunFileInfo,
};

const MAX_CONTENT_WIDTH: f32 = 1000.0;
const START_STACK_WIDTH: f32 = 600.0;
const WALL_WIDTH: f32 = 8.0;

pub struct ClickTimingTest {
    state: AppState,
    ui_state: UiState,
}

struct UiState {
    show_settings: bool,
    show_all_runs: bool,
    run_file_list: Vec<RunFileInfo>,
    viewed_run_filename: Option<String>,
    viewed_run_data: Option<RunData>,
    delete_year: u32,
    delete_month: u32,
    delete_day: u32,
    last_start_panel_size: Option<egui::Vec2>,
    last_end_panel_size: Option<egui::Vec2>,
    /// Height of the actions column measured on the previous frame, used to
    /// vertically center it against the freshly measured history column.
    last_actions_column_height: f32,
}

impl UiState {
    fn new() -> Self {
        let today = Local::now().date_naive();
        Self {
            show_settings: false,
            show_all_runs: false,
            run_file_list: Vec::new(),
            viewed_run_filename: None,
            viewed_run_data: None,
            delete_year: today.year().cast_unsigned(),
            delete_month: today.month(),
            delete_day: today.day(),
            last_start_panel_size: None,
            last_end_panel_size: None,
            last_actions_column_height: 0.0,
        }
    }
}

impl ClickTimingTest {
    pub fn new() -> Self {
        Self {
            state: AppState::new(storage::load_config(), storage::load_history_summary()),
            ui_state: UiState::new(),
        }
    }

    pub fn reset_to_start(&mut self) {
        storage::save_config(&self.state.config);
        self.state.reset_to_start();
        self.ui_state.show_settings = false;
        self.ui_state.show_all_runs = false;
        self.ui_state.viewed_run_filename = None;
        self.ui_state.viewed_run_data = None;
        self.ui_state.run_file_list.clear();
    }

    fn persist_finished_run(&mut self, run_data: RunData) {
        storage::save_run(&run_data);
        self.state.history_means = storage::load_history_summary();
    }

    fn draw_settings(&mut self, ui: &egui::Ui) {
        egui::Window::new("Settings")
            .open(&mut self.ui_state.show_settings)
            .order(egui::Order::Foreground)
            .default_pos(ui.ctx().content_rect().center())
            .pivot(egui::Align2::CENTER_CENTER)
            .default_width(560.0)
            .show(ui, |ui| {
                ui.heading("Crosshair");
                ui.horizontal(|ui| {
                    ui.label("Style:");
                    egui::ComboBox::from_id_salt("click-timing-crosshair-style")
                        .selected_text(crosshair_style_label(self.state.config.crosshair_style))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.state.config.crosshair_style,
                                CrosshairStyle::Regular,
                                "Regular",
                            );
                            ui.selectable_value(
                                &mut self.state.config.crosshair_style,
                                CrosshairStyle::Plus,
                                "Plus",
                            );
                            ui.selectable_value(
                                &mut self.state.config.crosshair_style,
                                CrosshairStyle::CircularDot,
                                "Circular dot",
                            );
                        });
                });
                Self::color_row(
                    ui,
                    "Color:",
                    self.state.config.crosshair_color_egui(),
                    |config, color| config.set_crosshair_color(color),
                    &mut self.state.config,
                );
                Self::color_row(
                    ui,
                    "Outline color:",
                    self.state.config.crosshair_outline_color_egui(),
                    |config, color| config.set_crosshair_outline_color(color),
                    &mut self.state.config,
                );
                drag_f32(
                    ui,
                    "Outline thickness (px):",
                    &mut self.state.config.crosshair_outline_thickness,
                    0.0..=25.0,
                );

                match self.state.config.crosshair_style {
                    CrosshairStyle::Regular => {
                        drag_f32(
                            ui,
                            "Length (px):",
                            &mut self.state.config.regular_length,
                            1.0..=500.0,
                        );
                        drag_f32(
                            ui,
                            "Thickness (px):",
                            &mut self.state.config.regular_thickness,
                            0.5..=100.0,
                        );
                        drag_f32(
                            ui,
                            "Center gap (px):",
                            &mut self.state.config.regular_center_gap,
                            0.0..=100.0,
                        );
                        ui.checkbox(&mut self.state.config.regular_t_style, "T style");
                        ui.checkbox(
                            &mut self.state.config.regular_center_square_dot,
                            "Center square dot",
                        );
                    }
                    CrosshairStyle::Plus => {
                        drag_f32(
                            ui,
                            "Length (px):",
                            &mut self.state.config.plus_length,
                            1.0..=500.0,
                        );
                        drag_f32(
                            ui,
                            "Thickness (px):",
                            &mut self.state.config.plus_thickness,
                            0.5..=100.0,
                        );
                    }
                    CrosshairStyle::CircularDot => {
                        drag_f32(
                            ui,
                            "Diameter (px):",
                            &mut self.state.config.dot_diameter,
                            1.0..=300.0,
                        );
                    }
                }

                ui.separator();
                ui.heading("Walls");
                drag_f32(
                    ui,
                    "Left distance from center (px):",
                    &mut self.state.config.left_wall_distance,
                    1.0..=5000.0,
                );
                drag_f32(
                    ui,
                    "Right distance from center (px):",
                    &mut self.state.config.right_wall_distance,
                    1.0..=5000.0,
                );

                ui.separator();
                ui.heading("Target");
                drag_f32(
                    ui,
                    "Visual radius (px):",
                    &mut self.state.config.target_visual_radius,
                    0.5..=300.0,
                );
                drag_f32(
                    ui,
                    "Clickable radius (px):",
                    &mut self.state.config.target_clickable_radius,
                    0.5..=300.0,
                );
                Self::color_row(
                    ui,
                    "Color:",
                    self.state.config.target_color_egui(),
                    |config, color| config.set_target_color(color),
                    &mut self.state.config,
                );
                ui.checkbox(
                    &mut self.state.config.stop_on_crosshair,
                    "Always stop on crosshair",
                );
                drag_f32(
                    ui,
                    "Full speed velocity (px/s):",
                    &mut self.state.config.full_speed_velocity,
                    1.0..=10000.0,
                );
                ui.horizontal(|ui| {
                    ui.label("Initial velocity:");
                    ui.radio_value(
                        &mut self.state.config.initial_velocity,
                        InitialVelocityMode::Full,
                        "Full velocity",
                    );
                    ui.radio_value(
                        &mut self.state.config.initial_velocity,
                        InitialVelocityMode::Mixed,
                        "Mixed",
                    );
                    ui.radio_value(
                        &mut self.state.config.initial_velocity,
                        InitialVelocityMode::Zero,
                        "Zero",
                    );
                });
                drag_u64(
                    ui,
                    "Acceleration duration (ms):",
                    &mut self.state.config.acceleration_duration_ms,
                    0..=10000,
                );
                drag_f32(
                    ui,
                    "Acceleration linearity:",
                    &mut self.state.config.acceleration_linearity,
                    0.1..=10.0,
                );
                drag_u64(
                    ui,
                    "Deceleration duration (ms):",
                    &mut self.state.config.deceleration_duration_ms,
                    0..=10000,
                );
                drag_f32(
                    ui,
                    "Deceleration linearity:",
                    &mut self.state.config.deceleration_linearity,
                    0.1..=10.0,
                );
                drag_u64(
                    ui,
                    "Notify miss after (ms):",
                    &mut self.state.config.notify_miss_after_ms,
                    0..=10000,
                );

                ui.horizontal(|ui| {
                    ui.label("Target direction:");
                    ui.checkbox(&mut self.state.config.target_direction_left, "Left");
                    ui.checkbox(&mut self.state.config.target_direction_right, "Right");
                });

                ui.separator();
                ui.heading("Run");
                drag_u64(
                    ui,
                    "Minimum wait (ms):",
                    &mut self.state.config.min_wait_ms,
                    0..=60000,
                );
                drag_u64(
                    ui,
                    "Maximum wait (ms):",
                    &mut self.state.config.max_wait_ms,
                    0..=60000,
                );
                if self.state.config.min_wait_ms > self.state.config.max_wait_ms {
                    self.state.config.max_wait_ms = self.state.config.min_wait_ms;
                }
                drag_usize(
                    ui,
                    "Round count:",
                    &mut self.state.config.round_count,
                    1..=100,
                );
                ui.horizontal(|ui| {
                    ui.label("On failure:");
                    ui.radio_value(
                        &mut self.state.config.false_click_action,
                        FalseClickAction::RetryRound,
                        "Retry round",
                    );
                    ui.radio_value(
                        &mut self.state.config.false_click_action,
                        FalseClickAction::EndRun,
                        "End run",
                    );
                });

                for message in self.state.config.validation_messages() {
                    ui.colored_label(Color32::YELLOW, message);
                }

                ui.separator();
                if ui.button("Reset to defaults").clicked() {
                    self.state.config = Configurables::default();
                }

                ui.separator();
                ui.label("Delete runs before:");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.ui_state.delete_year)
                            .range(2000..=2100)
                            .speed(1),
                    );
                    ui.label("-");
                    ui.add(
                        egui::DragValue::new(&mut self.ui_state.delete_month)
                            .range(1..=12)
                            .speed(1),
                    );
                    ui.label("-");
                    ui.add(
                        egui::DragValue::new(&mut self.ui_state.delete_day)
                            .range(1..=31)
                            .speed(1),
                    );
                });
                if ui.button("Delete").clicked()
                    && let Some(date) = NaiveDate::from_ymd_opt(
                        self.ui_state.delete_year as i32,
                        self.ui_state.delete_month,
                        self.ui_state.delete_day,
                    )
                {
                    storage::delete_runs_before(date);
                    self.state.history_means = storage::load_history_summary();
                    self.ui_state.run_file_list = storage::list_run_files();
                }
            });
    }

    fn color_row(
        ui: &mut egui::Ui,
        label: &str,
        mut color: Color32,
        set_color: impl FnOnce(&mut Configurables, Color32),
        config: &mut Configurables,
    ) {
        ui.horizontal(|ui| {
            ui.label(label);
            if ui.color_edit_button_srgba(&mut color).changed() {
                set_color(config, color);
            }
        });
    }

    fn draw_all_runs(&mut self, ui: &egui::Ui) {
        egui::Window::new("All Runs")
            .open(&mut self.ui_state.show_all_runs)
            .order(egui::Order::Foreground)
            .default_pos(ui.ctx().content_rect().center())
            .pivot(egui::Align2::CENTER_CENTER)
            .default_width(650.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for run_file in &self.ui_state.run_file_list.clone() {
                        let selected = self.ui_state.viewed_run_filename.as_deref()
                            == Some(&run_file.filename);
                        if ui
                            .selectable_label(selected, &run_file.display_name)
                            .clicked()
                        {
                            if selected {
                                self.ui_state.viewed_run_filename = None;
                                self.ui_state.viewed_run_data = None;
                            } else {
                                self.ui_state.viewed_run_data =
                                    storage::load_run_data(&run_file.filename);
                                self.ui_state.viewed_run_filename = Some(run_file.filename.clone());
                            }
                        }
                        if selected && let Some(ref data) = self.ui_state.viewed_run_data {
                            ui.indent("click-timing-run-detail", |ui| {
                                Self::draw_run_details(ui, data);
                            });
                        }
                    }
                    if self.ui_state.run_file_list.is_empty() {
                        ui.label("No saved runs yet.");
                    }
                });
            });
    }

    fn draw_run_details(ui: &mut egui::Ui, run_data: &RunData) {
        ui.label(format!("Timestamp: {}", run_data.timestamp));
        ui.label(format!(
            "Walls: {:.0}px left, {:.0}px right",
            run_data.config.left_wall_distance, run_data.config.right_wall_distance
        ));
        ui.label(format!(
            "Target: visual {:.1}px, clickable {:.1}px, speed {:.0}px/s",
            run_data.config.target_visual_radius,
            run_data.config.target_clickable_radius,
            run_data.config.full_speed_velocity
        ));
        ui.label(format!(
            "Round count setting: {}",
            run_data.config.round_count
        ));

        let errors = successful_errors(&run_data.attempts);
        let hit_count = run_data
            .attempts
            .iter()
            .filter(|attempt| attempt.outcome.is_hit())
            .count();
        let hit_rate = if run_data.attempts.is_empty() {
            0.0
        } else {
            hit_count as f64 / run_data.attempts.len() as f64 * 100.0
        };
        ui.separator();
        ui.label(format!(
            "Completed rounds: {} / {}",
            hit_count, run_data.config.round_count
        ));
        ui.label(format!(
            "Hit rate: {hit_rate:.0}% ({hit_count}/{} attempts)",
            run_data.attempts.len()
        ));
        if errors.is_empty() {
            ui.label("No successful timing errors to summarize.");
        } else {
            ui.label(format!(
                "Mean absolute error: {:.0} ms",
                compute_mean(&errors)
            ));
            ui.label(format!(
                "Median absolute error: {:.0} ms",
                compute_median(&errors)
            ));
        }

        ui.separator();
        for (index, attempt) in run_data.attempts.iter().enumerate() {
            let click = attempt
                .click_offset_ms
                .map(|offset| format!(", offset {offset:+.0} ms"))
                .unwrap_or_default();
            ui.label(format!(
                "Attempt {} (round {}): {}, {}{}, {}",
                attempt.attempt_number.max(index + 1),
                attempt.round_number.max(index + 1),
                attempt.outcome.label(),
                attempt.direction.label(),
                click,
                attempt.stop_behavior.label()
            ));
        }
        Self::draw_bar_chart(ui, &run_data.attempts);
    }

    fn draw_bar_chart(ui: &mut egui::Ui, attempts: &[RoundResult]) {
        let bars: Vec<Bar> = attempts
            .iter()
            .enumerate()
            .map(|(index, attempt)| {
                let value = attempt.click_offset_ms.map_or(0.0, f64::abs);
                Bar::new((index + 1) as f64, value)
                    .name(format!("A{}", index + 1))
                    .fill(if attempt.outcome.is_hit() {
                        Color32::from_rgb(100, 180, 255)
                    } else {
                        Color32::from_rgb(180, 90, 90)
                    })
                    .width(0.8)
            })
            .collect();
        let chart = BarChart::new("Timing errors", bars)
            .horizontal()
            .color(Color32::from_rgb(100, 180, 255))
            .width(0.8)
            .element_formatter(Box::new(|bar, _| {
                format!("{}: {:.0} ms", bar.name, bar.value)
            }));
        Plot::new("click-timing-errors")
            .height((attempts.len() as f32 * 30.0).max(120.0))
            .include_x(0.0)
            .sense(egui::Sense::hover())
            .allow_drag(false)
            .allow_scroll(false)
            .allow_zoom(false)
            .allow_boxed_zoom(false)
            .allow_axis_zoom_drag(false)
            .show_crosshair(false)
            .x_axis_formatter(|mark, _| format!("{:.0} ms", mark.value))
            .show(ui, |plot_ui| plot_ui.bar_chart(chart));
    }

    fn draw_line_chart(ui: &mut egui::Ui, data: &[f64]) {
        if data.is_empty() {
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), 200.0),
                egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                |ui| {
                    ui.label("No data");
                },
            );
            return;
        }
        let points: PlotPoints = data
            .iter()
            .enumerate()
            .map(|(index, &value)| [(index + 1) as f64, value])
            .collect();
        let line = Line::new("Mean absolute timing error", points)
            .color(Color32::from_rgb(100, 180, 255))
            .width(2.0);
        Plot::new("click-timing-history")
            .height(200.0)
            .sense(egui::Sense::hover())
            .allow_drag(false)
            .allow_scroll(false)
            .allow_zoom(false)
            .allow_boxed_zoom(false)
            .allow_axis_zoom_drag(false)
            .show_crosshair(false)
            .x_grid_spacer(|_| {
                (1..=data.len())
                    .map(|value| GridMark {
                        value: value as f64,
                        step_size: 1.0,
                    })
                    .collect()
            })
            .custom_x_axes(vec![
                AxisHints::new_x()
                    .formatter(|mark, _| format!("{:.0}", mark.value))
                    .label_spacing(0.0..=1.0),
            ])
            .y_axis_formatter(|mark, _| format!("{:.0} ms", mark.value))
            .label_formatter(|pos| match pos {
                HoverPosition::NearDataPoint {
                    plot_name: _,
                    position,
                    index,
                } => Some(format!("R{}: {:.0} ms", index + 1, position.y)),
                HoverPosition::Elsewhere { .. } => None,
            })
            .show(ui, |plot_ui| plot_ui.line(line));
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        if self.ui_state.last_start_panel_size.is_some() && self.state.screen != AppScreen::Start {
            self.ui_state.last_start_panel_size = None;
        }
        if self.ui_state.last_end_panel_size.is_some() && self.state.screen != AppScreen::End {
            self.ui_state.last_end_panel_size = None;
        }

        if self.state.screen == AppScreen::Round
            && self.state.round_state != RoundState::ResultShowing
        {
            ui.ctx().request_repaint();
        }
        if self.ui_state.show_settings {
            self.draw_settings(ui);
        }
        if self.ui_state.show_all_runs {
            self.draw_all_runs(ui);
        }
        match self.state.screen {
            AppScreen::Start => self.draw_start(ui),
            AppScreen::Round => self.draw_round(ui),
            AppScreen::End => self.draw_end(ui),
        }
    }

    fn draw_start_history(&mut self, ui: &mut egui::Ui) {
        if self.state.history_means.is_empty() {
            ui.label("No previous successful hits yet.");
        } else {
            let points: Vec<f64> = self
                .state
                .history_means
                .iter()
                .map(|(_, mean)| *mean)
                .collect();
            ui.label("Mean absolute timing error");
            Self::draw_line_chart(ui, &points);
        }
        ui.with_layout(
            egui::Layout::top_down(egui::Align::Center).with_cross_justify(true),
            |ui| {
                if ui.button("Show all runs").clicked() {
                    self.ui_state.show_all_runs = true;
                    self.ui_state.viewed_run_filename = None;
                    self.ui_state.viewed_run_data = None;
                    self.ui_state.run_file_list = storage::list_run_files();
                }
            },
        );
    }

    fn draw_start_actions(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            if ui.button("⚙ Settings").clicked() {
                self.ui_state.show_settings = !self.ui_state.show_settings;
                if !self.ui_state.show_settings {
                    storage::save_config(&self.state.config);
                    self.state.history_means = storage::load_history_summary();
                    self.ui_state.run_file_list.clear();
                }
            }
            ui.add_space(20.0);
            if ui.button("Start new run").clicked() {
                self.state.restart_run();
            }
        });
    }

    fn draw_start(&mut self, ui: &mut egui::Ui) {
        CentralPanel::default().show(ui, |ui| {
            let available_width = ui.available_width();
            let content_width = available_width.min(MAX_CONTENT_WIDTH);
            let stack_content = available_width < START_STACK_WIDTH;
            let panel_rect = ui.available_rect_before_wrap();
            let needs_resize = self.ui_state.last_start_panel_size.is_none_or(|last_size| {
                (last_size.x - panel_rect.width()).abs() > 0.5
                    || (last_size.y - panel_rect.height()).abs() > 0.5
            });
            self.ui_state.last_start_panel_size = Some(panel_rect.size());
            egui::Area::new(egui::Id::new("click-timing-start-content"))
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .constrain_to(panel_rect)
                .default_size(egui::vec2(content_width, panel_rect.height()))
                .sizing_pass(needs_resize)
                .show(ui.ctx(), |ui| {
                    ui.set_width(content_width);
                    egui::ScrollArea::vertical()
                        .max_height(panel_rect.height())
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            if stack_content {
                                self.draw_start_history(ui);
                                ui.add_space(20.0);
                                self.draw_start_actions(ui);
                            } else {
                                let column_width =
                                    (content_width - ui.spacing().item_spacing.x).max(0.0) / 2.0;
                                ui.horizontal(|ui| {
                                    let history_height = ui
                                        .vertical(|ui| {
                                            ui.set_width(column_width);
                                            self.draw_start_history(ui);
                                            ui.min_rect().height()
                                        })
                                        .inner;
                                    let actions_height = self.ui_state.last_actions_column_height;
                                    let top_offset =
                                        ((history_height - actions_height) / 2.0).max(0.0);
                                    ui.vertical(|ui| {
                                        ui.set_width(column_width);
                                        ui.add_space(top_offset);
                                        let actions_top = ui.cursor().top();
                                        self.draw_start_actions(ui);
                                        self.ui_state.last_actions_column_height =
                                            ui.min_rect().bottom() - actions_top;
                                    });
                                });
                            }
                        });
                });
        });
    }

    fn draw_round(&mut self, ui: &mut egui::Ui) {
        let now = Instant::now();
        CentralPanel::default()
            .frame(Frame::NONE.fill(Color32::from_rgb(24, 24, 28)))
            .show(ui, |ui| {
                let canvas_rect = ui.available_rect_before_wrap();
                let pressed = ui.input(|input| input.pointer.primary_pressed());
                if let Some(run_data) = self.state.update_at(now, pressed) {
                    self.persist_finished_run(run_data);
                }

                let painter = ui.painter_at(canvas_rect);
                let center = canvas_rect.center();
                let wall_color = lighter_color(Color32::from_rgb(24, 24, 28));
                let left_wall = center.x - self.state.config.left_wall_distance;
                let right_wall = center.x + self.state.config.right_wall_distance;
                painter.rect_filled(
                    Rect::from_min_max(
                        Pos2::new(left_wall - WALL_WIDTH / 2.0, canvas_rect.top()),
                        Pos2::new(left_wall + WALL_WIDTH / 2.0, canvas_rect.bottom()),
                    ),
                    0.0,
                    wall_color,
                );
                painter.rect_filled(
                    Rect::from_min_max(
                        Pos2::new(right_wall - WALL_WIDTH / 2.0, canvas_rect.top()),
                        Pos2::new(right_wall + WALL_WIDTH / 2.0, canvas_rect.bottom()),
                    ),
                    0.0,
                    wall_color,
                );
                if self.state.round_state != RoundState::ResultShowing {
                    draw_crosshair(&painter, center, &self.state.config);
                }

                if matches!(self.state.round_state, RoundState::Moving)
                    && let (Some(plan), Some(elapsed)) = (
                        self.state.plan.as_ref(),
                        self.state.movement_elapsed_ms(now),
                    )
                {
                    draw_target(
                        &painter,
                        plan,
                        center,
                        elapsed,
                        self.state.config.target_visual_radius,
                        self.state.config.target_color_egui(),
                    );
                }

                if self.state.config.left_wall_distance
                    + self.state.config.right_wall_distance
                    + self.state.config.target_visual_radius * 2.0
                    > canvas_rect.width()
                {
                    painter.text(
                        Pos2::new(canvas_rect.left() + 10.0, canvas_rect.top() + 10.0),
                        egui::Align2::LEFT_TOP,
                        "Wall distances exceed the current playfield width",
                        egui::FontId::proportional(14.0),
                        Color32::YELLOW,
                    );
                }

                if self.state.round_state == RoundState::ResultShowing
                    && let Some(result) = &self.state.last_result
                {
                    draw_result_overlay(
                        &painter,
                        canvas_rect,
                        result,
                        self.state.config.false_click_action,
                    );
                }
            });
    }

    fn draw_end(&mut self, ui: &mut egui::Ui) {
        CentralPanel::default().show(ui, |ui| {
            let available_width = ui.available_width();
            let content_width = available_width.min(MAX_CONTENT_WIDTH);
            let panel_rect = ui.available_rect_before_wrap();
            let needs_resize = self.ui_state.last_end_panel_size.is_none_or(|last_size| {
                (last_size.x - panel_rect.width()).abs() > 0.5
                    || (last_size.y - panel_rect.height()).abs() > 0.5
            });
            self.ui_state.last_end_panel_size = Some(panel_rect.size());
            egui::Area::new(egui::Id::new("click-timing-end-content"))
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .constrain_to(panel_rect)
                .default_size(egui::vec2(content_width, panel_rect.height()))
                .sizing_pass(needs_resize)
                .show(ui.ctx(), |ui| {
                    ui.set_width(content_width);
                    egui::ScrollArea::vertical()
                        .max_height(panel_rect.height())
                        .auto_shrink([true, true])
                        .show(ui, |ui| {
                            ui.heading("Run Results");
                            ui.separator();
                            let errors = successful_errors(&self.state.attempt_results);
                            let hit_count = self
                                .state
                                .attempt_results
                                .iter()
                                .filter(|attempt| attempt.outcome.is_hit())
                                .count();
                            let total = self.state.attempt_results.len();
                            let hit_rate = if total == 0 {
                                0.0
                            } else {
                                hit_count as f64 / total as f64 * 100.0
                            };
                            ui.label(format!(
                                "Completed rounds: {} / {}",
                                hit_count, self.state.config.round_count
                            ));
                            ui.label(format!(
                                "Hit rate: {hit_rate:.0}% ({hit_count}/{total} attempts)"
                            ));
                            if errors.is_empty() {
                                ui.label("No successful timing errors to summarize.");
                            } else {
                                ui.label(format!(
                                    "Mean absolute error: {:.0} ms",
                                    compute_mean(&errors)
                                ));
                                ui.label(format!(
                                    "Median absolute error: {:.0} ms",
                                    compute_median(&errors)
                                ));
                            }
                            ui.separator();
                            for (index, attempt) in self.state.attempt_results.iter().enumerate() {
                                ui.label(format!(
                                    "Attempt {} (round {}): {} ({})",
                                    attempt.attempt_number.max(index + 1),
                                    attempt.round_number.max(index + 1),
                                    attempt.outcome.label(),
                                    attempt.direction.label()
                                ));
                            }
                            ui.separator();
                            Self::draw_bar_chart(ui, &self.state.attempt_results);
                            ui.separator();
                            ui.vertical_centered(|ui| {
                                if ui.button("Try again").clicked() {
                                    self.state.go_to_start();
                                }
                            });
                        });
                });
        });
    }
}

fn drag_f32(ui: &mut egui::Ui, label: &str, value: &mut f32, range: std::ops::RangeInclusive<f32>) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(value).range(range).speed(0.5));
    });
}

fn drag_u64(ui: &mut egui::Ui, label: &str, value: &mut u64, range: std::ops::RangeInclusive<u64>) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(value).range(range).speed(1));
    });
}

fn drag_usize(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut usize,
    range: std::ops::RangeInclusive<usize>,
) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(value).range(range).speed(1));
    });
}

const fn crosshair_style_label(style: CrosshairStyle) -> &'static str {
    match style {
        CrosshairStyle::Regular => "Regular",
        CrosshairStyle::Plus => "Plus",
        CrosshairStyle::CircularDot => "Circular dot",
    }
}

fn draw_target(
    painter: &Painter,
    plan: &MotionPlan,
    center: Pos2,
    elapsed_ms: f64,
    radius: f32,
    color: Color32,
) {
    painter.circle_filled(
        Pos2::new(plan.x_at(center.x, elapsed_ms), center.y),
        radius.max(0.5),
        color,
    );
}

fn draw_crosshair(painter: &Painter, center: Pos2, config: &Configurables) {
    let color = config.crosshair_color_egui();
    let outline_color = config.crosshair_outline_color_egui();
    let outline = config.crosshair_outline_thickness.max(0.0);
    match config.crosshair_style {
        CrosshairStyle::Regular => {
            let length = config.regular_length.max(0.5);
            let thickness = config.regular_thickness.max(0.5);
            let gap = config.regular_center_gap.max(0.0);
            let segments = [
                (
                    Pos2::new(center.x - length, center.y),
                    Pos2::new(center.x - gap, center.y),
                ),
                (
                    Pos2::new(center.x + gap, center.y),
                    Pos2::new(center.x + length, center.y),
                ),
                (
                    Pos2::new(center.x, center.y + gap),
                    Pos2::new(center.x, center.y + length),
                ),
                (
                    Pos2::new(center.x, center.y - gap),
                    Pos2::new(center.x, center.y - length),
                ),
            ];
            for (start, end) in segments {
                if config.regular_t_style && end.y < center.y {
                    continue;
                }
                draw_stroked_segment(
                    painter,
                    start,
                    end,
                    thickness,
                    outline,
                    color,
                    outline_color,
                );
            }
            if config.regular_center_square_dot {
                painter.rect_filled(
                    Rect::from_center_size(center, egui::vec2(thickness, thickness)),
                    0.0,
                    color,
                );
                if outline > 0.0 {
                    painter.rect_stroke(
                        Rect::from_center_size(center, egui::vec2(thickness, thickness)),
                        0.0,
                        Stroke::new(outline, outline_color),
                        egui::epaint::StrokeKind::Outside,
                    );
                }
            }
        }
        CrosshairStyle::Plus => {
            let half_length = config.plus_length.max(0.5) / 2.0;
            let thickness = config.plus_thickness.max(0.5);
            draw_stroked_segment(
                painter,
                Pos2::new(center.x - half_length, center.y),
                Pos2::new(center.x + half_length, center.y),
                thickness,
                outline,
                color,
                outline_color,
            );
            draw_stroked_segment(
                painter,
                Pos2::new(center.x, center.y - half_length),
                Pos2::new(center.x, center.y + half_length),
                thickness,
                outline,
                color,
                outline_color,
            );
        }
        CrosshairStyle::CircularDot => {
            let radius = config.dot_diameter.max(0.5) / 2.0;
            if outline > 0.0 {
                painter.circle_filled(center, radius + outline, outline_color);
            }
            painter.circle_filled(center, radius, color);
        }
    }
}

fn draw_stroked_segment(
    painter: &Painter,
    start: Pos2,
    end: Pos2,
    thickness: f32,
    outline: f32,
    color: Color32,
    outline_color: Color32,
) {
    if outline > 0.0 {
        painter.line_segment(
            [start, end],
            Stroke::new(thickness + 2.0 * outline, outline_color),
        );
    }
    painter.line_segment([start, end], Stroke::new(thickness, color));
}

fn draw_result_overlay(
    painter: &Painter,
    canvas: Rect,
    result: &RoundResult,
    false_click_action: FalseClickAction,
) {
    let center = canvas.center();
    let (heading, description) = match result.outcome {
        RoundOutcome::Hit => {
            let offset = result.click_offset_ms.unwrap_or(0.0);
            if offset.abs() < 0.5 {
                ("Hit!".to_string(), "Right on the crosshair.".to_string())
            } else if offset < 0.0 {
                (
                    "Hit!".to_string(),
                    format!("{:.0} ms too early.", offset.abs()),
                )
            } else {
                (
                    "Hit!".to_string(),
                    format!("{:.0} ms too late.", offset.abs()),
                )
            }
        }
        RoundOutcome::TooSoon => (
            "Too soon!".to_string(),
            "You clicked before the target reached the visual radius.".to_string(),
        ),
        RoundOutcome::AlmostThereTooSoon => (
            "Almost there!".to_string(),
            "You hit the target, but not within its clickable radius. It was too early."
                .to_string(),
        ),
        RoundOutcome::AlmostThereTooLate => (
            "Almost there!".to_string(),
            "You hit the target, but not within its clickable radius. It was too late.".to_string(),
        ),
        RoundOutcome::TooLateClick => (
            "Too late!".to_string(),
            "You clicked after the target left its visual radius.".to_string(),
        ),
        RoundOutcome::TooLateNoClick => (
            "Too late!".to_string(),
            "The target passed the crosshair without a click.".to_string(),
        ),
    };
    painter.text(
        Pos2::new(center.x, center.y - 32.0),
        egui::Align2::CENTER_CENTER,
        heading,
        egui::FontId::proportional(28.0),
        Color32::WHITE,
    );
    painter.text(
        Pos2::new(center.x, center.y + 8.0),
        egui::Align2::CENTER_CENTER,
        description,
        egui::FontId::proportional(16.0),
        Color32::LIGHT_GRAY,
    );
    let action = if result.outcome.is_hit() {
        "Click to continue"
    } else {
        match false_click_action {
            FalseClickAction::RetryRound => "Click to retry this round",
            FalseClickAction::EndRun => "Click to end the run",
        }
    };
    painter.text(
        Pos2::new(center.x, center.y + 42.0),
        egui::Align2::CENTER_CENTER,
        action,
        egui::FontId::proportional(16.0),
        Color32::WHITE,
    );
}

fn successful_errors(attempts: &[RoundResult]) -> Vec<f64> {
    attempts
        .iter()
        .filter(|attempt| attempt.outcome.is_hit())
        .filter_map(|attempt| attempt.click_offset_ms.map(f64::abs))
        .collect()
}

const fn lighter_color(color: Color32) -> Color32 {
    Color32::from_rgb(
        color.r().saturating_add(35),
        color.g().saturating_add(35),
        color.b().saturating_add(35),
    )
}
