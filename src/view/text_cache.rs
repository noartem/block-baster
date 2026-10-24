//! Glowing and outlined text, rendered once into a texture and reused.
//!
//! Live shadows in femtovg blur an offscreen copy of the shape on every draw,
//! which is far too slow for text that is on screen every frame (the score) or
//! animates for seconds (banners). Each distinct text is drawn once here, at a
//! little more than its largest on-screen size, and then drawn as an image.

use std::collections::HashMap;

use femtovg::{Color, ImageFlags, ImageId, Paint, PixelFormat, RenderTarget};

use super::Canvas;
use super::draw::rect;

struct Entry {
    image: ImageId,
    /// Image size in pixels.
    width: usize,
    height: usize,
    /// Image pixels per logical pixel.
    scale: f32,
    last_used: u64,
}

#[derive(Default)]
pub struct TextCache {
    entries: HashMap<String, Entry>,
    frame: u64,
}

/// Unused entries are dropped after this many frames.
const KEEP_FRAMES: u64 = 90;

impl TextCache {
    /// Frees textures that weren't drawn lately; call once per frame.
    pub fn begin_frame(&mut self, c: &mut Canvas) {
        self.frame += 1;
        let frame = self.frame;
        self.entries.retain(|_, e| {
            let keep = frame - e.last_used <= KEEP_FRAMES;
            if !keep {
                c.delete_image(e.image);
            }
            keep
        });
    }

    pub fn clear(&mut self, c: &mut Canvas) {
        for (_, e) in self.entries.drain() {
            c.delete_image(e.image);
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Draws a cached text image centred at the origin of the current transform. `key` must
    /// describe everything that changes the look; on a miss, `draw` renders the text centred
    /// at (0, 0) of a `width`×`height` logical box (already transformed into the image) at
    /// `scale` image pixels per logical pixel.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&mut self, c: &mut Canvas, key: &str, width: f32, height: f32, scale: f32, alpha: f32, draw: impl FnOnce(&mut Canvas)) {
        if !self.entries.contains_key(key) {
            let (w, h) = ((width * scale).ceil().max(1.0) as usize, (height * scale).ceil().max(1.0) as usize);
            let Ok(image) = c.create_image_empty(w, h, PixelFormat::Rgba8, ImageFlags::PREMULTIPLIED | ImageFlags::FLIP_Y) else { return };
            c.save();
            c.set_render_target(RenderTarget::Image(image));
            c.clear_rect(0, 0, w as u32, h as u32, Color::rgbaf(0.0, 0.0, 0.0, 0.0));
            c.reset_transform();
            c.reset_scissor();
            c.set_global_alpha(1.0);
            c.translate(w as f32 / 2.0, h as f32 / 2.0);
            c.scale(scale, scale);
            draw(c);
            c.restore();
            c.set_render_target(RenderTarget::Screen);
            self.entries.insert(key.to_owned(), Entry { image, width: w, height: h, scale, last_used: self.frame });
        }
        let Some(e) = self.entries.get_mut(key) else { return };
        e.last_used = self.frame;
        let (w, h) = (e.width as f32 / e.scale, e.height as f32 / e.scale);
        c.fill_path(&rect(-w / 2.0, -h / 2.0, w, h), &Paint::image(e.image, -w / 2.0, -h / 2.0, w, h, 0.0, alpha));
    }
}
