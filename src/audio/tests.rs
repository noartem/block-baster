use super::engine::{Engine, MAX_VOICES};
use super::intensity::{BPM_MIN, IntensityTracker};
use super::*;
use crate::styles::TIERS;

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
        for (i, &s) in sfx.iter().enumerate() {
            let mut e = Engine::new(SR, 1);
            // Every palette of the tier, in turn.
            e.set_style(tier.level, i % 3);
            e.play(s);
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
    for _ in 0..20 {
        e.play(Sfx::TierUp(9));
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
        e.set_style(9, 0);
        e.play(Sfx::Clear { lines: 3, combo: 2 });
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
    a.set_style(3, 1);
    assert_eq!(a.style(), (3, 1));
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
    e.play(Sfx::GameOver);
    let out = render(&mut e, 1.0);
    assert_eq!(peak(&out), 0.0);
}

// ---------------------------------------------------------------------------
// Styles
// ---------------------------------------------------------------------------

/// Every style tier and palette variant.
fn styles() -> impl Iterator<Item = (usize, usize)> {
    TIERS
        .iter()
        .flat_map(|t| (0..t.variants.len()).map(move |v| (t.level, v)))
}

fn music_in(tier: usize, variant: usize, e_level: f32, seconds: f64) -> (Engine, Vec<f32>) {
    let mut e = Engine::new(SR, 7);
    e.set_style(tier, variant);
    e.set_music_state(IntensityTracker::new().set(e_level, e_level));
    e.start_music(1.0, false);
    let out = render(&mut e, seconds);
    (e, out)
}

/// Left channel.
fn left(x: &[f32]) -> Vec<f32> {
    x.iter().step_by(2).copied().collect()
}

/// Energy in third-octave bands from `lowest` Hz to 16 kHz, in dB relative to their mean: the spectral shape.
fn spectrum(x: &[f32], lowest: f32) -> Vec<f32> {
    let mono = left(x);
    let bands = ((16000.0 / lowest).log2() * 3.0).round() as usize + 1;
    let mut out = vec![0.0; bands];
    for (i, o) in out.iter_mut().enumerate() {
        let f = lowest * 2f32.powf(i as f32 / 3.0);
        let mut bq = dsp::Biquad::new(dsp::FilterKind::Bandpass, f, 4.3, SR as f64);
        let e: f64 = mono.iter().map(|&s| (bq.process(s) as f64).powi(2)).sum();
        *o = (10.0 * (e / mono.len() as f64 + 1e-12).log10()) as f32;
    }
    let mean = out.iter().sum::<f32>() / out.len() as f32;
    out.iter().map(|v| v - mean).collect()
}

/// RMS difference of two spectral shapes, in dB.
fn distance(a: &[f32], b: &[f32]) -> f32 {
    (a.iter().zip(b).map(|(x, y)| (x - y).powi(2)).sum::<f32>() / a.len() as f32).sqrt()
}

/// Roughness: energy of the sample-to-sample difference, relative to the level.
fn brightness(x: &[f32]) -> f32 {
    let l = left(x);
    let d: Vec<f32> = l.windows(2).map(|w| w[1] - w[0]).collect();
    rms(&d) / rms(&l)
}

/// The largest jump between neighbouring samples of either channel.
fn max_jump(x: &[f32]) -> f32 {
    x.chunks(2)
        .collect::<Vec<_>>()
        .windows(2)
        .map(|w| (w[1][0] - w[0][0]).abs().max((w[1][1] - w[0][1]).abs()))
        .fold(0.0, f32::max)
}

#[test]
fn every_style_plays_finite_audible_unclipped_music() {
    for (tier, variant) in styles() {
        // A different stage of the music for each palette.
        let level = [0.15, 0.6, 1.0][variant];
        let (e, out) = music_in(tier, variant, level, 4.0);
        let name = format!("{} {}", TIERS[tier].name, TIERS[tier].variants[variant].name);
        assert!(out.iter().all(|v| v.is_finite()), "{name}: not finite");
        let p = peak(&out);
        assert!(p < 0.9, "{name}: too close to clipping ({p})");
        let r = rms(&out[2 * SR as usize * 2..]);
        assert!(r > 0.04, "{name}: too quiet ({r})");
        assert!(r < 0.25, "{name}: too loud ({r})");
        assert!(e.active_voices() > 0 && e.active_voices() <= MAX_VOICES / 2, "{name}");
    }
}

#[test]
fn every_tier_keeps_the_lofi_to_edm_progression() {
    for tier in &TIERS {
        let (lofi_e, lofi) = music_in(tier.level, 0, 0.0, 8.0);
        let (edm_e, edm) = music_in(tier.level, 0, 1.0, 8.0);
        assert_eq!(lofi_e.music_state().bpm, BPM_MIN);
        assert_eq!(edm_e.music_state().bpm, intensity::BPM_MAX);
        let (a, b) = (&lofi[2 * SR as usize * 2..], &edm[2 * SR as usize * 2..]);
        assert!(rms(b) > rms(a), "{}: EDM not louder than lofi", tier.name);
        assert!(
            brightness(b) > brightness(a),
            "{}: EDM not brighter than lofi",
            tier.name
        );
    }
}

#[test]
fn the_tiers_sound_measurably_different() {
    // The lofi stage, where the instruments are most exposed.
    let shapes: Vec<_> = TIERS
        .iter()
        .map(|t| spectrum(&music_in(t.level, 0, 0.2, 6.0).1[2 * SR as usize * 2..], 250.0))
        .collect();
    for i in 0..shapes.len() {
        for j in i + 1..shapes.len() {
            let d = distance(&shapes[i], &shapes[j]);
            assert!(
                d > 0.8,
                "{} and {} sound alike ({d:.2} dB apart)",
                TIERS[i].name,
                TIERS[j].name
            );
        }
    }
}

#[test]
fn placing_a_block_sounds_like_the_material() {
    let place = |tier: usize, variant: usize| {
        let mut e = Engine::new(SR, 1);
        e.set_style(tier, variant);
        e.play(Sfx::Place);
        spectrum(&render(&mut e, 0.5), 62.5)
    };
    let shapes: Vec<_> = TIERS.iter().map(|t| place(t.level, 0)).collect();
    for i in 0..shapes.len() {
        for j in i + 1..shapes.len() {
            let d = distance(&shapes[i], &shapes[j]);
            assert!(
                d > 2.0,
                "placing in {} and {} sounds alike ({d:.2} dB apart)",
                TIERS[i].name,
                TIERS[j].name
            );
        }
    }
    // The palettes of a tier keep the material: closer to each other than to the other tiers.
    for (i, tier) in TIERS.iter().enumerate() {
        let nearest_other = (0..shapes.len())
            .filter(|&j| j != i)
            .map(|j| distance(&shapes[i], &shapes[j]))
            .fold(f32::MAX, f32::min);
        for v in 1..tier.variants.len() {
            let d = distance(&shapes[i], &place(i, v));
            assert!(d < nearest_other, "{}: palette {v} strays ({d:.2} dB)", tier.name);
        }
    }
}

#[test]
fn the_palettes_of_a_tier_differ_but_stay_close() {
    for tier in &TIERS {
        let shapes: Vec<_> = (0..3)
            .map(|v| spectrum(&music_in(tier.level, v, 0.2, 6.0).1[2 * SR as usize * 2..], 250.0))
            .collect();
        let tints = tier.sound.tints;
        for (a, b) in [(0, 1), (0, 2), (1, 2)] {
            assert_ne!(tints[a], tints[b], "{}: palettes {a} and {b}", tier.name);
            let d = distance(&shapes[a], &shapes[b]);
            assert!(d > 0.05, "{}: palettes {a} and {b} identical", tier.name);
            assert!(d < 3.0, "{}: palettes {a} and {b} too far apart ({d:.2} dB)", tier.name);
        }
    }
}

#[test]
fn a_new_style_takes_over_the_music_at_the_next_bar() {
    let (mut e, _) = music_in(0, 0, 0.5, 1.3);
    let step = e.seq.position();
    e.set_style(7, 1);
    // The effects switch at once, the music waits for the downbeat.
    assert_eq!(e.sound_style().tier, 7);
    assert_eq!(e.seq.style().tier, 0);
    assert_eq!(e.seq.pending_style().map(|s| (s.tier, s.variant)), Some((7, 1)));
    // A bar is at most 16 steps of a 16th at the slowest tempo.
    render(&mut e, 16.0 * 15.0 / BPM_MIN as f64 + 0.1);
    assert_eq!((e.seq.style().tier, e.seq.style().variant), (7, 1));
    assert!(e.seq.pending_style().is_none());
    // The sequencer carried on; nothing restarted.
    assert!(e.seq.position() > step + 8);
    assert!(e.music_active());
    // Switching back before the bar cancels.
    e.set_style(3, 0);
    e.set_style(7, 1);
    assert!(e.seq.pending_style().is_none());
}

#[test]
fn switching_styles_mid_music_is_seamless() {
    // A switch every ~2.3 s through all tiers and palettes, at a busy stage of the music...
    let level = 0.62;
    let mut e = Engine::new(SR, 5);
    e.set_music_state(IntensityTracker::new().set(level, level));
    e.start_music(1.0, false);
    let mut out = render(&mut e, 2.0);
    let order: Vec<_> = styles().collect();
    for &(t, v) in order.iter().skip(1).step_by(3).chain([(0, 0)].iter()) {
        e.set_style(t, v);
        out.extend(render(&mut e, 2.3));
    }
    assert!(out.iter().all(|v| v.is_finite()));
    assert!(peak(&out) < 0.9);
    // ...jumps no more between samples than any of those styles does on its own.
    let mut worst_alone = 0.0f32;
    for &(t, v) in order.iter().skip(1).step_by(3).chain([(0, 0)].iter()) {
        let (_, alone) = music_in(t, v, level, 3.0);
        worst_alone = worst_alone.max(max_jump(&alone[(0.2 * SR) as usize * 2..]));
    }
    let jump = max_jump(&out[(0.2 * SR) as usize * 2..]);
    assert!(
        jump <= worst_alone * 1.25,
        "a click: {jump} between samples, {worst_alone} within the styles"
    );
    // The level holds steady across the switches: no gaps, no restarts.
    let window = (0.5 * SR) as usize * 2;
    let levels: Vec<f32> = out[2 * SR as usize * 2..].chunks(window).map(rms).collect();
    let mean = levels.iter().sum::<f32>() / levels.len() as f32;
    assert!(
        levels.iter().all(|&l| l > mean * 0.35),
        "a drop-out: {levels:?}"
    );
}

#[test]
fn styles_resolve_their_tier_and_palette() {
    for (tier, variant) in styles() {
        let s = SoundStyle::new(tier, variant);
        assert_eq!((s.tier, s.variant), (tier, variant));
        assert_eq!(s.profile, TIERS[tier].sound);
        assert_eq!(s.tint, TIERS[tier].sound.tints[variant]);
        // Keys stay in a comfortable range, filters within reason.
        assert!(s.tint.transpose.abs() <= 5);
        assert!((0.8..=1.25).contains(&s.tint.tone));
    }
    // Out of range: the top tier, and variants wrap around like the board clears.
    let s = SoundStyle::new(99, 4);
    assert_eq!((s.tier, s.variant), (TIERS.len() - 1, 1));
    // Classic Lagoon is the default sound.
    assert_eq!(SoundStyle::default(), SoundStyle::new(0, 0));
    assert_eq!(timbre::timbre(crate::styles::Material::Classic), &timbre::CLASSIC);
}
