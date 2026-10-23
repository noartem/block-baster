//! Board frames made of a material (cookie, cardboard, iron, wood). They are drawn
//! as a band around the board surface, reaching slightly into the margin.

use std::f32::consts::{PI, TAU};

use tiny_skia::{FillRule, LineCap, LineJoin};

use super::skia::{Ctx, alpha, black, even_stops, linear, radial, rgba, white};
use super::sprites::{Rng, rivet, round_rect_length, round_rect_point, wavy_round_rect};
use crate::color::Rgb;
use crate::styles::{FrameKind, Palette};

pub struct FrameBox<'a> {
    /// Outer size of the framed board in CSS pixels; the frame starts at 0.
    pub size: f32,
    /// Where the board surface starts.
    pub inset: f32,
    pub cell: f32,
    pub outer_r: f32,
    pub inner_r: f32,
    pub palette: &'a Palette,
}

struct Band<'a> {
    size: f32,
    inset: f32,
    cell: f32,
    outer_r: f32,
    inner_r: f32,
    palette: &'a Palette,
    ext: f32,
    band: f32,
    mid: f32,
}

/// The outer edge of the frame, as a new path.
fn outer_path(g: &mut Ctx, b: &Band, shrink: f32) {
    g.begin_path();
    g.add_round_rect(
        -b.ext + shrink,
        -b.ext + shrink,
        b.size + b.ext * 2.0 - shrink * 2.0,
        b.size + b.ext * 2.0 - shrink * 2.0,
        b.outer_r + b.ext,
    );
}

/// The path of the frame band, between the outer edge and the board surface.
fn band_path(g: &mut Ctx, b: &Band) {
    g.begin_path();
    g.add_round_rect(
        -b.ext,
        -b.ext,
        b.size + b.ext * 2.0,
        b.size + b.ext * 2.0,
        b.outer_r + b.ext,
    );
    g.add_round_rect(
        b.inset,
        b.inset,
        b.size - b.inset * 2.0,
        b.size - b.inset * 2.0,
        b.inner_r,
    );
}

fn surface_path(g: &mut Ctx, b: &Band) {
    g.begin_path();
    g.add_round_rect(
        b.inset,
        b.inset,
        b.size - b.inset * 2.0,
        b.size - b.inset * 2.0,
        b.inner_r,
    );
}

/// Points spread along the middle of the band, with their outward normals.
fn along_band(b: &Band, count: usize, offset: f32) -> Vec<[f32; 4]> {
    let m = b.mid;
    let w = b.size - m * 2.0;
    let r = (b.outer_r - (m + b.ext)).max(1.0);
    let total = round_rect_length(w, w, r);
    (0..count)
        .map(|i| round_rect_point(m, m, w, w, r, ((i as f32 + offset) / count as f32) * total))
        .collect()
}

/// Dark line where the board surface sinks into the frame.
fn inner_shadow(g: &mut Ctx, b: &Band, a: f32) {
    g.save();
    g.set_line_width((b.cell * 0.035).max(1.0));
    surface_path(g, b);
    g.stroke(black(a));
    g.restore();
}

fn hex(s: &str) -> tiny_skia::Color {
    alpha(Rgb::hex(s), 1.0)
}

/// A gingerbread cookie rim with wavy white icing and candy buttons.
fn gingerbread(g: &mut Ctx, b: &Band) {
    let (size, cell, p) = (b.size, b.cell, b.palette);
    g.save();
    band_path(g, b);
    let dough = linear(
        0.0,
        -b.ext,
        0.0,
        size + b.ext,
        &[
            (0.0, hex("#d48c4a")),
            (0.5, hex("#b06630")),
            (1.0, hex("#84441b")),
        ],
    );
    g.fill_rule(dough, FillRule::EvenOdd);
    g.clip_rule(FillRule::EvenOdd);
    let mut rand = Rng::new(0x91a9);
    for _ in 0..120 {
        let color = if rand.next() < 0.75 {
            rgba(92, 40, 10, 0.4)
        } else {
            rgba(255, 220, 170, 0.35)
        };
        let [x, y, ..] = along_band(b, 1, rand.f())[0];
        let cx = x + (rand.f() - 0.5) * b.band;
        let cy = y + (rand.f() - 0.5) * b.band;
        let r = cell * (0.008 + rand.f() * 0.01);
        g.begin_path();
        g.arc(cx, cy, r, 0.0, TAU, false);
        g.fill(color);
    }
    g.restore();

    g.set_line_width((cell * 0.03).max(1.0));
    outer_path(g, b, 0.0);
    g.stroke("#5a2a0c");

    // Piped icing along the middle of the rim.
    let m = b.mid;
    let r = (b.outer_r - (m + b.ext)).max(1.0);
    g.set_line_cap(LineCap::Round);
    g.set_line_join(LineJoin::Round);
    g.set_line_width((b.band * 0.26).max(1.5));
    let lw = g.line_width();
    for (color, dy) in [(rgba(70, 25, 0, 0.35), lw * 0.45), (hex("#fffaf2"), 0.0)] {
        g.save();
        g.translate(0.0, dy);
        wavy_round_rect(
            g,
            m,
            m,
            size - m * 2.0,
            size - m * 2.0,
            r,
            b.band * 0.2,
            cell * 0.32,
        );
        g.stroke(color);
        g.restore();
    }

    // Candy buttons at the corners and the middle of each side, in the block colours.
    let s = size - m * 2.0;
    let side = s - 2.0 * r;
    let arc = (PI * r) / 2.0;
    for i in 0..8 {
        let k = (i / 2) as f32;
        // Even: middle of a side; odd: middle of the following corner.
        let d = k * (side + arc)
            + if i % 2 == 1 {
                side + arc / 2.0
            } else {
                side / 2.0
            };
        let [x, y, ..] = round_rect_point(m, m, s, s, r, d);
        let color = p.blocks[(i * 3) % p.blocks.len()];
        let cr = b.band * 0.42;
        g.begin_path();
        g.arc(x + cr * 0.15, y + cr * 0.3, cr, 0.0, TAU, false);
        g.fill(rgba(60, 20, 0, 0.4));
        let candy = radial(
            x - cr * 0.35,
            y - cr * 0.4,
            0.0,
            x,
            y,
            cr,
            &[
                (0.0, hex("#ffffff")),
                (0.35, alpha(color, 1.0)),
                (1.0, alpha(color.shade(-0.35), 1.0)),
            ],
        );
        g.begin_path();
        g.arc(x, y, cr, 0.0, TAU, false);
        g.fill(candy);
    }
    inner_shadow(g, b, 0.45);
}

/// The cut edge of a cardboard box, sealed with packing tape, around a corrugated surface.
fn cardboard(g: &mut Ctx, b: &Band) {
    let (size, cell, inset) = (b.size, b.cell, b.inset);
    // Corrugated flutes showing on the board surface between the cells.
    g.save();
    surface_path(g, b);
    g.clip();
    let mut x = inset as f64;
    while x < size as f64 {
        let xf = x as f32;
        g.fill_rect(xf, 0.0, cell * 0.05, size, black(0.12));
        g.fill_rect(
            xf + cell * 0.06,
            0.0,
            cell * 0.025,
            size,
            rgba(255, 230, 190, 0.04),
        );
        x += cell as f64 * 0.12;
    }
    g.restore();

    g.save();
    band_path(g, b);
    let kraft = linear(
        0.0,
        -b.ext,
        size,
        size + b.ext,
        &[
            (0.0, hex("#d9ae74")),
            (0.5, hex("#c39158")),
            (1.0, hex("#a87841")),
        ],
    );
    g.fill_rule(kraft, FillRule::EvenOdd);
    g.clip_rule(FillRule::EvenOdd);
    // Fluting visible in the cut edge: short ribs across the band.
    g.set_line_width((cell * 0.022).max(1.0));
    let total = ((4.0 * size) / (cell * 0.11)).round() as usize;
    for [x, y, nx, ny] in along_band(b, total, 0.0) {
        g.begin_path();
        g.move_to(x - nx * b.band * 0.22, y - ny * b.band * 0.22);
        g.line_to(x + nx * b.band * 0.22, y + ny * b.band * 0.22);
        g.stroke(rgba(110, 70, 25, 0.35));
    }
    g.restore();

    // Fold lines on both sides of the edge.
    g.set_line_width((cell * 0.025).max(1.0));
    let lw = g.line_width();
    outer_path(g, b, lw);
    g.stroke(rgba(255, 236, 200, 0.6));
    outer_path(g, b, 0.0);
    g.stroke("#6b4520");
    inner_shadow(g, b, 0.5);

    // Packing tape over the middle of the top and bottom edges and two corners.
    let tape = |g: &mut Ctx, x: f32, y: f32, angle: f32, len: f32, th: f32| {
        g.save();
        g.translate(x, y);
        g.rotate(angle);
        g.begin_path();
        // Torn, zigzag ends.
        let teeth = 5;
        g.move_to(-len / 2.0, -th / 2.0);
        g.line_to(len / 2.0, -th / 2.0);
        for i in 1..=teeth {
            g.line_to(
                len / 2.0 + if i % 2 == 1 { cell * 0.04 } else { 0.0 },
                -th / 2.0 + (th * i as f32) / teeth as f32,
            );
        }
        g.line_to(-len / 2.0, th / 2.0);
        for i in (0..teeth).rev() {
            g.line_to(
                -len / 2.0 - if i % 2 == 1 { cell * 0.04 } else { 0.0 },
                -th / 2.0 + (th * i as f32) / teeth as f32,
            );
        }
        g.close_path();
        g.fill(rgba(236, 214, 160, 0.78));
        g.fill_rect(-len / 2.0, -th / 2.0, len, th * 0.25, white(0.3));
        g.restore();
    };
    let len = b.band + cell * 0.18;
    let c = (inset - b.ext) / 2.0;
    // The tape folds over the edges of the box, so only the part on top of it shows.
    g.save();
    outer_path(g, b, 0.0);
    g.clip();
    tape(g, size / 2.0, c, PI / 2.0, len, cell * 0.5);
    tape(g, size / 2.0, size - c, PI / 2.0, len, cell * 0.5);
    // Corner pieces run across the corner, from one side of the box to the other.
    let k = cell * 0.13;
    tape(g, k, k, -PI / 4.0, cell, cell * 0.26);
    tape(g, size - k, size - k, -PI / 4.0, cell, cell * 0.26);
    g.restore();
}

/// A riveted, brushed iron frame.
fn iron(g: &mut Ctx, b: &Band) {
    let (size, cell) = (b.size, b.cell);
    g.save();
    band_path(g, b);
    let steel = linear(
        0.0,
        -b.ext,
        size,
        size + b.ext,
        &even_stops(&["#c3c9d1", "#737a84", "#3d424a", "#9aa1aa", "#2c3036"]),
    );
    g.fill_rule(steel, FillRule::EvenOdd);
    g.clip_rule(FillRule::EvenOdd);
    let mut rand = Rng::new(0x1205);
    for _ in 0..160 {
        let y = -b.ext + rand.f() * (size + b.ext * 2.0);
        let x = -b.ext + rand.f() * size;
        let color = if rand.next() < 0.5 {
            white(0.05 + rand.f() * 0.1)
        } else {
            black(0.06 + rand.f() * 0.12)
        };
        let w = cell * (0.6 + rand.f() * 1.4);
        g.fill_rect(x, y, w, (cell * 0.01).max(0.5), color);
    }
    g.restore();

    // Bevelled edges.
    g.set_line_width((cell * 0.025).max(1.0));
    let lw = g.line_width();
    outer_path(g, b, lw);
    g.stroke(white(0.45));
    outer_path(g, b, 0.0);
    g.stroke("#16181c");
    inner_shadow(g, b, 0.75);

    // A rivet roughly every cell along the frame.
    let count = ((4.0 * size) / cell).round().max(8.0) as usize;
    for [x, y, ..] in along_band(b, count - (count % 4), 0.5) {
        rivet(g, x, y, b.band * 0.24);
    }
}

/// Four mitred planks of wood, nailed at the corners.
fn wood(g: &mut Ctx, b: &Band) {
    let (size, cell, inset, ext) = (b.size, b.cell, b.inset, b.ext);
    let o0 = -ext;
    let o1 = size + ext;
    let i0 = inset;
    let i1 = size - inset;
    // (points, runs along x, tone)
    let planks: [([f32; 8], bool, f32); 4] = [
        ([o0, o0, o1, o0, i1, i0, i0, i0], true, 0.12), // top
        ([o1, o0, o1, o1, i1, i1, i1, i0], false, -0.05), // right
        ([o0, o1, i0, i1, i1, i1, o1, o1], true, -0.15), // bottom
        ([o0, o0, i0, i0, i0, i1, o0, o1], false, 0.04), // left
    ];
    let mut rand = Rng::new(0x00d);
    g.save();
    band_path(g, b);
    g.clip_rule(FillRule::EvenOdd);
    for (pts, along_x, tone) in planks {
        g.save();
        g.polygon(&pts);
        let stops = [(0.0, hex("#b97a42")), (1.0, hex("#94592b"))];
        let across = if along_x {
            linear(0.0, o0, 0.0, i0, &stops)
        } else {
            linear(o0, 0.0, i0, 0.0, &stops)
        };
        let light = if tone > 0.0 {
            rgba(255, 235, 200, tone)
        } else {
            rgba(40, 15, 0, -tone)
        };
        g.fill(across);
        g.fill(light);
        g.clip();
        // Grain running along the plank.
        for i in 0..6 {
            let off = o0 + ((i as f32 + 0.5) / 6.0) * b.band + (rand.f() - 0.5) * b.band * 0.1;
            let amp = b.band * (0.04 + rand.f() * 0.08);
            let freq = (PI * 2.0) / (cell * (1.5 + rand.f() * 2.0));
            let phase = rand.f() * 6.0;
            let color = rgba(80, 38, 10, 0.25 + rand.f() * 0.25);
            g.set_line_width((cell * (0.008 + rand.f() * 0.012)).max(0.6));
            for side in [off, size - off] {
                g.begin_path();
                let mut t = o0 as f64;
                while t <= o1 as f64 {
                    let tf = t as f32;
                    let wob = (tf * freq + phase).sin() * amp;
                    if along_x {
                        g.line_to(tf, side + wob);
                    } else {
                        g.line_to(side + wob, tf);
                    }
                    t += cell as f64 * 0.2;
                }
                g.stroke(color);
            }
        }
        g.restore();
    }
    // Mitre joints.
    g.set_line_width((cell * 0.022).max(1.0));
    g.begin_path();
    for [x0, y0, x1, y1] in [
        [o0, o0, i0, i0],
        [o1, o0, i1, i0],
        [o1, o1, i1, i1],
        [o0, o1, i0, i1],
    ] {
        g.move_to(x0, y0);
        g.line_to(x1, y1);
    }
    g.stroke(rgba(50, 22, 4, 0.7));
    g.restore();

    g.set_line_width((cell * 0.022).max(1.0));
    g.begin_path();
    g.add_round_rect(o0, o0, o1 - o0, o1 - o0, b.outer_r + ext);
    g.stroke("#3d1d06");
    let lw = g.line_width();
    g.begin_path();
    g.add_round_rect(
        o0 + lw,
        o0 + lw,
        o1 - o0 - lw * 2.0,
        o1 - o0 - lw * 2.0,
        b.outer_r + ext,
    );
    g.stroke(rgba(255, 230, 190, 0.35));
    inner_shadow(g, b, 0.6);

    // Nail heads either side of each joint.
    let n = b.mid;
    let k = cell * 0.32;
    for (x, y) in [
        (n + k, n),
        (n, n + k),
        (size - n - k, n),
        (size - n, n + k),
        (n + k, size - n),
        (n, size - n - k),
        (size - n - k, size - n),
        (size - n, size - n - k),
    ] {
        let r = (b.band * 0.13).max(1.0);
        g.begin_path();
        g.arc(x + r * 0.3, y + r * 0.4, r, 0.0, TAU, false);
        g.fill(rgba(255, 230, 190, 0.35));
        let head = radial(
            x - r * 0.3,
            y - r * 0.3,
            0.0,
            x,
            y,
            r,
            &[(0.0, hex("#c9c4bc")), (1.0, hex("#3a332c"))],
        );
        g.begin_path();
        g.arc(x, y, r, 0.0, TAU, false);
        g.fill(head);
    }
}

/// True for frames drawn by `draw_material_frame`; they need a wider band around the board.
pub fn is_material_frame(kind: FrameKind) -> bool {
    matches!(
        kind,
        FrameKind::Gingerbread | FrameKind::Cardboard | FrameKind::Iron | FrameKind::Wood
    )
}

/// Draws a material frame over the plain one, after the board surface has been filled.
pub fn draw_material_frame(g: &mut Ctx, kind: FrameKind, frame: &FrameBox) {
    let ext = frame.cell * 0.06;
    let b = Band {
        size: frame.size,
        inset: frame.inset,
        cell: frame.cell,
        outer_r: frame.outer_r,
        inner_r: frame.inner_r,
        palette: frame.palette,
        ext,
        band: frame.inset + ext,
        mid: (frame.inset - ext) / 2.0,
    };
    match kind {
        FrameKind::Gingerbread => gingerbread(g, &b),
        FrameKind::Cardboard => cardboard(g, &b),
        FrameKind::Iron => iron(g, &b),
        FrameKind::Wood => wood(g, &b),
        _ => {}
    }
}
