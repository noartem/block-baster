//! Renders the game's own adaptive soundtrack (src/audio/music.rs) offline, for the promo video and
//! for listening to (and analysing) the sound of each style.
//!
//! The music follows an intensity curve given as [time, score, speed] points (score progress and
//! action speed, both 0..1, linear in between), just as it follows a game. The random choices are
//! seeded, so the result is reproducible. Output: 44.1 kHz 16-bit stereo WAV.
//!
//! Usage (from the repository root):
//!   cargo run --release --bin soundtrack -- <out.wav> <seconds> '<[[time, score, speed], ...]>' [options]
//!   cargo run --release --bin soundtrack -- <out.wav> --sfx [options]
//!
//! Options:
//!   --style <tier>.<variant>       the style to play in (default 0.0, Classic Lagoon), e.g. 7.2
//!   --switch <time>=<tier>.<variant>  switches the style at a time in seconds, as the game does
//!                                  (repeatable)
//!   --sfx                          renders every sound effect in turn instead of the music
//!
//! It prints how fast it rendered (seconds of audio per second of CPU).
//! promo/audio/music.py runs this itself.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::process::exit;
use std::time::Instant;

use block_baster::audio::intensity::IntensityTracker;
use block_baster::audio::{Engine, Sfx};
use block_baster::styles::TIERS;

const SAMPLE_RATE: u32 = 44100;
/// The music's input is sampled this often, like the game's timer.
const TICK: f64 = 0.04;
const SEED: u32 = 7;

fn usage() -> ! {
    eprintln!(
        "Usage: cargo run --release --bin soundtrack -- <out.wav> <seconds> '<[[time, score, speed], ...]>' \
         [--style T.V] [--switch TIME=T.V]...\n       \
         cargo run --release --bin soundtrack -- <out.wav> --sfx [--style T.V]"
    );
    exit(1);
}

/// Piecewise-linear input between the points.
fn input(points: &[[f64; 3]], time: f64) -> (f32, f32) {
    let mut i = 0;
    while i < points.len() - 1 && points[i + 1][0] <= time {
        i += 1;
    }
    let [t0, s0, v0] = points[i];
    let [t1, s1, v1] = points[(i + 1).min(points.len() - 1)];
    let k = if t1 > t0 {
        ((time - t0) / (t1 - t0)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    ((s0 + (s1 - s0) * k) as f32, (v0 + (v1 - v0) * k) as f32)
}

fn write_wav(path: &str, samples: &[f32]) -> std::io::Result<()> {
    let mut w = BufWriter::new(File::create(path)?);
    let data = samples.len() as u32 * 2;
    w.write_all(b"RIFF")?;
    w.write_all(&(36 + data).to_le_bytes())?;
    w.write_all(b"WAVEfmt ")?;
    w.write_all(&16u32.to_le_bytes())?;
    w.write_all(&1u16.to_le_bytes())?; // PCM
    w.write_all(&2u16.to_le_bytes())?; // stereo
    w.write_all(&SAMPLE_RATE.to_le_bytes())?;
    w.write_all(&(SAMPLE_RATE * 4).to_le_bytes())?;
    w.write_all(&4u16.to_le_bytes())?;
    w.write_all(&16u16.to_le_bytes())?;
    w.write_all(b"data")?;
    w.write_all(&data.to_le_bytes())?;
    for &s in samples {
        w.write_all(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())?;
    }
    w.flush()
}

/// "7.2" → (7, 2).
fn parse_style(s: &str) -> (usize, usize) {
    let (t, v) = s.split_once('.').unwrap_or((s, "0"));
    let (Ok(t), Ok(v)) = (t.parse::<usize>(), v.parse::<usize>()) else {
        eprintln!("Bad style {s:?}: expected <tier>.<variant>, e.g. 7.2");
        usage()
    };
    if t >= TIERS.len() || v >= TIERS[t].variants.len() {
        eprintln!("No style {s}: tiers 0..={}, variants 0..=2", TIERS.len() - 1);
        exit(1);
    }
    (t, v)
}

/// The effects in the order `--sfx` plays them, each with the time it gets.
const EFFECTS: [(Sfx, f64); 14] = [
    (Sfx::Pickup, 0.5),
    (Sfx::Place, 0.6),
    (Sfx::Pickup, 0.5),
    (Sfx::Invalid, 0.6),
    (Sfx::Place, 0.6),
    (Sfx::Clear { lines: 1, combo: 1 }, 0.9),
    (Sfx::Clear { lines: 2, combo: 2 }, 0.9),
    (Sfx::Clear { lines: 4, combo: 4 }, 1.2),
    (Sfx::BoardClear, 1.4),
    (Sfx::StyleShift, 1.6),
    (Sfx::TierUp(3), 1.8),
    (Sfx::TierUp(8), 2.2),
    (Sfx::TierUp(9), 2.6),
    (Sfx::GameOver, 2.0),
];

fn report(audio_seconds: f64, started: Instant) {
    let cpu = started.elapsed().as_secs_f64();
    println!(
        "rendered {audio_seconds:.1} s of audio in {cpu:.2} s ({:.0}x real time)",
        audio_seconds / cpu.max(1e-9)
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut positional = Vec::new();
    let mut style = (0, 0);
    let mut switches: Vec<(f64, (usize, usize))> = Vec::new();
    let mut sfx = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--style" => style = parse_style(it.next().unwrap_or_else(|| usage())),
            "--switch" => {
                let v = it.next().unwrap_or_else(|| usage());
                let (t, s) = v.split_once('=').unwrap_or_else(|| usage());
                let t: f64 = t.parse().unwrap_or_else(|_| usage());
                switches.push((t, parse_style(s)));
            }
            "--sfx" => sfx = true,
            _ => positional.push(a.clone()),
        }
    }
    switches.sort_by(|a, b| a.0.total_cmp(&b.0));

    let sr = SAMPLE_RATE as f64;
    let mut engine = Engine::new(SAMPLE_RATE as f32, SEED);
    engine.set_style(style.0, style.1);

    if sfx {
        let [out] = positional.as_slice() else { usage() };
        let seconds: f64 = 0.3 + EFFECTS.iter().map(|e| e.1).sum::<f64>() + 1.5;
        let frames = (sr * seconds).ceil() as usize;
        let mut samples = vec![0.0f32; frames * 2];
        let started = Instant::now();
        let mut done = (0.3 * sr) as usize;
        engine.render(&mut samples[..done * 2], 2);
        for (effect, gap) in EFFECTS {
            engine.play(effect);
            let end = (done + (gap * sr) as usize).min(frames);
            engine.render(&mut samples[done * 2..end * 2], 2);
            done = end;
        }
        engine.render(&mut samples[done * 2..], 2);
        report(seconds, started);
        if let Err(e) = write_wav(out, &samples) {
            eprintln!("{out}: {e}");
            exit(1);
        }
        println!("{out}: {seconds:.1}s of effects");
        return;
    }

    let [out, seconds, points] = positional.as_slice() else {
        usage()
    };
    let seconds: f64 = seconds.parse().unwrap_or_else(|_| usage());
    let points: Vec<[f64; 3]> = serde_json::from_str(points).unwrap_or_else(|e| {
        eprintln!("Bad points: {e}");
        usage()
    });
    if !seconds.is_finite() || seconds <= 0.0 || points.is_empty() {
        usage();
    }

    let frames = (sr * seconds).ceil() as usize;
    let mut tracker = IntensityTracker::new();
    engine.start_music(1.0, false);
    let mut samples = vec![0.0f32; frames * 2];
    let mut done = 0;
    let mut tick = 0u64;
    let mut next_switch = 0;
    let (mut voices, mut max_voices) = (0usize, 0usize);
    let started = Instant::now();
    while done < frames {
        let time = tick as f64 * TICK;
        while next_switch < switches.len() && switches[next_switch].0 <= time {
            let (t, v) = switches[next_switch].1;
            engine.set_style(t, v);
            next_switch += 1;
        }
        let (score, speed) = input(&points, time);
        let state = if tick == 0 {
            tracker.set(score, speed)
        } else {
            tracker.update(TICK as f32, score, speed)
        };
        engine.set_music_state(state);
        let end = (((tick + 1) as f64 * TICK * sr).round() as usize).min(frames);
        engine.render(&mut samples[done * 2..end * 2], 2);
        done = end;
        tick += 1;
        voices += engine.active_voices();
        max_voices = max_voices.max(engine.active_voices());
    }
    report(seconds, started);
    println!(
        "voices: {:.1} on average, {max_voices} at most",
        voices as f64 / tick as f64
    );
    if let Err(e) = write_wav(out, &samples) {
        eprintln!("{out}: {e}");
        exit(1);
    }
    println!("{out}: {seconds:.1}s");
}
