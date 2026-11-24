use eframe::egui::{Color32, Painter, Pos2, Rect, Stroke, Vec2};

pub const COLOR_BG: Color32 = Color32::from_rgb(34, 36, 42);
pub const COLOR_PANEL: Color32 = Color32::from_rgb(45, 48, 56);
pub const COLOR_PANEL_BORDER_LIGHT: Color32 = Color32::from_rgb(85, 90, 105);
pub const COLOR_PANEL_BORDER_DARK: Color32 = Color32::from_rgb(18, 19, 23);
pub const COLOR_LCD_BG: Color32 = Color32::from_rgb(8, 12, 8);
pub const COLOR_LED_GREEN: Color32 = Color32::from_rgb(0, 235, 30);
pub const COLOR_LED_DIM_GREEN: Color32 = Color32::from_rgb(15, 45, 20);
pub const COLOR_LED_YELLOW: Color32 = Color32::from_rgb(240, 220, 20);
pub const COLOR_LED_RED: Color32 = Color32::from_rgb(245, 50, 40);
pub const COLOR_TEXT_DIM: Color32 = Color32::from_rgb(130, 140, 155);
pub const COLOR_TITLE_GRADIENT_START: Color32 = Color32::from_rgb(22, 54, 95);
pub const COLOR_TITLE_GRADIENT_END: Color32 = Color32::from_rgb(12, 28, 52);

/// Draws a classic 90s beveled border around a rect
pub fn draw_bevel(painter: &Painter, rect: Rect, depressed: bool) {
    let top_left = if depressed {
        COLOR_PANEL_BORDER_DARK
    } else {
        COLOR_PANEL_BORDER_LIGHT
    };
    let bottom_right = if depressed {
        COLOR_PANEL_BORDER_LIGHT
    } else {
        COLOR_PANEL_BORDER_DARK
    };

    // Outer border
    painter.line_segment(
        [rect.left_top(), rect.right_top()],
        Stroke::new(1.0_f32, top_left),
    );
    painter.line_segment(
        [rect.left_top(), rect.left_bottom()],
        Stroke::new(1.0_f32, top_left),
    );
    painter.line_segment(
        [rect.right_top(), rect.right_bottom()],
        Stroke::new(1.0_f32, bottom_right),
    );
    painter.line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0_f32, bottom_right),
    );
}

/// Draws an authentic 7-segment digit at the specified bounding box
pub fn draw_7segment_digit(painter: &Painter, rect: Rect, digit: char, on_color: Color32, off_color: Color32) {
    let w = rect.width();
    let h = rect.height();
    let t = (w * 0.16).clamp(1.5, 4.0); // thickness

    let segments = match digit {
        '0' => [true, true, true, true, true, true, false],
        '1' => [false, true, true, false, false, false, false],
        '2' => [true, true, false, true, true, false, true],
        '3' => [true, true, true, true, false, false, true],
        '4' => [false, true, true, false, false, true, true],
        '5' => [true, false, true, true, false, true, true],
        '6' => [true, false, true, true, true, true, true],
        '7' => [true, true, true, false, false, false, false],
        '8' => [true, true, true, true, true, true, true],
        '9' => [true, true, true, true, false, true, true],
        '-' => [false, false, false, false, false, false, true],
        _ => [false, false, false, false, false, false, false],
    };

    let p = rect.min;
    let mid_y = p.y + h * 0.5;

    // Segment rects:
    // a: top horiz
    let seg_a = Rect::from_min_size(Pos2::new(p.x + t, p.y), Vec2::new(w - 2.0 * t, t));
    // b: top right vert
    let seg_b = Rect::from_min_size(Pos2::new(p.x + w - t, p.y + t), Vec2::new(t, h * 0.5 - 1.5 * t));
    // c: bottom right vert
    let seg_c = Rect::from_min_size(Pos2::new(p.x + w - t, mid_y + 0.5 * t), Vec2::new(t, h * 0.5 - 1.5 * t));
    // d: bottom horiz
    let seg_d = Rect::from_min_size(Pos2::new(p.x + t, p.y + h - t), Vec2::new(w - 2.0 * t, t));
    // e: bottom left vert
    let seg_e = Rect::from_min_size(Pos2::new(p.x, mid_y + 0.5 * t), Vec2::new(t, h * 0.5 - 1.5 * t));
    // f: top left vert
    let seg_f = Rect::from_min_size(Pos2::new(p.x, p.y + t), Vec2::new(t, h * 0.5 - 1.5 * t));
    // g: middle horiz
    let seg_g = Rect::from_min_size(Pos2::new(p.x + t, mid_y - 0.5 * t), Vec2::new(w - 2.0 * t, t));

    let rects = [seg_a, seg_b, seg_c, seg_d, seg_e, seg_f, seg_g];

    for (i, &active) in segments.iter().enumerate() {
        let col = if active { on_color } else { off_color };
        painter.rect_filled(rects[i], 0.5, col);
    }
}

/// Draws digital clock time: e.g. "-02:45" or " 01:23"
pub fn draw_digital_clock(painter: &Painter, rect: Rect, seconds: f32, is_remaining: bool) {
    let abs_secs = seconds.abs().max(0.0) as u32;
    let mins = (abs_secs / 60).min(99);
    let secs = abs_secs % 60;

    let time_str = format!("{:02}{:02}", mins, secs);

    let digit_w = rect.width() * 0.17;
    let digit_h = rect.height() * 0.85;
    let start_y = rect.min.y + (rect.height() - digit_h) * 0.5;

    let mut cursor_x = rect.min.x + 2.0;

    // Negative indicator if remaining mode
    if is_remaining {
        let minus_rect = Rect::from_min_size(
            Pos2::new(cursor_x, start_y),
            Vec2::new(digit_w * 0.8, digit_h),
        );
        draw_7segment_digit(painter, minus_rect, '-', COLOR_LED_GREEN, COLOR_LED_DIM_GREEN);
    }
    cursor_x += digit_w * 0.85;

    // Minute digits (tens, ones)
    for (i, ch) in time_str.chars().take(2).enumerate() {
        let d_rect = Rect::from_min_size(
            Pos2::new(cursor_x + i as f32 * (digit_w + 2.0), start_y),
            Vec2::new(digit_w, digit_h),
        );
        draw_7segment_digit(painter, d_rect, ch, COLOR_LED_GREEN, COLOR_LED_DIM_GREEN);
    }
    cursor_x += 2.0 * (digit_w + 2.0) + 2.0;

    // Colon dots (pulsing or solid)
    let dot_r = (digit_w * 0.12).clamp(1.5, 3.0);
    let colon_top = Pos2::new(cursor_x + dot_r, start_y + digit_h * 0.3);
    let colon_bot = Pos2::new(cursor_x + dot_r, start_y + digit_h * 0.7);
    painter.circle_filled(colon_top, dot_r, COLOR_LED_GREEN);
    painter.circle_filled(colon_bot, dot_r, COLOR_LED_GREEN);
    cursor_x += dot_r * 2.0 + 4.0;

    // Second digits (tens, ones)
    for (i, ch) in time_str.chars().skip(2).take(2).enumerate() {
        let d_rect = Rect::from_min_size(
            Pos2::new(cursor_x + i as f32 * (digit_w + 2.0), start_y),
            Vec2::new(digit_w, digit_h),
        );
        draw_7segment_digit(painter, d_rect, ch, COLOR_LED_GREEN, COLOR_LED_DIM_GREEN);
    }
}
