use eframe::egui::{
    self, Align2, Color32, FontId, Id, Key, Painter, Pos2, Rect, Sense, Stroke, Ui, Vec2,
};

use crate::audio::{AudioEngine, PlaybackState};
use crate::skin::*;
use crate::storage::{self, PersistedState};
use crate::visualizer::{VisMode, VisualizerState, NUM_BANDS};

pub struct WinampApp {
    pub engine: AudioEngine,
    pub visualizer: VisualizerState,
    pub show_eq: bool,
    pub show_playlist: bool,
    pub show_vis_window: bool,
    pub vis_fullscreen: bool,
    last_vis_fullscreen: bool,
    pub windowshade: bool,
    pub remaining_time_mode: bool,
    pub marquee_offset: f32,
    pub selected_playlist_idx: Option<usize>,
    pub playlist_scroll: usize,
    pub eq_preset_name: String,
    last_save: std::time::Instant,
    screenshot_path: Option<String>,
    screenshot_vis_path: Option<String>,
    screenshot_frames: u32,
}

impl WinampApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut engine = AudioEngine::new();
        let vis_data = std::sync::Arc::clone(&engine.vis_data);
        let mut visualizer = VisualizerState::new(vis_data);

        let mut show_eq = true;
        let mut show_playlist = true;
        let mut windowshade = false;
        let mut remaining_time_mode = false;
        let mut eq_preset_name = "Flat".to_string();
        let mut selected_playlist_idx = Some(0);

        let args: Vec<String> = std::env::args().collect();
        let screenshot_path = args
            .windows(2)
            .find(|w| w[0] == "--screenshot")
            .map(|w| w[1].clone());
        let screenshot_vis_path = args
            .windows(2)
            .find(|w| w[0] == "--screenshot-vis")
            .map(|w| w[1].clone());
        let is_shot = screenshot_path.is_some() || screenshot_vis_path.is_some();

        if !is_shot {
            if let Some(saved) = storage::load_state() {
                engine.set_volume(saved.volume);
                engine.set_balance(saved.balance);
                engine.shuffle = saved.shuffle;
                engine.repeat = saved.repeat;

                show_eq = saved.show_eq;
                show_playlist = saved.show_playlist;
                windowshade = saved.windowshade;
                remaining_time_mode = saved.remaining_time_mode;
                visualizer.mode = saved.vis_mode;
                eq_preset_name = saved.eq_preset_name;

                if let Ok(mut eq) = engine.eq_state.lock() {
                    eq.enabled = saved.eq_enabled;
                    eq.preamp_db = saved.eq_preamp_db;
                    eq.bands_db = saved.eq_bands_db;
                }

                // Restore saved playlist (keep demo tracks and existing files)
                let valid_tracks: Vec<_> = saved
                    .playlist
                    .into_iter()
                    .filter(|t| t.is_demo || t.path.as_ref().map(|p| p.exists()).unwrap_or(false))
                    .collect();

                if !valid_tracks.is_empty() {
                    engine.playlist = valid_tracks;
                    let idx = saved
                        .current_index
                        .filter(|&i| i < engine.playlist.len())
                        .or(Some(0));
                    engine.current_index = idx;
                    selected_playlist_idx = idx;
                }
            }
        } else {
            eq_preset_name = "Rock".to_string();
            if let Ok(mut eq) = engine.eq_state.lock() {
                eq.bands_db = [4.8, 3.2, -1.5, -2.5, -1.0, 1.5, 4.0, 5.5, 6.0, 6.5];
            }
            engine.play_index(0);
        }

        Self {
            engine,
            visualizer,
            show_eq,
            show_playlist,
            show_vis_window: false,
            vis_fullscreen: false,
            last_vis_fullscreen: false,
            windowshade,
            remaining_time_mode,
            marquee_offset: 0.0,
            selected_playlist_idx,
            playlist_scroll: 0,
            eq_preset_name,
            last_save: std::time::Instant::now(),
            screenshot_path,
            screenshot_vis_path,
            screenshot_frames: 0,
        }
    }

    pub fn save_persisted_state(&self) {
        if self.screenshot_path.is_some() || self.screenshot_vis_path.is_some() {
            return;
        }
        let (eq_enabled, eq_preamp_db, eq_bands_db) = if let Ok(eq) = self.engine.eq_state.lock() {
            (eq.enabled, eq.preamp_db, eq.bands_db)
        } else {
            (true, 0.0, [0.0; 10])
        };

        let state = PersistedState {
            volume: self.engine.volume,
            balance: self.engine.balance,
            shuffle: self.engine.shuffle,
            repeat: self.engine.repeat,
            show_eq: self.show_eq,
            show_playlist: self.show_playlist,
            windowshade: self.windowshade,
            remaining_time_mode: self.remaining_time_mode,
            vis_mode: self.visualizer.mode,
            eq_enabled,
            eq_preamp_db,
            eq_bands_db,
            eq_preset_name: self.eq_preset_name.clone(),
            playlist: self.engine.playlist.clone(),
            current_index: self.engine.current_index,
        };
        storage::save_state(&state);
    }

    pub fn apply_eq_preset(&mut self, preset_name: &str) {
        self.eq_preset_name = preset_name.to_string();
        let (preamp, bands) = match preset_name {
            "Rock" => (0.0, [4.8, 3.2, -1.5, -2.5, -1.0, 1.5, 4.0, 5.5, 6.0, 6.5]),
            "Pop" => (0.0, [-1.5, 1.5, 4.2, 4.5, 3.0, -1.0, -1.5, -1.5, -1.0, -1.0]),
            "Techno" => (0.0, [5.5, 4.5, 0.0, -3.5, -3.0, 0.0, 4.8, 6.0, 6.0, 5.5]),
            "Bass Boost" => (2.0, [7.5, 6.0, 4.5, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
            "Classical" => (0.0, [3.5, 2.8, 2.0, 1.5, -1.0, -1.0, 0.0, 2.0, 3.2, 3.5]),
            "Club" => (0.0, [0.0, 0.0, 2.0, 3.5, 3.5, 3.5, 2.0, 0.0, 0.0, 0.0]),
            "Vocal" => (0.0, [-2.0, -2.5, -1.5, 1.5, 4.0, 4.0, 2.5, 0.0, -1.5, -2.5]),
            _ => (0.0, [0.0; 10]), // Flat
        };

        if let Ok(mut eq) = self.engine.eq_state.lock() {
            eq.preamp_db = preamp;
            eq.bands_db = bands;
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            // Drag and drop files
            if !i.raw.dropped_files.is_empty() {
                for file in &i.raw.dropped_files {
                    if let Some(ref path) = file.path {
                        let idx = self.engine.add_file(path);
                        if let Some(track_idx) = idx {
                            self.engine.play_index(track_idx);
                        }
                    }
                }
            }

            // Playback shortcuts
            if i.key_pressed(Key::Space) || i.key_pressed(Key::C) {
                self.engine.pause();
            }
            if i.key_pressed(Key::X) {
                self.engine.play();
            }
            if i.key_pressed(Key::V) {
                self.engine.stop();
            }
            if i.key_pressed(Key::Z) {
                self.engine.prev_track();
            }
            if i.key_pressed(Key::B) {
                self.engine.next_track();
            }
            if i.key_pressed(Key::L) {
                self.open_file_dialog();
            }

            // Volume
            if i.key_pressed(Key::ArrowUp) {
                let vol = (self.engine.volume + 0.05).min(1.0);
                self.engine.set_volume(vol);
            }
            if i.key_pressed(Key::ArrowDown) {
                let vol = (self.engine.volume - 0.05).max(0.0);
                self.engine.set_volume(vol);
            }

            // Seek
            if i.key_pressed(Key::ArrowLeft) {
                let target = (self.engine.elapsed_secs - 5.0).max(0.0);
                self.engine.seek(target);
            }
            if i.key_pressed(Key::ArrowRight) {
                let target = self.engine.elapsed_secs + 5.0;
                self.engine.seek(target);
            }
        });
    }

    pub fn open_file_dialog(&mut self) {
        if let Some(files) = rfd::FileDialog::new()
            .add_filter("Audio Files", &["mp3", "wav", "flac", "ogg"])
            .pick_files()
        {
            let mut first_new_idx = None;
            for path in files {
                let idx = self.engine.add_file(path);
                if first_new_idx.is_none() {
                    first_new_idx = idx;
                }
            }
            if let Some(idx) = first_new_idx {
                self.engine.play_index(idx);
            }
        }
    }
}

impl eframe::App for WinampApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Continuous redraw for 60fps retro visualizer & scrolling marquee
        ctx.request_repaint();

        self.handle_shortcuts(ctx);
        self.engine.update();
        self.visualizer.update();

        // Update marquee scroll
        self.marquee_offset += 0.8;

        // Custom Winamp Styling
        let mut style = (*ctx.style()).clone();
        style.visuals.panel_fill = COLOR_BG;
        style.visuals.window_fill = COLOR_BG;
        ctx.set_style(style);

        if self.screenshot_vis_path.is_some() {
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(Color32::from_rgb(5, 7, 5)))
                .show(ctx, |ui| {
                    let full_rect = ui.max_rect();
                    let painter = ui.painter_at(full_rect);
                    self.draw_visualizer_canvas(&painter, full_rect, true);
                    let track_label = self
                        .engine
                        .current_index
                        .and_then(|idx| self.engine.playlist.get(idx))
                        .map(|t| {
                            let secs = self.engine.elapsed_secs as u32;
                            format!("{}  [{:02}:{:02}]", t.title, secs / 60, secs % 60)
                        })
                        .unwrap_or_else(|| "RUSTAMP VISUALIZER".to_string());
                    painter.text(
                        full_rect.min + Vec2::new(14.0, 12.0),
                        Align2::LEFT_TOP,
                        track_label,
                        FontId::monospace(12.0),
                        Color32::from_rgba_unmultiplied(0, 235, 30, 180),
                    );
                    painter.text(
                        Pos2::new(full_rect.max.x - 14.0, full_rect.min.y + 12.0),
                        Align2::RIGHT_TOP,
                        "SPECTRUM  |  DBL-CLICK / ESC: EXIT FULLSCREEN  |  R-CLICK: MODE",
                        FontId::monospace(10.5),
                        Color32::from_rgba_unmultiplied(130, 165, 135, 170),
                    );
                });
        } else {
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(COLOR_BG))
                .show(ctx, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(0.0, 4.0);

                    // 1. MAIN PLAYER WINDOW
                    self.render_main_window(ui);

                    // 2. EQUALIZER WINDOW (if open)
                    if self.show_eq && !self.windowshade {
                        self.render_eq_window(ui);
                    }

                    // 3. PLAYLIST WINDOW (if open)
                    if self.show_playlist && !self.windowshade {
                        self.render_playlist_window(ui);
                    }
                });
        }

        // 4. SEPARATE LARGE / FULLSCREEN VISUALIZER WINDOW
        if self.show_vis_window {
            self.render_vis_window(ctx);
        }

        // Periodically persist settings & playlist every 3 seconds
        if self.last_save.elapsed().as_secs() >= 3 {
            self.save_persisted_state();
            self.last_save = std::time::Instant::now();
        }

        // Optional automated screenshot capture (--screenshot or --screenshot-vis <path.bmp>)
        let active_shot = self
            .screenshot_path
            .clone()
            .or_else(|| self.screenshot_vis_path.clone());
        if let Some(shot_path) = active_shot {
            self.screenshot_frames += 1;
            if self.screenshot_frames == 35 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
            }
            let mut captured = None;
            ctx.input(|i| {
                for ev in &i.raw.events {
                    if let egui::Event::Screenshot { image, .. } = ev {
                        captured = Some(image.clone());
                    }
                }
            });
            if let Some(img) = captured {
                write_color_image_bmp(&shot_path, &img);
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_persisted_state();
    }
}

impl WinampApp {
    // ==========================================
    // 1. MAIN PLAYER WINDOW
    // ==========================================
    fn render_main_window(&mut self, ui: &mut Ui) {
        let (rect, _response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), if self.windowshade { 28.0 } else { 152.0 }),
            Sense::hover(),
        );

        let painter = ui.painter_at(rect);

        // Background chassis
        painter.rect_filled(rect, 0.0, COLOR_BG);
        draw_bevel(&painter, rect, false);

        // Title Bar
        let title_rect = Rect::from_min_size(rect.min + Vec2::new(2.0, 2.0), Vec2::new(rect.width() - 4.0, 16.0));
        self.render_titlebar(&painter, title_rect, "RUSTAMP", true);

        if self.windowshade {
            // Windowshade mode: Mini compact bar
            self.render_windowshade_bar(ui, rect);
            return;
        }

        // --- LCD Screen Area ---
        let lcd_rect = Rect::from_min_size(rect.min + Vec2::new(8.0, 22.0), Vec2::new(rect.width() - 16.0, 52.0));
        painter.rect_filled(lcd_rect, 0.0, COLOR_LCD_BG);
        draw_bevel(&painter, lcd_rect, true);

        // Inside LCD: Visualizer (Left) + Digital Clock (Middle) + KBPS/KHZ/Stereo (Right)
        let vis_w = 76.0;
        let vis_rect = Rect::from_min_size(lcd_rect.min + Vec2::new(4.0, 4.0), Vec2::new(vis_w, 44.0));
        self.render_visualizer(ui, &painter, vis_rect);

        // Digital Clock (Middle)
        let clock_rect = Rect::from_min_size(
            Pos2::new(vis_rect.max.x + 8.0, lcd_rect.min.y + 4.0),
            Vec2::new(104.0, 22.0),
        );
        let current_track = self.engine.current_index.and_then(|idx| self.engine.playlist.get(idx));
        let total_duration = current_track.map(|t| t.duration_secs).unwrap_or(0.0);
        let display_secs = if self.remaining_time_mode && total_duration > 0.0 {
            total_duration - self.engine.elapsed_secs
        } else {
            self.engine.elapsed_secs
        };

        draw_digital_clock(&painter, clock_rect, display_secs, self.remaining_time_mode);

        // Clock clickable to toggle elapsed vs remaining
        let clock_response = ui.interact(clock_rect, Id::new("clock_toggle"), Sense::click());
        if clock_response.clicked() {
            self.remaining_time_mode = !self.remaining_time_mode;
        }

        // KBPS / KHZ / Stereo indicator
        let stats_rect = Rect::from_min_size(
            Pos2::new(clock_rect.max.x + 8.0, lcd_rect.min.y + 4.0),
            Vec2::new(lcd_rect.max.x - clock_rect.max.x - 12.0, 22.0),
        );
        let bitrate = current_track.map(|t| t.bitrate_kbps).unwrap_or(192);
        let sample_rate = current_track.map(|t| t.sample_rate / 1000).unwrap_or(44);
        let is_stereo = current_track.map(|t| t.channels >= 2).unwrap_or(true);

        painter.text(
            stats_rect.min + Vec2::new(0.0, 0.0),
            Align2::LEFT_TOP,
            format!("{:3} kbps", bitrate),
            FontId::monospace(9.5),
            COLOR_LED_GREEN,
        );
        painter.text(
            stats_rect.min + Vec2::new(0.0, 11.0),
            Align2::LEFT_TOP,
            format!("{:2} kHz  {}", sample_rate, if is_stereo { "STEREO" } else { "MONO" }),
            FontId::monospace(9.5),
            COLOR_LED_GREEN,
        );

        // Marquee scrolling text display (Bottom of LCD)
        let marquee_rect = Rect::from_min_size(
            Pos2::new(vis_rect.max.x + 8.0, lcd_rect.min.y + 28.0),
            Vec2::new(lcd_rect.max.x - vis_rect.max.x - 12.0, 18.0),
        );
        self.render_marquee(&painter, marquee_rect);

        // --- Track Seeker Bar ---
        let seeker_rect = Rect::from_min_size(
            rect.min + Vec2::new(8.0, 78.0),
            Vec2::new(rect.width() - 16.0, 12.0),
        );
        self.render_seeker(ui, &painter, seeker_rect, total_duration);

        // --- Volume & Balance Sliders ---
        let vol_rect = Rect::from_min_size(
            rect.min + Vec2::new(8.0, 94.0),
            Vec2::new(120.0, 14.0),
        );
        self.render_volume_slider(ui, &painter, vol_rect);

        let bal_rect = Rect::from_min_size(
            Pos2::new(vol_rect.max.x + 12.0, rect.min.y + 94.0),
            Vec2::new(80.0, 14.0),
        );
        self.render_balance_slider(ui, &painter, bal_rect);

        // Mode buttons: EQ, PL, SHUFFLE, REPEAT
        let eq_btn_rect = Rect::from_min_size(Pos2::new(bal_rect.max.x + 10.0, rect.min.y + 93.0), Vec2::new(26.0, 14.0));
        self.render_toggle_button(ui, &painter, eq_btn_rect, "EQ", self.show_eq, |app| {
            app.show_eq = !app.show_eq;
        });

        let pl_btn_rect = Rect::from_min_size(Pos2::new(eq_btn_rect.max.x + 4.0, rect.min.y + 93.0), Vec2::new(26.0, 14.0));
        self.render_toggle_button(ui, &painter, pl_btn_rect, "PL", self.show_playlist, |app| {
            app.show_playlist = !app.show_playlist;
        });

        let shuf_btn_rect = Rect::from_min_size(Pos2::new(pl_btn_rect.max.x + 6.0, rect.min.y + 93.0), Vec2::new(42.0, 14.0));
        self.render_toggle_button(ui, &painter, shuf_btn_rect, "SHUF", self.engine.shuffle, |app| {
            app.engine.shuffle = !app.engine.shuffle;
        });

        let rep_btn_rect = Rect::from_min_size(Pos2::new(shuf_btn_rect.max.x + 4.0, rect.min.y + 93.0), Vec2::new(42.0, 14.0));
        self.render_toggle_button(ui, &painter, rep_btn_rect, "REP", self.engine.repeat, |app| {
            app.engine.repeat = !app.engine.repeat;
        });

        // --- Transport Playback Buttons ---
        let trans_rect = Rect::from_min_size(
            rect.min + Vec2::new(8.0, 114.0),
            Vec2::new(rect.width() - 16.0, 28.0),
        );
        self.render_transport_buttons(ui, &painter, trans_rect);
    }

    // ==========================================
    // TITLEBAR
    // ==========================================
    fn render_titlebar(&mut self, painter: &Painter, rect: Rect, title: &str, is_main: bool) {
        // Brushed gradient
        painter.rect_filled(rect, 0.0, COLOR_TITLE_GRADIENT_START);
        painter.line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            Stroke::new(1.0_f32, COLOR_TITLE_GRADIENT_END),
        );

        // RustAmp R + Electric Lightning Bolt badge
        let badge_rect = Rect::from_min_size(rect.min + Vec2::new(3.0, 1.5), Vec2::new(20.0, 13.0));
        painter.rect_filled(badge_rect, 2.0, Color32::from_rgb(18, 22, 30));
        painter.rect_stroke(
            badge_rect,
            2.0,
            Stroke::new(1.0_f32, Color32::from_rgb(220, 175, 45)),
        );
        // 'R' on left in phosphor green
        painter.text(
            Pos2::new(badge_rect.min.x + 2.5, badge_rect.center().y),
            Align2::LEFT_CENTER,
            "R",
            FontId::monospace(9.0),
            COLOR_LED_GREEN,
        );
        // Electric lightning bolt on right (two crisp convex triangles)
        let bx = badge_rect.max.x - 6.0;
        let by = badge_rect.min.y + 1.5;
        let bh = badge_rect.height() - 3.0;
        let bolt_color = Color32::from_rgb(255, 215, 40);
        painter.add(egui::Shape::convex_polygon(
            vec![
                Pos2::new(bx + 1.4, by),
                Pos2::new(bx - 2.3, by + bh * 0.54),
                Pos2::new(bx + 0.7, by + bh * 0.48),
            ],
            bolt_color,
            Stroke::NONE,
        ));
        painter.add(egui::Shape::convex_polygon(
            vec![
                Pos2::new(bx - 0.3, by + bh * 0.44),
                Pos2::new(bx + 2.4, by + bh * 0.44),
                Pos2::new(bx - 1.3, by + bh),
            ],
            bolt_color,
            Stroke::NONE,
        ));

        // Title text
        painter.text(
            Pos2::new(badge_rect.max.x + 5.0, rect.min.y + 2.0),
            Align2::LEFT_TOP,
            title,
            FontId::proportional(11.0),
            Color32::from_rgb(230, 240, 255),
        );

        // Right side control buttons: Shade (=) and Close (X)
        if is_main {
            let shade_rect = Rect::from_min_size(rect.right_top() - Vec2::new(30.0, 0.0), Vec2::new(12.0, 12.0));
            painter.rect_filled(shade_rect, 0.0, COLOR_PANEL);
            draw_bevel(painter, shade_rect, false);
            painter.text(shade_rect.center(), Align2::CENTER_CENTER, "=", FontId::monospace(9.0), Color32::WHITE);

            let close_rect = Rect::from_min_size(rect.right_top() - Vec2::new(14.0, 0.0), Vec2::new(12.0, 12.0));
            painter.rect_filled(close_rect, 0.0, COLOR_PANEL);
            draw_bevel(painter, close_rect, false);
            painter.text(close_rect.center(), Align2::CENTER_CENTER, "X", FontId::monospace(9.0), Color32::WHITE);
        }
    }

    // ==========================================
    // VISUALIZER SCREEN
    // ==========================================
    fn render_visualizer(&mut self, ui: &mut Ui, painter: &Painter, rect: Rect) {
        painter.rect_filled(rect, 0.0, Color32::from_rgb(5, 7, 5));
        draw_bevel(painter, rect, true);

        // Single click cycles mode; double click pops out into a separate large visualizer window
        let resp = ui.interact(rect, Id::new("vis_click"), Sense::click());
        if resp.double_clicked() {
            if self.visualizer.mode == VisMode::Off {
                self.visualizer.mode = VisMode::Spectrum;
            }
            self.show_vis_window = true;
            self.vis_fullscreen = false;
        } else if resp.clicked() {
            self.visualizer.cycle_mode();
        }

        self.draw_visualizer_canvas(painter, rect, false);
    }

    fn draw_visualizer_canvas(&self, painter: &Painter, rect: Rect, large_mode: bool) {
        if large_mode {
            // Subtle CRT grid lines for the big visualizer window
            let grid_col = Color32::from_rgb(14, 24, 14);
            let cols = 12;
            let rows = 8;
            for c in 1..cols {
                let x = rect.min.x + rect.width() * (c as f32 / cols as f32);
                painter.line_segment(
                    [Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
                    Stroke::new(1.0_f32, grid_col),
                );
            }
            for r in 1..rows {
                let y = rect.min.y + rect.height() * (r as f32 / rows as f32);
                painter.line_segment(
                    [Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)],
                    Stroke::new(1.0_f32, grid_col),
                );
            }
        }

        match self.visualizer.mode {
            VisMode::Spectrum => {
                let bar_count = NUM_BANDS;
                let pad_x = if large_mode { 16.0 } else { 2.0 };
                let pad_y = if large_mode { 16.0 } else { 3.0 };
                let spacing = if large_mode { 6.0 } else { 1.0 };
                let bar_w = ((rect.width() - pad_x * 2.0 - (bar_count as f32 - 1.0) * spacing)
                    / bar_count as f32)
                    .max(2.0);
                let max_h = (rect.height() - pad_y * 2.0).max(8.0);
                let num_blocks: usize = if large_mode { 28 } else { 14 };
                let block_gap = if large_mode { 2.0 } else { 1.0 };

                for i in 0..bar_count {
                    let val = self.visualizer.bands[i].clamp(0.0, 1.0);
                    let x = rect.min.x + pad_x + i as f32 * (bar_w + spacing);

                    let block_h = max_h / num_blocks as f32;
                    let active_blocks = ((val * num_blocks as f32).ceil() as usize).min(num_blocks);

                    let green_limit = (num_blocks * 9) / 14;
                    let yellow_limit = (num_blocks * 12) / 14;

                    for b in 0..active_blocks {
                        let block_y = rect.max.y - pad_y - (b as f32 + 1.0) * block_h;
                        let b_rect = Rect::from_min_size(
                            Pos2::new(x, block_y),
                            Vec2::new(bar_w, (block_h - block_gap).max(1.0)),
                        );

                        let col = if b < green_limit {
                            COLOR_LED_GREEN
                        } else if b < yellow_limit {
                            COLOR_LED_YELLOW
                        } else {
                            COLOR_LED_RED
                        };
                        painter.rect_filled(b_rect, 0.0, col);
                    }

                    // Falling peak cap
                    let peak_val = self.visualizer.peaks[i].clamp(0.0, 1.0);
                    if peak_val > 0.05 {
                        let peak_y = rect.max.y - pad_y - (peak_val * max_h);
                        let p_rect = Rect::from_min_size(
                            Pos2::new(x, peak_y),
                            Vec2::new(bar_w, if large_mode { 4.0 } else { 1.5 }),
                        );
                        painter.rect_filled(p_rect, 0.0, Color32::from_rgb(160, 255, 210));
                    }
                }
            }
            VisMode::Oscilloscope => {
                let samples = &self.visualizer.osc_samples;
                if samples.len() > 10 {
                    let mid_y = rect.center().y;
                    let num_pts = if large_mode { 192 } else { 64 };
                    let step = rect.width() / num_pts as f32;
                    let stride = (samples.len() / num_pts).max(1);
                    let amp = rect.height() * 0.42;

                    for i in 0..(num_pts - 1) {
                        let s1 = samples.get(i * stride).copied().unwrap_or(0.0);
                        let s2 = samples.get((i + 1) * stride).copied().unwrap_or(0.0);

                        let p1 = Pos2::new(rect.min.x + i as f32 * step, mid_y - s1 * amp);
                        let p2 = Pos2::new(rect.min.x + (i + 1) as f32 * step, mid_y - s2 * amp);

                        if large_mode {
                            // Subtle phosphor outer glow
                            painter.line_segment(
                                [p1, p2],
                                Stroke::new(5.0_f32, Color32::from_rgba_unmultiplied(0, 235, 30, 45)),
                            );
                        }
                        painter.line_segment(
                            [p1, p2],
                            Stroke::new(if large_mode { 2.2_f32 } else { 1.2_f32 }, COLOR_LED_GREEN),
                        );
                    }
                } else {
                    let mid_y = rect.center().y;
                    painter.line_segment(
                        [Pos2::new(rect.min.x + 2.0, mid_y), Pos2::new(rect.max.x - 2.0, mid_y)],
                        Stroke::new(if large_mode { 2.0_f32 } else { 1.0_f32 }, COLOR_LED_DIM_GREEN),
                    );
                }
            }
            VisMode::Off => {
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    "OFF",
                    FontId::monospace(if large_mode { 18.0 } else { 9.0 }),
                    COLOR_LED_DIM_GREEN,
                );
            }
        }
    }

    fn render_vis_window(&mut self, ctx: &egui::Context) {
        let vis_id = egui::ViewportId::from_hash_of("rustamp_vis_window");
        let builder = egui::ViewportBuilder::default()
            .with_title("RustAmp Visualizer (Double-click for Fullscreen)")
            .with_inner_size(Vec2::new(760.0, 440.0))
            .with_min_inner_size(Vec2::new(320.0, 200.0))
            .with_icon(std::sync::Arc::new(crate::icon::build_window_icon()));

        ctx.show_viewport_immediate(vis_id, builder, |vis_ctx, _class| {
            vis_ctx.request_repaint();

            if vis_ctx.input(|i| i.viewport().close_requested()) {
                self.show_vis_window = false;
                self.vis_fullscreen = false;
                self.last_vis_fullscreen = false;
                return;
            }

            if vis_ctx.input(|i| i.key_pressed(Key::Escape)) {
                if self.vis_fullscreen {
                    self.vis_fullscreen = false;
                } else {
                    self.show_vis_window = false;
                    return;
                }
            }

            if vis_ctx.input(|i| i.key_pressed(Key::M)) {
                self.visualizer.mode = match self.visualizer.mode {
                    VisMode::Spectrum => VisMode::Oscilloscope,
                    VisMode::Oscilloscope => VisMode::Spectrum,
                    VisMode::Off => VisMode::Spectrum,
                };
            }

            if self.vis_fullscreen != self.last_vis_fullscreen {
                vis_ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.vis_fullscreen));
                self.last_vis_fullscreen = self.vis_fullscreen;
            }

            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(Color32::from_rgb(5, 7, 5)))
                .show(vis_ctx, |ui| {
                    let full_rect = ui.max_rect();
                    let painter = ui.painter_at(full_rect);

                    self.draw_visualizer_canvas(&painter, full_rect, true);

                    // Subtle HUD overlay: track info (top-left) and hint (top-right)
                    let track_label = self
                        .engine
                        .current_index
                        .and_then(|idx| self.engine.playlist.get(idx))
                        .map(|t| {
                            let secs = self.engine.elapsed_secs as u32;
                            format!("{}  [{:02}:{:02}]", t.title, secs / 60, secs % 60)
                        })
                        .unwrap_or_else(|| "RUSTAMP VISUALIZER".to_string());

                    painter.text(
                        full_rect.min + Vec2::new(14.0, 12.0),
                        Align2::LEFT_TOP,
                        track_label,
                        FontId::monospace(12.0),
                        Color32::from_rgba_unmultiplied(0, 235, 30, 180),
                    );

                    let mode_name = match self.visualizer.mode {
                        VisMode::Spectrum => "SPECTRUM",
                        VisMode::Oscilloscope => "OSCILLOSCOPE",
                        VisMode::Off => "OFF",
                    };
                    let hint = if self.vis_fullscreen {
                        format!("{}  |  DBL-CLICK / ESC: EXIT FULLSCREEN  |  R-CLICK: MODE", mode_name)
                    } else {
                        format!("{}  |  DBL-CLICK: FULLSCREEN  |  R-CLICK: MODE", mode_name)
                    };
                    painter.text(
                        Pos2::new(full_rect.max.x - 14.0, full_rect.min.y + 12.0),
                        Align2::RIGHT_TOP,
                        hint,
                        FontId::monospace(10.5),
                        Color32::from_rgba_unmultiplied(130, 165, 135, 170),
                    );

                    let resp = ui.interact(full_rect, Id::new("vis_win_interact"), Sense::click());
                    if resp.double_clicked() {
                        self.vis_fullscreen = !self.vis_fullscreen;
                    } else if resp.secondary_clicked() {
                        self.visualizer.mode = match self.visualizer.mode {
                            VisMode::Spectrum => VisMode::Oscilloscope,
                            VisMode::Oscilloscope => VisMode::Spectrum,
                            VisMode::Off => VisMode::Spectrum,
                        };
                    }
                });
        });
    }

    // ==========================================
    // MARQUEE DISPLAY
    // ==========================================
    fn render_marquee(&mut self, painter: &Painter, rect: Rect) {
        let clipped_painter = painter.with_clip_rect(rect);
        clipped_painter.rect_filled(rect, 0.0, Color32::from_rgb(4, 8, 4));
        draw_bevel(&clipped_painter, rect, true);

        let current_track = self.engine.current_index.and_then(|idx| self.engine.playlist.get(idx));
        let title = current_track
            .map(|t| format!("{}. {} ({:02}:{:02}) *** RUSTAMP ***", self.engine.current_index.unwrap_or(0) + 1, t.title, (t.duration_secs as u32) / 60, (t.duration_secs as u32) % 60))
            .unwrap_or_else(|| "RUSTAMP - RETRO AUDIO PLAYER IN RUST ***".to_string());

        let font = FontId::monospace(10.0);
        let galley = clipped_painter.layout_no_wrap(title, font, COLOR_LED_GREEN);
        let text_width = galley.size().x;
        let total_cycle = (text_width + rect.width()).max(1.0);

        let offset = self.marquee_offset % total_cycle;
        let x = rect.max.x - offset;
        let y = rect.center().y - galley.size().y * 0.5;

        clipped_painter.galley(Pos2::new(x, y), galley, COLOR_LED_GREEN);
    }

    // ==========================================
    // TRACK SEEKER BAR
    // ==========================================
    fn render_seeker(&mut self, ui: &mut Ui, painter: &Painter, rect: Rect, total_duration: f32) {
        draw_bevel(painter, rect, true);
        painter.rect_filled(rect, 0.0, Color32::from_rgb(18, 20, 24));

        let frac = if total_duration > 0.0 {
            (self.engine.elapsed_secs / total_duration).clamp(0.0, 1.0)
        } else {
            0.0
        };

        // Filled progress portion with striped retro pattern
        let fill_w = rect.width() * frac;
        if fill_w > 0.0 {
            let fill_rect = Rect::from_min_size(rect.min, Vec2::new(fill_w, rect.height()));
            painter.rect_filled(fill_rect, 0.0, Color32::from_rgb(30, 65, 45));
        }

        // Thumb slider
        let thumb_x = rect.min.x + fill_w;
        let thumb_rect = Rect::from_center_size(
            Pos2::new(thumb_x.clamp(rect.min.x + 6.0, rect.max.x - 6.0), rect.center().y),
            Vec2::new(12.0, rect.height() + 2.0),
        );
        painter.rect_filled(thumb_rect, 1.0, COLOR_PANEL);
        draw_bevel(painter, thumb_rect, false);

        // Interactive seek dragging
        let resp = ui.interact(rect, Id::new("seeker_drag"), Sense::click_and_drag());
        if resp.clicked() || resp.dragged() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let click_frac = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
                if total_duration > 0.0 {
                    self.engine.seek(click_frac * total_duration);
                }
            }
        }
    }

    // ==========================================
    // VOLUME SLIDER
    // ==========================================
    fn render_volume_slider(&mut self, ui: &mut Ui, painter: &Painter, rect: Rect) {
        painter.text(
            rect.min - Vec2::new(0.0, 10.0),
            Align2::LEFT_TOP,
            format!("VOL: {:3}%", (self.engine.volume * 100.0).round() as u32),
            FontId::monospace(8.5),
            COLOR_TEXT_DIM,
        );

        draw_bevel(painter, rect, true);
        painter.rect_filled(rect, 0.0, Color32::from_rgb(18, 20, 24));

        // Gauge bars
        let num_bars = 24;
        let bar_w = (rect.width() - 4.0) / num_bars as f32;
        let active_bars = (self.engine.volume * num_bars as f32).round() as usize;

        for b in 0..num_bars {
            let col = if b < active_bars {
                if b < 16 {
                    COLOR_LED_GREEN
                } else if b < 20 {
                    COLOR_LED_YELLOW
                } else {
                    COLOR_LED_RED
                }
            } else {
                Color32::from_rgb(25, 28, 32)
            };
            let b_rect = Rect::from_min_size(
                Pos2::new(rect.min.x + 2.0 + b as f32 * bar_w, rect.min.y + 2.0),
                Vec2::new((bar_w - 1.0).max(1.0), rect.height() - 4.0),
            );
            painter.rect_filled(b_rect, 0.0, col);
        }

        let resp = ui.interact(rect, Id::new("vol_drag"), Sense::click_and_drag());
        if resp.clicked() || resp.dragged() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let frac = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
                self.engine.set_volume(frac);
            }
        }
    }

    // ==========================================
    // BALANCE SLIDER
    // ==========================================
    fn render_balance_slider(&mut self, ui: &mut Ui, painter: &Painter, rect: Rect) {
        let bal_label = if self.engine.balance.abs() < 0.01 {
            "BAL: CENTER".to_string()
        } else if self.engine.balance < 0.0 {
            format!("BAL: L {:2}%", (-self.engine.balance * 100.0).round() as u32)
        } else {
            format!("BAL: R {:2}%", (self.engine.balance * 100.0).round() as u32)
        };
        painter.text(
            rect.min - Vec2::new(0.0, 10.0),
            Align2::LEFT_TOP,
            bal_label,
            FontId::monospace(8.5),
            COLOR_TEXT_DIM,
        );

        draw_bevel(painter, rect, true);
        painter.rect_filled(rect, 0.0, Color32::from_rgb(18, 20, 24));

        // Center tick
        let mid_x = rect.center().x;
        painter.line_segment(
            [Pos2::new(mid_x, rect.min.y + 2.0), Pos2::new(mid_x, rect.max.y - 2.0)],
            Stroke::new(1.0_f32, COLOR_TEXT_DIM),
        );

        // Thumb position
        let thumb_x = mid_x + self.engine.balance * (rect.width() * 0.45);
        let thumb_rect = Rect::from_center_size(
            Pos2::new(thumb_x, rect.center().y),
            Vec2::new(8.0, rect.height() - 2.0),
        );
        painter.rect_filled(thumb_rect, 0.5, COLOR_PANEL);
        draw_bevel(painter, thumb_rect, false);

        let resp = ui.interact(rect, Id::new("bal_drag"), Sense::click_and_drag());
        if resp.clicked() || resp.dragged() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let frac = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
                let bal = (frac - 0.5) * 2.0;
                // Deadzone near center for easy zeroing
                let bal = if bal.abs() < 0.08 { 0.0 } else { bal };
                self.engine.set_balance(bal);
            }
        }
    }

    // ==========================================
    // TOGGLE BUTTON (EQ, PL, SHUF, REP)
    // ==========================================
    fn render_toggle_button<F: FnOnce(&mut Self)>(
        &mut self,
        ui: &mut Ui,
        painter: &Painter,
        rect: Rect,
        text: &str,
        active: bool,
        on_click: F,
    ) {
        let resp = ui.interact(rect, Id::new(format!("btn_{}", text)), Sense::click());
        let depressed = resp.is_pointer_button_down_on();

        let bg = if active {
            Color32::from_rgb(55, 62, 70)
        } else {
            COLOR_PANEL
        };
        painter.rect_filled(rect, 0.5, bg);
        draw_bevel(painter, rect, depressed || active);

        let text_color = if active {
            COLOR_LED_GREEN
        } else {
            COLOR_TEXT_DIM
        };

        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            text,
            FontId::monospace(8.5),
            text_color,
        );

        if resp.clicked() {
            on_click(self);
        }
    }

    // ==========================================
    // TRANSPORT BUTTONS (Prev, Play, Pause, Stop, Next, Eject)
    // ==========================================
    fn render_transport_buttons(&mut self, ui: &mut Ui, painter: &Painter, rect: Rect) {
        let buttons = [
            ("⏮", "prev"),
            ("▶", "play"),
            ("⏸", "pause"),
            ("⏹", "stop"),
            ("⏭", "next"),
            ("⏏", "eject"),
        ];

        let spacing = 4.0;
        let btn_w = (rect.width() - spacing * (buttons.len() as f32 - 1.0)) / buttons.len() as f32;

        for (i, (symbol, name)) in buttons.iter().enumerate() {
            let b_rect = Rect::from_min_size(
                Pos2::new(rect.min.x + i as f32 * (btn_w + spacing), rect.min.y),
                Vec2::new(btn_w, rect.height()),
            );

            let resp = ui.interact(b_rect, Id::new(format!("trans_{}", name)), Sense::click());
            let depressed = resp.is_pointer_button_down_on();

            let is_active = match *name {
                "play" => self.engine.state == PlaybackState::Playing,
                "pause" => self.engine.state == PlaybackState::Paused,
                "stop" => self.engine.state == PlaybackState::Stopped,
                _ => false,
            };

            let bg = if is_active {
                Color32::from_rgb(50, 60, 50)
            } else {
                COLOR_PANEL
            };
            painter.rect_filled(b_rect, 1.0, bg);
            draw_bevel(painter, b_rect, depressed);

            let col = if is_active {
                COLOR_LED_GREEN
            } else {
                Color32::from_rgb(220, 230, 240)
            };

            painter.text(
                b_rect.center(),
                Align2::CENTER_CENTER,
                *symbol,
                FontId::proportional(14.0),
                col,
            );

            if resp.clicked() {
                match *name {
                    "prev" => self.engine.prev_track(),
                    "play" => self.engine.play(),
                    "pause" => self.engine.pause(),
                    "stop" => self.engine.stop(),
                    "next" => self.engine.next_track(),
                    "eject" => self.open_file_dialog(),
                    _ => {}
                }
            }
        }
    }

    // ==========================================
    // WINDOWSHADE MODE BAR
    // ==========================================
    fn render_windowshade_bar(&mut self, ui: &mut Ui, rect: Rect) {
        let shade_btn = Rect::from_min_size(rect.right_top() - Vec2::new(30.0, -2.0), Vec2::new(12.0, 12.0));
        let resp = ui.interact(shade_btn, Id::new("shade_unroll"), Sense::click());
        if resp.clicked() {
            self.windowshade = false;
        }

        // Mini controls on shade bar
        let painter = ui.painter_at(rect);
        let trans_rect = Rect::from_min_size(rect.min + Vec2::new(180.0, 4.0), Vec2::new(120.0, 18.0));
        self.render_transport_buttons(ui, &painter, trans_rect);
    }

    // ==========================================
    // 2. EQUALIZER WINDOW
    // ==========================================
    fn render_eq_window(&mut self, ui: &mut Ui) {
        let (rect, _response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), 168.0),
            Sense::hover(),
        );

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, COLOR_BG);
        draw_bevel(&painter, rect, false);

        // EQ Title bar
        let title_rect = Rect::from_min_size(rect.min + Vec2::new(2.0, 2.0), Vec2::new(rect.width() - 4.0, 16.0));
        self.render_titlebar(&painter, title_rect, "RUSTAMP EQUALIZER", false);

        // Fetch EQ state
        let (mut eq_enabled, mut preamp_db, mut bands_db) = if let Ok(eq) = self.engine.eq_state.lock() {
            (eq.enabled, eq.preamp_db, eq.bands_db)
        } else {
            (true, 0.0, [0.0; 10])
        };

        // ON / AUTO toggles + Presets dropdown
        let on_rect = Rect::from_min_size(rect.min + Vec2::new(12.0, 24.0), Vec2::new(32.0, 16.0));
        self.render_toggle_button(ui, &painter, on_rect, "ON", eq_enabled, |_app| {
            eq_enabled = !eq_enabled;
        });

        let preset_btn_rect = Rect::from_min_size(rect.min + Vec2::new(50.0, 24.0), Vec2::new(104.0, 16.0));
        let preset_resp = ui.interact(preset_btn_rect, Id::new("eq_preset_cycle"), Sense::click());
        let preset_depressed = preset_resp.is_pointer_button_down_on();
        painter.rect_filled(
            preset_btn_rect,
            0.5,
            if preset_resp.hovered() {
                Color32::from_rgb(58, 62, 72)
            } else {
                COLOR_PANEL
            },
        );
        draw_bevel(&painter, preset_btn_rect, preset_depressed);
        painter.text(
            preset_btn_rect.center(),
            Align2::CENTER_CENTER,
            format!("PRESET: {}", self.eq_preset_name),
            FontId::monospace(8.0),
            COLOR_LED_YELLOW,
        );

        if preset_resp.clicked() {
            let presets = ["Flat", "Rock", "Pop", "Techno", "Bass Boost", "Club", "Classical", "Vocal"];
            let cur_idx = presets.iter().position(|&p| p == self.eq_preset_name).unwrap_or(0);
            let next_p = presets[(cur_idx + 1) % presets.len()];
            self.apply_eq_preset(next_p);
            if let Ok(eq) = self.engine.eq_state.lock() {
                preamp_db = eq.preamp_db;
                bands_db = eq.bands_db;
            }
        }

        // Real-time EQ Frequency Curve Display (Graph)
        let graph_rect = Rect::from_min_size(
            rect.min + Vec2::new(160.0, 22.0),
            Vec2::new(rect.width() - 172.0, 20.0),
        );
        painter.rect_filled(graph_rect, 0.0, COLOR_LCD_BG);
        draw_bevel(&painter, graph_rect, true);

        // Draw zero baseline
        let mid_y = graph_rect.center().y;
        painter.line_segment(
            [Pos2::new(graph_rect.min.x, mid_y), Pos2::new(graph_rect.max.x, mid_y)],
            Stroke::new(1.0_f32, Color32::from_rgb(25, 45, 25)),
        );

        // Draw EQ frequency response spline curve
        let num_curve_pts = 32;
        let mut prev_pt = None;
        for c in 0..=num_curve_pts {
            let frac = c as f32 / num_curve_pts as f32;
            let band_idx = (frac * 9.0).clamp(0.0, 9.0);
            let b0 = band_idx.floor() as usize;
            let b1 = (b0 + 1).min(9);
            let t = band_idx.fract();
            let gain = bands_db[b0] * (1.0 - t) + bands_db[b1] * t + preamp_db * 0.5;

            let pt_x = graph_rect.min.x + frac * graph_rect.width();
            let pt_y = (mid_y - (gain / 12.0) * (graph_rect.height() * 0.42)).clamp(graph_rect.min.y + 1.0, graph_rect.max.y - 1.0);
            let cur_pt = Pos2::new(pt_x, pt_y);

            if let Some(p) = prev_pt {
                painter.line_segment([p, cur_pt], Stroke::new(1.2_f32, COLOR_LED_GREEN));
            }
            prev_pt = Some(cur_pt);
        }

        // 11 Vertical Sliders: Preamp + 10 EQ Bands
        let labels = ["PRE", "60", "170", "310", "600", "1K", "3K", "6K", "12K", "14K", "16K"];
        let slider_area_rect = Rect::from_min_size(
            rect.min + Vec2::new(10.0, 48.0),
            Vec2::new(rect.width() - 20.0, 110.0),
        );

        let col_w = slider_area_rect.width() / 11.0;

        for (i, label) in labels.iter().enumerate() {
            let col_x = slider_area_rect.min.x + i as f32 * col_w;
            let track_rect = Rect::from_min_size(
                Pos2::new(col_x + col_w * 0.5 - 2.0, slider_area_rect.min.y + 4.0),
                Vec2::new(4.0, 80.0),
            );

            // Groove
            painter.rect_filled(track_rect, 1.0, Color32::from_rgb(18, 20, 24));
            draw_bevel(&painter, track_rect, true);

            // Zero line tick
            let track_mid_y = track_rect.center().y;
            painter.line_segment(
                [Pos2::new(track_rect.min.x - 3.0, track_mid_y), Pos2::new(track_rect.max.x + 3.0, track_mid_y)],
                Stroke::new(1.0_f32, COLOR_TEXT_DIM),
            );

            let cur_val = if i == 0 { preamp_db } else { bands_db[i - 1] };
            let frac = ((cur_val + 12.0) / 24.0).clamp(0.0, 1.0); // 0.0 = -12dB, 1.0 = +12dB
            let thumb_y = track_rect.max.y - frac * track_rect.height();

            // Slider thumb
            let thumb_rect = Rect::from_center_size(
                Pos2::new(track_rect.center().x, thumb_y),
                Vec2::new(14.0, 8.0),
            );
            painter.rect_filled(thumb_rect, 0.5, COLOR_PANEL);
            draw_bevel(&painter, thumb_rect, false);

            // Label text under slider
            painter.text(
                Pos2::new(col_x + col_w * 0.5, slider_area_rect.min.y + 92.0),
                Align2::CENTER_TOP,
                *label,
                FontId::monospace(8.0),
                COLOR_TEXT_DIM,
            );

            // Interactable slider
            let col_interact_rect = Rect::from_min_size(
                Pos2::new(col_x, slider_area_rect.min.y),
                Vec2::new(col_w, 90.0),
            );
            let resp = ui.interact(col_interact_rect, Id::new(format!("eq_slider_{}", i)), Sense::click_and_drag());
            if resp.clicked() || resp.dragged() {
                if let Some(pos) = resp.interact_pointer_pos() {
                    let click_frac = 1.0 - ((pos.y - track_rect.min.y) / track_rect.height()).clamp(0.0, 1.0);
                    let new_db = (click_frac * 24.0 - 12.0).clamp(-12.0, 12.0);
                    if i == 0 {
                        preamp_db = new_db;
                    } else {
                        bands_db[i - 1] = new_db;
                    }
                }
            }
        }

        // Save EQ state back
        if let Ok(mut eq) = self.engine.eq_state.lock() {
            eq.enabled = eq_enabled;
            eq.preamp_db = preamp_db;
            eq.bands_db = bands_db;
        }
    }

    // ==========================================
    // 3. PLAYLIST EDITOR WINDOW
    // ==========================================
    fn render_playlist_window(&mut self, ui: &mut Ui) {
        let playlist_h = ui.available_height().max(168.0);
        let (rect, _response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), playlist_h),
            Sense::hover(),
        );

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, COLOR_BG);
        draw_bevel(&painter, rect, false);

        // Playlist Title bar
        let title_rect = Rect::from_min_size(rect.min + Vec2::new(2.0, 2.0), Vec2::new(rect.width() - 4.0, 16.0));
        self.render_titlebar(&painter, title_rect, "RUSTAMP PLAYLIST", false);

        // Playlist Table Area (LCD-style list, auto-expands with window height)
        let list_h = (rect.height() - 54.0).max(114.0);
        let list_rect = Rect::from_min_size(
            rect.min + Vec2::new(8.0, 22.0),
            Vec2::new(rect.width() - 16.0, list_h),
        );
        painter.rect_filled(list_rect, 0.0, COLOR_LCD_BG);
        draw_bevel(&painter, list_rect, true);

        // Render tracks in scrollable list
        let mut play_track_idx = None;
        let mut remove_track_idx = None;

        let row_h = 16.0;
        let visible_rows = ((list_rect.height() - 4.0) / row_h).floor().max(1.0) as usize;
        let total_rows = self.engine.playlist.len();
        let max_scroll = total_rows.saturating_sub(visible_rows);
        self.playlist_scroll = self.playlist_scroll.min(max_scroll);

        // Mouse wheel scrolling over the playlist area
        let scroll_delta = ui.input(|i| {
            if i.pointer
                .hover_pos()
                .map(|p| list_rect.contains(p))
                .unwrap_or(false)
            {
                i.raw_scroll_delta.y
            } else {
                0.0
            }
        });
        if scroll_delta > 0.0 {
            self.playlist_scroll = self.playlist_scroll.saturating_sub(1);
        } else if scroll_delta < 0.0 {
            self.playlist_scroll = (self.playlist_scroll + 1).min(max_scroll);
        }

        let scrollbar_w = 12.0;
        let rows_w = list_rect.width() - scrollbar_w - 6.0;

        for (view_i, idx) in (self.playlist_scroll..total_rows).take(visible_rows).enumerate() {
            let track = &self.engine.playlist[idx];
            let row_y = list_rect.min.y + 2.0 + view_i as f32 * row_h;
            let row_rect = Rect::from_min_size(
                Pos2::new(list_rect.min.x + 2.0, row_y),
                Vec2::new(rows_w, row_h),
            );

            let is_current = self.engine.current_index == Some(idx);
            let is_selected = self.selected_playlist_idx == Some(idx);

            if is_selected {
                painter.rect_filled(row_rect, 0.0, Color32::from_rgb(15, 35, 60));
            }

            let text_color = if is_current {
                COLOR_LED_GREEN
            } else if is_selected {
                Color32::from_rgb(220, 240, 255)
            } else {
                Color32::from_rgb(160, 175, 190)
            };

            let duration_str = format!(
                "{:02}:{:02}",
                (track.duration_secs as u32) / 60,
                (track.duration_secs as u32) % 60
            );
            let max_title_chars = ((rows_w - 52.0) / 5.8).max(8.0) as usize;
            let raw_title = format!("{}. {}", idx + 1, track.title);
            let title_display = if raw_title.chars().count() > max_title_chars {
                let clipped: String = raw_title.chars().take(max_title_chars.saturating_sub(2)).collect();
                format!("{}..", clipped)
            } else {
                raw_title
            };

            // Title
            painter.text(
                row_rect.min + Vec2::new(4.0, 2.0),
                Align2::LEFT_TOP,
                title_display,
                FontId::monospace(9.5),
                text_color,
            );

            // Duration right aligned
            painter.text(
                row_rect.right_top() - Vec2::new(6.0, -2.0),
                Align2::RIGHT_TOP,
                duration_str,
                FontId::monospace(9.5),
                text_color,
            );

            let resp = ui.interact(row_rect, Id::new(("pl_row", idx)), Sense::click());
            if resp.clicked() {
                self.selected_playlist_idx = Some(idx);
            }
            if resp.double_clicked() {
                play_track_idx = Some(idx);
            }
        }

        if let Some(idx) = play_track_idx {
            self.engine.play_index(idx);
        }

        // Classic Winamp Vertical Scrollbar
        let sb_rect = Rect::from_min_size(
            Pos2::new(list_rect.max.x - scrollbar_w - 2.0, list_rect.min.y + 2.0),
            Vec2::new(scrollbar_w, list_rect.height() - 4.0),
        );
        painter.rect_filled(sb_rect, 0.0, Color32::from_rgb(16, 18, 22));
        draw_bevel(&painter, sb_rect, true);

        let thumb_h = if total_rows <= visible_rows || total_rows == 0 {
            sb_rect.height() - 2.0
        } else {
            ((visible_rows as f32 / total_rows as f32) * (sb_rect.height() - 2.0)).clamp(16.0, sb_rect.height() - 2.0)
        };
        let scroll_frac = if max_scroll > 0 {
            self.playlist_scroll as f32 / max_scroll as f32
        } else {
            0.0
        };
        let travel = (sb_rect.height() - 2.0 - thumb_h).max(0.0);
        let thumb_y = sb_rect.min.y + 1.0 + scroll_frac * travel;
        let sb_thumb = Rect::from_min_size(
            Pos2::new(sb_rect.min.x + 1.0, thumb_y),
            Vec2::new(sb_rect.width() - 2.0, thumb_h),
        );
        painter.rect_filled(
            sb_thumb,
            0.5,
            if max_scroll > 0 {
                COLOR_PANEL
            } else {
                Color32::from_rgb(32, 34, 40)
            },
        );
        draw_bevel(&painter, sb_thumb, false);

        if max_scroll > 0 {
            let sb_resp = ui.interact(sb_rect, Id::new("pl_scrollbar"), Sense::click_and_drag());
            if sb_resp.clicked() || sb_resp.dragged() {
                if let Some(pos) = sb_resp.interact_pointer_pos() {
                    let click_frac = ((pos.y - sb_rect.min.y - thumb_h * 0.5) / travel.max(1.0)).clamp(0.0, 1.0);
                    self.playlist_scroll = (click_frac * max_scroll as f32).round() as usize;
                }
            }
        }

        // Playlist Bottom Action Buttons (pinned to bottom of playlist panel)
        let bottom_rect = Rect::from_min_size(
            Pos2::new(rect.min.x + 8.0, rect.max.y - 28.0),
            Vec2::new(rect.width() - 16.0, 22.0),
        );

        let action_btns = ["+ FILE", "+ DEMO", "- REM", "CLEAR"];
        let btn_w = 58.0;

        for (i, &action) in action_btns.iter().enumerate() {
            let b_rect = Rect::from_min_size(
                Pos2::new(bottom_rect.min.x + i as f32 * (btn_w + 4.0), bottom_rect.min.y),
                Vec2::new(btn_w, 20.0),
            );

            let resp = ui.interact(b_rect, Id::new(format!("pl_act_{}", action)), Sense::click());
            let depressed = resp.is_pointer_button_down_on();

            painter.rect_filled(b_rect, 0.5, COLOR_PANEL);
            draw_bevel(&painter, b_rect, depressed);
            painter.text(
                b_rect.center(),
                Align2::CENTER_CENTER,
                action,
                FontId::monospace(9.0),
                Color32::from_rgb(220, 230, 240),
            );

            if resp.clicked() {
                match action {
                    "+ FILE" => self.open_file_dialog(),
                    "+ DEMO" => self.engine.add_demo_tracks(),
                    "- REM" => {
                        if let Some(sel) = self.selected_playlist_idx {
                            remove_track_idx = Some(sel);
                        }
                    }
                    "CLEAR" => {
                        self.engine.stop();
                        self.engine.playlist.clear();
                        self.engine.current_index = None;
                        self.selected_playlist_idx = None;
                    }
                    _ => {}
                }
            }
        }

        if let Some(rem) = remove_track_idx {
            if rem < self.engine.playlist.len() {
                self.engine.playlist.remove(rem);
                if self.engine.current_index == Some(rem) {
                    self.engine.stop();
                    self.engine.current_index = None;
                }
                self.selected_playlist_idx = None;
            }
        }

        // Playlist Stats (Right bottom)
        let total_secs: f32 = self.engine.playlist.iter().map(|t| t.duration_secs).sum();
        let stats_str = format!(
            "{} TRACKS  {:02}:{:02}",
            self.engine.playlist.len(),
            (total_secs as u32) / 60,
            (total_secs as u32) % 60
        );
        painter.text(
            bottom_rect.right_center() - Vec2::new(4.0, 0.0),
            Align2::RIGHT_CENTER,
            stats_str,
            FontId::monospace(9.0),
            COLOR_LED_GREEN,
        );
    }
}

fn write_color_image_bmp(path: &str, img: &egui::ColorImage) {
    let w = img.width() as u32;
    let h = img.height() as u32;
    let pixel_bytes = w * h * 4;
    let file_size = 54 + pixel_bytes;
    let mut bmp = Vec::with_capacity(file_size as usize);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&file_size.to_le_bytes());
    bmp.extend_from_slice(&0u32.to_le_bytes());
    bmp.extend_from_slice(&54u32.to_le_bytes());
    bmp.extend_from_slice(&40u32.to_le_bytes());
    bmp.extend_from_slice(&(w as i32).to_le_bytes());
    bmp.extend_from_slice(&(-(h as i32)).to_le_bytes());
    bmp.extend_from_slice(&1u16.to_le_bytes());
    bmp.extend_from_slice(&32u16.to_le_bytes());
    bmp.extend_from_slice(&0u32.to_le_bytes());
    bmp.extend_from_slice(&pixel_bytes.to_le_bytes());
    bmp.extend_from_slice(&0i32.to_le_bytes());
    bmp.extend_from_slice(&0i32.to_le_bytes());
    bmp.extend_from_slice(&0u32.to_le_bytes());
    bmp.extend_from_slice(&0u32.to_le_bytes());
    for c in &img.pixels {
        bmp.extend_from_slice(&[c.b(), c.g(), c.r(), c.a()]);
    }
    let _ = std::fs::write(path, bmp);
}
