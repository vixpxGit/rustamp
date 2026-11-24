/// Helper to blend a foreground RGBA color over a background RGBA pixel.
#[inline]
fn blend_pixel(dst: &mut [u8], r: u8, g: u8, b: u8, a: f32) {
    let a = a.clamp(0.0, 1.0);
    let inv = 1.0 - a;
    dst[0] = (r as f32 * a + dst[0] as f32 * inv).round() as u8;
    dst[1] = (g as f32 * a + dst[1] as f32 * inv).round() as u8;
    dst[2] = (b as f32 * a + dst[2] as f32 * inv).round() as u8;
    dst[3] = ((a + (dst[3] as f32 / 255.0) * inv).min(1.0) * 255.0).round() as u8;
}

/// Signed distance from point (px, py) to segment (ax, ay)-(bx, by).
fn dist_to_segment(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let dx = bx - ax;
    let dy = by - ay;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-6 {
        return ((px - ax).powi(2) + (py - ay).powi(2)).sqrt();
    }
    let t = (((px - ax) * dx + (py - ay) * dy) / len_sq).clamp(0.0, 1.0);
    let proj_x = ax + t * dx;
    let proj_y = ay + t * dy;
    ((px - proj_x).powi(2) + (py - proj_y).powi(2)).sqrt()
}

/// Point-in-polygon test using ray casting.
fn point_in_poly(px: f32, py: f32, poly: &[(f32, f32)]) -> bool {
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[j];
        let intersect = ((yi > py) != (yj > py))
            && (px < (xj - xi) * (py - yi) / (yj - yi + 1e-9) + xi);
        if intersect {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Distance to polygon boundary for anti-aliased edge smoothing.
fn dist_to_poly_edges(px: f32, py: f32, poly: &[(f32, f32)]) -> f32 {
    let mut min_d = f32::MAX;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        let d = dist_to_segment(px, py, a.0, a.1, b.0, b.1);
        if d < min_d {
            min_d = d;
        }
    }
    min_d
}

/// Renders the WinampR (W / Lightning / R) logo into an RGBA buffer of `size x size`.
pub fn render_winampr_rgba(size: u32) -> Vec<u8> {
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let s = size as f32;

    // Normalized coordinate space [0.0, 1.0]
    let radius = 0.16;
    let margin = 0.04;

    // Electric lightning bolt polygon (normalized [0..1])
    let bolt: [(f32, f32); 7] = [
        (0.56, 0.08),
        (0.36, 0.48),
        (0.50, 0.48),
        (0.40, 0.92),
        (0.67, 0.43),
        (0.52, 0.43),
        (0.64, 0.08),
    ];

    // Letter 'W' segments (left side, normalized)
    let w_segs: [((f32, f32), (f32, f32)); 4] = [
        ((0.12, 0.24), (0.18, 0.72)),
        ((0.18, 0.72), (0.25, 0.44)),
        ((0.25, 0.44), (0.32, 0.72)),
        ((0.32, 0.72), (0.38, 0.24)),
    ];

    // Letter 'R' segments + leg (right side, normalized)
    let r_segs: [((f32, f32), (f32, f32)); 5] = [
        ((0.66, 0.24), (0.66, 0.74)), // vertical stem
        ((0.66, 0.24), (0.82, 0.24)), // top bar
        ((0.82, 0.24), (0.86, 0.36)), // upper right curve
        ((0.86, 0.36), (0.80, 0.48)), // lower loop return
        ((0.66, 0.48), (0.80, 0.48)), // mid bar
    ];
    let r_leg: ((f32, f32), (f32, f32)) = ((0.74, 0.48), (0.87, 0.74));

    let stroke_r = 0.042;
    let aa = 1.2 / s;

    for y in 0..size {
        for x in 0..size {
            let nx = (x as f32 + 0.5) / s;
            let ny = (y as f32 + 0.5) / s;
            let idx = ((y * size + x) * 4) as usize;
            let px = &mut rgba[idx..idx + 4];

            // Rounded-rect SDF for badge background
            let rx = (nx - 0.5).abs() - (0.5 - margin - radius);
            let ry = (ny - 0.5).abs() - (0.5 - margin - radius);
            let outside = (rx.max(0.0).powi(2) + ry.max(0.0).powi(2)).sqrt()
                + rx.max(ry).min(0.0)
                - radius;

            if outside > aa {
                continue;
            }
            let bg_alpha = ((aa - outside) / (2.0 * aa)).clamp(0.0, 1.0);

            // Dark metallic brushed gradient
            let bg_r = (20.0 + ny * 18.0) as u8;
            let bg_g = (24.0 + ny * 22.0) as u8;
            let bg_b = (32.0 + ny * 28.0) as u8;
            blend_pixel(px, bg_r, bg_g, bg_b, bg_alpha);

            // Gold/amber beveled outer border
            if outside > -0.055 {
                let is_top_left = nx + ny < 1.05;
                let (br, bg, bb) = if is_top_left {
                    (235, 190, 50)
                } else {
                    (130, 90, 20)
                };
                blend_pixel(px, br, bg, bb, bg_alpha * 0.9);
            }

            // Subtle dark inner LCD glow in center
            let center_dist = ((nx - 0.5).powi(2) + (ny - 0.5).powi(2)).sqrt();
            if center_dist < 0.36 {
                let glow = (1.0 - center_dist / 0.36) * 0.28;
                blend_pixel(px, 0, 180, 40, glow);
            }

            // Draw 'W' (silver-white with dark shadow)
            let mut min_w = f32::MAX;
            let mut min_w_shadow = f32::MAX;
            for &(a, b) in &w_segs {
                min_w = min_w.min(dist_to_segment(nx, ny, a.0, a.1, b.0, b.1));
                min_w_shadow = min_w_shadow.min(dist_to_segment(
                    nx - 0.015,
                    ny - 0.015,
                    a.0,
                    a.1,
                    b.0,
                    b.1,
                ));
            }
            if min_w_shadow < stroke_r + aa {
                let a = ((stroke_r + aa - min_w_shadow) / (2.0 * aa)).clamp(0.0, 1.0);
                blend_pixel(px, 8, 10, 14, a * 0.75);
            }
            if min_w < stroke_r + aa {
                let a = ((stroke_r + aa - min_w) / (2.0 * aa)).clamp(0.0, 1.0);
                let shade = (245.0 - ny * 40.0) as u8;
                blend_pixel(px, shade, shade, 255, a);
            }

            // Draw 'R' (Winamp phosphor green with dark shadow)
            let mut min_r = dist_to_segment(nx, ny, r_leg.0 .0, r_leg.0 .1, r_leg.1 .0, r_leg.1 .1);
            let mut min_r_shadow = dist_to_segment(
                nx - 0.015,
                ny - 0.015,
                r_leg.0 .0,
                r_leg.0 .1,
                r_leg.1 .0,
                r_leg.1 .1,
            );
            for &(a, b) in &r_segs {
                min_r = min_r.min(dist_to_segment(nx, ny, a.0, a.1, b.0, b.1));
                min_r_shadow = min_r_shadow.min(dist_to_segment(
                    nx - 0.015,
                    ny - 0.015,
                    a.0,
                    a.1,
                    b.0,
                    b.1,
                ));
            }
            if min_r_shadow < stroke_r + aa {
                let a = ((stroke_r + aa - min_r_shadow) / (2.0 * aa)).clamp(0.0, 1.0);
                blend_pixel(px, 8, 10, 14, a * 0.75);
            }
            if min_r < stroke_r + aa {
                let a = ((stroke_r + aa - min_r) / (2.0 * aa)).clamp(0.0, 1.0);
                let g = (250.0 - ny * 35.0) as u8;
                blend_pixel(px, 25, g, 65, a);
            }

            // Draw electric lightning bolt on top (gold -> fiery orange)
            let inside_bolt = point_in_poly(nx, ny, &bolt);
            let edge_d = dist_to_poly_edges(nx, ny, &bolt);

            // Dark outline around bolt for contrast
            if !inside_bolt && edge_d < 0.022 + aa {
                let a = ((0.022 + aa - edge_d) / (2.0 * aa)).clamp(0.0, 1.0);
                blend_pixel(px, 15, 10, 5, a * 0.9);
            }
            if inside_bolt || edge_d < aa {
                let a = if inside_bolt {
                    1.0
                } else {
                    ((aa - edge_d) / (2.0 * aa)).clamp(0.0, 1.0)
                };
                let r = 255;
                let g = (245.0 - ny * 110.0).clamp(110.0, 255.0) as u8;
                let b = (70.0 - ny * 55.0).clamp(10.0, 90.0) as u8;
                blend_pixel(px, r, g, b, a);
            }
        }
    }

    rgba
}
