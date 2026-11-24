use eframe::egui::IconData;

include!("logo_render.rs");

/// Builds `eframe::egui::IconData` for the window and taskbar icon.
pub fn build_window_icon() -> IconData {
    let size = 64;
    IconData {
        rgba: render_winampr_rgba(size),
        width: size,
        height: size,
    }
}
