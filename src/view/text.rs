//! Bundled fonts and text helpers.

use femtovg::{Align, Baseline, Color, FontId, Paint};

use super::Canvas;
use crate::styles::Font;

static BALOO_600: &[u8] = include_bytes!("../../assets/fonts/Baloo2-SemiBold.ttf");
static BALOO_800: &[u8] = include_bytes!("../../assets/fonts/Baloo2-ExtraBold.ttf");
static LILITA: &[u8] = include_bytes!("../../assets/fonts/LilitaOne-Regular.ttf");
static RIGHTEOUS: &[u8] = include_bytes!("../../assets/fonts/Righteous-Regular.ttf");
static ORBITRON: &[u8] = include_bytes!("../../assets/fonts/Orbitron-Black.ttf");
static CINZEL: &[u8] = include_bytes!("../../assets/fonts/CinzelDecorative-Black.ttf");

#[derive(Clone, Copy, Debug)]
pub struct Fonts {
    /// Body text (Baloo 2 SemiBold).
    pub regular: FontId,
    /// Bold UI text (Baloo 2 ExtraBold).
    pub bold: FontId,
    lilita: FontId,
    righteous: FontId,
    orbitron: FontId,
    cinzel: FontId,
}

impl Fonts {
    pub fn load(c: &mut Canvas) -> Self {
        let mut add = |data: &'static [u8]| c.add_font_mem(data).expect("bundled font loads");
        Fonts {
            regular: add(BALOO_600),
            bold: add(BALOO_800),
            lilita: add(LILITA),
            righteous: add(RIGHTEOUS),
            orbitron: add(ORBITRON),
            cinzel: add(CINZEL),
        }
    }

    /// A style's display font, falling back to Baloo 2 for missing glyphs.
    pub fn display(&self, font: Font) -> [FontId; 2] {
        let id = match font {
            Font::Baloo => self.bold,
            Font::Lilita => self.lilita,
            Font::Righteous => self.righteous,
            Font::Orbitron => self.orbitron,
            Font::Cinzel => self.cinzel,
        };
        [id, self.bold]
    }
}

/// A text paint: fonts, size, horizontal alignment, vertically centred.
pub fn paint(fonts: &[FontId], size: f32, align: Align, color: Color) -> Paint {
    styled(Paint::color(color), fonts, size, align)
}

/// Gives any paint (a colour or a gradient) the text settings.
pub fn styled(mut p: Paint, fonts: &[FontId], size: f32, align: Align) -> Paint {
    p.set_font(fonts);
    p.set_font_size(size.max(1.0));
    p.set_text_align(align);
    p.set_text_baseline(Baseline::Middle);
    p
}

pub fn width(c: &Canvas, text: &str, paint: &Paint) -> f32 {
    c.measure_text(0.0, 0.0, text, paint).map_or(0.0, |m| m.width())
}

pub fn fill(c: &mut Canvas, x: f32, y: f32, text: &str, paint: &Paint) {
    let _ = c.fill_text(x, y, text, paint);
}

pub fn stroke(c: &mut Canvas, x: f32, y: f32, text: &str, paint: &Paint) {
    let _ = c.stroke_text(x, y, text, paint);
}

/// Text with a soft drop shadow straight below it (like CSS `text-shadow: 0 Npx 0 rgba(0,0,0,a)`).
pub fn fill_shadowed(c: &mut Canvas, x: f32, y: f32, text: &str, paint: &Paint, offset: f32, alpha: f32) {
    let mut shadow = paint.clone();
    shadow.set_color(Color::rgbaf(0.0, 0.0, 0.0, alpha));
    fill(c, x, y + offset, text, &shadow);
    fill(c, x, y, text, paint);
}

/// Splits text into lines that fit `max_width`.
pub fn wrap(c: &Canvas, text: &str, max_width: f32, paint: &Paint) -> Vec<String> {
    let mut lines = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            let candidate = if line.is_empty() { word.to_owned() } else { format!("{line} {word}") };
            if !line.is_empty() && width(c, &candidate, paint) > max_width {
                lines.push(std::mem::take(&mut line));
                line = word.to_owned();
            } else {
                line = candidate;
            }
        }
        lines.push(line);
    }
    lines
}

/// Fills text centred at the origin with a gradient. femtovg maps gradients on text per glyph,
/// so the text is drawn solid and the gradient laid over it with `SourceIn`; this only works on
/// a render target holding nothing else (see TextCache). `w`×`h` must cover the text.
pub fn fill_gradient(c: &mut Canvas, text: &str, paint: &Paint, gradient: &Paint, w: f32, h: f32) {
    let mut solid = paint.clone();
    solid.set_color(Color::white());
    fill(c, 0.0, 0.0, text, &solid);
    c.global_composite_operation(femtovg::CompositeOperation::SourceIn);
    c.fill_path(&super::draw::rect(-w / 2.0, -h / 2.0, w, h), gradient);
    c.global_composite_operation(femtovg::CompositeOperation::SourceOver);
}
