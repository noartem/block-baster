//! Drawing helpers shared by the scenes: easing, the game's bevelled blocks, the logo and device
//! mockups. Everything is drawn in a local coordinate system and placed with a transform, so the
//! markup itself stays static and only the transform changes from frame to frame.

use fframes::{Svgr, svgr};
use fframes::usvgr::svgtree::svgrtypes::Transform;
use std::sync::LazyLock;

pub const W: f32 = 1920.;
pub const H: f32 = 1080.;

/// Block colours of the game (src/render/palette.ts).
pub const BLOCK_COLORS: [&str; 7] = ["#ef4b5a", "#f7962f", "#f6cd30", "#4cc85d", "#2fbde8", "#4f6ef2", "#a75ae6"];
pub const RED: usize = 0;
pub const ORANGE: usize = 1;
pub const YELLOW: usize = 2;
pub const GREEN: usize = 3;
pub const CYAN: usize = 4;
pub const BLUE: usize = 5;
pub const PURPLE: usize = 6;

pub const ACCENT: &str = "#ffcf3f";
pub const MUTED: &str = "#b9c3f5";
pub const FONT: &str = "Baloo 2";

// ---------------------------------------------------------------------------------------------
// Timing
// ---------------------------------------------------------------------------------------------

pub fn clamp01(x: f32) -> f32 {
    x.clamp(0., 1.)
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Linear progress of `t` through the window `[start, start + dur]`, clamped to 0..1.
pub fn prog(t: f32, start: f32, dur: f32) -> f32 {
    clamp01((t - start) / dur)
}

pub fn ease_out_cubic(x: f32) -> f32 {
    1. - (1. - clamp01(x)).powi(3)
}

pub fn ease_in_cubic(x: f32) -> f32 {
    clamp01(x).powi(3)
}

pub fn ease_in_out_cubic(x: f32) -> f32 {
    let x = clamp01(x);
    if x < 0.5 { 4. * x * x * x } else { 1. - (-2. * x + 2.).powi(3) / 2. }
}

pub fn ease_out_back(x: f32) -> f32 {
    let x = clamp01(x);
    let c1 = 1.70158;
    let c3 = c1 + 1.;
    1. + c3 * (x - 1.).powi(3) + c1 * (x - 1.).powi(2)
}

/// A damped spring from 0 to 1 that starts at `start` (seconds) and overshoots a little.
pub fn spring(t: f32, start: f32) -> f32 {
    let s = t - start;
    if s <= 0. {
        return 0.;
    }
    let (zeta, omega) = (0.45_f32, 14.0_f32);
    let wd = omega * (1. - zeta * zeta).sqrt();
    1. - (-zeta * omega * s).exp() * ((wd * s).cos() + zeta * omega / wd * (wd * s).sin())
}

/// Opacity of a scene of `dur` seconds that crossfades with its neighbours.
pub fn scene_alpha(t: f32, dur: f32) -> f32 {
    ease_in_out_cubic(prog(t, 0., 0.4)).min(ease_in_out_cubic(1. - prog(t, dur - 0.4, 0.4)))
}

// ---------------------------------------------------------------------------------------------
// Transforms
// ---------------------------------------------------------------------------------------------

/// Places a local box of `w` x `h` units so that its centre lands on (`cx`, `cy`), scaled by
/// `scale` and rotated by `deg` degrees around that centre.
pub fn place(cx: f32, cy: f32, w: f32, h: f32, scale: f32, deg: f32) -> Transform {
    let (sin, cos) = deg.to_radians().sin_cos();
    let (a, b, c, d) = (scale * cos, scale * sin, -scale * sin, scale * cos);
    let e = cx - (a * w / 2. + c * h / 2.);
    let f = cy - (b * w / 2. + d * h / 2.);
    Transform::new(a as f64, b as f64, c as f64, d as f64, e as f64, f as f64)
}

/// Translation followed by a uniform scale around the new origin.
pub fn at(x: f32, y: f32, scale: f32) -> Transform {
    Transform::new(scale as f64, 0., 0., scale as f64, x as f64, y as f64)
}

// ---------------------------------------------------------------------------------------------
// Blocks
// ---------------------------------------------------------------------------------------------

/// The shades used for one bevelled block, matching the game's sprite renderer.
pub struct Shades {
    pub base: String,
    pub top: String,
    pub left: String,
    pub right: String,
    pub bottom: String,
    pub face: String,
    pub edge: String,
}

fn shade(hex: &str, amount: f32) -> String {
    let n = u32::from_str_radix(&hex[1..], 16).unwrap_or(0);
    let target = if amount > 0. { 255. } else { 0. };
    let t = amount.abs();
    let mix = |v: u32| (v as f32 + (target - v as f32) * t).round() as u32;
    format!("#{:02x}{:02x}{:02x}", mix((n >> 16) & 255), mix((n >> 8) & 255), mix(n & 255))
}

pub static SHADES: LazyLock<Vec<Shades>> = LazyLock::new(|| {
    BLOCK_COLORS
        .iter()
        .map(|c| Shades {
            base: (*c).to_owned(),
            top: shade(c, 0.42),
            left: shade(c, 0.2),
            right: shade(c, -0.18),
            bottom: shade(c, -0.32),
            face: shade(c, 0.08),
            edge: shade(c, -0.45),
        })
        .collect()
});

/// One game block in a 100x100 local box (uses the `#block-clip` clip path from the root defs).
pub fn block<'a>(color: usize, transform: Transform, opacity: f32) -> Svgr<'a> {
    let s = &SHADES[color % SHADES.len()];
    svgr!(
        <g transform={transform} opacity={opacity}>
            <g clip-path="url(#block-clip)">
                <rect width="100" height="100" fill={s.base.as_str()} />
                <polygon points="0,0 100,0 85,15 15,15" fill={s.top.as_str()} />
                <polygon points="0,0 15,15 15,85 0,100" fill={s.left.as_str()} />
                <polygon points="100,0 100,100 85,85 85,15" fill={s.right.as_str()} />
                <polygon points="0,100 15,85 85,85 100,100" fill={s.bottom.as_str()} />
                <rect x="15" y="15" width="70" height="70" fill={s.face.as_str()} />
                <ellipse cx="31" cy="28" rx="15" ry="9" fill="#ffffff" opacity={0.4} />
            </g>
            <rect x="1.25" y="1.25" width="97.5" height="97.5" rx="10" fill="none" stroke={s.edge.as_str()} stroke-width="2.5" />
        </g>
    )
}

/// A block of `size` pixels with its centre at (`cx`, `cy`).
pub fn block_at<'a>(color: usize, cx: f32, cy: f32, size: f32, deg: f32, opacity: f32) -> Svgr<'a> {
    block(color, place(cx, cy, 100., 100., size / 100., deg), opacity)
}

// ---------------------------------------------------------------------------------------------
// Logo
// ---------------------------------------------------------------------------------------------

/// Block positions of the app icon (assets/icon.svg) on a 1024 grid: colour and top-left corner.
const LOGO_BLOCKS: [(usize, f32, f32); 7] = [
    (RED, 150.7, 150.7),
    (YELLOW, 396.5, 150.7),
    (PURPLE, 642.3, 150.7),
    (YELLOW, 396.5, 396.5),
    (GREEN, 150.7, 642.3),
    (CYAN, 396.5, 642.3),
    (BLUE, 642.3, 642.3),
];

/// The app icon, assembled from flying blocks. `t` is the time since the assembly started; the
/// blocks land one after another during the first second.
pub fn logo<'a>(transform: Transform, t: f32) -> Svgr<'a> {
    let tile = ease_out_back(prog(t, 0., 0.45));
    let blocks: Vec<Svgr> = LOGO_BLOCKS
        .iter()
        .enumerate()
        .map(|(i, &(color, x, y))| {
            let start = 0.1 + i as f32 * 0.09;
            let p = spring(t, start);
            // Each block drops in from above, slightly rotated, and settles into its cell.
            let from_y = -520. - i as f32 * 40.;
            let dy = from_y * (1. - p);
            let deg = (1. - p) * if i % 2 == 0 { -24. } else { 20. };
            let visible = if t >= start { 1. } else { 0. };
            block(color, place(x + 115.5, y + 115.5 + dy, 100., 100., 2.31, deg), visible)
        })
        .collect();
    svgr!(
        <g transform={transform}>
            <g transform={place(512., 512., 1024., 1024., tile, 0.)} opacity={clamp01(tile * 2.)}>
                <rect y="18" width="1024" height="1024" rx="225" fill="#0e1442" opacity={0.45} />
                <rect width="1024" height="1024" rx="225" fill="url(#icon-bg)" />
                <rect x="113.9" y="113.9" width="796.3" height="796.3" rx="61.4" fill="#1b2556" />
            </g>
            {blocks}
        </g>
    )
}

// ---------------------------------------------------------------------------------------------
// Devices
// ---------------------------------------------------------------------------------------------

/// Local size of the phone mockup: a 390x844 screen inside a 16 unit bezel.
pub const PHONE_W: f32 = 422.;
pub const PHONE_H: f32 = 876.;

/// A phone showing `screen` (an `<image>` filling x=16, y=16, 390x844 in local units).
pub fn phone<'a>(transform: Transform, screen: Svgr<'a>, opacity: f32) -> Svgr<'a> {
    svgr!(
        <g transform={transform} opacity={opacity}>
            <rect x="10" y="44" width="402" height="840" rx="70" fill="#060a24" opacity={0.55} filter="url(#soft-shadow)" />
            <rect x="-4" y="190" width="6" height="62" rx="3" fill="#2a3474" />
            <rect x="-4" y="270" width="6" height="96" rx="3" fill="#2a3474" />
            <rect x="420" y="230" width="6" height="120" rx="3" fill="#2a3474" />
            <rect width="422" height="876" rx="64" fill="#0b1030" />
            <rect x="1.5" y="1.5" width="419" height="873" rx="62.5" fill="none" stroke="#4a58b8" stroke-width="3" />
            <g clip-path="url(#phone-screen)">
                <rect x="16" y="16" width="390" height="844" fill="#2f43ab" />
                {screen}
            </g>
            <circle cx="211" cy="8.5" r="3.5" fill="#1d2552" />
        </g>
    )
}

/// The `<image>` element for a phone screen from the media folder.
pub fn screen_image<'a>(file: &'static str) -> Svgr<'a> {
    svgr!(<image href={file} x="16" y="16" width="390" height="844" preserveAspectRatio="none" />)
}

/// Local size of the browser mockup: a 1100x688 viewport under a 52 unit title bar.
pub const BROWSER_W: f32 = 1100.;
pub const BROWSER_H: f32 = 740.;

/// A desktop browser window showing `desktop.png` at the game's web address.
pub fn browser<'a>(transform: Transform, opacity: f32) -> Svgr<'a> {
    svgr!(
        <g transform={transform} opacity={opacity}>
            <rect x="10" y="40" width="1080" height="720" rx="24" fill="#060a24" opacity={0.5} filter="url(#soft-shadow)" />
            <g clip-path="url(#browser-window)">
                <rect width="1100" height="740" fill="#2f43ab" />
                <image href="desktop.png" y="52" width="1100" height="688" preserveAspectRatio="xMidYMid slice" />
                <rect width="1100" height="52" fill="#141a44" />
                <circle cx="28" cy="26" r="7" fill="#ef4b5a" />
                <circle cx="52" cy="26" r="7" fill="#f6cd30" />
                <circle cx="76" cy="26" r="7" fill="#4cc85d" />
                <rect x="300" y="11" width="500" height="30" rx="15" fill="#232c66" />
                <path d="M 326 24 v -3 a 5 5 0 0 1 10 0 v 3 M 323 24 h 16 v 10 h -16 z" fill="none" stroke="#b9c3f5" stroke-width="2" />
                <text x={550.} y="32.5" text-anchor="middle" font-family="Baloo 2" font-weight="600" font-size="17" fill="#d8defc">"noartem.github.io/block-baster"</text>
            </g>
            <rect x="1" y="1" width="1098" height="738" rx="16" fill="none" stroke="#4a58b8" stroke-width="2" />
        </g>
    )
}

// ---------------------------------------------------------------------------------------------
// Text & badges
// ---------------------------------------------------------------------------------------------

/// A rounded "pill" label centred on (`cx`, `cy`).
pub fn pill<'a>(cx: f32, cy: f32, w: f32, text: &'a str, fill: &'a str, color: &'a str, scale: f32, opacity: f32) -> Svgr<'a> {
    let h = 64.;
    svgr!(
        <g transform={place(cx, cy, w, h, scale, 0.)} opacity={opacity}>
            <rect y="6" width={w} height={h} rx="32" fill="#0b1030" opacity={0.35} />
            <rect width={w} height={h} rx="32" fill={fill} />
            <text x={w / 2.} y="43" text-anchor="middle" font-family="Baloo 2" font-weight="800" font-size="30" fill={color}>{text}</text>
        </g>
    )
}
