//! The page behind the game: a gradient in the style's colours and, from the
//! Cardboard tier on, an animated pattern for each tier (bubbles, snow, wood
//! grain, embers, glints, a synthwave grid, aurora curtains, rays of light).

use std::f32::consts::PI;

use femtovg::{Paint, Path};

use super::Canvas;
use super::draw::{self, circle, fill_rect, linear, radial, rect};
use crate::color::{Rgb, black, white};

/// Colours the background is drawn in (they fade between styles).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BgColors {
    pub bg: [Rgb; 3],
    pub glow: Rgb,
    pub glow2: Rgb,
    pub accent: Rgb,
}

impl BgColors {
    pub fn lerp(&self, to: &BgColors, t: f32) -> BgColors {
        BgColors {
            bg: [self.bg[0].lerp(to.bg[0], t), self.bg[1].lerp(to.bg[1], t), self.bg[2].lerp(to.bg[2], t)],
            glow: self.glow.lerp(to.glow, t),
            glow2: self.glow2.lerp(to.glow2, t),
            accent: self.accent.lerp(to.accent, t),
        }
    }
}

/// Whether the tier's background moves (and so needs frames all the time).
pub fn is_animated(tier: usize) -> bool {
    matches!(tier, 2 | 3 | 5 | 6 | 7 | 8 | 9)
}

/// CSS `ease-in-out` on 0..1, approximated.
fn ease_in_out(t: f32) -> f32 {
    0.5 - 0.5 * (t * PI).cos()
}

/// An `alternate` animation: 0 → 1 → 0 over `2 * period` seconds, eased.
fn alternate(secs: f32, period: f32, delay: f32) -> f32 {
    let t = ((secs - delay) / period).rem_euclid(2.0);
    ease_in_out(if t > 1.0 { 2.0 - t } else { t })
}

/// A repeating tile pattern of soft dots: `dots` are (x, y, radius, colour, alpha) inside a `tw`×`th` tile.
#[allow(clippy::too_many_arguments)]
fn dot_pattern(c: &mut Canvas, w: f32, h: f32, tw: f32, th: f32, ox: f32, oy: f32, dots: &[(f32, f32, f32, Rgb, f32)], opacity: f32) {
    if opacity <= 0.0 {
        return;
    }
    for &(dx, dy, r, color, a) in dots {
        let mut path = Path::new();
        let mut ty = (oy % th) - th;
        while ty < h + th {
            let mut tx = (ox % tw) - tw;
            while tx < w + tw {
                let (x, y) = (tx + dx, ty + dy);
                if x > -r && x < w + r && y > -r && y < h + r {
                    path.circle(x, y, r);
                }
                tx += tw;
            }
            ty += th;
        }
        draw::fill_color(c, &path, color.alpha(a * opacity));
    }
}

/// Draws the background of a `w`×`h` screen at `secs` seconds.
pub fn draw(c: &mut Canvas, w: f32, h: f32, tier: usize, colors: &BgColors, secs: f32, still: bool) {
    let [top, mid, bottom] = colors.bg;
    c.fill_path(&rect(0.0, 0.0, w, h), &linear(0.0, 0.0, 0.0, h, &[(0.0, top.fv()), (0.5, mid.fv()), (1.0, bottom.fv())]));
    // Players who ask for less motion get the patterns standing still.
    let secs = if still { 0.0 } else { secs };
    let (glow, glow2, accent) = (colors.glow, colors.glow2, colors.accent);
    match tier {
        // Cardboard: the corrugated flutes of a big box.
        1 => {
            let mut dark = Path::new();
            let mut light = Path::new();
            let mut x = 0.0;
            while x < w {
                dark.rect(x, 0.0, 5.0, h);
                light.rect(x + 9.0, 0.0, 2.0, h);
                x += 16.0;
            }
            draw::fill_color(c, &dark, black(0.06));
            draw::fill_color(c, &light, white(0.04));
        }
        // Candy: bubbles drifting up.
        2 => {
            let oy = -(secs / 26.0).fract() * 320.0;
            let t = 320.0;
            let mut fills = Path::new();
            let mut ring_a = Path::new();
            let mut ring_b = Path::new();
            let mut ty = (oy % t) - t;
            while ty < h + t {
                let mut tx = 0.0;
                while tx < w + t {
                    let at = |px: f32, py: f32| (tx + px * t, ty + py * t);
                    let (x, y) = at(0.12, 0.2);
                    fills.circle(x, y, 10.5);
                    let (x, y) = at(0.35, 0.8);
                    fills.circle(x, y, 6.5);
                    let (x, y) = at(0.7, 0.45);
                    ring_a.circle(x, y, 17.0);
                    let (x, y) = at(0.88, 0.88);
                    ring_b.circle(x, y, 23.0);
                    tx += t;
                }
                ty += t;
            }
            draw::fill_color(c, &fills, white(0.21));
            draw::stroke(c, &ring_a, white(0.3), 2.5);
            draw::stroke(c, &ring_b, white(0.2), 2.5);
            let mut top_bubbles = Path::new();
            let mut ty = (oy % t) - t;
            while ty < h + t {
                let mut tx = 0.0;
                while tx < w + t {
                    top_bubbles.circle(tx + 0.5 * t, ty + 0.08 * t, 14.5);
                    tx += t;
                }
                ty += t;
            }
            draw::fill_color(c, &top_bubbles, white(0.14));
        }
        // Gingerbread: snow falling past the window.
        3 => {
            let oy = (secs / 22.0).fract() * 320.0;
            let dots = [
                (0.15 * 260.0, 0.10 * 320.0, 3.0, Rgb::WHITE, 0.55),
                (0.62 * 260.0, 0.35 * 320.0, 2.3, Rgb::WHITE, 0.4),
                (0.38 * 260.0, 0.62 * 320.0, 3.5, Rgb::WHITE, 0.5),
                (0.85 * 260.0, 0.80 * 320.0, 2.5, Rgb::WHITE, 0.35),
                (0.08 * 260.0, 0.88 * 320.0, 2.0, Rgb::WHITE, 0.3),
            ];
            dot_pattern(c, w, h, 260.0, 320.0, 0.0, oy, &dots, 1.0);
        }
        // Wooden: long streaks of grain across wall panelling.
        4 => {
            c.save();
            c.translate(w / 2.0, h / 2.0);
            c.rotate(-2.0f32.to_radians());
            let reach = w.hypot(h);
            let mut dark = Path::new();
            let mut light = Path::new();
            let mut y = -reach / 2.0;
            while y < reach / 2.0 {
                dark.rect(-reach / 2.0, y, reach, 2.0);
                light.rect(-reach / 2.0, y + 13.0, reach, 2.0);
                y += 31.0;
            }
            draw::fill_color(c, &dark, Rgb::new(60, 25, 0).alpha(0.1));
            draw::fill_color(c, &light, Rgb::new(255, 230, 190).alpha(0.05));
            c.restore();
            let mut panels = Path::new();
            for i in 1..4 {
                panels.rect(w * 0.25 * i as f32 - 2.0, 0.0, 2.0, h);
            }
            panels.rect(w - 2.0, 0.0, 2.0, h);
            draw::fill_color(c, &panels, black(0.18));
        }
        // Gems: twinkling glints.
        5 => {
            let a = 0.15 + 0.75 * alternate(secs, 3.2, 0.0);
            let b = 0.15 + 0.75 * alternate(secs, 3.2, -1.6);
            dot_pattern(c, w, h, 140.0, 170.0, 70.0, 85.0, &[(0.0, 0.0, 2.0, glow2, 1.0)], a);
            dot_pattern(c, w, h, 90.0, 110.0, 40.0 + 45.0, 60.0 + 55.0, &[(0.0, 0.0, 1.5, glow, 1.0)], a);
            dot_pattern(c, w, h, 200.0, 150.0, 70.0 + 100.0, 30.0 + 75.0, &[(0.0, 0.0, 2.0, glow2, 1.0)], b);
            dot_pattern(c, w, h, 120.0, 130.0, 10.0 + 60.0, 90.0 + 65.0, &[(0.0, 0.0, 1.5, glow, 1.0)], b);
        }
        // Iron: embers rising from the forge, and the glow of the fire below.
        6 => {
            let oy = -(secs / 12.0).fract() * 320.0;
            let dots = [
                (0.2 * 240.0, 0.3 * 320.0, 2.0, glow, 1.0),
                (0.7 * 240.0, 0.6 * 320.0, 1.5, glow2, 1.0),
                (0.45 * 240.0, 0.9 * 320.0, 1.7, glow, 1.0),
                (0.9 * 240.0, 0.15 * 320.0, 1.5, accent, 1.0),
            ];
            dot_pattern(c, w, h, 240.0, 320.0, 0.0, oy, &dots, 0.7);
            let gh = h * 0.4;
            let paint = Paint::elliptical_gradient_stops(w / 2.0, h, 0.0, 0.0, w * 0.7 / 2.0 * 1.4, gh * 0.6 * 1.4, [(0.0, glow.alpha(0.18)), (0.7, glow.alpha(0.0)), (1.0, glow.alpha(0.0))]);
            c.fill_path(&rect(0.0, h - gh, w, gh), &paint);
        }
        // Neon: a synthwave grid rolling towards you under a glowing horizon.
        7 => {
            let hh = h * 0.45;
            let base = h * 0.72;
            let paint = Paint::elliptical_gradient_stops(w / 2.0, base, 0.0, 0.0, w * 0.6 * 1.4 / 2.0 * 2.0, hh * 0.55 * 1.4, [(0.0, glow2.alpha(0.3)), (0.7, glow2.alpha(0.0)), (1.0, glow2.alpha(0.0))]);
            c.fill_path(&rect(0.0, base - hh, w, hh), &paint);
            neon_grid(c, w, h, glow, secs);
        }
        // Aurora: slow drifting curtains of light over faint stars.
        8 => {
            dot_pattern(c, w, h, 170.0, 190.0, 20.0 + 85.0, 40.0 + 95.0, &[(0.0, 0.0, 1.3, Rgb::WHITE, 1.0)], 0.35);
            dot_pattern(c, w, h, 110.0, 90.0, 60.0 + 55.0, 10.0 + 45.0, &[(0.0, 0.0, 1.1, Rgb::WHITE, 1.0)], 0.35);
            // Keyframes 0% → 50% → 100%, alternating over 14 s each way.
            let k = alternate(secs, 14.0, 0.0);
            let (tx, ty, rot, sc) = if k < 0.5 {
                let t = k * 2.0;
                (-0.06 + 0.11 * t, -0.03 + 0.05 * t, -4.0 + 7.0 * t, 1.0 + 0.1 * t)
            } else {
                let t = (k - 0.5) * 2.0;
                (0.05 - 0.08 * t, 0.02 + 0.02 * t, 3.0 - 5.0 * t, 1.1 - 0.05 * t)
            };
            let (bw, bh) = (w * 1.4, h * 1.4);
            c.save();
            c.translate(w / 2.0 + tx * bw, h / 2.0 + ty * bh);
            c.rotate(rot.to_radians());
            c.scale(sc, sc);
            c.translate(-bw / 2.0, -bh / 2.0);
            for (cx, cy, rx, ry, color) in [(0.3, 0.25, 0.4, 0.25, glow), (0.72, 0.32, 0.35, 0.22, glow2), (0.5, 0.72, 0.5, 0.25, glow)] {
                // Blurred in CSS; a wider, softer gradient looks the same.
                let paint = Paint::elliptical_gradient_stops(cx * bw, cy * bh, 0.0, 0.0, rx * bw * 1.15, ry * bh * 1.15, [(0.0, color.alpha(0.42)), (0.45, color.alpha(0.2)), (0.8, color.alpha(0.0)), (1.0, color.alpha(0.0))]);
                c.fill_path(&rect(0.0, 0.0, bw, bh), &paint);
            }
            c.restore();
        }
        // Legendary: turning rays of light and a twinkling golden starfield.
        9 => {
            let (cx, cy) = (w / 2.0, h * 0.38);
            let reach = w.max(h) * 0.76;
            c.save();
            c.translate(cx, cy);
            c.rotate((secs / 80.0).fract() * PI * 2.0);
            let mut rays = Path::new();
            for i in 0..24 {
                let a0 = (i as f32 * 15.0 - 90.0).to_radians();
                let a1 = a0 + 5.0f32.to_radians();
                rays.move_to(0.0, 0.0);
                rays.line_to(a0.cos() * reach * 1.1, a0.sin() * reach * 1.1);
                rays.line_to(a1.cos() * reach * 1.1, a1.sin() * reach * 1.1);
                rays.close();
            }
            c.fill_path(&rays, &radial(0.0, 0.0, 0.0, reach, &[(0.0, glow.alpha(0.16)), (1.0, glow.alpha(0.0))]));
            c.restore();
            let a = 0.15 + 0.75 * alternate(secs, 2.6, 0.0);
            dot_pattern(c, w, h, 150.0, 130.0, 10.0 + 75.0, 20.0 + 65.0, &[(0.0, 0.0, 2.0, glow2, 1.0)], a);
            dot_pattern(c, w, h, 95.0, 115.0, 55.0 + 47.0, 75.0 + 57.0, &[(0.0, 0.0, 1.5, Rgb::WHITE, 1.0)], a);
            dot_pattern(c, w, h, 210.0, 180.0, 120.0 + 105.0, 40.0 + 90.0, &[(0.0, 0.0, 1.7, accent, 1.0)], a);
        }
        _ => {}
    }
}

/// The neon floor: a grid on a plane tilted away (CSS `perspective(260px) rotateX(62deg)`), fading towards the horizon.
fn neon_grid(c: &mut Canvas, w: f32, h: f32, glow: Rgb, secs: f32) {
    let height = h * 0.6;
    let origin_y = h + 60.0;
    let (sin, cos) = 62.0f32.to_radians().sin_cos();
    let p = 260.0;
    // A point `d` up the plane from its bottom edge and `x` across from its centre, on screen.
    let project = |x: f32, d: f32| {
        let s = p / (p + d * sin);
        (w / 2.0 + x * s, origin_y - d * cos * s)
    };
    // The CSS mask: opaque up to 25% of the plane, transparent from 90%.
    let mask = |d: f32| 1.0 - ((d / height - 0.25) / 0.65).clamp(0.0, 1.0);
    let alpha = 0.4;
    let shift = (secs / 1.8).fract() * 60.0;
    // Lines across, moving towards the viewer.
    let mut d = 60.0 - shift;
    while d < height {
        if d >= 0.0 {
            let (x0, y) = project(-w, d);
            let (x1, _) = project(w, d);
            let thick = 2.0 * p / (p + d * sin) * cos;
            fill_rect(c, x0, y - thick, x1 - x0, thick.max(0.6), glow.alpha(alpha * mask(d)));
        }
        d += 60.0;
    }
    // Lines running away to the horizon.
    let (_, y_far) = project(0.0, height * 0.9);
    let (_, y_near) = project(0.0, 0.0);
    let fade = linear(0.0, y_near, 0.0, y_far, &[(0.0, glow.alpha(alpha)), (0.25 / 0.9, glow.alpha(alpha)), (1.0, glow.alpha(0.0))]);
    let mut lines = Path::new();
    let mut x = -w + ((w * 2.0) % 60.0) / 2.0;
    while x <= w {
        let (x0, y0) = project(x, 0.0);
        let (x1, y1) = project(x, height * 0.9);
        lines.move_to(x0, y0);
        lines.line_to(x1, y1);
        x += 60.0;
    }
    let mut paint = fade;
    paint.set_line_width(2.0);
    c.stroke_path(&lines, &paint);
    let _ = circle;
}
