mod anywhere;
mod best;
mod features;
mod gameplay;
mod intro;
mod outro;

pub use anywhere::Anywhere;
pub use best::Best;
pub use features::Features;
pub use gameplay::{Gameplay, Touch};
pub use intro::Intro;
pub use outro::Outro;

use crate::draw::*;
use fframes::{Svgr, svgr};

/// Every scene after the first crossfades with the previous one for this long.
///
/// fframes starts an overlapping scene this much earlier, so a scene that should be on screen for
/// `DURATION` seconds (including its crossfade) declares `DURATION - CROSSFADE`.
pub const CROSSFADE: f32 = 0.4;

/// A headline line that slides in from the left at `start`.
pub fn headline<'a>(t: f32, start: f32, x: f32, y: f32, size: f32, fill: &'a str, text: &'a str) -> Svgr<'a> {
    let p = ease_out_cubic(prog(t, start, 0.5));
    let pop = ease_out_back(prog(t, start, 0.45));
    let scale = lerp(0.92, 1., pop);
    svgr!(
        <g transform={at(x - 50. * (1. - p), y, scale)} opacity={p}>
            <text x="0" y="0" font-family="Baloo 2" font-weight="800" font-size={size} fill={fill} filter="url(#text-shadow)">{text}</text>
        </g>
    )
}

/// A muted caption line that fades up at `start`.
pub fn caption<'a>(t: f32, start: f32, x: f32, y: f32, text: &'a str) -> Svgr<'a> {
    let p = ease_out_cubic(prog(t, start, 0.5));
    svgr!(
        <text x={x} y={y + 16. * (1. - p)} opacity={p} font-family="Baloo 2" font-weight="600" font-size="40" fill={MUTED}>{text}</text>
    )
}

/// A group of elements that leaves (slides left and fades) between `start` and `start + 0.35`.
pub fn exit<'a>(t: f32, start: f32, content: Svgr<'a>) -> Svgr<'a> {
    let p = ease_in_cubic(prog(t, start, 0.35));
    if p >= 1. {
        return Svgr::empty();
    }
    svgr!(<g transform={at(-90. * p, 0., 1.)} opacity={1. - p}>{content}</g>)
}

/// A small burst of blocks flying out from (`cx`, `cy`), starting at `start`.
pub fn burst<'a>(t: f32, start: f32, cx: f32, cy: f32, radius: f32, count: usize) -> Svgr<'a> {
    let s = t - start;
    if !(0. ..1.2).contains(&s) {
        return Svgr::empty();
    }
    (0..count)
        .map(|i| {
            let angle = i as f32 / count as f32 * std::f32::consts::TAU + 0.35 * (i % 3) as f32;
            let speed = radius * (0.75 + 0.5 * ((i * 7 % 5) as f32 / 5.));
            let d = ease_out_cubic(s / 1.2) * speed;
            let x = cx + angle.cos() * d;
            let y = cy + angle.sin() * d + 120. * s * s;
            let size = 26. + (i * 13 % 4) as f32 * 8.;
            block_at(i % 7, x, y, size * (1. - s / 1.6), s * 240. * if i % 2 == 0 { 1. } else { -1. }, 1. - prog(s, 0.6, 0.6))
        })
        .collect()
}
