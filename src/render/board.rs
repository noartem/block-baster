//! The static board image: frame, surface and the empty cells. It is drawn
//! once per board size, pixel ratio and style, and cached by the renderer.

use tiny_skia::Pixmap;

use super::frames::{FrameBox, draw_material_frame, is_material_frame};
use super::skia::{Ctx, alpha, even_stops, js_round, linear, white};
use crate::game::board::SIZE;
use crate::styles::{FrameKind, Style};

/// Extra room around the board for frames and glows, in CSS pixels.
pub fn board_margin(cell: f32) -> f32 {
    js_round(cell * 0.6)
}

/// Padding between the frame's inner edge and the cells, in CSS pixels.
pub fn board_pad(cell: f32) -> f32 {
    js_round(cell * 0.18)
}

/// Side of the board background image in device pixels.
pub fn board_background_px(cell: f32, board_size: f32, dpr: f32) -> u32 {
    let size = board_size + board_pad(cell) * 2.0;
    js_round((size + board_margin(cell) * 2.0) * dpr).max(1.0) as u32
}

/// Renders the board background. The image covers the board plus `board_pad`
/// and `board_margin` on every side: draw it at `board_x - pad - margin`,
/// `board_size + 2 * (pad + margin)` CSS pixels wide.
pub fn board_background(style: &Style, cell: f32, board_size: f32, dpr: f32) -> Pixmap {
    let (p, tier) = (style.palette, style.tier);
    let m = board_margin(cell);
    let pad = board_pad(cell);
    let size = board_size + pad * 2.0;
    let px = board_background_px(cell, board_size, dpr);
    let mut g = Ctx::new(px, px);
    g.scale(dpr, dpr);
    g.translate(m, m);
    let frame = tier.frame;
    let outer_r = cell * if frame == FrameKind::Soft { 0.45 } else { 0.3 };
    let inner_r = cell * if frame == FrameKind::Soft { 0.38 } else { 0.25 };

    // Outer frame.
    g.save();
    if matches!(frame, FrameKind::Neon | FrameKind::Glass | FrameKind::Gold) {
        let color = if frame == FrameKind::Gold {
            alpha(p.glow, 0.8)
        } else {
            alpha(p.glow, 1.0)
        };
        g.set_shadow(
            color,
            cell * if frame == FrameKind::Glass { 0.6 } else { 0.45 },
        );
    }
    let fill = if matches!(frame, FrameKind::Metal | FrameKind::Gold) {
        let stops = if frame == FrameKind::Gold {
            even_stops(&["#fff3b0", "#e2b13c", "#8a5d0a", "#f5d36b", "#6b4500"])
        } else {
            even_stops(&["#f2f4ff", "#9aa3c0", "#4b5272", "#c9cfe6", "#3a3f58"])
        };
        linear(0.0, 0.0, size, size, &stops)
    } else if frame == FrameKind::Glass {
        tiny_skia::Shader::SolidColor(alpha(p.board_edge, 0.75))
    } else {
        tiny_skia::Shader::SolidColor(alpha(p.board_edge, 1.0))
    };
    g.round_rect(0.0, 0.0, size, size, outer_r);
    g.fill(fill);
    g.restore();
    if frame == FrameKind::Neon {
        g.save();
        g.set_shadow(alpha(p.glow, 1.0), cell * 0.3);
        g.set_line_width((cell * 0.06).max(2.0));
        let lw = g.line_width();
        g.round_rect(lw / 2.0, lw / 2.0, size - lw, size - lw, outer_r);
        g.stroke(p.glow);
        g.restore();
    }

    // Board surface.
    let material = is_material_frame(frame);
    let inset = if material {
        pad * 0.8
    } else if matches!(frame, FrameKind::Metal | FrameKind::Gold) {
        pad * 0.5
    } else {
        pad * 0.35
    };
    g.round_rect(
        inset,
        inset,
        size - inset * 2.0,
        size - inset * 2.0,
        inner_r,
    );
    if frame == FrameKind::Glass {
        g.fill(alpha(p.board, 0.82));
    } else {
        g.fill(p.board);
    }
    // Cookie, cardboard, iron and wood frames are drawn over the plain one.
    if material {
        draw_material_frame(
            &mut g,
            frame,
            &FrameBox {
                size,
                inset,
                cell,
                outer_r,
                inner_r,
                palette: p,
            },
        );
    }
    if frame == FrameKind::Glass {
        g.set_line_width((cell * 0.03).max(1.0));
        g.stroke(white(0.35));
    }

    // Cells.
    let ci = (cell * 0.04).max(1.0);
    for r in 0..SIZE {
        for col in 0..SIZE {
            let x = pad + col as f32 * cell + ci;
            let y = pad + r as f32 * cell + ci;
            let w = cell - ci * 2.0;
            if frame == FrameKind::Neon {
                g.set_line_width((cell * 0.025).max(1.0));
                g.round_rect(x + ci, y + ci, w - ci * 2.0, w - ci * 2.0, cell * 0.1);
                g.fill(p.cell_inner);
                g.stroke(alpha(p.glow, 0.16));
                continue;
            }
            let cr = cell * if frame == FrameKind::Soft { 0.2 } else { 0.1 };
            g.round_rect(x, y, w, w, cr);
            g.fill(p.cell);
            g.round_rect(x + ci, y + ci * 2.0, w - ci * 2.0, w - ci * 2.0, cr * 0.8);
            g.fill(p.cell_inner);
        }
    }
    g.into_pixmap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::styles::TIERS;

    #[test]
    fn board_background_size_matches_the_formula() {
        let style = Style {
            tier: &TIERS[0],
            palette: &TIERS[0].variants[0],
            variant: 0,
        };
        let (cell, dpr) = (48.0, 2.0);
        let board = cell * SIZE as f32;
        assert_eq!(board_pad(cell), 9.0);
        assert_eq!(board_margin(cell), 29.0);
        let img = board_background(&style, cell, board, dpr);
        let expected = ((board + 2.0 * (9.0 + 29.0)) * dpr) as u32;
        assert_eq!((img.width(), img.height()), (expected, expected));
        // The corners of the margin stay transparent, the board middle is opaque.
        assert_eq!(img.pixel(0, 0).unwrap().alpha(), 0);
        assert_eq!(img.pixel(expected / 2, expected / 2).unwrap().alpha(), 255);
    }

    #[test]
    fn every_style_renders() {
        for tier in TIERS.iter() {
            for (variant, palette) in tier.variants.iter().enumerate() {
                let style = Style {
                    tier,
                    palette,
                    variant,
                };
                let img = board_background(&style, 20.0, 160.0, 1.0);
                assert_eq!(img.width(), board_background_px(20.0, 160.0, 1.0));
            }
        }
    }
}
