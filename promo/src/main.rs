//! Block Baster promo video, rendered with fframes (https://fframes.studio).
//!
//! Every screen shown in the video is a real screenshot or a real gameplay recording of the game,
//! captured by `promo/capture/capture.mjs` into `promo/media`.
//!
//! Render: `cargo run --release -- render -o block-baster-promo.mp4`
//! Preview a frame: `cargo run --release -- frame 12s`

mod draw;
mod scenes;

use draw::*;
use fframes::{
    AudioMap, AudioTimestamp, AudioTrack, Color, Duration, EncoderOptions, FFramesContext, Frame, MediaDirectory, RenderOptions, Scene,
    Scenes, Svgr, Video, svgr,
};
use scenes::{Anywhere, Best, Features, Gameplay, Intro, Outro, Styles, Touch};
use std::path::Path;
use std::process::ExitCode;

struct Promo {
    intro: Intro,
    gameplay: Gameplay,
    styles: Styles,
    features: Features,
    best: Best,
    anywhere: Anywhere,
    outro: Outro,
}

impl Video for Promo {
    const FPS: usize = 30;
    const WIDTH: usize = 1920;
    const HEIGHT: usize = 1080;
    const BACKGROUND_COLOR: Color = Color::BLACK;

    fn duration(&self) -> Duration<'_> {
        Duration::Auto
    }

    fn define_scenes(&self) -> Scenes<'_> {
        let scenes: Vec<&dyn Scene> = vec![&self.intro, &self.gameplay, &self.styles, &self.features, &self.best, &self.anywhere, &self.outro];
        Scenes::from(scenes)
    }

    fn audio(&self) -> AudioMap<'_> {
        AudioMap::from([AudioTrack::new("music.wav", AudioTimestamp::Second(0.)..AudioTimestamp::Eof).fade_out(1.5)])
    }

    fn render_frame<'a>(&'a self, frame: Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let t = frame.seconds();
        svgr!(
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1920 1080" width={Self::WIDTH} height={Self::HEIGHT}>
                <defs>
                    <linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="0" stop-color="#1fa2a0" />
                        <stop offset="0.5" stop-color="#137a80" />
                        <stop offset="1" stop-color="#0a4250" />
                    </linearGradient>
                    <radialGradient id="glow" cx="0.5" cy="0.5" r="0.5">
                        <stop offset="0" stop-color="#6fe3d6" stop-opacity="0.55" />
                        <stop offset="1" stop-color="#6fe3d6" stop-opacity="0" />
                    </radialGradient>
                    <radialGradient id="vignette" cx="0.5" cy="0.5" r="0.75">
                        <stop offset="0.6" stop-color="#031c22" stop-opacity="0" />
                        <stop offset="1" stop-color="#031c22" stop-opacity="0.55" />
                    </radialGradient>
                    <linearGradient id="icon-bg" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="0" stop-color="#22b0ac" />
                        <stop offset="1" stop-color="#0b4f5c" />
                    </linearGradient>
                    <linearGradient id="gold-gradient" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="0" stop-color="#ffd27a" />
                        <stop offset="1" stop-color="#ff9f1c" />
                    </linearGradient>
                    <filter id="soft-shadow" x="-30%" y="-30%" width="160%" height="160%">
                        <feGaussianBlur stdDeviation="28" />
                    </filter>
                    <filter id="text-shadow" x="-10%" y="-30%" width="120%" height="160%">
                        <feDropShadow dx="0" dy="6" stdDeviation="0" flood-color="#031c22" flood-opacity="0.35" />
                    </filter>
                    <clipPath id="block-clip">
                        <rect width="100" height="100" rx="10" />
                    </clipPath>
                    <clipPath id="phone-screen">
                        <rect x="16" y="16" width="390" height="844" rx="48" />
                    </clipPath>
                    <clipPath id="browser-window">
                        <rect width="1100" height="740" rx="16" />
                    </clipPath>
                </defs>
                <rect width="1920" height="1080" fill="url(#bg)" />
                <ellipse cx={960. + 260. * (t * 0.35).sin()} cy="380" rx="900" ry="520" fill="url(#glow)" />
                {background_blocks(t)}
                {ctx.render_scenes(&frame)}
                <rect width="1920" height="1080" fill="url(#vignette)" />
            </svg>
        )
    }
}

/// Large, faint blocks drifting upwards behind everything.
fn background_blocks<'a>(t: f32) -> Svgr<'a> {
    const COUNT: usize = 16;
    (0..COUNT)
        .map(|i| {
            // Deterministic pseudo-random layout.
            let r = |k: u32| (((i as u32 * 2654435761 ^ k.wrapping_mul(40503)) % 1000) as f32) / 1000.;
            let size = 70. + r(1) * 110.;
            let speed = 22. + r(2) * 40.;
            let span = H + 400.;
            let y = (r(3) * span - t * speed).rem_euclid(span) - 200.;
            let x = (i as f32 + 0.5) / COUNT as f32 * W + (r(4) - 0.5) * 120.;
            let deg = (r(5) - 0.5) * 50. + t * (r(6) - 0.5) * 20.;
            block_at(i % 7, x, y, size, deg, 0.10 + r(7) * 0.08)
        })
        .collect()
}

fn load_touch(path: &Path) -> Vec<Option<Touch>> {
    #[derive(serde::Deserialize)]
    struct File {
        width: f32,
        height: f32,
        frames: Vec<Option<Touch>>,
    }
    let file: File = serde_json::from_str(&std::fs::read_to_string(path).expect("gameplay-touch.json")).expect("valid touch data");
    // Normalise to the phone's 390x844 screen units.
    file.frames
        .into_iter()
        .map(|t| t.map(|t| Touch { x: t.x / file.width * 390., y: t.y / file.height * 844., down: t.down }))
        .collect()
}

fn main() -> ExitCode {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let media_dir = MediaDirectory::read_folder(root.join("media")).expect("promo/media folder");
    let media = media_dir.process_media_source().expect("media files");

    let video = Promo {
        intro: Intro,
        gameplay: Gameplay { touch: load_touch(&root.join("media/gameplay-touch.json")) },
        styles: Styles,
        features: Features,
        best: Best,
        anywhere: Anywhere,
        outro: Outro,
    };

    let options = RenderOptions {
        media: Some(&media),
        default_font: FONT,
        video_encoder_options: EncoderOptions {
            preferred_encoder: Some("libx264"),
            codec_params: Some(&[("crf", "17"), ("preset", "slow"), ("tune", "animation")]),
            ..Default::default()
        },
        ..Default::default()
    };
    fframes::cli::new(&video, options).default_output(root.join("block-baster-promo.mp4")).run()
}
