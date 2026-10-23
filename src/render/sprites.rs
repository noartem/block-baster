//! STUB.
use crate::color::Rgb;
use crate::styles::BlockKind;
use tiny_skia::{Paint, Pixmap, Rect, Transform};

pub fn block_sprite(_kind: BlockKind, base: Rgb, px: u32) -> Pixmap {
    let mut pm = Pixmap::new(px, px).unwrap();
    let mut paint = Paint::default();
    paint.set_color(base.skia(1.0));
    let g = px as f32 * 0.05;
    pm.fill_rect(Rect::from_xywh(g, g, px as f32 - 2.0 * g, px as f32 - 2.0 * g).unwrap(), &paint, Transform::identity(), None);
    pm
}
