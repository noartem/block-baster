//! Renders the game's own adaptive soundtrack (src/audio/music.rs) offline, for the promo video.
//!
//! The music follows an intensity curve given as [time, score, speed] points (score progress and
//! action speed, both 0..1, linear in between), just as it follows a game. The random choices are
//! seeded, so the result is reproducible. Output: 44.1 kHz 16-bit stereo WAV.
//!
//! Usage (from the repository root):
//!   cargo run --release --bin soundtrack -- <out.wav> <seconds> '<[[time, score, speed], ...]>'
//! promo/audio/music.py runs this itself.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::process::exit;

use block_baster::audio::Engine;
use block_baster::audio::intensity::IntensityTracker;

const SAMPLE_RATE: u32 = 44100;
/// The music's input is sampled this often, like the game's timer.
const TICK: f64 = 0.04;
const SEED: u32 = 7;

fn usage() -> ! {
    eprintln!(
        "Usage: cargo run --release --bin soundtrack -- <out.wav> <seconds> '<[[time, score, speed], ...]>'"
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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [out, seconds, points] = args.as_slice() else {
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

    let sr = SAMPLE_RATE as f64;
    let frames = (sr * seconds).ceil() as usize;
    let mut engine = Engine::new(SAMPLE_RATE as f32, SEED);
    let mut tracker = IntensityTracker::new();
    engine.start_music(1.0, false);
    let mut samples = vec![0.0f32; frames * 2];
    let mut done = 0;
    let mut tick = 0u64;
    while done < frames {
        let time = tick as f64 * TICK;
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
    }
    if let Err(e) = write_wav(out, &samples) {
        eprintln!("{out}: {e}");
        exit(1);
    }
    println!("{out}: {seconds:.1}s");
}
