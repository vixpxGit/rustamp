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
    let initial_size = if is_vis_shot {
        Vec2::new(960.0, 540.0)
    } else {
        Vec2::new(420.0, 500.0)
    };

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size(initial_size)
            .with_min_inner_size(Vec2::new(380.0, 160.0))
            .with_title("WinampR - It really whips the llama's ass!")
            .with_icon(Arc::new(icon::build_window_icon()))
            .with_resizable(true),
        ..Default::default()
    };

    eframe::run_native(
        "WinampR",
        options,
        Box::new(|cc| Ok(Box::new(WinampApp::new(cc)))),
    )
}
