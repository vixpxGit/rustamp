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

/// Renders the RustAmp (R + Lightning Bolt) logo into an RGBA buffer of `size x size`.
pub fn render_winampr_rgba(size: u32) -> Vec<u8> {
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let s = size as f32;

    let radius = 0.16;
    let margin = 0.04;

    // Electric lightning bolt polygon (right half, normalized [0..1])
    let bolt: [(f32, f32); 7] = [
        (0.71, 0.11),
        (0.51, 0.51),
        (0.65, 0.51),
        (0.55, 0.89),
        (0.84, 0.45),
        (0.68, 0.45),
        (0.80, 0.11),
    ];

    // Bold 'R' on the left side
    let r_segs: [((f32, f32), (f32, f32)); 5] = [
        ((0.20, 0.22), (0.20, 0.78)), // vertical stem
        ((0.20, 0.22), (0.40, 0.22)), // top bar
        ((0.40, 0.22), (0.45, 0.35)), // upper right curve
        ((0.45, 0.35), (0.39, 0.49)), // lower loop return
        ((0.20, 0.49), (0.39, 0.49)), // mid bar
    ];
    let r_leg: ((f32, f32), (f32, f32)) = ((0.30, 0.49), (0.46, 0.78));

    let stroke_r = 0.052;
    let aa = 1.2 / s;

    for y in 0..size {
        for x in 0..size {
            let nx = (x as f32 + 0.5) / s;
            let ny = (y as f32 + 0.5) / s;
            let idx = ((y * size + x) * 4) as usize;
            let px = &mut rgba[idx..idx + 4];

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

            // Subtle phosphor green glow in center
            let center_dist = ((nx - 0.5).powi(2) + (ny - 0.5).powi(2)).sqrt();
            if center_dist < 0.36 {
                let glow = (1.0 - center_dist / 0.36) * 0.28;
                blend_pixel(px, 0, 180, 40, glow);
            }

            // Draw 'R' (Winamp phosphor green with dark shadow)
            let mut min_r = dist_to_segment(nx, ny, r_leg.0 .0, r_leg.0 .1, r_leg.1 .0, r_leg.1 .1);
            let mut min_r_shadow = dist_to_segment(
                nx - 0.016,
                ny - 0.016,
                r_leg.0 .0,
                r_leg.0 .1,
                r_leg.1 .0,
                r_leg.1 .1,
            );
            for &(a, b) in &r_segs {
                min_r = min_r.min(dist_to_segment(nx, ny, a.0, a.1, b.0, b.1));
                min_r_shadow = min_r_shadow.min(dist_to_segment(
                    nx - 0.016,
                    ny - 0.016,
                    a.0,
                    a.1,
                    b.0,
                    b.1,
                ));
            }
            if min_r_shadow < stroke_r + aa {
                let a = ((stroke_r + aa - min_r_shadow) / (2.0 * aa)).clamp(0.0, 1.0);
                blend_pixel(px, 8, 10, 14, a * 0.8);
            }
            if min_r < stroke_r + aa {
                let a = ((stroke_r + aa - min_r) / (2.0 * aa)).clamp(0.0, 1.0);
                let g = (250.0 - ny * 35.0) as u8;
                blend_pixel(px, 25, g, 65, a);
            }

            // Draw electric lightning bolt (gold -> fiery orange)
            let inside_bolt = point_in_poly(nx, ny, &bolt);
            let edge_d = dist_to_poly_edges(nx, ny, &bolt);

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
