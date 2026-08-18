mod modes;
mod ui;

use eframe::wgpu::PresentMode;
use eframe::{NativeOptions, SurfaceConfig, WgpuConfiguration};

fn main() {
    let options = NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default().with_inner_size([800.0, 600.0]),
        wgpu_options: WgpuConfiguration::default().with_surface_config(SurfaceConfig {
            present_mode: PresentMode::AutoNoVsync,
            desired_maximum_frame_latency: Some(1),
        }),
        ..Default::default()
    };
    eframe::run_native(
        "reactionlab",
        options,
        Box::new(|_cc| Ok(Box::new(ui::ReactionLab::new()))),
    )
    .ok();
}
