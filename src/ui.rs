#![allow(
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap
)]

use chrono::{Datelike, Local, NaiveDate};
use eframe::egui::{self, CentralPanel, Color32, Frame};
use egui_plot::{Bar, BarChart, Line, Plot, PlotPoints};

use crate::state::{AppState, compute_mean, compute_median};
use crate::storage;
use crate::types::{FalseClickAction, RoundResult, RoundState, RunData, RunFileInfo};

const MAX_CONTENT_WIDTH: f32 = 1000.0;

pub struct ReactionLab {
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
            delete_year: today.year() as u32,
            delete_month: today.month(),
            delete_day: today.day(),
        }
    }
}

impl ReactionLab {
    pub fn new() -> Self {
        let state = AppState::new(storage::load_config(), storage::load_history_summary());
        Self {
            state,
            ui_state: UiState::new(),
        }
    }

    fn persist_finished_run(&mut self, run_data: RunData) {
        storage::save_run(&run_data);
        self.state.history_means = storage::load_history_summary();
    }

    fn draw_settings(&mut self, ui: &egui::Ui) {
        egui::Window::new("Settings")
            .open(&mut self.ui_state.show_settings)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Wait color:");
                    let mut c = self.state.config.wait_color_egui();
                    ui.color_edit_button_srgba(&mut c);
                    self.state.config.set_wait_color(c);
                });
                ui.horizontal(|ui| {
                    ui.label("React color:");
                    let mut c = self.state.config.react_color_egui();
                    ui.color_edit_button_srgba(&mut c);
                    self.state.config.set_react_color(c);
                });

                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Min wait (ms):");
                    ui.add(
                        egui::Slider::new(&mut self.state.config.min_wait_ms, 50..=5000)
                            .step_by(50.0),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Max wait (ms):");
                    ui.add(
                        egui::Slider::new(&mut self.state.config.max_wait_ms, 100..=20000)
                            .step_by(100.0),
                    );
                });
                if self.state.config.min_wait_ms > self.state.config.max_wait_ms {
                    self.state.config.min_wait_ms = self.state.config.max_wait_ms;
                }
                ui.horizontal(|ui| {
                    ui.label("Round count:");
                    ui.add(
                        egui::Slider::new(&mut self.state.config.round_count, 1..=50).step_by(1.0),
                    );
                });

                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("On false click:");
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

                ui.separator();

                if ui.button("Reset to defaults").clicked() {
                    self.state.config = crate::types::Configurables::default();
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

    fn draw_all_runs(&mut self, ui: &egui::Ui) {
        egui::Window::new("All Runs")
            .open(&mut self.ui_state.show_all_runs)
            .default_width(600.0)
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
                            ui.indent("run_detail", |ui| {
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
            "Wait color: RGB({}, {}, {})",
            run_data.config.wait_color[0],
            run_data.config.wait_color[1],
            run_data.config.wait_color[2]
        ));
        ui.label(format!(
            "React color: RGB({}, {}, {})",
            run_data.config.react_color[0],
            run_data.config.react_color[1],
            run_data.config.react_color[2]
        ));
        ui.label(format!(
            "Wait range: {}-{} ms",
            run_data.config.min_wait_ms, run_data.config.max_wait_ms
        ));
        ui.label(format!("Round count: {}", run_data.config.round_count));

        ui.separator();

        let times: Vec<f64> = run_data.rounds.iter().map(|r| r.reaction_time_ms).collect();
        let mean = compute_mean(&times);
        let median = compute_median(&times);
        ui.label(format!("Mean: {mean:.0} ms"));
        ui.label(format!("Median: {median:.0} ms"));

        ui.separator();

        for (i, round) in run_data.rounds.iter().enumerate() {
            ui.label(format!(
                "Round {}: wait {:.0} ms, reaction {:.0} ms",
                i + 1,
                round.wait_time_ms,
                round.reaction_time_ms
            ));
        }

        Self::draw_bar_chart(ui, &run_data.rounds);
    }

    fn draw_bar_chart(ui: &mut egui::Ui, rounds: &[RoundResult]) {
        let bar_color = Color32::from_rgb(100, 140, 255);
        let bars: Vec<Bar> = rounds
            .iter()
            .enumerate()
            .map(|(i, round)| {
                Bar::new((i + 1) as f64, round.reaction_time_ms)
                    .name(format!("R{}", i + 1))
                    .fill(bar_color)
                    .width(0.8)
            })
            .collect();
        let chart = BarChart::new("Reaction times", bars)
            .horizontal()
            .color(bar_color)
            .width(0.8)
            .element_formatter(Box::new(|bar, _| {
                format!("{}: {:.0} ms", bar.name, bar.value)
            }));

        Plot::new("reaction-times")
            .height((rounds.len() as f32 * 30.0).max(120.0))
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
        if data.len() <= 1 {
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), 200.0),
                egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                |ui| {
                    ui.label(if data.is_empty() {
                        "No data"
                    } else {
                        "Need more data"
                    });
                },
            );
            return;
        }

        let points: PlotPoints = data
            .iter()
            .enumerate()
            .map(|(i, &value)| [i as f64, value])
            .collect();
        let line_color = Color32::from_rgb(100, 140, 255);
        let line = Line::new("Mean reaction time", points)
            .color(line_color)
            .width(2.0);

        Plot::new("history-means")
            .height(200.0)
            .sense(egui::Sense::hover())
            .allow_drag(false)
            .allow_scroll(false)
            .allow_zoom(false)
            .allow_boxed_zoom(false)
            .allow_axis_zoom_drag(false)
            .show_crosshair(false)
            .y_axis_formatter(|mark, _| format!("{:.0} ms", mark.value))
            .show(ui, |plot_ui| plot_ui.line(line));
    }
}

impl eframe::App for ReactionLab {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.state.screen == crate::types::AppScreen::Round
            && self.state.round_state == RoundState::Waiting
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
            crate::types::AppScreen::Start => self.draw_start(ui),
            crate::types::AppScreen::Round => self.draw_round(ui),
            crate::types::AppScreen::End => self.draw_end(ui),
        }
    }
}

impl ReactionLab {
    fn draw_start(&mut self, ui: &mut egui::Ui) {
        CentralPanel::default().show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.set_max_width(MAX_CONTENT_WIDTH);
                ui.columns(2, |cols| {
                    if self.state.history_means.is_empty() {
                        cols[0].label("No previous runs yet.");
                    } else {
                        let points: Vec<f64> = self
                            .state
                            .history_means
                            .iter()
                            .map(|(_, mean)| *mean)
                            .collect();
                        Self::draw_line_chart(&mut cols[0], &points);
                    }

                    if cols[0].button("Show all runs").clicked() {
                        self.ui_state.show_all_runs = true;
                        self.ui_state.viewed_run_filename = None;
                        self.ui_state.viewed_run_data = None;
                        self.ui_state.run_file_list = storage::list_run_files();
                    }

                    cols[1].vertical_centered(|ui| {
                        ui.add_space(60.0);
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
                });
            });
        });
    }

    fn draw_round(&mut self, ui: &mut egui::Ui) {
        self.state.update_waiting();

        let fill_color = match self.state.round_state {
            RoundState::Waiting => self.state.config.wait_color_egui(),
            RoundState::Reacting => self.state.config.react_color_egui(),
            RoundState::ResultShowing | RoundState::TooSoon => Color32::from_rgb(30, 30, 30),
        };

        CentralPanel::default()
            .frame(Frame::NONE.fill(fill_color))
            .show(ui, |ui| {
                let pressed = ui.input(|i| i.pointer.primary_pressed());

                if pressed && let Some(run_data) = self.state.handle_click() {
                    self.persist_finished_run(run_data);
                }

                if self.state.round_state == RoundState::ResultShowing {
                    ui.vertical_centered(|ui| {
                        ui.add_space(ui.available_height() * 0.35);
                        if let Some(ms) = self.state.last_reaction_ms {
                            ui.heading(format!("{ms:.0} ms"));
                        }
                        ui.add_space(10.0);
                        ui.label(format!(
                            "Round {}/{} — click to continue",
                            self.state.current_round + 1,
                            self.state.config.round_count
                        ));
                    });
                }

                if self.state.round_state == RoundState::TooSoon {
                    let action_text = match self.state.config.false_click_action {
                        FalseClickAction::RetryRound => "Click to try again",
                        FalseClickAction::EndRun => "Click to end the run",
                    };
                    ui.vertical_centered(|ui| {
                        ui.add_space(ui.available_height() * 0.35);
                        ui.heading("Too soon!");
                        ui.add_space(10.0);
                        ui.label(action_text);
                    });
                }
            });
    }

    fn draw_end(&mut self, ui: &mut egui::Ui) {
        CentralPanel::default().show(ui, |ui| {
            ui.centered_and_justified(|ui| {
                ui.set_max_width(MAX_CONTENT_WIDTH);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let times: Vec<f64> = self
                        .state
                        .round_results
                        .iter()
                        .map(|r| r.reaction_time_ms)
                        .collect();
                    let mean = compute_mean(&times);
                    let median = compute_median(&times);

                    ui.heading("Run Results");
                    ui.separator();

                    ui.label(format!("Mean: {mean:.0} ms"));
                    ui.label(format!("Median: {median:.0} ms"));

                    ui.separator();

                    for (i, round) in self.state.round_results.iter().enumerate() {
                        ui.label(format!(
                            "Round {}: wait {:.0} ms, reaction {:.0} ms",
                            i + 1,
                            round.wait_time_ms,
                            round.reaction_time_ms
                        ));
                    }

                    ui.separator();

                    Self::draw_bar_chart(ui, &self.state.round_results);

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
