use eframe::egui::{self, CentralPanel};

pub struct ClickTimingReactionTest;

impl ClickTimingReactionTest {
    pub const fn new() -> Self {
        Self
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        CentralPanel::default().show(ui, |_| {});
    }
}
