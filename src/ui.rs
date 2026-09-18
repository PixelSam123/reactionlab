use eframe::egui;

use crate::modes::ModeId;
use crate::modes::click_timing_test::ClickTimingTest;
use crate::modes::simple_reaction_time_test::SimpleReactionTimeTest;
use crate::modes::video_click_timing_test::VideoClickTimingTest;

pub struct ReactionLab {
    active_mode: ModeId,
    simple_reaction_time_test: SimpleReactionTimeTest,
    click_timing_test: ClickTimingTest,
    video_click_timing_test: VideoClickTimingTest,
}

impl ReactionLab {
    pub fn new() -> Self {
        Self {
            active_mode: ModeId::SimpleReactionTimeTest,
            simple_reaction_time_test: SimpleReactionTimeTest::new(),
            click_timing_test: ClickTimingTest::new(),
            video_click_timing_test: VideoClickTimingTest::new(),
        }
    }

    fn draw_mode_tabs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for mode in [
                ModeId::SimpleReactionTimeTest,
                ModeId::ClickTimingTest,
                ModeId::VideoClickTimingTest,
            ] {
                if ui
                    .selectable_label(self.active_mode == mode, mode.label())
                    .clicked()
                {
                    self.change_mode(mode);
                }
            }
        });
    }

    fn change_mode(&mut self, mode: ModeId) {
        if mode != self.active_mode {
            match self.active_mode {
                ModeId::SimpleReactionTimeTest => self.simple_reaction_time_test.reset_to_start(),
                ModeId::ClickTimingTest => {
                    self.click_timing_test.reset_to_start();
                }
                ModeId::VideoClickTimingTest => self.video_click_timing_test.reset_to_start(),
            }

            self.active_mode = mode;
        }
    }
}

impl eframe::App for ReactionLab {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("mode-tabs").show(ui, |ui| {
            self.draw_mode_tabs(ui);
        });

        match self.active_mode {
            ModeId::SimpleReactionTimeTest => self.simple_reaction_time_test.show(ui),
            ModeId::ClickTimingTest => self.click_timing_test.show(ui),
            ModeId::VideoClickTimingTest => self.video_click_timing_test.show(ui),
        }
    }
}
