use super::{CROSSFADE, caption, exit, headline};
use crate::draw::*;
use fframes::{Duration, FFramesContext, FFramesSyncedVideoFrame, Frame, Overlap, Scene, Svgr, SyncVideoFrameInput, svgr};

/// Finger position on the phone screen (390x844 units) in one frame of the gameplay clip.
#[derive(Debug, Clone, Copy, serde::Deserialize)]
pub struct Touch {
    pub x: f32,
    pub y: f32,
    /// 0 = hovering above the screen, 1 = pressed.
    pub down: f32,
}

/// The real gameplay recording on a phone, with the beats of the clip spelled out next to it.
#[derive(Debug)]
pub struct Gameplay {
    pub touch: Vec<Option<Touch>>,
}

/// When the clip starts playing, relative to the scene (the phone slides in first).
const CLIP_START: f32 = 0.45;
/// Length of `gameplay.mp4` (244 frames at 30 fps).
const CLIP_LENGTH: f32 = 244. / 30.;
const DURATION: f32 = CLIP_START + CLIP_LENGTH;

impl Scene for Gameplay {
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(DURATION - CROSSFADE)
    }

    fn overlap(&self) -> Overlap {
        Overlap::Previous(CROSSFADE)
    }

    fn render_frame<'a>(&'a self, frame: Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let t = frame.seconds();
        let alpha = scene_alpha(t, DURATION);
        // Time inside the gameplay clip.
        let c = t - CLIP_START;

        let screen = match frame.get_synced_video_frame(ctx, "gameplay.mp4", &SyncVideoFrameInput { start_from: CLIP_START, ..Default::default() }) {
            Some(video) => {
                let image = video.into_image();
                svgr!(<image href={image.href()} x="16" y="16" width="390" height="844" preserveAspectRatio="none" />)
            }
            None if c < 1. => screen_image("screen-board.png"),
            None => screen_image("screen-board-clear.png"),
        };

        let touch = self.touch.get((c * 30.).round().max(0.) as usize).copied().flatten().filter(|_| c >= 0.);
        let finger = match touch {
            Some(Touch { x, y, down }) => {
                let r = lerp(46., 34., down);
                svgr!(
                    <g opacity={0.25 + 0.75 * down}>
                        <circle cx={x} cy={y} r={r} fill="#ffffff" opacity="0.28" />
                        <circle cx={x} cy={y} r={r} fill="none" stroke="#ffffff" stroke-width="4" opacity="0.85" />
                    </g>
                )
            }
            None => Svgr::empty(),
        };

        let enter = spring(t, 0.05);
        let phone_y = lerp(1250., 545., enter);
        let tilt = lerp(-8., 0., enter) + 0.6 * (t * 1.3).sin();
        let phone = phone(place(1350., phone_y, PHONE_W, PHONE_H, 1.04, tilt), svgr!(<g>{screen}{finger}</g>), 1.);

        // Text beats follow the clip: first drop at 1.6 s, combo at 3.7 s, board clear at 6.8 s.
        let drag_drop_blast = exit(
            c,
            3.25,
            svgr!(
                <g>
                    {headline(c, 0.5, 200., 380., 150., "#ffffff", "Drag.")}
                    {headline(c, 1.45, 200., 530., 150., "#ffffff", "Drop.")}
                    {headline(c, 1.75, 200., 680., 150., "url(#gold-gradient)", "Blast!")}
                    {caption(c, 2.1, 206., 780., "Fill a row or a column to clear it.")}
                </g>
            ),
        );
        let combos = if c > 3.4 {
            exit(
                c,
                5.65,
                svgr!(
                    <g>
                        {headline(c, 3.5, 200., 430., 150., "#ffffff", "Chain")}
                        {headline(c, 3.65, 200., 580., 150., "url(#gold-gradient)", "combos.")}
                        {caption(c, 4.0, 206., 680., "Clear lines move after move to")}
                        {caption(c, 4.1, 206., 730., "multiply your points.")}
                    </g>
                ),
            )
        } else {
            Svgr::empty()
        };
        let board_clear = if c > 5.8 {
            let badge = spring(c, 6.85);
            svgr!(
                <g>
                    {headline(c, 5.85, 200., 430., 128., "#ffffff", "Clear the")}
                    {headline(c, 6.0, 200., 570., 128., "url(#gold-gradient)", "whole board.")}
                    {pill(380., 690., 360., "+300 BONUS", "#7ee8ff", "#0b1f4a", badge.max(0.), clamp01(badge * 3.))}
                </g>
            )
        } else {
            Svgr::empty()
        };

        svgr!(
            <g opacity={alpha}>
                {phone}
                {drag_drop_blast}
                {combos}
                {board_clear}
            </g>
        )
    }
}
