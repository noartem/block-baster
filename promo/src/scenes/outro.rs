use super::{CROSSFADE, burst};
use crate::draw::*;
use fframes::{Duration, FFramesContext, Frame, Overlap, Scene, Svgr, svgr};

/// Logo, name and where to get the game.
#[derive(Debug)]
pub struct Outro;

const DURATION: f32 = 5.6;

impl Scene for Outro {
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(DURATION - CROSSFADE)
    }

    fn overlap(&self) -> Overlap {
        Overlap::Previous(CROSSFADE)
    }

    fn render_frame<'a>(&'a self, frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let t = frame.seconds();
        // The end card stays on screen until the last frame.
        let fade_in = ease_in_out_cubic(prog(t, 0., CROSSFADE));

        let title = ease_out_cubic(prog(t, 0.85, 0.5));
        let tag = ease_out_cubic(prog(t, 1.15, 0.5));
        let cta = ease_out_back(prog(t, 1.55, 0.5));
        let link = ease_out_cubic(prog(t, 1.9, 0.5));
        // A gentle pulse draws the eye to the call to action once everything is in.
        let pulse = 1. + 0.025 * ((t - 2.2).max(0.) * 4.).sin();

        svgr!(
            <g opacity={fade_in}>
                {burst(t, 0.95, 960., 300., 520., 16)}
                {logo(place(960., 300., 1024., 1024., 260. / 1024., 0.), t)}
                <g opacity={title} transform={at(960., 560. + 30. * (1. - title), 1.)}>
                    <text x={0.} y="0" text-anchor="middle" font-family="Baloo 2" font-weight="800" font-size="150" fill="#ffffff" filter="url(#text-shadow)">"Block Baster"</text>
                </g>
                <g opacity={tag} transform={at(960., 640. + 20. * (1. - tag), 1.)}>
                    <text x={0.} y="0" text-anchor="middle" font-family="Baloo 2" font-weight="700" font-size="50" fill={MUTED}>"Free. No ads. Open source."</text>
                </g>
                <g transform={place(960., 770., 760., 96., cta.max(0.) * pulse, 0.)} opacity={clamp01(cta * 2.)}>
                    <rect y="9" width="760" height="96" rx="48" fill="#0b1030" opacity={0.35} />
                    <rect width="760" height="96" rx="48" fill="url(#gold-gradient)" />
                    <path d="M 56 30 L 86 48 L 56 66 Z" fill="#3b2600" stroke="#3b2600" stroke-width="6" stroke-linejoin="round" />
                    <text x={410.} y="63" text-anchor="middle" font-family="Baloo 2" font-weight="800" font-size="44" fill="#3b2600">"noartem.github.io/block-baster"</text>
                </g>
                <g opacity={link} transform={at(960., 890. + 16. * (1. - link), 1.)}>
                    <text x={0.} y="0" text-anchor="middle" font-family="Baloo 2" font-weight="600" font-size="36" fill={MUTED}>"Android APK & source: "<tspan fill="#ffffff" font-weight="700">"github.com/noartem/block-baster"</tspan></text>
                </g>
            </g>
        )
    }
}
