use super::{CROSSFADE, headline};
use crate::draw::*;
use fframes::{Duration, FFramesContext, Frame, Overlap, Scene, Svgr, svgr};

/// What the game does not do (and a few things it does), next to its own pause menu.
#[derive(Debug)]
pub struct Features;

const DURATION: f32 = 6.6;
const CARD_W: f32 = 480.;

enum Icon {
    NoAds,
    NoTracking,
    Offline,
    Fair,
    Save,
    Code,
}

const CARDS: [(usize, Icon, &str, &str); 6] = [
    (CORAL, Icon::NoAds, "No ads. Ever.", "Not a single banner or video."),
    (MAGENTA, Icon::NoTracking, "No tracking", "No accounts, no analytics."),
    (SKY, Icon::Offline, "Plays offline", "No network needed at all."),
    (LIME, Icon::Fair, "Fair dealing", "Every hand can be placed."),
    (TANGERINE, Icon::Save, "Auto-save", "Pick up where you left off."),
    (GRAPE, Icon::Code, "Open source", "MIT licensed on GitHub."),
];

/// White line icon in a 48x48 box.
fn icon<'a>(icon: &Icon) -> Svgr<'a> {
    match icon {
        Icon::NoAds => svgr!(
            <g opacity={1.}>
                <rect x="5" y="11" width="38" height="26" rx="6" fill="none" stroke="#ffffff" stroke-width="4" />
                <text x={24.} y="31" text-anchor="middle" font-family="Baloo 2" font-weight="800" font-size="17" fill="#ffffff">"AD"</text>
                <path d="M 6 42 L 42 6" stroke-width="9" stroke-linecap="round" stroke="#000000" stroke-opacity="0.25" />
                <path d="M 6 42 L 42 6" stroke="#ffffff" stroke-width="4.5" stroke-linecap="round" />
            </g>
        ),
        Icon::NoTracking => svgr!(
            <g opacity={1.} fill="none" stroke="#ffffff" stroke-width="4" stroke-linecap="round" stroke-linejoin="round">
                <path d="M 3 24 C 11 12 37 12 45 24 C 37 36 11 36 3 24 Z" />
                <circle cx="24" cy="24" r="6" />
                <path d="M 7 42 L 41 6" stroke-width="4.5" />
            </g>
        ),
        Icon::Offline => svgr!(
            <g opacity={1.} fill="none" stroke="#ffffff" stroke-width="4" stroke-linecap="round">
                <path d="M 6 19 C 16 9 32 9 42 19" />
                <path d="M 12 26 C 19 19 29 19 36 26" />
                <path d="M 18 33 C 21 30 27 30 30 33" />
                <circle cx="24" cy="39" r="2.5" fill="#ffffff" />
                <path d="M 7 43 L 41 5" stroke-width="4.5" />
            </g>
        ),
        Icon::Fair => svgr!(
            <g opacity={1.} fill="none" stroke="#ffffff" stroke-width="5" stroke-linecap="round" stroke-linejoin="round">
                <path d="M 10 25 L 20 35 L 39 14" />
            </g>
        ),
        Icon::Save => svgr!(
            <g opacity={1.} fill="none" stroke="#ffffff" stroke-width="4" stroke-linejoin="round">
                <path d="M 8 6 H 34 L 42 14 V 42 H 8 Z" />
                <path d="M 15 6 V 16 H 31 V 6" />
                <rect x="15" y="27" width="20" height="15" />
            </g>
        ),
        Icon::Code => svgr!(
            <g opacity={1.} fill="none" stroke="#ffffff" stroke-width="4.5" stroke-linecap="round" stroke-linejoin="round">
                <path d="M 15 12 L 4 24 L 15 36" />
                <path d="M 33 12 L 44 24 L 33 36" />
                <path d="M 28 8 L 20 40" />
            </g>
        ),
    }
}

fn card<'a>(t: f32, i: usize) -> Svgr<'a> {
    let (color, ref kind, title, subtitle) = CARDS[i];
    let start = 0.75 + i as f32 * 0.2;
    let p = ease_out_back(prog(t, start, 0.5));
    let fade = clamp01(prog(t, start, 0.25));
    let (col, row) = (i % 2, i / 2);
    let (x, y) = (150. + col as f32 * (CARD_W + 22.), 392. + row as f32 * 172.);
    svgr!(
        <g transform={place(x + CARD_W / 2., y + 74. + 30. * (1. - p), CARD_W, 148., lerp(0.85, 1., p), 0.)} opacity={fade}>
            <rect width={CARD_W} height="148" rx="26" fill="#ffffff" opacity={0.09} />
            <rect x="1" y="1" width={CARD_W - 2.} height="146" rx="25" fill="none" stroke="#ffffff" stroke-opacity="0.16" stroke-width="2" />
            {block_at(color, 74., 74., 92., 0., 1.)}
            <g transform={at(50., 50., 1.)}>{icon(kind)}</g>
            <text x="142" y="66" font-family="Baloo 2" font-weight="800" font-size="38" fill="#ffffff">{title}</text>
            <text x="142" y="106" font-family="Baloo 2" font-weight="600" font-size="25" fill={MUTED}>{subtitle}</text>
        </g>
    )
}

impl Scene for Features {
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(DURATION - CROSSFADE)
    }

    fn overlap(&self) -> Overlap {
        Overlap::Previous(CROSSFADE)
    }

    fn render_frame<'a>(&'a self, frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let t = frame.seconds();
        let enter = spring(t, 0.25);
        let phone = phone(
            place(lerp(2150., 1490., enter), 560. + 10. * (t * 1.6).sin(), PHONE_W, PHONE_H, 0.98, lerp(14., 4., enter)),
            screen_image("screen-menu.png"),
            1.,
        );
        let cards: Vec<Svgr> = (0..CARDS.len()).map(|i| card(t, i)).collect();
        svgr!(
            <g opacity={scene_alpha(t, DURATION)}>
                {phone}
                {headline(t, 0.15, 150., 225., 96., "#ffffff", "Everything you want.")}
                {headline(t, 0.4, 150., 330., 96., "url(#gold-gradient)", "Nothing you don't.")}
                {cards}
            </g>
        )
    }
}
