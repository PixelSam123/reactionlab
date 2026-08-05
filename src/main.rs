#![allow(
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap
)]

mod types;

use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use chrono::{Datelike, Local, NaiveDate, NaiveDateTime};
use eframe::egui::{self, Align2, CentralPanel, Color32, Frame, Sense};
use eframe::wgpu::PresentMode;
use eframe::{NativeOptions, SurfaceConfig, WgpuConfiguration};
use rand::RngExt;
use types::{AppScreen, Configurables, FalseClickAction, RoundResult, RoundState, RunData, RunFileInfo};

const MAX_CONTENT_WIDTH: f32 = 1000.0;

struct ReactionLab {
    screen: AppScreen,
    config: Configurables,
    show_settings: bool,
    show_all_runs: bool,
    run_file_list: Vec<RunFileInfo>,
    viewed_run_filename: Option<String>,
    viewed_run_data: Option<RunData>,
    round_state: RoundState,
    current_round: usize,
    round_results: Vec<RoundResult>,
    wait_start: Option<Instant>,
    react_start: Option<Instant>,
    random_wait_ms: f64,
    last_reaction_ms: Option<f64>,
    history_means: Vec<(NaiveDateTime, f64)>,
    delete_year: u32,
    delete_month: u32,
    delete_day: u32,
}

impl ReactionLab {
    fn new() -> Self {
        let config = load_config();
        let history = load_history_summary();
        let today = Local::now().date_naive();
        Self {
            screen: AppScreen::Start,
            config,
            show_settings: false,
            show_all_runs: false,
            run_file_list: Vec::new(),
            viewed_run_filename: None,
            viewed_run_data: None,
            round_state: RoundState::Waiting,
            current_round: 0,
            round_results: Vec::new(),
            wait_start: None,
            react_start: None,
            random_wait_ms: 0.0,
            last_reaction_ms: None,
            history_means: history,
            delete_year: today.year() as u32,
            delete_month: today.month(),
            delete_day: today.day(),
        }
    }

    fn data_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("reactionlab")
    }

    fn config_path() -> PathBuf {
        Self::data_dir().join("config.json")
    }

    fn start_new_round(&mut self) {
        let mut rng = rand::rng();
        self.random_wait_ms =
            rng.random_range(self.config.min_wait_ms as f64..=self.config.max_wait_ms as f64);
        self.round_state = RoundState::Waiting;
        self.wait_start = Some(Instant::now());
        self.react_start = None;
        self.last_reaction_ms = None;
    }

    fn finish_run(&mut self) {
        let runs_dir = Self::data_dir();
        fs::create_dir_all(&runs_dir).ok();
        let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
        let filename = format!("reactionlab-{timestamp}.json");
        let run_data = RunData {
            timestamp,
            config: self.config.clone(),
            rounds: self.round_results.clone(),
        };
        if let Ok(json) = serde_json::to_string_pretty(&run_data) {
            fs::write(runs_dir.join(&filename), json).ok();
        }
        self.history_means = load_history_summary();
        self.screen = AppScreen::End;
    }

    fn restart_run(&mut self) {
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

    fn go_to_start(&mut self) {
        self.screen = AppScreen::Start;
        self.round_results.clear();
        self.current_round = 0;
    }

    fn compute_mean(times: &[f64]) -> f64 {
        if times.is_empty() {
            return 0.0;
        }
        times.iter().sum::<f64>() / times.len() as f64
    }

    fn compute_median(times: &[f64]) -> f64 {
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

    fn list_run_files() -> Vec<RunFileInfo> {
        let dir = Self::data_dir();
        if !dir.exists() {
            return Vec::new();
        }
        let mut files: Vec<RunFileInfo> = Vec::new();
        if let Ok(read_dir) = fs::read_dir(&dir) {
            for entry in read_dir.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "json")
                    && let Some(fname) = path.file_name()
                {
                    let filename = fname.to_string_lossy().to_string();
                    if filename.starts_with("reactionlab-") {
                        let display_name = Self::filename_to_display(&filename);
                        files.push(RunFileInfo {
                            filename,
                            display_name,
                        });
                    }
                }
            }
        }
        files.sort_by(|a, b| b.filename.cmp(&a.filename));
        files
    }

    fn filename_to_display(filename: &str) -> String {
        let ts_str = filename
            .strip_prefix("reactionlab-")
            .and_then(|s| s.strip_suffix(".json"))
            .unwrap_or("");
        NaiveDateTime::parse_from_str(ts_str, "%Y-%m-%d_%H-%M-%S")
            .map(|dt| {
                dt.format("%e %B %Y, %H:%M:%S")
                    .to_string()
                    .trim_start()
                    .to_string()
            })
            .unwrap_or_else(|_| filename.to_string())
    }

    fn load_run_data(filename: &str) -> Option<RunData> {
        let path = Self::data_dir().join(filename);
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
    }

    fn delete_runs_before(date: NaiveDate) {
        let dir = Self::data_dir();
        if !dir.exists() {
            return;
        }
        if let Ok(read_dir) = fs::read_dir(&dir) {
            for entry in read_dir.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "json")
                    && let Some(stem) = path.file_stem()
                {
                    let name = stem.to_string_lossy().to_string();
                    if let Some(ts_str) = name.strip_prefix("reactionlab-")
                        && let Ok(file_date) = NaiveDate::parse_from_str(&ts_str[..10], "%Y-%m-%d")
                        && file_date < date
                    {
                        fs::remove_file(&path).ok();
                    }
                }
            }
        }
    }

    fn draw_settings(&mut self, ui: &egui::Ui) {
        egui::Window::new("Settings")
            .open(&mut self.show_settings)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Wait color:");
                    let mut c = self.config.wait_color_egui();
                    ui.color_edit_button_srgba(&mut c);
                    self.config.set_wait_color(c);
                });
                ui.horizontal(|ui| {
                    ui.label("React color:");
                    let mut c = self.config.react_color_egui();
                    ui.color_edit_button_srgba(&mut c);
                    self.config.set_react_color(c);
                });

                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Min wait (ms):");
                    ui.add(
                        egui::Slider::new(&mut self.config.min_wait_ms, 50..=5000).step_by(50.0),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Max wait (ms):");
                    ui.add(
                        egui::Slider::new(&mut self.config.max_wait_ms, 100..=20000).step_by(100.0),
                    );
                });
                if self.config.min_wait_ms > self.config.max_wait_ms {
                    self.config.min_wait_ms = self.config.max_wait_ms;
                }
                ui.horizontal(|ui| {
                    ui.label("Round count:");
                    ui.add(egui::Slider::new(&mut self.config.round_count, 1..=50).step_by(1.0));
                });

                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("On false click:");
                    ui.radio_value(
                        &mut self.config.false_click_action,
                        FalseClickAction::RetryRound,
                        "Retry round",
                    );
                    ui.radio_value(
                        &mut self.config.false_click_action,
                        FalseClickAction::EndRun,
                        "End run",
                    );
                });

                ui.separator();

                if ui.button("Reset to defaults").clicked() {
                    self.config = Configurables::default();
                }

                ui.separator();

                ui.label("Delete runs before:");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.delete_year)
                            .range(2000..=2100)
                            .speed(1),
                    );
                    ui.label("-");
                    ui.add(
                        egui::DragValue::new(&mut self.delete_month)
                            .range(1..=12)
                            .speed(1),
                    );
                    ui.label("-");
                    ui.add(
                        egui::DragValue::new(&mut self.delete_day)
                            .range(1..=31)
                            .speed(1),
                    );
                });
                if ui.button("Delete").clicked()
                    && let Some(date) = NaiveDate::from_ymd_opt(
                        self.delete_year as i32,
                        self.delete_month,
                        self.delete_day,
                    )
                {
                    Self::delete_runs_before(date);
                    self.history_means = load_history_summary();
                    self.run_file_list = Self::list_run_files();
                }
            });
    }

    fn draw_all_runs(&mut self, ui: &egui::Ui) {
        egui::Window::new("All Runs")
            .open(&mut self.show_all_runs)
            .default_width(600.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for run_file in &self.run_file_list.clone() {
                        let selected = self.viewed_run_filename.as_deref() == Some(&run_file.filename);
                        if ui.selectable_label(selected, &run_file.display_name).clicked() {
                            if selected {
                                self.viewed_run_filename = None;
                                self.viewed_run_data = None;
                            } else {
                                self.viewed_run_data = Self::load_run_data(&run_file.filename);
                                self.viewed_run_filename = Some(run_file.filename.clone());
                            }
                        }
                        if selected && let Some(ref data) = self.viewed_run_data {
                            ui.indent("run_detail", |ui| {
                                Self::draw_run_details(ui, data);
                            });
                        }
                    }
                    if self.run_file_list.is_empty() {
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
        let mean = Self::compute_mean(&times);
        let median = Self::compute_median(&times);
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
        let max_time = rounds
            .iter()
            .map(|r| r.reaction_time_ms)
            .fold(0.0f64, f64::max)
            .max(1.0);
        let bar_height = 24.0;
        let spacing = 6.0;
        let total_height = rounds.len() as f32 * (bar_height + spacing);
        let label_reserve = 100.0;
        let full_width = ui.available_width();
        let chart_width = (full_width - label_reserve).max(10.0);

        let (response, painter) =
            ui.allocate_painter(egui::vec2(full_width, total_height), Sense::hover());
        let rect = response.rect;
        let bar_color = Color32::from_rgb(100, 140, 255);

        for (i, round) in rounds.iter().enumerate() {
            let y = (i as f32).mul_add(bar_height + spacing, rect.top());
            let bar_width = (round.reaction_time_ms / max_time) as f32 * chart_width;
            let bar_width = bar_width.max(2.0);

            let bar_rect = egui::Rect::from_min_size(
                egui::pos2(rect.left(), y),
                egui::vec2(bar_width, bar_height),
            );
            painter.rect_filled(bar_rect, 4.0, bar_color);

            painter.text(
                egui::pos2(rect.left() + chart_width + 5.0, bar_rect.center().y),
                Align2::LEFT_CENTER,
                format!("R{}: {:.0} ms", i + 1, round.reaction_time_ms),
                egui::FontId::proportional(13.0),
                Color32::WHITE,
            );
        }
    }

    fn draw_line_chart(ui: &mut egui::Ui, data: &[f64]) {
        let desired_size = egui::vec2(ui.available_width(), 200.0);
        let (response, painter) = ui.allocate_painter(desired_size, Sense::hover());
        let rect = response.rect;

        if data.len() <= 1 {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                if data.is_empty() {
                    "No data"
                } else {
                    "Need more data"
                },
                egui::FontId::proportional(14.0),
                Color32::GRAY,
            );
            return;
        }

        let min_y = data.iter().copied().fold(f64::INFINITY, f64::min);
        let max_y = data.iter().copied().fold(0.0f64, f64::max);
        let y_range = (max_y - min_y).max(1.0);

        let plot_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left() + 55.0, rect.top() + 10.0),
            egui::pos2(rect.right() - 10.0, rect.bottom() - 10.0),
        );
        let n = data.len();
        let axis_color = Color32::from_gray(128);

        painter.line_segment(
            [plot_rect.left_bottom(), plot_rect.right_bottom()],
            (1.0, axis_color),
        );
        painter.line_segment(
            [plot_rect.left_bottom(), plot_rect.left_top()],
            (1.0, axis_color),
        );

        painter.text(
            egui::pos2(plot_rect.left(), plot_rect.top()),
            Align2::RIGHT_TOP,
            format!("{max_y:.0} ms"),
            egui::FontId::proportional(11.0),
            Color32::GRAY,
        );
        painter.text(
            egui::pos2(plot_rect.left(), plot_rect.bottom()),
            Align2::RIGHT_BOTTOM,
            format!("{min_y:.0} ms"),
            egui::FontId::proportional(11.0),
            Color32::GRAY,
        );

        let line_color = Color32::from_rgb(100, 140, 255);
        let mut prev: Option<egui::Pos2> = None;
        for (i, &value) in data.iter().enumerate() {
            let x = (i as f32 / (n as f32 - 1.0)).mul_add(plot_rect.width(), plot_rect.left());
            let y = ((value - min_y) as f32 / y_range as f32)
                .mul_add(-plot_rect.height(), plot_rect.bottom());
            let pos = egui::pos2(x, y);
            painter.circle_filled(pos, 3.0, line_color);
            if let Some(prev_pos) = prev {
                painter.line_segment([prev_pos, pos], (2.0, line_color));
            }
            prev = Some(pos);
        }
    }
}

impl eframe::App for ReactionLab {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.screen == AppScreen::Round && self.round_state == RoundState::Waiting {
            ui.ctx().request_repaint();
        }

        if self.show_settings {
            self.draw_settings(ui);
        }
        if self.show_all_runs {
            self.draw_all_runs(ui);
        }

        match self.screen {
            AppScreen::Start => self.draw_start(ui),
            AppScreen::Round => self.draw_round(ui),
            AppScreen::End => self.draw_end(ui),
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
                    if self.history_means.is_empty() {
                        cols[0].label("No previous runs yet.");
                    } else {
                        let points: Vec<f64> =
                            self.history_means.iter().map(|(_, mean)| *mean).collect();
                        Self::draw_line_chart(&mut cols[0], &points);
                    }

                    if cols[0].button("Show all runs").clicked() {
                        self.show_all_runs = true;
                        self.viewed_run_filename = None;
                        self.viewed_run_data = None;
                        self.run_file_list = Self::list_run_files();
                    }

                    cols[1].vertical_centered(|ui| {
                        ui.add_space(60.0);
                        if ui.button("⚙ Settings").clicked() {
                            self.show_settings = !self.show_settings;
                            if !self.show_settings {
                                save_config(&self.config);
                                self.history_means = load_history_summary();
                                self.run_file_list.clear();
                            }
                        }
                        ui.add_space(20.0);
                        if ui.button("Start new run").clicked() {
                            self.restart_run();
                        }
                    });
                });
            });
        });
    }

    fn draw_round(&mut self, ui: &mut egui::Ui) {
        let time_multiplier = 1.0;

        if self.round_state == RoundState::Waiting
            && let Some(start) = self.wait_start
        {
            let elapsed = start.elapsed().as_secs_f64() * 1000.0;
            if elapsed >= self.random_wait_ms * time_multiplier {
                self.round_state = RoundState::Reacting;
                self.react_start = Some(Instant::now());
            }
        }

        let fill_color = match self.round_state {
            RoundState::Waiting => self.config.wait_color_egui(),
            RoundState::Reacting => self.config.react_color_egui(),
            RoundState::ResultShowing | RoundState::TooSoon => Color32::from_rgb(30, 30, 30),
        };

        CentralPanel::default()
            .frame(Frame::NONE.fill(fill_color))
            .show(ui, |ui| {
                let pressed = ui.input(|i| i.pointer.primary_pressed());

                if pressed {
                    if self.round_state == RoundState::Waiting {
                        self.round_state = RoundState::TooSoon;
                    } else if self.round_state == RoundState::Reacting {
                        let reaction = self.react_start.unwrap().elapsed().as_secs_f64() * 1000.0;
                        self.last_reaction_ms = Some(reaction);
                        self.round_results.push(RoundResult {
                            wait_time_ms: self.random_wait_ms,
                            reaction_time_ms: reaction,
                        });
                        self.round_state = RoundState::ResultShowing;
                    } else if self.round_state == RoundState::ResultShowing {
                        self.current_round += 1;
                        if self.current_round >= self.config.round_count {
                            self.finish_run();
                        } else {
                            self.start_new_round();
                        }
                    } else if self.round_state == RoundState::TooSoon {
                        match self.config.false_click_action {
                            FalseClickAction::RetryRound => self.start_new_round(),
                            FalseClickAction::EndRun => self.finish_run(),
                        }
                    }
                }

                if self.round_state == RoundState::ResultShowing {
                    ui.vertical_centered(|ui| {
                        ui.add_space(ui.available_height() * 0.35);
                        if let Some(ms) = self.last_reaction_ms {
                            ui.heading(format!("{ms:.0} ms"));
                        }
                        ui.add_space(10.0);
                        ui.label(format!(
                            "Round {}/{} — click to continue",
                            self.current_round + 1,
                            self.config.round_count
                        ));
                    });
                }

                if self.round_state == RoundState::TooSoon {
                    let action_text = match self.config.false_click_action {
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
                        .round_results
                        .iter()
                        .map(|r| r.reaction_time_ms)
                        .collect();
                    let mean = Self::compute_mean(&times);
                    let median = Self::compute_median(&times);

                    ui.heading("Run Results");
                    ui.separator();

                    ui.label(format!("Mean: {mean:.0} ms"));
                    ui.label(format!("Median: {median:.0} ms"));

                    ui.separator();

                    for (i, round) in self.round_results.iter().enumerate() {
                        ui.label(format!(
                            "Round {}: wait {:.0} ms, reaction {:.0} ms",
                            i + 1,
                            round.wait_time_ms,
                            round.reaction_time_ms
                        ));
                    }

                    ui.separator();

                    Self::draw_bar_chart(ui, &self.round_results);

                    ui.separator();

                    ui.vertical_centered(|ui| {
                        if ui.button("Try again").clicked() {
                            self.go_to_start();
                        }
                    });
                });
            });
        });
    }
}

fn load_config() -> Configurables {
    let path = ReactionLab::config_path();
    if path.exists() {
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        Configurables::default()
    }
}

fn save_config(config: &Configurables) {
    let path = ReactionLab::config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
    if let Ok(json) = serde_json::to_string_pretty(config) {
        fs::write(path, json).ok();
    }
}

fn load_history_summary() -> Vec<(NaiveDateTime, f64)> {
    let dir = ReactionLab::data_dir();
    if !dir.exists() {
        return Vec::new();
    }
    let mut entries: Vec<(NaiveDateTime, f64)> = Vec::new();
    if let Ok(read_dir) = fs::read_dir(&dir) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json")
                && path
                    .file_stem()
                    .is_some_and(|s| s.to_string_lossy().starts_with("reactionlab-"))
                && let Ok(content) = fs::read_to_string(&path)
                && let Ok(run_data) = serde_json::from_str::<RunData>(&content)
            {
                let times: Vec<f64> = run_data.rounds.iter().map(|r| r.reaction_time_ms).collect();
                let mean = ReactionLab::compute_mean(&times);
                if let Ok(dt) =
                    NaiveDateTime::parse_from_str(&run_data.timestamp, "%Y-%m-%d_%H-%M-%S")
                {
                    entries.push((dt, mean));
                }
            }
        }
    }
    entries.sort_by_key(|b| std::cmp::Reverse(b.0));
    entries.truncate(10);
    entries.reverse();
    entries
}

fn main() {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([800.0, 600.0]),
        wgpu_options: WgpuConfiguration::default().with_surface_config(SurfaceConfig {
            present_mode: PresentMode::AutoNoVsync,
            desired_maximum_frame_latency: Some(1),
        }),
        ..Default::default()
    };
    eframe::run_native(
        "reactionlab",
        options,
        Box::new(|_cc| Ok(Box::new(ReactionLab::new()))),
    )
    .ok();
}
