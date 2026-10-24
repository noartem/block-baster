//! A small Canvas-2D-style drawing context on top of tiny-skia, so the
//! procedural graphics can be ported from the TypeScript version line by line.
//!
//! Like a canvas, `Ctx` has a current path (built in user space and stored in
//! device space, so later transform changes don't move it), a state stack with
//! the transform, clip, global alpha, composite mode, line style and shadow,
//! and fill/stroke operations that use the state at the time they are called.

use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::rc::Rc;

use tiny_skia::{
    BlendMode, Color, FillRule, GradientStop, LineCap, LineJoin, LinearGradient, Mask, Paint, Path,
    PathBuilder, Pixmap, PixmapPaint, Point, RadialGradient, Rect, SpreadMode, Stroke, Transform,
};

use crate::color::Rgb;

pub type Shader = tiny_skia::Shader<'static>;

/// A colour with a (float) alpha, like CSS `rgba(r,g,b,a)`.
pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color {
    Color::from_rgba8(r, g, b, 255).with_alpha(a)
}

/// An `Rgb` with alpha, like the TS `rgba(hex, alpha)`.
pub fn alpha(c: Rgb, a: f32) -> Color {
    rgba(c.r, c.g, c.b, a)
}

pub fn white(a: f32) -> Color {
    rgba(255, 255, 255, a)
}

pub fn black(a: f32) -> Color {
    rgba(0, 0, 0, a)
}

trait WithAlpha {
    fn with_alpha(self, a: f32) -> Self;
}

impl WithAlpha for Color {
    fn with_alpha(mut self, a: f32) -> Self {
        self.set_alpha(a.clamp(0.0, 1.0));
        self
    }
}

/// Anything usable as a fill or stroke style.
pub trait IntoShader {
    fn into_shader(self) -> Shader;
}

impl IntoShader for Shader {
    fn into_shader(self) -> Shader {
        self
    }
}

impl IntoShader for Color {
    fn into_shader(self) -> Shader {
        Shader::SolidColor(self)
    }
}

impl IntoShader for Rgb {
    fn into_shader(self) -> Shader {
        Shader::SolidColor(alpha(self, 1.0))
    }
}

impl IntoShader for &str {
    /// "#rrggbb".
    fn into_shader(self) -> Shader {
        Rgb::hex(self).into_shader()
    }
}

fn stops(list: &[(f32, Color)]) -> Vec<GradientStop> {
    list.iter().map(|&(t, c)| GradientStop::new(t, c)).collect()
}

/// `createLinearGradient(x0, y0, x1, y1)` with colour stops. A degenerate
/// gradient paints nothing, like on a canvas.
pub fn linear(x0: f32, y0: f32, x1: f32, y1: f32, list: &[(f32, Color)]) -> Shader {
    if x0 == x1 && y0 == y1 {
        return Shader::SolidColor(Color::TRANSPARENT);
    }
    LinearGradient::new(
        Point::from_xy(x0, y0),
        Point::from_xy(x1, y1),
        stops(list),
        SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap_or(Shader::SolidColor(Color::TRANSPARENT))
}

/// `createRadialGradient(x0, y0, r0, x1, y1, r1)` (a two-point conical gradient).
pub fn radial(
    x0: f32,
    y0: f32,
    r0: f32,
    x1: f32,
    y1: f32,
    r1: f32,
    list: &[(f32, Color)],
) -> Shader {
    RadialGradient::new(
        Point::from_xy(x0, y0),
        r0.max(0.0),
        Point::from_xy(x1, y1),
        r1.max(0.0),
        stops(list),
        SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap_or(Shader::SolidColor(Color::TRANSPARENT))
}

/// Evenly spaced stops, like `stops.forEach((col, i) => g.addColorStop(i / (n - 1), col))`.
pub fn even_stops(colors: &[&str]) -> Vec<(f32, Color)> {
    let n = colors.len().max(2) - 1;
    colors
        .iter()
        .enumerate()
        .map(|(i, c)| (i as f32 / n as f32, alpha(Rgb::hex(c), 1.0)))
        .collect()
}

#[derive(Clone)]
struct State {
    transform: Transform,
    clip: Option<Rc<Mask>>,
    alpha: f32,
    blend: BlendMode,
    line_width: f32,
    line_cap: LineCap,
    line_join: LineJoin,
    shadow_color: Color,
    shadow_blur: f32,
}

impl Default for State {
    fn default() -> Self {
        State {
            transform: Transform::identity(),
            clip: None,
            alpha: 1.0,
            blend: BlendMode::SourceOver,
            line_width: 1.0,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            shadow_color: Color::TRANSPARENT,
            shadow_blur: 0.0,
        }
    }
}

/// A canvas: a pixmap plus the drawing state.
pub struct Ctx {
    pub pixmap: Pixmap,
    state: State,
    stack: Vec<State>,
    /// The current path, in device space.
    path: PathBuilder,
    /// Start of the current subpath and the current point, in device space.
    start: Option<Point>,
    current: Option<Point>,
}

impl Ctx {
    /// A transparent canvas of the given size (at least 1×1).
    pub fn new(width: u32, height: u32) -> Ctx {
        Ctx {
            pixmap: Pixmap::new(width.max(1), height.max(1)).expect("pixmap size"),
            state: State::default(),
            stack: Vec::new(),
            path: PathBuilder::new(),
            start: None,
            current: None,
        }
    }

    pub fn into_pixmap(self) -> Pixmap {
        self.pixmap
    }

    pub fn width(&self) -> u32 {
        self.pixmap.width()
    }

    pub fn height(&self) -> u32 {
        self.pixmap.height()
    }

    // --- State ---------------------------------------------------------------

    pub fn save(&mut self) {
        self.stack.push(self.state.clone());
    }

    pub fn restore(&mut self) {
        if let Some(s) = self.stack.pop() {
            self.state = s;
        }
    }

    pub fn translate(&mut self, x: f32, y: f32) {
        self.state.transform = self.state.transform.pre_translate(x, y);
    }

    /// Rotation in radians, clockwise on screen (like the canvas).
    pub fn rotate(&mut self, angle: f32) {
        self.state.transform = self
            .state
            .transform
            .pre_concat(Transform::from_rotate(angle.to_degrees()));
    }

    pub fn scale(&mut self, x: f32, y: f32) {
        self.state.transform = self.state.transform.pre_scale(x, y);
    }

    pub fn transform(&self) -> Transform {
        self.state.transform
    }

    pub fn set_transform(&mut self, ts: Transform) {
        self.state.transform = ts;
    }

    pub fn set_global_alpha(&mut self, a: f32) {
        self.state.alpha = a.clamp(0.0, 1.0);
    }

    /// `globalCompositeOperation`: `SourceOver`, `Multiply`, `Plus` ("lighter")…
    pub fn set_blend(&mut self, mode: BlendMode) {
        self.state.blend = mode;
    }

    pub fn set_line_width(&mut self, w: f32) {
        if w > 0.0 && w.is_finite() {
            self.state.line_width = w;
        }
    }

    pub fn line_width(&self) -> f32 {
        self.state.line_width
    }

    pub fn set_line_cap(&mut self, cap: LineCap) {
        self.state.line_cap = cap;
    }

    pub fn set_line_join(&mut self, join: LineJoin) {
        self.state.line_join = join;
    }

    /// `shadowColor` + `shadowBlur` (no offset). The blur is in device pixels,
    /// unaffected by the transform, like on a canvas. A blur of 0 turns it off.
    pub fn set_shadow(&mut self, color: Color, blur: f32) {
        self.state.shadow_color = color;
        self.state.shadow_blur = blur.max(0.0);
    }

    pub fn set_shadow_blur(&mut self, blur: f32) {
        self.state.shadow_blur = blur.max(0.0);
    }

    // --- Path ----------------------------------------------------------------

    fn map(&self, x: f32, y: f32) -> Point {
        let mut p = Point::from_xy(x, y);
        self.state.transform.map_point(&mut p);
        p
    }

    /// The current point in user space.
    fn current_user(&self) -> Option<Point> {
        let mut p = self.current?;
        self.state.transform.invert()?.map_point(&mut p);
        Some(p)
    }

    pub fn begin_path(&mut self) {
        self.path.clear();
        self.start = None;
        self.current = None;
    }

    pub fn move_to(&mut self, x: f32, y: f32) {
        let p = self.map(x, y);
        self.path.move_to(p.x, p.y);
        self.start = Some(p);
        self.current = Some(p);
    }

    /// Without a current point this acts as `move_to`, like on a canvas.
    pub fn line_to(&mut self, x: f32, y: f32) {
        if self.current.is_none() {
            return self.move_to(x, y);
        }
        let p = self.map(x, y);
        self.path.line_to(p.x, p.y);
        self.current = Some(p);
    }

    pub fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        if self.current.is_none() {
            self.move_to(cx, cy);
        }
        let c = self.map(cx, cy);
        let p = self.map(x, y);
        self.path.quad_to(c.x, c.y, p.x, p.y);
        self.current = Some(p);
    }

    pub fn bezier_to(&mut self, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) {
        if self.current.is_none() {
            self.move_to(c1x, c1y);
        }
        let c1 = self.map(c1x, c1y);
        let c2 = self.map(c2x, c2y);
        let p = self.map(x, y);
        self.path.cubic_to(c1.x, c1.y, c2.x, c2.y, p.x, p.y);
        self.current = Some(p);
    }

    pub fn close_path(&mut self) {
        if self.current.is_some() {
            self.path.close();
            self.current = self.start;
        }
    }

    /// `rect(x, y, w, h)`: a closed subpath; the next one starts at (x, y).
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.move_to(x, y);
        self.line_to(x + w, y);
        self.line_to(x + w, y + h);
        self.line_to(x, y + h);
        self.close_path();
    }

    /// `arcTo(x1, y1, x2, y2, r)` as specified for the canvas.
    pub fn arc_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, r: f32) {
        let Some(p0) = self.current_user() else {
            return self.move_to(x1, y1);
        };
        let (p1, p2) = (Point::from_xy(x1, y1), Point::from_xy(x2, y2));
        let v1 = Point::from_xy(p0.x - p1.x, p0.y - p1.y);
        let v2 = Point::from_xy(p2.x - p1.x, p2.y - p1.y);
        let cross = v1.x * v2.y - v1.y * v2.x;
        let (l1, l2) = (v1.length(), v2.length());
        if r <= 0.0 || l1 == 0.0 || l2 == 0.0 || cross.abs() <= 1e-6 * l1 * l2 {
            return self.line_to(x1, y1);
        }
        let u1 = Point::from_xy(v1.x / l1, v1.y / l1);
        let u2 = Point::from_xy(v2.x / l2, v2.y / l2);
        // Angle at p1 between the two lines.
        let theta = (u1.x * u2.x + u1.y * u2.y).clamp(-1.0, 1.0).acos();
        let d = r / (theta / 2.0).tan();
        let t1 = Point::from_xy(p1.x + u1.x * d, p1.y + u1.y * d);
        let mut bis = Point::from_xy(u1.x + u2.x, u1.y + u2.y);
        let bl = bis.length();
        bis = Point::from_xy(bis.x / bl, bis.y / bl);
        let h = r / (theta / 2.0).sin();
        let c = Point::from_xy(p1.x + bis.x * h, p1.y + bis.y * h);
        self.line_to(t1.x, t1.y);
        let a0 = (t1.y - c.y).atan2(t1.x - c.x);
        let sweep = (PI - theta) * if cross < 0.0 { 1.0 } else { -1.0 };
        self.arc_segments(c.x, c.y, r, r, 0.0, a0, sweep);
    }

    /// `arc(x, y, r, start, end, anticlockwise)`.
    pub fn arc(&mut self, x: f32, y: f32, r: f32, start: f32, end: f32, ccw: bool) {
        self.ellipse(x, y, r, r, 0.0, start, end, ccw);
    }

    /// A full circle as a new subpath of the current path.
    pub fn circle(&mut self, x: f32, y: f32, r: f32) {
        self.ellipse(x, y, r, r, 0.0, 0.0, TAU, false);
    }

    /// `ellipse(x, y, rx, ry, rotation, start, end, anticlockwise)`.
    #[allow(clippy::too_many_arguments)]
    pub fn ellipse(
        &mut self,
        x: f32,
        y: f32,
        rx: f32,
        ry: f32,
        rotation: f32,
        start: f32,
        end: f32,
        ccw: bool,
    ) {
        if rx < 0.0 || ry < 0.0 {
            return;
        }
        let sweep = if !ccw && end - start >= TAU {
            TAU
        } else if ccw && start - end >= TAU {
            -TAU
        } else if !ccw {
            (end - start).rem_euclid(TAU)
        } else {
            -(start - end).rem_euclid(TAU)
        };
        let (sr, cr) = rotation.sin_cos();
        let (s0, c0) = start.sin_cos();
        let px = x + rx * c0 * cr - ry * s0 * sr;
        let py = y + rx * c0 * sr + ry * s0 * cr;
        if self.current.is_some() {
            self.line_to(px, py);
        } else {
            self.move_to(px, py);
        }
        self.arc_segments(x, y, rx, ry, rotation, start, sweep);
    }

    /// Appends an elliptical arc from angle `a0` sweeping `sweep` radians, as
    /// cubic segments of at most 90°. The current point must be at its start.
    #[allow(clippy::too_many_arguments)]
    fn arc_segments(
        &mut self,
        x: f32,
        y: f32,
        rx: f32,
        ry: f32,
        rotation: f32,
        a0: f32,
        sweep: f32,
    ) {
        if sweep == 0.0 {
            return;
        }
        let n = ((sweep.abs() / FRAC_PI_2) - 1e-4).ceil().max(1.0) as usize;
        let step = sweep / n as f32;
        let k = 4.0 / 3.0 * (step / 4.0).tan();
        let (sr, cr) = rotation.sin_cos();
        let pt = |ux: f32, uy: f32| {
            (
                x + rx * ux * cr - ry * uy * sr,
                y + rx * ux * sr + ry * uy * cr,
            )
        };
        let mut a = a0;
        for _ in 0..n {
            let b = a + step;
            let (sa, ca) = a.sin_cos();
            let (sb, cb) = b.sin_cos();
            let c1 = pt(ca - k * sa, sa + k * ca);
            let c2 = pt(cb + k * sb, sb - k * cb);
            let p = pt(cb, sb);
            self.bezier_to(c1.0, c1.1, c2.0, c2.1, p.0, p.1);
            a = b;
        }
    }

    /// The `roundRect()` helper of the TS renderer: a new path with a rounded
    /// rectangle built from `arcTo` calls.
    pub fn round_rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32) {
        self.begin_path();
        self.add_round_rect(x, y, w, h, r);
    }

    /// Adds a rounded rectangle to the current path without starting a new one.
    pub fn add_round_rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32) {
        self.move_to(x + r, y);
        self.arc_to(x + w, y, x + w, y + h, r);
        self.arc_to(x + w, y + h, x, y + h, r);
        self.arc_to(x, y + h, x, y, r);
        self.arc_to(x, y, x + w, y, r);
        self.close_path();
    }

    /// A new closed polygon path from flat [x0, y0, x1, y1, …] points.
    pub fn polygon(&mut self, pts: &[f32]) {
        self.begin_path();
        self.move_to(pts[0], pts[1]);
        for p in pts[2..].chunks_exact(2) {
            self.line_to(p[0], p[1]);
        }
        self.close_path();
    }

    /// The TS `poly()` helper: a polygon path, filled.
    pub fn poly(&mut self, pts: &[f32], fill: impl IntoShader) {
        self.polygon(pts);
        self.fill(fill);
    }

    fn device_path(&self) -> Option<Path> {
        self.path.clone().finish()
    }

    // --- Drawing ---------------------------------------------------------------

    fn paint(&self, shader: Shader) -> Paint<'static> {
        Paint {
            shader,
            blend_mode: self.state.blend,
            anti_alias: true,
            ..Paint::default()
        }
    }

    fn stroke_style(&self) -> Stroke {
        Stroke {
            width: self.state.line_width,
            miter_limit: 10.0,
            line_cap: self.state.line_cap,
            line_join: self.state.line_join,
            dash: None,
        }
    }

    fn shadow_on(&self) -> bool {
        self.state.shadow_blur > 0.0 && self.state.shadow_color.alpha() > 0.0
    }

    /// Fills a device-space path with a user-space shader.
    fn fill_device(&mut self, path: &Path, shader: Shader, rule: FillRule) {
        let mut shader = shader;
        shader.transform(self.state.transform);
        if self.shadow_on() {
            self.draw_shadow(path, shader.clone(), rule);
        }
        shader.apply_opacity(self.state.alpha);
        let paint = self.paint(shader);
        let clip = self.state.clip.clone();
        self.pixmap
            .fill_path(path, &paint, rule, Transform::identity(), clip.as_deref());
    }

    /// Fills the current path (nonzero rule).
    pub fn fill(&mut self, style: impl IntoShader) {
        self.fill_rule(style, FillRule::Winding);
    }

    pub fn fill_rule(&mut self, style: impl IntoShader, rule: FillRule) {
        if let Some(path) = self.device_path() {
            self.fill_device(&path, style.into_shader(), rule);
        }
    }

    /// Strokes the current path with the current line style.
    pub fn stroke(&mut self, style: impl IntoShader) {
        let Some(path) = self.device_path() else {
            return;
        };
        let ts = self.state.transform;
        let Some(user) = ts.invert().and_then(|inv| path.transform(inv)) else {
            return;
        };
        let stroke = self.stroke_style();
        let mut shader = style.into_shader();
        if self.shadow_on() {
            let res = tiny_skia::PathStroker::compute_resolution_scale(&ts);
            if let Some(outline) = user.stroke(&stroke, res).and_then(|o| o.transform(ts)) {
                let mut s = shader.clone();
                s.transform(ts);
                self.draw_shadow(&outline, s, FillRule::Winding);
            }
        }
        shader.apply_opacity(self.state.alpha);
        let paint = self.paint(shader);
        let clip = self.state.clip.clone();
        self.pixmap
            .stroke_path(&user, &paint, &stroke, ts, clip.as_deref());
    }

    /// `fillRect`: doesn't touch the current path.
    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, style: impl IntoShader) {
        if w == 0.0 || h == 0.0 {
            return;
        }
        let (x, w) = if w < 0.0 { (x + w, -w) } else { (x, w) };
        let (y, h) = if h < 0.0 { (y + h, -h) } else { (y, h) };
        let Some(rect) = Rect::from_xywh(x, y, w, h) else {
            return;
        };
        let ts = self.state.transform;
        if !ts.has_skew() && !self.shadow_on() {
            // Axis-aligned: tiny-skia's analytic rect filler is much faster
            // than the path rasteriser for tall, thin rectangles.
            let (mut a, mut b) = (Point::from_xy(x, y), Point::from_xy(x + w, y + h));
            ts.map_point(&mut a);
            ts.map_point(&mut b);
            let Some(dev) = Rect::from_ltrb(a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y))
            else {
                return;
            };
            let mut shader = style.into_shader();
            shader.transform(ts);
            shader.apply_opacity(self.state.alpha);
            let paint = self.paint(shader);
            let clip = self.state.clip.clone();
            self.pixmap
                .fill_rect(dev, &paint, Transform::identity(), clip.as_deref());
            return;
        }
        let Some(path) = PathBuilder::from_rect(rect).transform(ts) else {
            return;
        };
        self.fill_device(&path, style.into_shader(), FillRule::Winding);
    }

    /// `strokeRect`: doesn't touch the current path.
    pub fn stroke_rect(&mut self, x: f32, y: f32, w: f32, h: f32, style: impl IntoShader) {
        let saved = (self.path.clone(), self.start, self.current);
        self.begin_path();
        self.rect(x, y, w, h);
        self.stroke(style);
        (self.path, self.start, self.current) = saved;
    }

    /// Intersects the clip with the current path (nonzero rule).
    pub fn clip(&mut self) {
        self.clip_rule(FillRule::Winding);
    }

    pub fn clip_rule(&mut self, rule: FillRule) {
        let (w, h) = (self.width(), self.height());
        let path = self.device_path();
        let mask = match self.state.clip.take() {
            Some(mut m) => {
                match &path {
                    Some(p) => {
                        Rc::make_mut(&mut m).intersect_path(p, rule, true, Transform::identity())
                    }
                    None => Rc::make_mut(&mut m).clear(),
                }
                m
            }
            None => {
                let mut m = Mask::new(w, h).expect("mask size");
                if let Some(p) = &path {
                    m.fill_path(p, rule, true, Transform::identity());
                }
                Rc::new(m)
            }
        };
        self.state.clip = Some(mask);
    }

    /// Draws a pixmap with its top-left at (x, y) in user space, at its own
    /// size scaled by `w / width` and `h / height`.
    pub fn draw_pixmap(&mut self, pixmap: &Pixmap, x: f32, y: f32, w: f32, h: f32) {
        let ts = self
            .state
            .transform
            .pre_translate(x, y)
            .pre_scale(w / pixmap.width() as f32, h / pixmap.height() as f32);
        let paint = PixmapPaint {
            opacity: self.state.alpha,
            blend_mode: self.state.blend,
            quality: tiny_skia::FilterQuality::Bilinear,
        };
        let clip = self.state.clip.clone();
        self.pixmap
            .draw_pixmap(0, 0, pixmap.as_ref(), &paint, ts, clip.as_deref());
    }

    /// Canvas shadows: the shape's alpha is rendered to a temporary pixmap,
    /// blurred with a Gaussian of sigma = shadowBlur / 2 (three box blurs),
    /// tinted with the shadow colour and composited under the shape.
    /// Wide blurs are computed at a reduced resolution and scaled back up
    /// with bilinear filtering, which looks the same and is much faster.
    fn draw_shadow(&mut self, path: &Path, shader: Shader, rule: FillRule) {
        let sigma = self.state.shadow_blur / 2.0;
        let margin = (sigma * 3.0).ceil() + 1.0;
        let b = path.bounds();
        let (pw, ph) = (self.width() as f32, self.height() as f32);
        let x0 = (b.left().floor() - margin).max(-margin);
        let y0 = (b.top().floor() - margin).max(-margin);
        let x1 = (b.right().ceil() + margin).min(pw + margin);
        let y1 = (b.bottom().ceil() + margin).min(ph + margin);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let k = (sigma / 2.5).floor().clamp(1.0, 8.0);
        let (w, h) = (((x1 - x0) / k).ceil() as u32, ((y1 - y0) / k).ceil() as u32);
        let Some(mut tmp) = Pixmap::new(w, h) else {
            return;
        };
        let paint = Paint {
            shader,
            anti_alias: true,
            ..Paint::default()
        };
        tmp.fill_path(
            path,
            &paint,
            rule,
            Transform::from_scale(1.0 / k, 1.0 / k).pre_translate(-x0, -y0),
            None,
        );

        let mut a: Vec<f32> = tmp
            .pixels()
            .iter()
            .map(|p| p.alpha() as f32 / 255.0)
            .collect();
        gaussian_blur(&mut a, w as usize, h as usize, sigma / k);

        let c = self.state.shadow_color;
        let (r, g, bl, ca) = (c.red(), c.green(), c.blue(), c.alpha());
        let clip = self.state.clip.clone();

        if self.state.blend != BlendMode::SourceOver {
            // Rare: tint the small image and let tiny-skia scale and blend it.
            let data = tmp.data_mut();
            for (px, &v) in data.chunks_exact_mut(4).zip(a.iter()) {
                let alpha = (v * ca).clamp(0.0, 1.0);
                let a8 = (alpha * 255.0 + 0.5) as u8;
                let a8f = a8 as f32;
                px[0] = ((r * a8f + 0.5) as u8).min(a8);
                px[1] = ((g * a8f + 0.5) as u8).min(a8);
                px[2] = ((bl * a8f + 0.5) as u8).min(a8);
                px[3] = a8;
            }
            let quality = if k > 1.0 {
                tiny_skia::FilterQuality::Bilinear
            } else {
                tiny_skia::FilterQuality::Nearest
            };
            let paint = PixmapPaint {
                opacity: self.state.alpha,
                blend_mode: self.state.blend,
                quality,
            };
            let ts = Transform::from_translate(x0, y0).pre_scale(k, k);
            self.pixmap
                .draw_pixmap(0, 0, tmp.as_ref(), &paint, ts, clip.as_deref());
            return;
        }

        // Source-over: scale up (bilinear), tint and blend straight into the pixmap.
        let (w, h) = (w as usize, h as usize);
        let pw = self.width() as usize;
        let (gx0, gy0) = (x0.max(0.0) as usize, y0.max(0.0) as usize);
        let (gx1, gy1) = (x1.min(pw as f32) as usize, y1.min(ph) as usize);
        let sample = |p: f32, n: usize| {
            let f = p.clamp(0.0, (n - 1) as f32);
            let i = f.floor() as usize;
            (i, (i + 1).min(n - 1), f - i as f32)
        };
        let cols: Vec<(usize, usize, f32)> = (gx0..gx1)
            .map(|x| sample((x as f32 + 0.5 - x0) / k - 0.5, w))
            .collect();
        let tint = [r * 255.0, g * 255.0, bl * 255.0, 255.0];
        let strength = ca * self.state.alpha;
        let mut row = vec![0.0f32; w];
        let data = self.pixmap.data_mut();
        for y in gy0..gy1 {
            let (j0, j1, ty) = sample((y as f32 + 0.5 - y0) / k - 0.5, h);
            let (r0, r1) = (&a[j0 * w..(j0 + 1) * w], &a[j1 * w..(j1 + 1) * w]);
            for ((o, &p), &q) in row.iter_mut().zip(r0).zip(r1) {
                *o = p + (q - p) * ty;
            }
            let mask_row = clip.as_ref().map(|m| &m.data()[y * pw..(y + 1) * pw]);
            for (x, &(i0, i1, tx)) in (gx0..gx1).zip(&cols) {
                let mut sa = (row[i0] + (row[i1] - row[i0]) * tx) * strength;
                if let Some(m) = mask_row {
                    sa *= m[x] as f32 / 255.0;
                }
                if sa <= 0.0 {
                    continue;
                }
                let sa = sa.min(1.0);
                let px = &mut data[(y * pw + x) * 4..(y * pw + x) * 4 + 4];
                for (d, t) in px.iter_mut().zip(tint) {
                    *d = (t * sa + *d as f32 * (1.0 - sa) + 0.5) as u8;
                }
            }
        }
    }
}

/// Box sizes for approximating a Gaussian with `n` box blurs.
fn boxes_for_gauss(sigma: f32, n: usize) -> Vec<usize> {
    let w_ideal = (12.0 * sigma * sigma / n as f32 + 1.0).sqrt();
    let mut wl = w_ideal.floor() as i32;
    if wl % 2 == 0 {
        wl -= 1;
    }
    let wl = wl.max(1);
    let wu = wl + 2;
    let (nf, wlf) = (n as f32, wl as f32);
    let m = ((12.0 * sigma * sigma - nf * wlf * wlf - 4.0 * nf * wlf - 3.0 * nf)
        / (-4.0 * wlf - 4.0))
        .round() as i32;
    (0..n as i32)
        .map(|i| if i < m { wl as usize } else { wu as usize })
        .collect()
}

/// Blurs a single-channel image in place (pixels outside count as 0).
pub fn gaussian_blur(data: &mut [f32], w: usize, h: usize, sigma: f32) {
    if sigma < 0.3 || w == 0 || h == 0 {
        return;
    }
    let mut tmp = vec![0.0f32; data.len()];
    for size in boxes_for_gauss(sigma, 3) {
        let r = (size - 1) / 2;
        if r == 0 {
            continue;
        }
        box_blur_h(data, &mut tmp, w, h, r);
        box_blur_v(&tmp, data, w, h, r);
    }
}

fn box_blur_h(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize) {
    let inv = 1.0 / (2 * r + 1) as f32;
    for y in 0..h {
        let row = &src[y * w..(y + 1) * w];
        let out = &mut dst[y * w..(y + 1) * w];
        let mut acc: f32 = row[..r.min(w)].iter().sum();
        for x in 0..w {
            if x + r < w {
                acc += row[x + r];
            }
            out[x] = acc * inv;
            if x >= r {
                acc -= row[x - r];
            }
        }
    }
}

fn box_blur_v(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize) {
    let inv = 1.0 / (2 * r + 1) as f32;
    let mut acc = vec![0.0f32; w];
    for y in 0..r.min(h) {
        for (a, v) in acc.iter_mut().zip(&src[y * w..(y + 1) * w]) {
            *a += v;
        }
    }
    for y in 0..h {
        if y + r < h {
            for (a, v) in acc.iter_mut().zip(&src[(y + r) * w..(y + r + 1) * w]) {
                *a += v;
            }
        }
        for (o, a) in dst[y * w..(y + 1) * w].iter_mut().zip(&acc) {
            *o = a * inv;
        }
        if y >= r {
            for (a, v) in acc.iter_mut().zip(&src[(y - r) * w..(y - r + 1) * w]) {
                *a -= v;
            }
        }
    }
}

/// `Math.round` (halves round up).
pub fn js_round(v: f32) -> f32 {
    (v + 0.5).floor()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha_at(ctx: &Ctx, x: u32, y: u32) -> u8 {
        ctx.pixmap.pixel(x, y).unwrap().alpha()
    }

    #[test]
    fn round_rect_and_clip() {
        let mut ctx = Ctx::new(40, 40);
        ctx.round_rect(0.0, 0.0, 40.0, 40.0, 12.0);
        ctx.clip();
        ctx.fill_rect(0.0, 0.0, 40.0, 40.0, Rgb::WHITE);
        assert_eq!(alpha_at(&ctx, 20, 20), 255);
        assert_eq!(alpha_at(&ctx, 0, 0), 0);
        assert_eq!(alpha_at(&ctx, 20, 0), 255);
    }

    #[test]
    fn save_restore_transform_and_clip() {
        let mut ctx = Ctx::new(20, 20);
        ctx.save();
        ctx.rect(0.0, 0.0, 10.0, 10.0);
        ctx.clip();
        ctx.translate(5.0, 5.0);
        ctx.restore();
        ctx.fill_rect(10.0, 10.0, 5.0, 5.0, Rgb::WHITE);
        assert_eq!(alpha_at(&ctx, 12, 12), 255);
    }

    #[test]
    fn shadow_spreads_beyond_the_shape() {
        let mut ctx = Ctx::new(40, 40);
        ctx.set_shadow(alpha(Rgb::WHITE, 1.0), 8.0);
        ctx.fill_rect(15.0, 15.0, 10.0, 10.0, Rgb::WHITE);
        assert!(alpha_at(&ctx, 12, 20) > 0);
        assert_eq!(alpha_at(&ctx, 0, 0), 0);
    }

    #[test]
    fn line_to_without_a_point_moves() {
        let mut ctx = Ctx::new(20, 20);
        ctx.begin_path();
        ctx.line_to(2.0, 10.0);
        ctx.line_to(18.0, 10.0);
        ctx.set_line_width(4.0);
        ctx.stroke(Rgb::WHITE);
        assert_eq!(alpha_at(&ctx, 10, 10), 255);
        assert_eq!(alpha_at(&ctx, 1, 1), 0);
    }
}
