//! Procedurally generated block graphics, one look per style tier. Sprites are
//! rendered once per style/colour/size (the renderer caches them).

use std::f32::consts::{PI, TAU};

use tiny_skia::{BlendMode, LineCap, LineJoin, Pixmap};

use super::skia::{Ctx, alpha, black, linear, radial, rgba, white};
use crate::color::Rgb;
use crate::styles::BlockKind;

/// Renders one block of the given look and colour into a `px`×`px` pixmap
/// (premultiplied RGBA).
pub fn block_sprite(kind: BlockKind, base: Rgb, px: u32) -> Pixmap {
    let px = px.max(1);
    let mut ctx = Ctx::new(px, px);
    let s = px as f32;
    match kind {
        BlockKind::Jewel => jewel(&mut ctx, base, s),
        BlockKind::Cardboard => cardboard(&mut ctx, base, s),
        BlockKind::Candy => candy(&mut ctx, base, s),
        BlockKind::Gingerbread => gingerbread(&mut ctx, base, s),
        BlockKind::Wood => wood(&mut ctx, base, s),
        BlockKind::Gem => gem(&mut ctx, base, s),
        BlockKind::Iron => iron(&mut ctx, base, s),
        BlockKind::Neon => neon(&mut ctx, base, s),
        BlockKind::Glass => glass(&mut ctx, base, s),
        BlockKind::Royal => royal(&mut ctx, base, s),
    }
    ctx.into_pixmap()
}

/// Small seeded PRNG (xorshift32), so textures like wood grain look the same on every redraw.
#[derive(Clone, Debug)]
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        Rng(if seed == 0 { 0x9e3779b9 } else { seed })
    }

    /// A float in [0, 1).
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x as f64 / 4294967296.0
    }

    /// `next()` as f32, for geometry.
    pub fn f(&mut self) -> f32 {
        self.next() as f32
    }
}

/// FNV-1a hash of a string (of its UTF-16 code units, like the TS; colour strings are ASCII).
pub fn hash(text: &str) -> u32 {
    let mut h: u32 = 2166136261;
    for c in text.encode_utf16() {
        h = (h ^ c as u32).wrapping_mul(16777619);
    }
    h
}

/// The seed of a colour's texture: the TS hashes the palette's "#rrggbb" string.
fn seed(base: Rgb, salt: u32) -> Rng {
    Rng::new(hash(&base.to_hex()) ^ salt)
}

/// Length of the outline of a rounded rectangle.
pub fn round_rect_length(w: f32, h: f32, r: f32) -> f32 {
    2.0 * (w - 2.0 * r) + 2.0 * (h - 2.0 * r) + 2.0 * PI * r
}

/// The point at distance `d` along the outline of a rounded rectangle, clockwise
/// from the start of its top edge, and the outward normal there: [x, y, nx, ny].
pub fn round_rect_point(x: f32, y: f32, w: f32, h: f32, r: f32, d: f32) -> [f32; 4] {
    let r = r.max(1e-3);
    let sw = w - 2.0 * r;
    let sh = h - 2.0 * r;
    let arc = (PI * r) / 2.0;
    let total = round_rect_length(w, h, r);
    let mut t = ((d % total) + total) % total;
    let lens = [sw, arc, sh, arc, sw, arc, sh, arc];
    let mut seg = 0;
    while seg < 7 && t > lens[seg] {
        t -= lens[seg];
        seg += 1;
    }
    let corner = |cx: f32, cy: f32, a0: f32| {
        let a = a0 + t / r;
        [cx + a.cos() * r, cy + a.sin() * r, a.cos(), a.sin()]
    };
    match seg {
        0 => [x + r + t, y, 0.0, -1.0],
        1 => corner(x + w - r, y + r, -PI / 2.0),
        2 => [x + w, y + r + t, 1.0, 0.0],
        3 => corner(x + w - r, y + h - r, 0.0),
        4 => [x + w - r - t, y + h, 0.0, 1.0],
        5 => corner(x + r, y + h - r, PI / 2.0),
        6 => [x, y + h - r - t, -1.0, 0.0],
        _ => corner(x + r, y + r, PI),
    }
}

/// A rounded rectangle outline that wiggles in and out, like piped icing.
#[allow(clippy::too_many_arguments)]
pub fn wavy_round_rect(
    ctx: &mut Ctx,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    amp: f32,
    wavelength: f32,
) {
    let total = round_rect_length(w, h, r);
    let waves = (total / wavelength).round().max(4.0);
    let steps = (waves * 10.0) as usize;
    ctx.begin_path();
    for i in 0..=steps {
        let k = i as f32 / steps as f32;
        let [px, py, nx, ny] = round_rect_point(x, y, w, h, r, k * total);
        let o = amp * (k * waves * PI * 2.0).sin();
        if i == 0 {
            ctx.move_to(px + nx * o, py + ny * o);
        } else {
            ctx.line_to(px + nx * o, py + ny * o);
        }
    }
    ctx.close_path();
}

/// A four-pointed twinkle.
fn sparkle(ctx: &mut Ctx, x: f32, y: f32, r: f32, a: f32) {
    ctx.begin_path();
    ctx.move_to(x, y - r);
    ctx.quad_to(x, y, x + r, y);
    ctx.quad_to(x, y, x, y + r);
    ctx.quad_to(x, y, x - r, y);
    ctx.quad_to(x, y, x, y - r);
    ctx.fill(white(a));
}

/// Strokes a rounded rectangle inset by half the line width, so the stroke stays inside.
fn inner_outline(ctx: &mut Ctx, x: f32, w: f32, radius: f32, color: Rgb) {
    let h = ctx.line_width() / 2.0;
    ctx.round_rect(x + h, x + h, w - h * 2.0, w - h * 2.0, radius);
    ctx.stroke(color);
}

/// Classic: a bevelled square with a light top-left edge, dark bottom-right edge and a soft glossy face.
fn jewel(ctx: &mut Ctx, base: Rgb, s: f32) {
    let gap = (s * 0.03).max(1.0);
    let x0 = gap;
    let x1 = s - gap;
    let radius = s * 0.1;
    let bevel = s * 0.15;

    ctx.save();
    ctx.round_rect(x0, x0, x1 - x0, x1 - x0, radius);
    ctx.clip();

    ctx.fill_rect(0.0, 0.0, s, s, base);

    let i0 = x0 + bevel;
    let i1 = x1 - bevel;
    ctx.poly(&[x0, x0, x1, x0, i1, i0, i0, i0], base.shade(0.42)); // top
    ctx.poly(&[x0, x0, i0, i0, i0, i1, x0, x1], base.shade(0.2)); // left
    ctx.poly(&[x1, x0, x1, x1, i1, i1, i1, i0], base.shade(-0.18)); // right
    ctx.poly(&[x0, x1, i0, i1, i1, i1, x1, x1], base.shade(-0.32)); // bottom

    let face = linear(
        0.0,
        i0,
        0.0,
        i1,
        &[
            (0.0, alpha(base.shade(0.12), 1.0)),
            (1.0, alpha(base.shade(-0.06), 1.0)),
        ],
    );
    ctx.fill_rect(i0, i0, i1 - i0, i1 - i0, face);

    // Glossy highlight in the upper-left of the face.
    let (gx, gy) = (i0 + s * 0.12, i0 + s * 0.1);
    let gloss = radial(
        gx,
        gy,
        0.0,
        gx,
        gy,
        s * 0.32,
        &[(0.0, white(0.45)), (1.0, white(0.0))],
    );
    ctx.fill_rect(i0, i0, i1 - i0, i1 - i0, gloss);
    ctx.restore();

    ctx.set_line_width((s * 0.025).max(1.0));
    inner_outline(ctx, x0, x1 - x0, radius, base.shade(-0.45));
}

/// Candy: a rounded, glossy jelly sweet with a big shine on top.
fn candy(ctx: &mut Ctx, base: Rgb, s: f32) {
    let gap = (s * 0.04).max(1.0);
    let w = s - gap * 2.0;
    let radius = s * 0.26;

    ctx.save();
    ctx.round_rect(gap, gap, w, w, radius);
    ctx.clip();
    let body = linear(
        0.0,
        gap,
        0.0,
        s - gap,
        &[
            (0.0, alpha(base.shade(0.25), 1.0)),
            (0.55, alpha(base, 1.0)),
            (1.0, alpha(base.shade(-0.28), 1.0)),
        ],
    );
    ctx.fill_rect(0.0, 0.0, s, s, body);

    // Inner glow from the bottom, like light passing through jelly.
    let inner = radial(
        s / 2.0,
        s * 0.95,
        0.0,
        s / 2.0,
        s * 0.95,
        s * 0.6,
        &[(0.0, white(0.35)), (1.0, white(0.0))],
    );
    ctx.fill_rect(0.0, 0.0, s, s, inner);

    // Big shine across the top half.
    ctx.begin_path();
    ctx.ellipse(s * 0.5, s * 0.27, s * 0.34, s * 0.15, 0.0, 0.0, TAU, false);
    ctx.fill(white(0.5));
    ctx.begin_path();
    ctx.ellipse(
        s * 0.3,
        s * 0.26,
        s * 0.07,
        s * 0.045,
        -0.5,
        0.0,
        TAU,
        false,
    );
    ctx.fill(white(0.9));
    ctx.restore();

    ctx.set_line_width((s * 0.035).max(1.0));
    inner_outline(ctx, gap, w, radius, base.shade(-0.4));
}

/// Gems: a cut stone with eight facets around an octagonal table and a sparkle.
fn gem(ctx: &mut Ctx, base: Rgb, s: f32) {
    let a = (s * 0.03).max(1.0);
    let b = s - a;
    let t0 = s * 0.22;
    let t1 = s - t0;
    let k = s * 0.1; // corner cut of the table
    let radius = s * 0.08;

    ctx.save();
    ctx.round_rect(a, a, b - a, b - a, radius);
    ctx.clip();
    let o = [[a, a], [b, a], [b, b], [a, b]];
    let tt = [
        t0 + k,
        t0,
        t1 - k,
        t0,
        t1,
        t0 + k,
        t1,
        t1 - k,
        t1 - k,
        t1,
        t0 + k,
        t1,
        t0,
        t1 - k,
        t0,
        t0 + k,
    ];
    let t = |i: usize| [tt[i * 2], tt[i * 2 + 1]];
    let facet = |pts: &[[f32; 2]]| pts.iter().flatten().copied().collect::<Vec<f32>>();
    ctx.poly(&facet(&[o[0], o[1], t(1), t(0)]), base.shade(0.45)); // top
    ctx.poly(&facet(&[o[1], t(2), t(1)]), base.shade(0.2));
    ctx.poly(&facet(&[o[1], o[2], t(3), t(2)]), base.shade(-0.2)); // right
    ctx.poly(&facet(&[o[2], t(4), t(3)]), base.shade(-0.42));
    ctx.poly(&facet(&[o[2], o[3], t(5), t(4)]), base.shade(-0.35)); // bottom
    ctx.poly(&facet(&[o[3], t(6), t(5)]), base.shade(-0.1));
    ctx.poly(&facet(&[o[3], o[0], t(7), t(6)]), base.shade(0.15)); // left
    ctx.poly(&facet(&[o[0], t(0), t(7)]), base.shade(0.55));

    let table = linear(
        t0,
        t0,
        t1,
        t1,
        &[
            (0.0, alpha(base.shade(0.3), 1.0)),
            (0.5, alpha(base, 1.0)),
            (1.0, alpha(base.shade(-0.12), 1.0)),
        ],
    );
    ctx.poly(&tt, "#000000");
    ctx.fill(table);
    // Light streak across the table.
    ctx.poly(
        &[t0 + k, t0, t0 + k * 2.6, t0, t0, t0 + k * 2.6, t0, t0 + k],
        white(0.4),
    );
    ctx.restore();

    ctx.set_line_width((s * 0.025).max(1.0));
    inner_outline(ctx, a, b - a, radius, base.shade(-0.55));
    sparkle(ctx, s * 0.3, s * 0.3, s * 0.16, 0.95);
}

/// Neon: a dark tile outlined by a glowing tube.
fn neon(ctx: &mut Ctx, base: Rgb, s: f32) {
    let pad = s * 0.12;
    let w = s - pad * 2.0;
    let radius = s * 0.14;
    let face = linear(
        0.0,
        pad,
        0.0,
        s - pad,
        &[(0.0, alpha(base, 0.38)), (1.0, alpha(base, 0.14))],
    );
    ctx.round_rect(pad, pad, w, w, radius);
    ctx.fill("#05010f");
    ctx.fill(face);

    ctx.set_shadow(alpha(base, 1.0), s * 0.16);
    ctx.set_line_width((s * 0.075).max(1.5));
    ctx.stroke(base);
    ctx.stroke(base);
    ctx.set_shadow_blur(0.0);
    // Hot white core of the tube.
    ctx.set_line_width((s * 0.025).max(1.0));
    ctx.stroke(base.shade(0.65));
    // Small inner square for depth.
    let ip = s * 0.34;
    ctx.round_rect(ip, ip, s - ip * 2.0, s - ip * 2.0, s * 0.06);
    ctx.fill(alpha(base, 0.35));
}

/// Aurora: a translucent glass tile with a bright rim and a diagonal reflection.
fn glass(ctx: &mut Ctx, base: Rgb, s: f32) {
    let gap = (s * 0.04).max(1.0);
    let w = s - gap * 2.0;
    let radius = s * 0.18;

    ctx.save();
    ctx.round_rect(gap, gap, w, w, radius);
    ctx.clip();
    let body = linear(
        gap,
        gap,
        s - gap,
        s - gap,
        &[
            (0.0, alpha(base, 0.95)),
            (0.5, alpha(base, 0.6)),
            (1.0, alpha(base, 0.85)),
        ],
    );
    ctx.fill_rect(0.0, 0.0, s, s, body);
    // Glow from the centre.
    let core = radial(
        s / 2.0,
        s / 2.0,
        0.0,
        s / 2.0,
        s / 2.0,
        s * 0.5,
        &[(0.0, white(0.4)), (1.0, white(0.0))],
    );
    ctx.fill_rect(0.0, 0.0, s, s, core);
    // Diagonal reflection bands.
    ctx.poly(
        &[0.0, s * 0.55, s * 0.55, 0.0, s * 0.75, 0.0, 0.0, s * 0.75],
        white(0.28),
    );
    ctx.poly(
        &[0.0, s * 0.88, s * 0.88, 0.0, s * 0.94, 0.0, 0.0, s * 0.94],
        white(0.18),
    );
    ctx.restore();

    ctx.set_line_width((s * 0.05).max(1.0));
    let rim = linear(
        0.0,
        0.0,
        s,
        s,
        &[
            (0.0, white(0.95)),
            (0.5, alpha(base, 0.6)),
            (1.0, white(0.6)),
        ],
    );
    let h = ctx.line_width() / 2.0;
    ctx.round_rect(gap + h, gap + h, w - h * 2.0, w - h * 2.0, radius);
    ctx.stroke(rim);
    sparkle(ctx, s * 0.27, s * 0.27, s * 0.12, 0.9);
}

/// Legendary: a jewel set in a bevelled gold frame.
fn royal(ctx: &mut Ctx, base: Rgb, s: f32) {
    let gap = (s * 0.025).max(1.0);
    let w = s - gap * 2.0;
    let radius = s * 0.12;

    let gold = linear(
        0.0,
        0.0,
        s,
        s,
        &[
            (0.0, alpha(Rgb::hex("#fff6c8"), 1.0)),
            (0.3, alpha(Rgb::hex("#ffd34d"), 1.0)),
            (0.65, alpha(Rgb::hex("#c8901a"), 1.0)),
            (1.0, alpha(Rgb::hex("#7a5208"), 1.0)),
        ],
    );
    ctx.round_rect(gap, gap, w, w, radius);
    ctx.fill(gold);

    let f = s * 0.15;
    let fw = s - f * 2.0;
    let face = radial(
        s * 0.38,
        s * 0.35,
        0.0,
        s / 2.0,
        s / 2.0,
        s * 0.5,
        &[
            (0.0, alpha(base.shade(0.45), 1.0)),
            (0.45, alpha(base, 1.0)),
            (1.0, alpha(base.shade(-0.5), 1.0)),
        ],
    );
    ctx.round_rect(f, f, fw, fw, s * 0.07);
    ctx.fill(face);
    ctx.set_line_width((s * 0.03).max(1.0));
    ctx.stroke("#6b4500");
    // Inner bevel of the frame.
    ctx.set_line_width((s * 0.02).max(1.0));
    ctx.round_rect(
        gap + s * 0.03,
        gap + s * 0.03,
        w - s * 0.06,
        w - s * 0.06,
        radius * 0.8,
    );
    ctx.stroke(rgba(255, 250, 220, 0.7));
    ctx.set_line_width((s * 0.025).max(1.0));
    ctx.round_rect(gap, gap, w, w, radius);
    ctx.stroke("#5a3a00");
    sparkle(ctx, s * 0.36, s * 0.34, s * 0.14, 0.95);
}

/// Cardboard: a matte piece of corrugated kraft board printed in the block colour, with a strip of tape over a corner.
fn cardboard(ctx: &mut Ctx, base: Rgb, s: f32) {
    let gap = (s * 0.035).max(1.0);
    let w = s - gap * 2.0;
    let radius = s * 0.05;
    let mut rand = seed(base, 0xc0ffee);

    ctx.save();
    ctx.round_rect(gap, gap, w, w, radius);
    ctx.clip();
    // Ink soaked into brown paper: the kraft shows through and dulls it a little.
    ctx.fill_rect(0.0, 0.0, s, s, base);
    ctx.fill_rect(0.0, 0.0, s, s, rgba(176, 128, 72, 0.26));
    let light = linear(
        0.0,
        0.0,
        0.0,
        s,
        &[
            (0.0, rgba(255, 240, 215, 0.16)),
            (1.0, rgba(60, 30, 0, 0.16)),
        ],
    );
    ctx.fill_rect(0.0, 0.0, s, s, light);

    // Flutes of the corrugation pressing through the liner.
    let flute = s as f64 * 0.1;
    let mut x = gap as f64 + flute * 0.3;
    while x < s as f64 {
        let xf = x as f32;
        let fl = flute as f32;
        ctx.fill_rect(xf, 0.0, fl * 0.42, s, rgba(70, 35, 0, 0.09));
        ctx.fill_rect(xf + fl * 0.5, 0.0, fl * 0.18, s, rgba(255, 245, 225, 0.07));
        x += flute;
    }
    // Paper fibres.
    ctx.set_line_width((s * 0.008).max(0.5));
    for _ in 0..14 {
        let x = rand.f() * s;
        let y = rand.f() * s;
        let a = rand.f() * PI;
        let l = s * (0.03 + rand.f() * 0.05);
        let color = if rand.next() < 0.5 {
            rgba(60, 30, 0, 0.16)
        } else {
            rgba(255, 240, 210, 0.14)
        };
        ctx.begin_path();
        ctx.move_to(x, y);
        ctx.line_to(x + a.cos() * l, y + a.sin() * l);
        ctx.stroke(color);
    }

    // Printed "this way up" arrows in a darker ink.
    let ink = alpha(base.shade(-0.55), 0.55);
    ctx.set_line_width((s * 0.035).max(1.0));
    ctx.set_line_cap(LineCap::Round);
    ctx.set_line_join(LineJoin::Round);
    for ax in [s * 0.2, s * 0.33] {
        ctx.begin_path();
        ctx.move_to(ax, s * 0.8);
        ctx.line_to(ax, s * 0.62);
        ctx.move_to(ax - s * 0.05, s * 0.67);
        ctx.line_to(ax, s * 0.62);
        ctx.line_to(ax + s * 0.05, s * 0.67);
        ctx.stroke(ink);
    }
    ctx.begin_path();
    ctx.move_to(s * 0.13, s * 0.85);
    ctx.line_to(s * 0.4, s * 0.85);
    ctx.stroke(ink);

    // A strip of packing tape across the top-right corner.
    ctx.save();
    ctx.translate(s * 0.8, s * 0.2);
    ctx.rotate(PI / 4.0);
    let tw = s * 0.62;
    let th = s * 0.2;
    ctx.fill_rect(-tw / 2.0, -th / 2.0, tw, th, rgba(246, 228, 186, 0.72));
    ctx.fill_rect(-tw / 2.0, -th / 2.0, tw, th * 0.28, white(0.32));
    let edge = (s * 0.015).max(0.5);
    ctx.fill_rect(
        -tw / 2.0,
        th / 2.0 - edge,
        tw,
        edge,
        rgba(120, 80, 30, 0.25),
    );
    ctx.restore();
    ctx.restore();

    // Crisp cut edge: light on top and left, darker all round.
    ctx.set_line_width((s * 0.03).max(1.0));
    let h = ctx.line_width() / 2.0;
    inner_outline(ctx, gap, w, radius, base.shade(-0.5));
    ctx.set_line_width((s * 0.02).max(1.0));
    ctx.begin_path();
    ctx.move_to(gap + h * 2.0, s - gap - h * 3.0);
    ctx.line_to(gap + h * 2.0, gap + h * 2.0);
    ctx.line_to(s - gap - h * 3.0, gap + h * 2.0);
    ctx.stroke(rgba(255, 240, 215, 0.35));
}

/// Gingerbread: a baked cookie with a coloured glaze, white piped icing around it and a sugar pearl.
fn gingerbread(ctx: &mut Ctx, base: Rgb, s: f32) {
    let gap = (s * 0.03).max(1.0);
    let w = s - gap * 2.0;
    let radius = s * 0.2;
    let mut rand = seed(base, 0x6b1d);

    ctx.save();
    ctx.round_rect(gap, gap, w, w, radius);
    ctx.clip();
    let dough = radial(
        s * 0.38,
        s * 0.32,
        s * 0.05,
        s / 2.0,
        s / 2.0,
        s * 0.72,
        &[
            (0.0, alpha(Rgb::hex("#d9944f"), 1.0)),
            (0.55, alpha(Rgb::hex("#b66a30"), 1.0)),
            (1.0, alpha(Rgb::hex("#7d3f17"), 1.0)),
        ],
    );
    ctx.fill_rect(0.0, 0.0, s, s, dough);
    // Baked freckles and spice specks.
    for _ in 0..16 {
        let color = if rand.next() < 0.7 {
            rgba(92, 40, 10, 0.45)
        } else {
            rgba(255, 214, 160, 0.35)
        };
        let x = gap + rand.f() * w;
        let y = gap + rand.f() * w;
        let r = s * (0.008 + rand.f() * 0.012);
        ctx.begin_path();
        ctx.arc(x, y, r, 0.0, TAU, false);
        ctx.fill(color);
    }

    // Coloured glaze in the middle.
    let g0 = s * 0.2;
    let gw = s - g0 * 2.0;
    let gr = s * 0.13;
    ctx.round_rect(g0 + s * 0.01, g0 + s * 0.025, gw, gw, gr);
    ctx.fill(rgba(70, 28, 6, 0.4));
    let glaze = linear(
        0.0,
        g0,
        0.0,
        g0 + gw,
        &[
            (0.0, alpha(base.shade(0.3), 1.0)),
            (0.5, alpha(base, 1.0)),
            (1.0, alpha(base.shade(-0.18), 1.0)),
        ],
    );
    ctx.round_rect(g0, g0, gw, gw, gr);
    ctx.fill(glaze);
    ctx.save();
    ctx.clip();
    ctx.begin_path();
    ctx.ellipse(s * 0.42, s * 0.3, s * 0.2, s * 0.07, -0.25, 0.0, TAU, false);
    ctx.fill(white(0.38));
    ctx.restore();

    // Piped royal icing: a wavy white line framing the glaze, with a soft shadow.
    let pi = s * 0.125;
    ctx.set_line_cap(LineCap::Round);
    ctx.set_line_join(LineJoin::Round);
    ctx.set_line_width((s * 0.05).max(1.2));
    ctx.save();
    ctx.translate(s * 0.01, s * 0.02);
    wavy_round_rect(
        ctx,
        pi,
        pi,
        s - pi * 2.0,
        s - pi * 2.0,
        s * 0.15,
        s * 0.022,
        s * 0.11,
    );
    ctx.stroke(rgba(80, 30, 5, 0.4));
    ctx.restore();
    wavy_round_rect(
        ctx,
        pi,
        pi,
        s - pi * 2.0,
        s - pi * 2.0,
        s * 0.15,
        s * 0.022,
        s * 0.11,
    );
    ctx.stroke("#fffaf2");
    ctx.restore();

    // A sugar pearl on the glaze.
    let px = s * 0.66;
    let py = s * 0.64;
    let pr = s * 0.065;
    let pearl = radial(
        px - pr * 0.35,
        py - pr * 0.4,
        0.0,
        px,
        py,
        pr,
        &[
            (0.0, alpha(Rgb::hex("#ffffff"), 1.0)),
            (0.6, alpha(Rgb::hex("#f4ede4"), 1.0)),
            (1.0, alpha(Rgb::hex("#bfae9c"), 1.0)),
        ],
    );
    ctx.begin_path();
    ctx.arc(px + pr * 0.15, py + pr * 0.3, pr, 0.0, TAU, false);
    ctx.fill(rgba(60, 20, 0, 0.3));
    ctx.begin_path();
    ctx.arc(px, py, pr, 0.0, TAU, false);
    ctx.fill(pearl);

    ctx.set_line_width((s * 0.03).max(1.0));
    inner_outline(ctx, gap, w, radius, Rgb::hex("#5a2a0c"));
}

/// Wooden: two stained planks with flowing grain, a knot and a satin varnish.
fn wood(ctx: &mut Ctx, base: Rgb, s: f32) {
    let gap = (s * 0.03).max(1.0);
    let w = s - gap * 2.0;
    let radius = s * 0.07;
    let mut rand = seed(base, 0x0a4);

    ctx.save();
    ctx.round_rect(gap, gap, w, w, radius);
    ctx.clip();
    ctx.fill_rect(0.0, 0.0, s, s, base.shade(0.08));

    // The stain soaks into the wood: multiply light and dark wood tones over the colour.
    ctx.set_blend(BlendMode::Multiply);
    let tone = linear(
        0.0,
        0.0,
        s,
        0.0,
        &[
            (0.0, alpha(Rgb::hex("#f2d3a8"), 1.0)),
            (0.5, alpha(Rgb::hex("#ffe9c8"), 1.0)),
            (1.0, alpha(Rgb::hex("#e6be8a"), 1.0)),
        ],
    );
    ctx.fill_rect(0.0, 0.0, s, s, tone);
    let half = s / 2.0;
    // A knot on one of the planks; the grain flows around it.
    let kx = s * (0.3 + rand.f() * 0.4);
    let knot_plank = if rand.next() < 0.5 { 0 } else { 1 };
    let ky = (knot_plank as f32 + 0.5) * half;
    for plank in 0..2 {
        let y0 = plank as f32 * half;
        // Slight tone difference between the planks.
        let tint = if plank == 1 {
            rgba(170, 120, 70, 0.22)
        } else {
            rgba(255, 230, 190, 0.18)
        };
        ctx.fill_rect(0.0, y0, s, half, tint);
        // Grain lines running along the plank: long gentle waves, darker late wood and fine streaks.
        let lines = 7;
        let phase = rand.f() * PI * 2.0;
        let freq = (0.8 + rand.f()) * (PI / s);
        for i in 0..lines {
            let yy = y0 + ((i as f32 + 0.25 + rand.f() * 0.5) / lines as f32) * half;
            let amp = s * (0.01 + rand.f() * 0.018);
            let fine = i % 2 == 1;
            let a = if fine {
                0.15 + rand.f() * 0.12
            } else {
                0.3 + rand.f() * 0.25
            };
            let color = rgba(90, 45, 10, a);
            ctx.set_line_width(
                (s * if fine {
                    0.007
                } else {
                    0.012 + rand.f() * 0.014
                })
                .max(0.5),
            );
            let away = if plank == knot_plank {
                let d = yy - ky;
                if d > 0.0 {
                    1.0
                } else if d < 0.0 {
                    -1.0
                } else {
                    1.0
                }
            } else {
                0.0
            };
            let push = away * s * 0.07 * (1.0 - (yy - ky).abs() / (half * 0.75)).max(0.0);
            ctx.begin_path();
            let step = s as f64 / 24.0;
            let mut x = 0.0f64;
            while x <= s as f64 {
                let xf = x as f32;
                let bump = push * (-((xf - kx) / (s * 0.13)).powi(2)).exp();
                let y = yy
                    + (xf * freq + phase + i as f32 * 0.45).sin() * amp
                    + (xf * freq * 3.1 + i as f32).sin() * amp * 0.25
                    + bump;
                if x == 0.0 {
                    ctx.move_to(xf, y);
                } else {
                    ctx.line_to(xf, y);
                }
                x += step;
            }
            ctx.stroke(color);
        }
    }
    for i in (1..=3).rev() {
        let i = i as f32;
        ctx.set_line_width((s * 0.014).max(0.6));
        ctx.begin_path();
        ctx.ellipse(kx, ky, s * 0.045 * i, s * 0.022 * i, 0.0, 0.0, TAU, false);
        ctx.stroke(rgba(80, 38, 8, 0.22 + 0.1 * (3.0 - i)));
    }
    ctx.begin_path();
    ctx.ellipse(kx, ky, s * 0.03, s * 0.014, 0.0, 0.0, TAU, false);
    ctx.fill(rgba(70, 30, 5, 0.55));
    ctx.set_blend(BlendMode::SourceOver);

    // Seam between the planks.
    ctx.fill_rect(
        0.0,
        half - (s * 0.016).max(0.75),
        s,
        (s * 0.03).max(1.0),
        alpha(base.shade(-0.7), 0.6),
    );
    ctx.fill_rect(
        0.0,
        half + (s * 0.016).max(0.75),
        s,
        (s * 0.012).max(0.5),
        rgba(255, 240, 215, 0.25),
    );
    // Satin varnish sheen.
    let sheen = linear(
        0.0,
        0.0,
        0.0,
        s,
        &[
            (0.0, white(0.28)),
            (0.18, white(0.06)),
            (0.5, white(0.0)),
            (0.62, white(0.08)),
            (1.0, black(0.18)),
        ],
    );
    ctx.fill_rect(0.0, 0.0, s, s, sheen);
    ctx.restore();

    // Rounded-over edges.
    ctx.set_line_width((s * 0.03).max(1.0));
    let h = ctx.line_width() / 2.0;
    inner_outline(ctx, gap, w, radius, base.shade(-0.6));
    ctx.set_line_width((s * 0.022).max(1.0));
    ctx.round_rect(
        gap + h * 2.4,
        gap + h * 2.4,
        w - h * 4.8,
        w - h * 4.8,
        radius * 0.7,
    );
    ctx.stroke(rgba(255, 236, 205, 0.4));
}

/// A domed steel rivet.
pub fn rivet(ctx: &mut Ctx, x: f32, y: f32, r: f32) {
    ctx.begin_path();
    ctx.arc(x + r * 0.25, y + r * 0.35, r * 1.05, 0.0, TAU, false);
    ctx.fill(black(0.45));
    let g = radial(
        x - r * 0.35,
        y - r * 0.4,
        0.0,
        x,
        y,
        r,
        &[
            (0.0, alpha(Rgb::hex("#ffffff"), 1.0)),
            (0.35, alpha(Rgb::hex("#d5dbe2"), 1.0)),
            (0.8, alpha(Rgb::hex("#7b838e"), 1.0)),
            (1.0, alpha(Rgb::hex("#4a5058"), 1.0)),
        ],
    );
    ctx.begin_path();
    ctx.arc(x, y, r, 0.0, TAU, false);
    ctx.fill(g);
}

/// Iron: a painted, brushed metal plate with a hard bevel and a rivet in each corner.
fn iron(ctx: &mut Ctx, base: Rgb, s: f32) {
    let gap = (s * 0.025).max(1.0);
    let x0 = gap;
    let x1 = s - gap;
    let radius = s * 0.06;
    let bevel = s * 0.1;
    let mut rand = seed(base, 0x1e0);

    ctx.save();
    ctx.round_rect(x0, x0, x1 - x0, x1 - x0, radius);
    ctx.clip();
    let plate = linear(
        0.0,
        0.0,
        s,
        s,
        &[
            (0.0, alpha(base.shade(0.3), 1.0)),
            (0.45, alpha(base, 1.0)),
            (1.0, alpha(base.shade(-0.35), 1.0)),
        ],
    );
    ctx.fill_rect(0.0, 0.0, s, s, plate);
    // Brushed finish: fine horizontal scratches.
    for _ in 0..34 {
        let y = rand.f() * s;
        let x = rand.f() * s * 0.6 - s * 0.1;
        let color = if rand.next() < 0.5 {
            white(0.05 + rand.f() * 0.1)
        } else {
            black(0.05 + rand.f() * 0.1)
        };
        let len = s * (0.4 + rand.f() * 0.6);
        ctx.fill_rect(x, y, len, (s * 0.008).max(0.5), color);
    }
    // Metallic sheen band.
    let sheen = linear(
        0.0,
        0.0,
        s,
        s,
        &[
            (0.2, white(0.0)),
            (0.38, white(0.32)),
            (0.46, white(0.05)),
            (0.6, white(0.14)),
            (0.75, white(0.0)),
        ],
    );
    ctx.fill_rect(0.0, 0.0, s, s, sheen);

    let i0 = x0 + bevel;
    let i1 = x1 - bevel;
    ctx.poly(&[x0, x0, x1, x0, i1, i0, i0, i0], white(0.45)); // top
    ctx.poly(&[x0, x0, i0, i0, i0, i1, x0, x1], white(0.2)); // left
    ctx.poly(&[x1, x0, x1, x1, i1, i1, i1, i0], black(0.25)); // right
    ctx.poly(&[x0, x1, i0, i1, i1, i1, x1, x1], black(0.42)); // bottom
    ctx.restore();

    // Engraved line around the face.
    ctx.set_line_width((s * 0.018).max(1.0));
    ctx.stroke_rect(i0, i0, i1 - i0, i1 - i0, alpha(base.shade(-0.7), 0.55));
    let lw = ctx.line_width();
    ctx.stroke_rect(
        i0 + lw,
        i0 + lw,
        i1 - i0 - lw * 2.0,
        i1 - i0 - lw * 2.0,
        white(0.25),
    );

    let rv = s * 0.215;
    for (x, y) in [(rv, rv), (s - rv, rv), (rv, s - rv), (s - rv, s - rv)] {
        rivet(ctx, x, y, s * 0.055);
    }

    ctx.set_line_width((s * 0.025).max(1.0));
    inner_outline(ctx, x0, x1 - x0, radius, base.shade(-0.65));
}

#[cfg(test)]
mod tests {
    use super::*;

    const KINDS: [BlockKind; 10] = [
        BlockKind::Jewel,
        BlockKind::Cardboard,
        BlockKind::Candy,
        BlockKind::Gingerbread,
        BlockKind::Wood,
        BlockKind::Gem,
        BlockKind::Iron,
        BlockKind::Neon,
        BlockKind::Glass,
        BlockKind::Royal,
    ];

    #[test]
    fn rng_and_hash_match_the_typescript() {
        // FNV-1a of "#ff6f61" and the first xorshift values, as computed by the TS.
        assert_eq!(hash(""), 2166136261);
        assert_eq!(hash("a"), 0xe40c292c);
        let mut r = Rng::new(1);
        assert_eq!(r.next(), 270369.0 / 4294967296.0);
        assert_eq!(Rng::new(0).0, 0x9e3779b9);
    }

    #[test]
    fn sprites_have_the_requested_size() {
        for kind in KINDS {
            for px in [4, 5, 17, 64, 150, 256] {
                let p = block_sprite(kind, Rgb::hex("#ff6f61"), px);
                assert_eq!((p.width(), p.height()), (px, px));
                // Something was drawn in the middle.
                assert!(
                    p.pixel(px / 2, px / 2).unwrap().alpha() > 0,
                    "{kind:?} at {px}px"
                );
            }
        }
    }

    #[test]
    fn round_rect_point_walks_the_outline() {
        let total = round_rect_length(10.0, 10.0, 2.0);
        let [x, y, nx, ny] = round_rect_point(0.0, 0.0, 10.0, 10.0, 2.0, 0.0);
        assert_eq!((x, y, nx, ny), (2.0, 0.0, 0.0, -1.0));
        let [x, y, ..] = round_rect_point(0.0, 0.0, 10.0, 10.0, 2.0, total / 2.0);
        assert!((x - 8.0).abs() < 1e-4 && (y - 10.0).abs() < 1e-4);
    }
}
