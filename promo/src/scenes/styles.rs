use super::{CROSSFADE, burst, caption, headline};
use crate::draw::*;
use fframes::{Duration, FFramesContext, Frame, Overlap, Scene, Svgr, svgr};

/// The six style levels, unlocked by score: the same position in each, side by side.
#[derive(Debug)]
pub struct Styles;

pub const DURATION: f32 = 5.8;

/// Screenshot, name and unlock score of each level shown (src/styles.rs).
const LEVELS: [(&str, &str, &str); 6] = [
    ("screen-style-0.png", "Classic", "Start"),
    ("screen-style-1.png", "Candy", "12k"),
    ("screen-style-2.png", "Gems", "60k"),
    ("screen-style-3.png", "Neon", "130k"),
    ("screen-style-4.png", "Aurora", "200k"),
    ("screen-style-5.png", "Legendary", "350k"),
];

/// When the phones land, one after another.
const FIRST: f32 = 0.5;
const STAGGER: f32 = 0.32;
/// When the top level gets its moment.
const CROWN: f32 = FIRST + 5. * STAGGER + 0.45;

const SCALE: f32 = 0.5;
const ROW_Y: f32 = 615.;
const GAP: f32 = 292.;

impl Scene for Styles {
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(DURATION - CROSSFADE)
    }

    fn overlap(&self) -> Overlap {
        Overlap::Previous(CROSSFADE)
    }

    fn render_frame<'a>(&'a self, frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let t = frame.seconds();
        let crown = spring(t, CROWN);

        let phones: Vec<Svgr> = LEVELS
            .iter()
            .enumerate()
            .map(|(i, &(screen, name, score))| {
                let start = FIRST + i as f32 * STAGGER;
                let p = spring(t, start);
                let visible = if t >= start { 1. } else { 0. };
                let x = W / 2. + (i as f32 - 2.5) * GAP;
                let fan = (i as f32 - 2.5) * 1.6;
                let top = i == LEVELS.len() - 1;
                // The top level lifts and grows a little once every phone has landed.
                let lift = if top { crown * 24. } else { 0. };
                let grow = if top { 1. + crown * 0.1 } else { 1. };
                let y = lerp(1450., ROW_Y, p) - lift + 6. * (t * 1.4 + i as f32).sin();
                let label = ease_out_cubic(prog(t, start + 0.15, 0.4));
                let label_y = ROW_Y + PHONE_H * SCALE / 2. + 58.;
                let score_fill = if top { "url(#gold-gradient)" } else { ACCENT };
                svgr!(
                    <g opacity={visible}>
                        {phone(place(x, y, PHONE_W, PHONE_H, SCALE * grow, lerp(14., fan, p)), screen_image(screen), 1.)}
                        <g opacity={label} transform={at(0., 14. * (1. - label), 1.)}>
                            <text x={x} y={label_y} text-anchor="middle" font-family="Baloo 2" font-weight="800" font-size="40" fill="#ffffff" filter="url(#text-shadow)">{name}</text>
                            <text x={x} y={label_y + 42.} text-anchor="middle" font-family="Baloo 2" font-weight="800" font-size="32" fill={score_fill}>{score}</text>
                        </g>
                    </g>
                )
            })
            .collect();

        let top_x = W / 2. + 2.5 * GAP;
        svgr!(
            <g opacity={scene_alpha(t, DURATION)}>
                <ellipse cx={top_x} cy={ROW_Y} rx="190" ry="300" fill="url(#gold-gradient)" opacity={0.45 * clamp01(crown)} filter="url(#soft-shadow)" />
                {phones}
                {burst(t, CROWN, top_x, ROW_Y - 120., 280., 14)}
                {headline(t, 0.1, 150., 150., 92., "#ffffff", "Level up your look.")}
                {caption(t, 0.35, 156., 214., "Six styles with new blocks, effects, fonts and sounds.")}
            </g>
        )
    }
}
