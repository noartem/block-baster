use super::{CROSSFADE, burst, caption, headline};
use crate::draw::*;
use fframes::{Duration, FFramesContext, Frame, Overlap, Scene, Svgr, svgr};

/// The game over screen with a new record, and a score counter racing up to it.
#[derive(Debug)]
pub struct Best;

const DURATION: f32 = 4.8;
/// The final score on `screen-gameover.png`.
const SCORE: f32 = 2487.;

impl Scene for Best {
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(DURATION - CROSSFADE)
    }

    fn overlap(&self) -> Overlap {
        Overlap::Previous(CROSSFADE)
    }

    fn render_frame<'a>(&'a self, frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let t = frame.seconds();
        let enter = spring(t, 0.1);
        let phone = phone(
            place(560., lerp(1300., 548., enter) + 8. * (t * 1.5).sin(), PHONE_W, PHONE_H, 1.0, lerp(-12., -4., enter)),
            screen_image("screen-gameover.png"),
            1.,
        );

        let count = ease_out_cubic(prog(t, 0.8, 1.5));
        let score = format!("{}", (SCORE * count).round() as u32);
        let counter_p = clamp01(prog(t, 0.7, 0.3));
        let stamp = spring(t, 2.35);
        let crown = ease_out_back(prog(t, 0.3, 0.5));

        svgr!(
            <g opacity={scene_alpha(t, DURATION)}>
                {phone}
                <g transform={place(1046., 250., 48., 36., 1.9 * crown, -8. * (1. - crown))} opacity={clamp01(crown)}>
                    <path d="M 2 10 L 13 20 L 24 2 L 35 20 L 46 10 L 41 34 H 7 Z" fill="url(#gold-gradient)" stroke="#b07a0c" stroke-width="2" stroke-linejoin="round" />
                </g>
                {headline(t, 0.35, 1000., 420., 120., "#ffffff", "Chase your")}
                {headline(t, 0.55, 1000., 545., 120., "url(#gold-gradient)", "best score.")}
                <g opacity={counter_p}>
                    <text x="1000" y="760" font-family="Baloo 2" font-weight="800" font-size="200" fill="#ffffff" filter="url(#text-shadow)">{score}</text>
                </g>
                {burst(t, 2.35, 1640., 690., 260., 12)}
                <g transform={place(1640., 690., 330., 80., stamp.max(0.) * 1.0, -7.)} opacity={clamp01(stamp * 3.)}>
                    <rect y="7" width="330" height="80" rx="40" fill="#0b1030" opacity={0.35} />
                    <rect width="330" height="80" rx="40" fill="url(#gold-gradient)" />
                    <text x={165.} y="55" text-anchor="middle" font-family="Baloo 2" font-weight="800" font-size="40" fill="#3b2600">"NEW BEST!"</text>
                </g>
                {caption(t, 1.6, 1006., 850., "One more game? Of course.")}
            </g>
        )
    }
}
