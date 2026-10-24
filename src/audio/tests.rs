use super::engine::{Engine, MAX_VOICES};
use super::intensity::{BPM_MIN, IntensityTracker};
use super::*;
use crate::styles::{TIERS, default_sound_profile};

const SR: f32 = 44100.0;

/// Renders `seconds` of stereo output.
fn render(e: &mut Engine, seconds: f64) -> Vec<f32> {
    let mut out = vec![0.0; (seconds * SR as f64) as usize * 2];
    // Odd chunk sizes, like a real device.
    for chunk in out.chunks_mut(441 * 2) {
        e.render(chunk, 2);
    }
    out
}

fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|v| v * v).sum::<f32>() / x.len().max(1) as f32).sqrt()
}

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0f32, |m, v| m.max(v.abs()))
}

fn music_at(e_level: f32, seconds: f64) -> (Engine, Vec<f32>) {
    let mut e = Engine::new(SR, 7);
    let mut t = IntensityTracker::new();
    e.set_music_state(t.set(e_level, e_level));
    e.start_music(1.0, false);
    let out = render(&mut e, seconds);
    (e, out)
}

#[test]
fn music_is_finite_audible_and_bounded_from_lofi_to_edm() {
    for level in [0.0, 0.5, 1.0] {
        let (e, out) = music_at(level, 6.0);
        assert!(out.iter().all(|v| v.is_finite()), "e={level}: not finite");
        assert!(peak(&out) <= 1.0, "e={level}: out of range");
        // After the 1.5 s fade-in.
        let r = rms(&out[(2.0 * SR as f64) as usize * 2..]);
        assert!(r > 0.02, "e={level}: too quiet ({r})");
        assert!(r < 0.4, "e={level}: too loud ({r})");
        assert!(e.active_voices() <= MAX_VOICES);
        assert!(e.active_voices() > 0);
    }
}

#[test]
fn edm_is_faster_and_louder_than_lofi() {
    let (lofi, a) = music_at(0.0, 6.0);
    let (edm, b) = music_at(1.0, 6.0);
    assert_eq!(lofi.music_state().bpm, BPM_MIN);
    assert_eq!(edm.music_state().bpm, intensity::BPM_MAX);
    assert!(rms(&b[4 * SR as usize..]) > rms(&a[4 * SR as usize..]));
}

#[test]
fn every_effect_sounds_and_then_decays_to_silence() {
    let sfx = [
        Sfx::Pickup,
        Sfx::Place,
        Sfx::Invalid,
        Sfx::Clear { lines: 1, combo: 1 },
        Sfx::Clear { lines: 4, combo: 0 },
        Sfx::Clear { lines: 6, combo: 9 },
        Sfx::BoardClear,
        Sfx::StyleShift,
        Sfx::TierUp(1),
        Sfx::TierUp(7),
        Sfx::TierUp(8),
        Sfx::TierUp(9),
        Sfx::GameOver,
    ];
    for tier in &TIERS {
        for &s in &sfx {
            let mut e = Engine::new(SR, 1);
            e.play(s, &tier.sound);
            let out = render(&mut e, 2.5);
            assert!(out.iter().all(|v| v.is_finite()));
            assert!(peak(&out) > 0.01, "{s:?} in {}: silent", tier.name);
            assert!(peak(&out) <= 1.0);
            // The voices end and are freed...
            assert_eq!(e.active_voices(), 0, "{s:?} in {}: voices left", tier.name);
            // ...and once the echo dies away, all is quiet.
            let tail = render(&mut e, 1.5);
            assert!(
                peak(&tail[tail.len() - 2000..]) < 1e-4,
                "{s:?} in {}: still ringing",
                tier.name
            );
        }
    }
}

#[test]
fn the_voice_pool_is_capped_and_empties() {
    let mut e = Engine::new(SR, 1);
    let p = default_sound_profile();
    for _ in 0..20 {
        e.play(Sfx::TierUp(9), &p);
    }
    assert!(e.active_voices() <= MAX_VOICES);
    render(&mut e, 3.0);
    assert_eq!(e.active_voices(), 0);
}

#[test]
fn stopping_the_music_fades_out_and_frees_everything() {
    let (mut e, _) = music_at(1.0, 3.0);
    e.stop_music();
    let out = render(&mut e, 1.0);
    assert!(!e.music_active());
    assert_eq!(e.active_voices(), 0);
    assert!(peak(&out[out.len() / 2..]) < 1e-3);
    // And it starts again.
    e.start_music(1.0, false);
    let out = render(&mut e, 3.0);
    assert!(rms(&out[2 * SR as usize..]) > 0.02);
}

#[test]
fn restarting_while_fading_out_keeps_playing() {
    let (mut e, _) = music_at(0.3, 3.0);
    e.stop_music();
    render(&mut e, 0.2);
    e.start_music(1.0, false);
    let out = render(&mut e, 3.0);
    assert!(e.music_active());
    assert!(rms(&out[2 * SR as usize..]) > 0.02);
}

#[test]
fn the_muffle_darkens_the_music() {
    let (mut e, _) = music_at(1.0, 2.0);
    let open = render(&mut e, 2.0);
    e.set_music_paused(true);
    render(&mut e, 0.5);
    let muffled = render(&mut e, 2.0);
    // Roughness: energy of the sample-to-sample difference, relative to the level.
    let bright = |x: &[f32]| {
        let d: Vec<f32> = x
            .chunks(2)
            .collect::<Vec<_>>()
            .windows(2)
            .map(|w| w[1][0] - w[0][0])
            .collect();
        rms(&d) / rms(x)
    };
    assert!(bright(&muffled) < bright(&open) * 0.5);
}

#[test]
fn renders_at_other_rates_and_channel_counts() {
    for (sr, channels) in [(48000.0, 2), (22050.0, 1), (96000.0, 4)] {
        let mut e = Engine::new(sr, 3);
        e.set_music_state(IntensityTracker::new().set(1.0, 1.0));
        e.start_music(1.0, false);
        e.play(Sfx::Clear { lines: 3, combo: 2 }, &TIERS[9].sound);
        let mut out = vec![0.0; sr as usize * 3 * channels];
        e.render(&mut out, channels);
        assert!(out.iter().all(|v| v.is_finite() && v.abs() <= 1.0));
        assert!(rms(&out) > 0.01, "{sr} Hz, {channels} channels");
        if channels == 4 {
            assert!(out.chunks(4).all(|f| f[2] == 0.0 && f[3] == 0.0));
        }
    }
}

#[test]
fn audio_without_a_device_tracks_the_intensity() {
    let mut a = Audio::new();
    assert!(!a.music_playing());
    // Not unlocked: effects are ignored, nothing opens.
    a.play(Sfx::Place);
    a.start_music();
    a.start_music();
    assert!(a.music_playing());
    assert!(a.music_state().bpm < 80.0);
    let mut now = 1000.0;
    a.update(now, 0.0, 0.0);
    for _ in 0..(15_000 / 16) {
        now += 16.0;
        a.update(now, 1.0, 0.0);
    }
    assert!(a.music_state().bpm > 100.0, "bpm {}", a.music_state().bpm);
    a.stop_music();
    assert!(!a.music_playing());
    // Stopped: the tracker holds still.
    let bpm = a.music_state().bpm;
    a.update(now + 10_000.0, 0.0, 0.0);
    assert_eq!(a.music_state().bpm, bpm);
}

#[test]
fn volumes_are_clamped() {
    let mut a = Audio::new();
    a.set_sound_volume(2.0);
    a.set_music_volume(-1.0);
    assert_eq!(a.sound_volume(), 1.0);
    assert_eq!(a.music_volume(), 0.0);
    a.set_profile(TIERS[3].sound);
    a.suspend();
    a.resume();
}

#[test]
fn starting_again_while_playing_changes_nothing() {
    let (mut e, out) = music_at(1.0, 3.0);
    let before = rms(&out[2 * SR as usize..]);
    e.start_music(1.0, false);
    // No new fade-in from silence.
    let after = render(&mut e, 0.5);
    assert!(rms(&after) > before * 0.5);
}

#[test]
fn the_engine_follows_the_music_state() {
    let mut e = Engine::new(SR, 1);
    let mut t = IntensityTracker::new();
    let s = t.set(0.8, 0.5);
    e.set_music_state(s);
    assert_eq!(e.music_state(), s);
    assert!(s.bpm > BPM_MIN);
}

#[test]
fn a_silent_volume_is_silent() {
    let mut e = Engine::new(SR, 1);
    e.init_sound_volume(0.0);
    e.play(Sfx::GameOver, &default_sound_profile());
    let out = render(&mut e, 1.0);
    assert_eq!(peak(&out), 0.0);
}
