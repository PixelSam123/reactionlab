use eframe::egui;

use crate::modes::ModeId;
use crate::modes::click_timing_reaction_test::ClickTimingReactionTest;
use crate::modes::simple_reaction_time_test::SimpleReactionTimeTest;

pub struct ReactionLab {
    active_mode: ModeId,
    simple_reaction_time_test: SimpleReactionTimeTest,
    click_timing_reaction_test: ClickTimingReactionTest,
}

impl ReactionLab {
    pub fn new() -> Self {
        Self {
            active_mode: ModeId::SimpleReactionTimeTest,
            simple_reaction_time_test: SimpleReactionTimeTest::new(),
            click_timing_reaction_test: ClickTimingReactionTest::new(),
        }
    }

    fn draw_mode_tabs(&mut self, ui: &mut egui::Ui) {
        let mut selected_mode = None;
        ui.horizontal(|ui| {
            for mode in [
                ModeId::SimpleReactionTimeTest,
                ModeId::ClickTimingReactionTest,
            ] {
                if ui
                    .selectable_label(self.active_mode == mode, mode.label())
                    .clicked()
                {
                    selected_mode = Some(mode);
                }
            }
        });

        if let Some(mode) = selected_mode
            && mode != self.active_mode
        {
            if self.active_mode == ModeId::SimpleReactionTimeTest {
                self.simple_reaction_time_test.reset_to_start();
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
            ModeId::ClickTimingReactionTest => self.click_timing_reaction_test.show(ui),
        }
    }
}
