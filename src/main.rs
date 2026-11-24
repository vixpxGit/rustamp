#![windows_subsystem = "windows"]

mod app;
mod audio;
mod biquad;
mod icon;
mod skin;
mod visualizer;

use app::WinampApp;
use eframe::egui::Vec2;
use std::sync::Arc;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size(Vec2::new(420.0, 500.0))
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
