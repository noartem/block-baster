use super::{CROSSFADE, headline};
use crate::draw::*;
use fframes::{Duration, FFramesContext, Frame, Overlap, Scene, Svgr, svgr};

/// The same game in a desktop browser and on a phone.
#[derive(Debug)]
pub struct Anywhere;

const DURATION: f32 = 5.0;

const CHIPS: [&str; 4] = ["Any browser", "Installable PWA", "Android APK", "Works offline"];

impl Scene for Anywhere {
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(DURATION - CROSSFADE)
    }

    fn overlap(&self) -> Overlap {
        Overlap::Previous(CROSSFADE)
    }

    fn render_frame<'a>(&'a self, frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let t = frame.seconds();
        // A slow push-in over the whole scene.
        let zoom = lerp(1., 1.035, ease_in_out_cubic(prog(t, 0., DURATION)));

        let b = ease_out_cubic(prog(t, 0.1, 0.8));
        let browser = browser(place(lerp(420., 760., b), 610., BROWSER_W, BROWSER_H, 0.95, 0.), b);
        let p = spring(t, 0.55);
        let phone = phone(place(1330., lerp(1400., 655., p), PHONE_W, PHONE_H, 0.8, lerp(10., 3., p)), screen_image("screen-board.png"), 1.);

        let chips: Vec<Svgr> = CHIPS
            .iter()
            .enumerate()
            .map(|(i, text)| {
                let c = ease_out_back(prog(t, 1.2 + i as f32 * 0.18, 0.45));
                let fill = if i == 0 { "url(#gold-gradient)" } else { "#ffffff" };
                let color = if i == 0 { "#3b2600" } else { "#137a80" };
                pill(1712., 420. + i as f32 * 92., 300., text, fill, color, c, clamp01(c * 2.))
            })
            .collect();

        svgr!(
            <g opacity={scene_alpha(t, DURATION)}>
                <g transform={place(960., 540., 1920., 1080., zoom, 0.)}>
                    {browser}
                    {phone}
                    {chips}
                </g>
                {headline(t, 0.15, 237., 175., 96., "#ffffff", "Play anywhere.")}
            </g>
        )
    }
}
