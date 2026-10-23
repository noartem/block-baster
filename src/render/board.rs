//! STUB.
use crate::styles::Style;
use tiny_skia::{Paint, Pixmap, Rect, Transform};

pub fn board_pad(cell: f32) -> f32 {
    (cell * 0.18).round()
}

pub fn board_margin(cell: f32) -> f32 {
    (cell * 0.6).round()
}

pub fn board_background(style: &Style, cell: f32, board_size: f32, dpr: f32) -> Pixmap {
    let pad = board_pad(cell);
    let m = board_margin(cell);
    let size = board_size + pad * 2.0;
    let px = ((size + m * 2.0) * dpr).round() as u32;
    let mut pm = Pixmap::new(px, px).unwrap();
    let mut paint = Paint::default();
    paint.set_color(style.palette.board_edge.skia(1.0));
    pm.fill_rect(Rect::from_xywh(m * dpr, m * dpr, size * dpr, size * dpr).unwrap(), &paint, Transform::identity(), None);
    paint.set_color(style.palette.cell.skia(1.0));
    for r in 0..8 {
        for c in 0..8 {
            let x = (m + pad + c as f32 * cell + 2.0) * dpr;
            let y = (m + pad + r as f32 * cell + 2.0) * dpr;
            pm.fill_rect(Rect::from_xywh(x, y, (cell - 4.0) * dpr, (cell - 4.0) * dpr).unwrap(), &paint, Transform::identity(), None);
        }
    }
    pm
}
