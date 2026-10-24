//! An sRGB colour plus the shading helpers the styles use.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

const fn hex_digit(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => panic!("bad hex digit"),
    }
}

impl Rgb {
    pub const WHITE: Rgb = Rgb::new(255, 255, 255);
    pub const BLACK: Rgb = Rgb::new(0, 0, 0);

    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Rgb { r, g, b }
    }

    /// Parses "#rrggbb" (usable in constants).
    pub const fn hex(s: &str) -> Self {
        let b = s.as_bytes();
        assert!(b.len() == 7 && b[0] == b'#');
        Rgb {
            r: hex_digit(b[1]) * 16 + hex_digit(b[2]),
            g: hex_digit(b[3]) * 16 + hex_digit(b[4]),
            b: hex_digit(b[5]) * 16 + hex_digit(b[6]),
        }
    }

    /// Mixes the colour towards white (amount > 0) or black (amount < 0).
    pub fn shade(self, amount: f32) -> Rgb {
        let target = if amount > 0.0 { 255.0 } else { 0.0 };
        let t = amount.abs();
        let mix = |v: u8| {
            let v = v as f32;
            // Math.round in JS rounds .5 up.
            (v + (target - v) * t + 0.5).floor().clamp(0.0, 255.0) as u8
        };
        Rgb::new(mix(self.r), mix(self.g), mix(self.b))
    }

    /// Linear interpolation between two colours (0 = self, 1 = other).
    pub fn lerp(self, other: Rgb, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Rgb::new(mix(self.r, other.r), mix(self.g, other.g), mix(self.b, other.b))
    }

    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// Colour with alpha for femtovg.
    pub fn alpha(self, a: f32) -> femtovg::Color {
        femtovg::Color::rgbaf(self.r as f32 / 255.0, self.g as f32 / 255.0, self.b as f32 / 255.0, a.clamp(0.0, 1.0))
    }

    pub fn fv(self) -> femtovg::Color {
        femtovg::Color::rgb(self.r, self.g, self.b)
    }

    /// Colour with alpha for tiny-skia.
    pub fn skia(self, a: f32) -> tiny_skia::Color {
        tiny_skia::Color::from_rgba8(self.r, self.g, self.b, (a.clamp(0.0, 1.0) * 255.0).round() as u8)
    }
}

/// White with the given alpha, for femtovg.
pub fn white(a: f32) -> femtovg::Color {
    Rgb::WHITE.alpha(a)
}

/// Black with the given alpha, for femtovg.
pub fn black(a: f32) -> femtovg::Color {
    Rgb::BLACK.alpha(a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_shades() {
        let c = Rgb::hex("#ff6f61");
        assert_eq!(c, Rgb::new(255, 111, 97));
        assert_eq!(c.to_hex(), "#ff6f61");
        assert_eq!(Rgb::hex("#808080").shade(0.5), Rgb::new(192, 192, 192));
        assert_eq!(Rgb::hex("#808080").shade(-0.5), Rgb::new(64, 64, 64));
        assert_eq!(Rgb::hex("#123456").shade(0.0), Rgb::hex("#123456"));
    }
}
