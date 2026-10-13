use super::burst;
use crate::draw::*;
use fframes::{Duration, FFramesContext, Frame, Scene, Svgr, svgr};

/// The logo assembles from falling blocks, then the name and the promise appear next to it.
#[derive(Debug)]
pub struct Intro;

const DURATION: f32 = 4.4;

impl Scene for Intro {
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(DURATION)
    }

    fn render_frame<'a>(&'a self, frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let t = frame.seconds();
        let out = 1. - ease_in_out_cubic(prog(t, DURATION - 0.4, 0.4));

        // The logo lands in the middle, then makes room for the title.
        let side = ease_in_out_cubic(prog(t, 1.35, 0.7));
        let logo_x = lerp(960., 445., side);
        let logo_size = lerp(400., 300., side);
        let logo = logo(place(logo_x, 540., 1024., 1024., logo_size / 1024., 0.), t);

        let title_p = ease_out_cubic(prog(t, 1.7, 0.6));
        let tag_p = ease_out_cubic(prog(t, 2.25, 0.6));

        svgr!(
            <g opacity={out} transform={place(960., 540., 1920., 1080., lerp(1., 1.04, prog(t, 0., DURATION)), 0.)}>
                {burst(t, 0.95, 960., 540., 420., 14)}
                {logo}
                <g opacity={title_p} transform={at(635. + 40. * (1. - title_p), 560., 1.)}>
                    <text x={0.} y="0" font-family="Baloo 2" font-weight="800" font-size="168" fill="#ffffff" filter="url(#text-shadow)">"Block Baster"</text>
                </g>
                <g opacity={tag_p} transform={at(640., 650. + 20. * (1. - tag_p), 1.)}>
                    <text x={0.} y="0" font-family="Baloo 2" font-weight="700" font-size="52" fill={MUTED}>"The block puzzle "<tspan fill={ACCENT}>"without the ads."</tspan></text>
                </g>
            </g>
        )
    }
}
