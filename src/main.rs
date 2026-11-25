#![windows_subsystem = "windows"]

mod app;
mod audio;
mod biquad;
mod icon;
mod skin;
mod storage;
mod visualizer;

use app::WinampApp;
use eframe::egui::Vec2;
use std::sync::Arc;

fn main() -> eframe::Result<()> {
    let is_vis_shot = std::env::args().any(|a| a == "--screenshot-vis");
    let (initial_size, min_size, max_size) = if is_vis_shot {
        (
            Vec2::new(960.0, 540.0),
            Vec2::new(320.0, 200.0),
            Vec2::new(4000.0, 4000.0),
        )
    } else {
        (
            Vec2::new(420.0, 500.0),
            Vec2::new(420.0, 160.0),
            Vec2::new(420.0, 4000.0),
        )
    };

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size(initial_size)
            .with_min_inner_size(min_size)
            .with_max_inner_size(max_size)
            .with_title("RustAmp - Retro Audio Player")
            .with_icon(Arc::new(icon::build_window_icon()))
            .with_resizable(true),
        ..Default::default()
    };

    eframe::run_native(
        "RustAmp",
        options,
        Box::new(|cc| Ok(Box::new(WinampApp::new(cc)))),
    )
}
