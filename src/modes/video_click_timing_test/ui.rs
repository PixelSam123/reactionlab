#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap
)]

use std::time::Instant;

use eframe::egui::{self, CentralPanel, Color32, Frame, Pos2, Rect, TextureHandle, TextureOptions};
use egui_plot::{Bar, BarChart, Line, Plot, PlotPoints};

use super::extraction::probe_fps;
use super::state::{AppState, Phase, compute_mean, compute_median};
use super::storage;
use super::types::{
    AppScreen, FalseClickAction, MeasurementUnit, RoundOutcome, RoundResult, RunData, RunFileInfo,
    TimestampInput, VideoConfig, VideoGroup,
};

const MAX_CONTENT_WIDTH: f32 = 1000.0;
const START_STACK_WIDTH: f32 = 600.0;
/// Alpha used to ghost the frozen frame on the result screen so the overlay
/// text stays the focal point.
const RESULT_FRAME_TINT_ALPHA: u8 = 128;

pub struct VideoClickTimingTest {
    state: AppState,
    ui_state: UiState,
}

struct UiState {
    show_configure: bool,
    show_settings: bool,
    show_all_runs: bool,
    run_file_list: Vec<RunFileInfo>,
    viewed_run_filename: Option<String>,
    viewed_run_data: Option<RunData>,
    video_texture: Option<TextureHandle>,
    last_start_panel_size: Option<egui::Vec2>,
    last_end_panel_size: Option<egui::Vec2>,
    /// Height of the actions column measured on the previous frame, used to
    /// vertically center it against the freshly measured history column.
    last_actions_column_height: f32,
}

impl UiState {
    fn new() -> Self {
        Self {
            show_configure: false,
            show_settings: false,
            show_all_runs: false,
            run_file_list: Vec::new(),
            viewed_run_filename: None,
            viewed_run_data: None,
            video_texture: None,
            last_start_panel_size: None,
            last_end_panel_size: None,
            last_actions_column_height: 0.0,
        }
    }
}

impl VideoClickTimingTest {
    pub fn new() -> Self {
        Self {
            state: AppState::new(storage::load_config(), storage::load_history_summary()),
            ui_state: UiState::new(),
        }
    }

    pub fn reset_to_start(&mut self) {
        storage::save_config(&self.state.config);
        self.state.reset_to_start();
        self.ui_state.show_configure = false;
        self.ui_state.show_settings = false;
        self.ui_state.show_all_runs = false;
        self.ui_state.viewed_run_filename = None;
        self.ui_state.viewed_run_data = None;
        self.ui_state.run_file_list.clear();
        self.ui_state.video_texture = None;
    }

    fn persist_finished_run(&mut self, run_data: RunData) {
        storage::save_run(&run_data);
        self.state.history_means = storage::load_history_summary();
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        if self.state.screen == AppScreen::Preparing {
            self.state.poll_preload();
        }
        if self.ui_state.show_configure {
            self.draw_configure(ui);
        }
        if self.ui_state.show_settings {
            self.draw_settings(ui);
        }
        if self.state.screen == AppScreen::SelectGroup {
            self.draw_select_group(ui);
        }
        if self.ui_state.show_all_runs {
            self.draw_all_runs(ui);
        }
        match self.state.screen {
            AppScreen::Start => self.draw_start(ui),
            AppScreen::Preparing => self.draw_preparing(ui),
            AppScreen::Round => self.draw_round(ui),
            AppScreen::End => self.draw_end(ui),
            AppScreen::SelectGroup => self.draw_start(ui),
        }
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
            egui::Area::new(egui::Id::new("video-start-content"))
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
                            if let Some(error) = &self.state.last_error {
                                ui.colored_label(Color32::RED, error);
                            }
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
            Self::draw_line_chart(ui, &points, "start");
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
            if ui.button("⚙ Configure video groups").clicked() {
                self.ui_state.show_configure = !self.ui_state.show_configure;
                if !self.ui_state.show_configure {
                    storage::save_config(&self.state.config);
                }
            }
            ui.add_space(12.0);
            if ui.button("⚙ Run settings").clicked() {
                self.ui_state.show_settings = !self.ui_state.show_settings;
            }
            ui.add_space(12.0);
            let can_start = self.state.config.has_usable_videos();
            ui.add_enabled_ui(can_start, |ui| {
                if ui.button("Start new run").clicked() && can_start {
                    self.state.screen = AppScreen::SelectGroup;
                }
            });
            if !can_start {
                ui.colored_label(
                    Color32::YELLOW,
                    "Add and configure at least one usable video group first.",
                );
            }
        });
    }

    fn draw_configure(&mut self, ui: &egui::Ui) {
        let mut close = false;
        egui::Window::new("Configure video groups")
            .open(&mut self.ui_state.show_configure)
            .order(egui::Order::Foreground)
            .default_pos(ui.ctx().content_rect().center())
            .pivot(egui::Align2::CENTER_CENTER)
            .default_width(640.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut remove_group: Option<usize> = None;
                    for (group_index, group) in self.state.config.groups.iter_mut().enumerate() {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label("Name:");
                                ui.text_edit_singleline(&mut group.name);
                                if ui.button("Remove group").clicked() {
                                    remove_group = Some(group_index);
                                }
                            });
                            let mut remove_video: Option<usize> = None;
                            for (video_index, video) in group.videos.iter_mut().enumerate() {
                                ui.group(|ui| {
                                    Self::draw_video_editor(
                                        ui,
                                        video,
                                        &format!("group-{group_index}-video-{video_index}"),
                                    );
                                    if ui.button("Remove video").clicked() {
                                        remove_video = Some(video_index);
                                    }
                                });
                            }
                            if let Some(video_index) = remove_video {
                                group.videos.remove(video_index);
                            }
                            if ui.button("Add video").clicked() {
                                group.videos.push(VideoConfig::default());
                            }
                        });
                    }
                    if let Some(group_index) = remove_group {
                        self.state.config.groups.remove(group_index);
                    }
                    if ui.button("Add group").clicked() {
                        let next = self.state.config.groups.len() + 1;
                        self.state.config.groups.push(VideoGroup {
                            name: format!("Group {next}"),
                            videos: Vec::new(),
                        });
                    }
                    ui.separator();
                    if ui.button("Save & close").clicked() {
                        storage::save_config(&self.state.config);
                        close = true;
                    }
                });
            });
        if close {
            self.ui_state.show_configure = false;
        }
    }

    fn draw_video_editor(ui: &mut egui::Ui, video: &mut VideoConfig, id_prefix: &str) {
        ui.horizontal(|ui| {
            ui.label("Path:");
            ui.text_edit_singleline(&mut video.path);
            if ui.button("Browse").clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("Video", &["mp4", "mkv", "mov", "webm", "avi", "m4v"])
                    .pick_file()
            {
                video.path = path.to_string_lossy().to_string();
                video.fps = probe_fps(&video.path);
            }
        });
        ui.horizontal(|ui| {
            ui.label("FPS:");
            match video.fps {
                Some(fps) => ui.label(format!("{fps:.3}")),
                None => ui.colored_label(Color32::YELLOW, "unknown"),
            };
            if ui.button("Probe").clicked() {
                video.fps = probe_fps(&video.path);
            }
        });
        ui.horizontal(|ui| {
            ui.label("Measurement unit:");
            egui::ComboBox::from_id_salt(format!("video-unit-{id_prefix}"))
                .selected_text(video.unit.label())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut video.unit, MeasurementUnit::Time, "Time");
                    ui.selectable_value(&mut video.unit, MeasurementUnit::Frame, "Frame");
                });
        });
        ui.checkbox(&mut video.has_pre_wait, "Has pre-wait snippet");
        if video.has_pre_wait {
            ui.horizontal(|ui| {
                ui.label("Pre-wait start:");
                Self::draw_timestamp(ui, &mut video.pre_wait_start, video.unit);
            });
            ui.horizontal(|ui| {
                ui.label("Pre-wait end:");
                Self::draw_timestamp(ui, &mut video.pre_wait_end, video.unit);
            });
        }
        ui.horizontal(|ui| {
            ui.label("Click point:");
            Self::draw_timestamp(ui, &mut video.click_point, video.unit);
        });
        ui.checkbox(&mut video.has_post_click, "Has post-click snippet");
        if video.has_post_click {
            ui.horizontal(|ui| {
                ui.label("Post-click end:");
                Self::draw_timestamp(ui, &mut video.post_click_end, video.unit);
            });
        }
        for message in video.validation_messages() {
            ui.colored_label(Color32::YELLOW, message);
        }
    }

    fn draw_timestamp(ui: &mut egui::Ui, value: &mut TimestampInput, unit: MeasurementUnit) {
        match unit {
            MeasurementUnit::Time => {
                ui.add(egui::DragValue::new(&mut value.time.minutes).range(0..=599));
                ui.label("m");
                ui.add(egui::DragValue::new(&mut value.time.seconds).range(0..=59));
                ui.label("s");
                ui.add(egui::DragValue::new(&mut value.time.milliseconds).range(0..=999));
                ui.label("ms");
            }
            MeasurementUnit::Frame => {
                ui.add(egui::DragValue::new(&mut value.frame).range(0..=1_000_000));
                ui.label("frames");
            }
        }
    }

    fn draw_settings(&mut self, ui: &egui::Ui) {
        egui::Window::new("Run settings")
            .open(&mut self.ui_state.show_settings)
            .order(egui::Order::Foreground)
            .default_pos(ui.ctx().content_rect().center())
            .pivot(egui::Align2::CENTER_CENTER)
            .default_width(420.0)
            .show(ui, |ui| {
                drag_u64(
                    ui,
                    "Minimum wait (ms):",
                    &mut self.state.run_settings.min_wait_ms,
                    0..=60000,
                );
                drag_u64(
                    ui,
                    "Maximum wait (ms):",
                    &mut self.state.run_settings.max_wait_ms,
                    0..=60000,
                );
                if self.state.run_settings.min_wait_ms > self.state.run_settings.max_wait_ms {
                    self.state.run_settings.max_wait_ms = self.state.run_settings.min_wait_ms;
                }
                drag_usize(
                    ui,
                    "Round count:",
                    &mut self.state.run_settings.round_count,
                    1..=100,
                );
                drag_u64(
                    ui,
                    "Notify miss after (ms):",
                    &mut self.state.run_settings.notify_miss_after_ms,
                    0..=10000,
                );
                ui.horizontal(|ui| {
                    ui.label("On failure:");
                    ui.radio_value(
                        &mut self.state.run_settings.false_click_action,
                        FalseClickAction::RetryRound,
                        "Retry round",
                    );
                    ui.radio_value(
                        &mut self.state.run_settings.false_click_action,
                        FalseClickAction::EndRun,
                        "End run",
                    );
                });
                for message in self.state.run_settings.validation_messages() {
                    ui.colored_label(Color32::YELLOW, message);
                }
            });
    }

    fn draw_select_group(&mut self, ui: &egui::Ui) {
        let mut chosen: Option<usize> = None;
        egui::Window::new("Select group")
            .open(&mut (self.state.screen == AppScreen::SelectGroup))
            .order(egui::Order::Foreground)
            .default_pos(ui.ctx().content_rect().center())
            .pivot(egui::Align2::CENTER_CENTER)
            .default_width(420.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if self.state.config.groups.is_empty() {
                        ui.label("No groups configured.");
                    }
                    for (group_index, group) in self.state.config.groups.iter().enumerate() {
                        let usable = group.usable_videos().count();
                        let label = format!("{} ({} video(s))", group.name, usable);
                        let enabled = usable > 0;
                        ui.add_enabled_ui(enabled, |ui| {
                            if ui.button(&label).clicked() {
                                chosen = Some(group_index);
                            }
                        });
                    }
                });
            });
        if let Some(group_index) = chosen {
            self.state.start_run(group_index);
        }
    }

    fn draw_preparing(&mut self, ui: &mut egui::Ui) {
        let (fraction, processed, total) =
            self.state.preload.as_ref().map_or((1.0, 0, 1), |preload| {
                (
                    preload.progress_fraction(),
                    preload.processed,
                    preload.total,
                )
            });
        CentralPanel::default()
            .frame(Frame::NONE.fill(Color32::from_rgb(24, 24, 28)))
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() * 0.35);
                    ui.heading("Preparing videos…");
                    ui.add_space(16.0);
                    ui.add(
                        egui::ProgressBar::new(fraction)
                            .text(format!("{processed} / {total}"))
                            .desired_width(320.0),
                    );
                    ui.add_space(16.0);
                    if ui.button("Cancel").clicked() {
                        self.state.cancel_preload();
                    }
                });
            });
        if self.state.screen == AppScreen::Preparing {
            ui.ctx().request_repaint();
        }
    }

    fn draw_round(&mut self, ui: &mut egui::Ui) {
        let now = Instant::now();
        CentralPanel::default()
            .frame(Frame::NONE.fill(Color32::from_rgb(24, 24, 28)))
            .show(ui, |ui| {
                let canvas = ui.available_rect_before_wrap();
                let pressed = ui.input(|input| input.pointer.primary_pressed());
                if let Some(run_data) = self.state.update_at(now, pressed) {
                    self.persist_finished_run(run_data);
                }
                if let Some(image) = self.state.current_frame() {
                    let texture = self.ui_state.video_texture.get_or_insert_with(|| {
                        ui.ctx().load_texture(
                            "video-click-texture",
                            image.clone(),
                            TextureOptions::LINEAR,
                        )
                    });
                    texture.set(image.clone(), TextureOptions::LINEAR);
                    let scale = (canvas.width() / image.width() as f32)
                        .min(canvas.height() / image.height() as f32)
                        .max(0.01);
                    let size =
                        egui::vec2(image.width() as f32 * scale, image.height() as f32 * scale);
                    let rect = Rect::from_center_size(canvas.center(), size);
                    let painter = ui.painter_at(canvas);
                    let tint = if self.state.phase == Phase::Result {
                        Color32::from_white_alpha(RESULT_FRAME_TINT_ALPHA)
                    } else {
                        Color32::WHITE
                    };
                    painter.image(
                        texture.id(),
                        rect,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        tint,
                    );
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.colored_label(Color32::YELLOW, "No frame to display.");
                    });
                }
                if self.state.phase == Phase::Result
                    && let Some(result) = &self.state.last_result
                {
                    let painter = ui.painter_at(canvas);
                    Self::draw_result_overlay(
                        &painter,
                        canvas,
                        result,
                        self.state.run_settings.false_click_action,
                    );
                }
            });
        if matches!(
            self.state.phase,
            Phase::PreWait
                | Phase::Waiting
                | Phase::PlayingToClick
                | Phase::ClickPending
                | Phase::PostClick
        ) {
            ui.ctx().request_repaint();
        }
    }

    fn draw_result_overlay(
        painter: &egui::Painter,
        canvas: Rect,
        result: &RoundResult,
        false_click_action: FalseClickAction,
    ) {
        let center = canvas.center();
        let (heading, description) = match result.outcome {
            RoundOutcome::Hit => {
                let offset = result.click_offset_ms.unwrap_or(0.0);
                if offset.abs() < 0.5 {
                    ("Hit!".to_string(), "Right on the click point.".to_string())
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
                "You clicked before the video reached the click point.".to_string(),
            ),
            RoundOutcome::TooLateNoClick => (
                "Too late!".to_string(),
                "The click point passed without a click.".to_string(),
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
            egui::Area::new(egui::Id::new("video-end-content"))
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
                            let errors: Vec<f64> = self
                                .state
                                .attempt_results
                                .iter()
                                .filter(|attempt| attempt.outcome.is_hit())
                                .filter_map(|attempt| attempt.click_offset_ms.map(f64::abs))
                                .collect();
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
                                hit_count, self.state.run_settings.round_count
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
                                let offset = attempt
                                    .click_offset_ms
                                    .map(|value| format!(", offset {value:+.0} ms"))
                                    .unwrap_or_default();
                                let group = &attempt.group_name;
                                ui.label(format!(
                                    "Attempt {} (round {}): {} [{}]{}, {}",
                                    attempt.attempt_number.max(index + 1),
                                    attempt.round_number.max(index + 1),
                                    attempt.outcome.label(),
                                    group,
                                    offset,
                                    attempt.video_path,
                                ));
                            }
                            ui.separator();
                            Self::draw_bar_chart(ui, &self.state.attempt_results, "end");
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
                            ui.indent("video-run-detail", |ui| {
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
            "Round count setting: {}",
            run_data.settings.round_count
        ));
        let errors: Vec<f64> = run_data
            .attempts
            .iter()
            .filter(|attempt| attempt.outcome.is_hit())
            .filter_map(|attempt| attempt.click_offset_ms.map(f64::abs))
            .collect();
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
            hit_count, run_data.settings.round_count
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
            let offset = attempt
                .click_offset_ms
                .map(|value| format!(", offset {value:+.0} ms"))
                .unwrap_or_default();
            ui.label(format!(
                "Attempt {} (round {}): {} [{}]{}, {}",
                attempt.attempt_number.max(index + 1),
                attempt.round_number.max(index + 1),
                attempt.outcome.label(),
                attempt.group_name,
                offset,
                attempt.video_path,
            ));
        }
        Self::draw_bar_chart(ui, &run_data.attempts, "details");
    }

    fn draw_bar_chart(ui: &mut egui::Ui, attempts: &[RoundResult], id_suffix: &str) {
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
        Plot::new(format!("video-errors-{id_suffix}"))
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

    fn draw_line_chart(ui: &mut egui::Ui, data: &[f64], id_suffix: &str) {
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
            .map(|(index, &value)| [index as f64, value])
            .collect();
        let line = Line::new("Mean absolute timing error", points)
            .color(Color32::from_rgb(100, 180, 255))
            .width(2.0);
        Plot::new(format!("video-history-{id_suffix}"))
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
