//! Small helpers that make femtovg read like the Canvas 2D code it replaces.

use femtovg::{Color, CompositeOperation, Paint, Path};

use super::Canvas;

pub fn rect(x: f32, y: f32, w: f32, h: f32) -> Path {
    let mut p = Path::new();
    p.rect(x, y, w, h);
    p
}

pub fn round_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Path {
    let mut p = Path::new();
    p.rounded_rect(x, y, w, h, r.max(0.0).min(w.abs() / 2.0).min(h.abs() / 2.0));
    p
}

pub fn circle(x: f32, y: f32, r: f32) -> Path {
    let mut p = Path::new();
    p.circle(x, y, r.max(0.0));
    p
}

pub fn fill_color(c: &mut Canvas, path: &Path, color: Color) {
    c.fill_path(path, &Paint::color(color));
}

pub fn fill_rect(c: &mut Canvas, x: f32, y: f32, w: f32, h: f32, color: Color) {
    if w > 0.0 && h > 0.0 {
        c.fill_path(&rect(x, y, w, h), &Paint::color(color));
    }
}

pub fn stroke(c: &mut Canvas, path: &Path, color: Color, width: f32) {
    let mut paint = Paint::color(color);
    paint.set_line_width(width);
    c.stroke_path(path, &paint);
}

/// A linear gradient; two stops stay on the GPU uniforms, more go through a texture.
pub fn linear(x0: f32, y0: f32, x1: f32, y1: f32, stops: &[(f32, Color)]) -> Paint {
    if stops.len() == 2 && stops[0].0 == 0.0 && stops[1].0 == 1.0 {
        Paint::linear_gradient(x0, y0, x1, y1, stops[0].1, stops[1].1)
    } else {
        Paint::linear_gradient_stops(x0, y0, x1, y1, stops.iter().copied())
    }
}

/// A radial gradient between two radii around one centre (Canvas `createRadialGradient(x, y, r0, x, y, r1)`).
pub fn radial(cx: f32, cy: f32, r0: f32, r1: f32, stops: &[(f32, Color)]) -> Paint {
    if stops.len() == 2 && stops[0].0 == 0.0 && stops[1].0 == 1.0 {
        Paint::radial_gradient(cx, cy, r0, r1.max(r0 + 0.01), stops[0].1, stops[1].1)
    } else {
        Paint::radial_gradient_stops(cx, cy, r0, r1.max(r0 + 0.01), stops.iter().copied())
    }
}

/// Runs `f` with additive ("lighter") blending.
pub fn lighter(c: &mut Canvas, on: bool, f: impl FnOnce(&mut Canvas)) {
    if on {
        c.global_composite_operation(CompositeOperation::Lighter);
    }
    f(c);
    if on {
        c.global_composite_operation(CompositeOperation::SourceOver);
    }
}

pub const fn ease_out_back(t: f32) -> f32 {
    let u = t - 1.0;
    1.0 + 2.2 * u * u * u + 1.2 * u * u
}

pub fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

pub fn clamp01(t: f32) -> f32 {
    t.clamp(0.0, 1.0)
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
