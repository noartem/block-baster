//! Adaptive soundtrack, synthesised live. It drifts from relaxed lofi through
//! chill house and house to fast EDM as the score grows and the player speeds
//! up:
//!
//!   e 0    – 0.25  lofi: jazz chords on the keys, bass, swung boom-bap, a sparse melody, vinyl
//!   e 0.15 – 0.4   livelier lofi: more bass notes, 16th hats, a clap; the vinyl fades
//!   e 0.35 – 0.6   chill house: four-on-the-floor kick and offbeat hats replace the swung drums
//!   e 0.5  – 0.75  house: clap on 2 and 4, pumping chords, the keys leave
//!   e 0.65 – 0.85  builds every 8 bars: snare roll, noise riser, the kick drops out before the downbeat
//!   e 0.75 – 1     EDM: arpeggiated lead, 16th hats, rolling bass
//!
//! Layers fade in and out between two thresholds of `e` instead of switching
//! abruptly. Lofi and EDM layers share one progression (jazz voicings vs. plain
//! triads over the same roots), so they never clash during a crossfade.
//!
//! The style decides what this is played on (see [`super::timbre`]): the
//! tier's material picks the instruments, the drum kit and the mix (Classic is
//! an FM e-piano, a sine melody, a square arpeggio and saw chords; Gingerbread a
//! music box, a celesta and sleigh bells; Neon detuned saws and gated snares…),
//! and the palette transposes it, picks one of three progressions and shades
//! its brightness. A new style takes over at the next bar: notes already
//! playing ring out on their old instrument while the mix glides over, so the
//! music never restarts or clicks, and tempo and intensity carry on.

use super::dsp::{
    Biquad, Compressor, DelayLine, FilterKind, Param, QUANTUM, Reverb, midi_to_hz, saturate,
};
use super::engine::{BUSES, Bus, DEFAULT_Q, Dest, NoiseBuf, Source, Voice, VoiceFilter, Voices};
use super::intensity::MusicState;
use super::timbre::{
    Accent, Clap, Fm, Hat, HatKind, Kick, Mix, Patch, Snare, SnareKind, SoundStyle, Src, Texture,
};
use crate::game::rng::Rng;
use crate::styles::{Mode, Wave};

/// Gain of the music, before the volume.
pub const VOLUME: f32 = 0.4;
/// Notes are scheduled this far ahead...
const LOOKAHEAD: f64 = 0.25;
/// ...every this many seconds.
const TICK: f64 = 0.04;
const STEPS: u64 = 16;

/// One progression over the 8-bar form; the last bar holds two chords.
pub struct Progression {
    /// Bass notes (MIDI).
    pub roots: [i32; 9],
    /// Jazz voicings for the lofi keys.
    pub jazz: [[i32; 4]; 9],
    /// Plain triads over the same roots for house and EDM.
    pub triads: [[i32; 3]; 9],
    /// Dominant E chords, over which the melody's G is raised to G#.
    pub dominant: [bool; 9],
}

/// Dm9, G13, C^9, Am9, F^9, E7b9, Am9, Gm9, C9: the original progression.
const DORIAN: Progression = Progression {
    roots: [38, 43, 36, 45, 41, 40, 45, 43, 36],
    jazz: [
        [53, 57, 60, 64], // Dm9
        [53, 57, 59, 64], // G13
        [52, 55, 59, 62], // C^9
        [52, 55, 59, 60], // Am9
        [52, 55, 57, 60], // F^9
        [50, 53, 56, 59], // E7b9
        [52, 55, 59, 60], // Am9
        [53, 57, 58, 62], // Gm9
        [52, 55, 58, 62], // C9
    ],
    triads: [
        [62, 65, 69], // Dm
        [62, 67, 71], // G
        [64, 67, 72], // C
        [64, 69, 72], // Am
        [65, 69, 72], // F
        [64, 68, 71], // E
        [64, 69, 72], // Am
        [62, 67, 70], // Gm
        [64, 67, 72], // C
    ],
    dominant: [false, false, false, false, false, true, false, false, false],
};

/// Am9, F^9, Dm9, E7b9, Am9, F^9, G13, Dm9 → E7b9: darker, in A minor.
const MINOR: Progression = Progression {
    roots: [45, 41, 38, 40, 45, 41, 43, 38, 40],
    jazz: [
        [52, 55, 59, 60], // Am9
        [52, 55, 57, 60], // F^9
        [53, 57, 60, 64], // Dm9
        [50, 53, 56, 59], // E7b9
        [52, 55, 59, 60], // Am9
        [52, 55, 57, 60], // F^9
        [53, 57, 59, 64], // G13
        [53, 57, 60, 64], // Dm9
        [50, 53, 56, 59], // E7b9
    ],
    triads: [
        [64, 69, 72], // Am
        [65, 69, 72], // F
        [62, 65, 69], // Dm
        [64, 68, 71], // E
        [64, 69, 72], // Am
        [65, 69, 72], // F
        [62, 67, 71], // G
        [62, 65, 69], // Dm
        [64, 68, 71], // E
    ],
    dominant: [false, false, false, true, false, false, false, false, true],
};

/// C^9, Am9, F^9, G13, Em7, Am9, Dm9, F^9 → G13: brighter, in C major.
const MAJOR: Progression = Progression {
    roots: [36, 45, 41, 43, 40, 45, 38, 41, 43],
    jazz: [
        [52, 55, 59, 62], // C^9
        [52, 55, 59, 60], // Am9
        [52, 55, 57, 60], // F^9
        [53, 57, 59, 64], // G13
        [50, 54, 55, 59], // Em9
        [52, 55, 59, 60], // Am9
        [53, 57, 60, 64], // Dm9
        [52, 55, 57, 60], // F^9
        [53, 57, 59, 64], // G13
    ],
    triads: [
        [64, 67, 72], // C
        [64, 69, 72], // Am
        [65, 69, 72], // F
        [62, 67, 71], // G
        [64, 67, 71], // Em
        [64, 69, 72], // Am
        [62, 65, 69], // Dm
        [65, 69, 72], // F
        [62, 67, 71], // G
    ],
    dominant: [false; 9],
};

pub fn progression(mode: Mode) -> &'static Progression {
    match mode {
        Mode::Dorian => &DORIAN,
        Mode::Minor => &MINOR,
        Mode::Major => &MAJOR,
    }
}

/// C major pentatonic from C4 (A minor pentatonic too); over E7b9 the G is raised to G#.
const PENTA: [i32; 10] = [60, 62, 64, 67, 69, 72, 74, 76, 79, 81];
const ARP: [usize; 16] = [0, 1, 2, 3, 1, 2, 3, 4, 2, 3, 4, 5, 3, 4, 5, 6];

// Rhythms as 16th steps within a bar.
const KEYS_HITS: [&[u64]; 4] = [&[0], &[0, 10], &[0, 14], &[4]];
const LOFI_BASS: [&[u64]; 2] = [&[0, 12], &[0, 6]];
const LOFI_BASS_EXTRA: [u64; 2] = [6, 15];
const LOFI_KICK: [&[u64]; 2] = [&[0, 10], &[0, 6, 10]];
/// Swing for the lofi layers: how late each 16th of a beat lands, in steps.
const SWING: [f64; 4] = [0.0, 0.33, 0.4, 0.4];

/// Which chord of the progression sounds at a step.
pub fn chord_at(bar: u64, step: u64) -> usize {
    let b = (bar % 8) as usize;
    if b == 7 && step >= 8 { 8 } else { b }
}

/// 0 below `a`, 1 above `b`, linear in between.
fn ramp(x: f64, a: f64, b: f64) -> f64 {
    super::intensity::ramp(x as f32, a as f32, b as f32) as f64
}

// ---------------------------------------------------------------------------
// Instruments
// ---------------------------------------------------------------------------

/// Options of a synth note.
#[derive(Clone, Copy, Debug)]
struct ToneOpts {
    wave: Wave,
    /// MIDI note number.
    note: f64,
    vol: f64,
    attack: Option<f64>,
    decay: Option<f64>,
    /// Sustain level relative to `vol`, 0..1.
    sustain: Option<f64>,
    release: Option<f64>,
    /// How long the note is held before the release starts, in seconds.
    hold: Option<f64>,
    detune: f64,
    /// Optional per-note low-pass with an envelope: from `cutoff` down to `cutoff_end` over the decay.
    cutoff: Option<f64>,
    cutoff_end: Option<f64>,
    q: Option<f64>,
}

fn opts(wave: Wave, note: f64, vol: f64) -> ToneOpts {
    ToneOpts {
        wave,
        note,
        vol,
        attack: None,
        decay: None,
        sustain: None,
        release: None,
        hold: None,
        detune: 0.0,
        cutoff: None,
        cutoff_end: None,
        q: None,
    }
}

/// The note options of a patch.
fn patch_opts(p: &Patch, note: f64, vol: f64, hold: Option<f64>) -> ToneOpts {
    let filtered = p.cutoff > 0.0;
    ToneOpts {
        attack: Some(p.attack),
        decay: Some(p.decay),
        sustain: Some(p.sustain),
        release: Some(p.release),
        hold,
        cutoff: filtered.then_some(p.cutoff),
        cutoff_end: filtered.then_some(p.cutoff_end),
        q: filtered.then_some(p.q),
        ..opts(src_wave(p.src), note, vol)
    }
}

/// The oscillator of a source (FM voices start as a sine and get their FM source after).
fn src_wave(s: Src) -> Wave {
    match s {
        Src::Osc(w) => w,
        Src::Fm(_) => Wave::Sine,
    }
}

/// Envelope options; see [`Instruments::envelope`].
#[derive(Clone, Copy, Debug, Default)]
struct Env {
    attack: Option<f64>,
    decay: Option<f64>,
    sustain: Option<f64>,
    hold: Option<f64>,
    release: Option<f64>,
}

/// A filtered noise hit.
#[derive(Clone, Copy, Debug)]
struct Hit {
    kind: FilterKind,
    freq: f64,
    q: Option<f64>,
    decay: f64,
    attack: Option<f64>,
}

fn hit(kind: FilterKind, freq: f64, decay: f64) -> Hit {
    Hit {
        kind,
        freq,
        q: None,
        decay,
        attack: None,
    }
}

/// An anvil's ring: inharmonic FM, bright and quickly dulled.
const ANVIL: Fm = Fm {
    ratio: 1.41,
    index: 4.0,
    settle_to: 0.1,
    settle: 0.05,
    vibrato: 0.0,
};

/// A high, glassy bell for chimes and glints.
const BELL: Fm = Fm {
    ratio: 3.5,
    index: 1.2,
    settle_to: 0.2,
    settle: 0.3,
    vibrato: 0.0,
};

/// Synthesised instruments for the soundtrack. Every hit is one or two
/// short-lived voices, freed once the note ends.
struct Instruments<'a> {
    voices: &'a mut Voices,
    rng: &'a mut Rng,
    sr: f64,
    white_len: usize,
}

impl Instruments<'_> {
    /// The first sample at or after `t` seconds.
    fn sample(&self, t: f64) -> u64 {
        (t * self.sr).ceil() as u64
    }

    /// Attack, then decay to sustain, hold, release. Returns when the sound is silent.
    fn envelope(p: &mut Param, t: f64, vol: f64, o: Env) -> f64 {
        let a = o.attack.unwrap_or(0.003);
        let d = o.decay.unwrap_or(0.2);
        let s = o.sustain.unwrap_or(0.0);
        let r = o.release.unwrap_or(0.05);
        let end = t + a.max(o.hold.unwrap_or(a + d));
        p.set_value_at(0.0, t);
        p.linear_ramp_to(vol, t + a);
        p.set_target_at(vol * s, t + a, d / 4.0);
        p.set_target_at(0.0, end, r / 4.0);
        end + r * 1.5
    }

    fn tone_voice(&self, dest: Bus, t: f64, o: ToneOpts) -> Voice {
        let first = self.sample(t);
        let freq = midi_to_hz(o.note);
        let filter = o.cutoff.map(|cutoff| {
            let mut f = Param::starting_at(cutoff as f32, self.sr, first);
            f.set_value_at(cutoff, t);
            if let Some(end) = o.cutoff_end {
                f.set_target_at(end, t, o.decay.unwrap_or(0.2) / 3.0);
            }
            VoiceFilter::new(FilterKind::Lowpass, f, o.q.unwrap_or(0.7) as f32)
        });
        let mut gain = Param::starting_at(0.0, self.sr, first);
        let stop = Self::envelope(
            &mut gain,
            t,
            o.vol,
            Env {
                attack: o.attack,
                decay: o.decay,
                sustain: o.sustain,
                hold: o.hold,
                release: o.release,
            },
        );
        Voice {
            start: first,
            stop: self.sample(stop),
            source: Source::Osc {
                wave: o.wave,
                phase: 0.0,
                freq: Param::new(freq as f32, self.sr),
                detune: 2f64.powf(o.detune / 1200.0),
            },
            filter,
            gain,
            dest: Dest::Music(dest),
            send: 0.0,
        }
    }

    fn tone(&mut self, dest: Bus, t: f64, o: ToneOpts) {
        let v = self.tone_voice(dest, t, o);
        self.voices.add(v);
    }

    /// A note of `src` with the options `o`.
    fn src_tone(&mut self, dest: Bus, t: f64, src: Src, o: ToneOpts) {
        let mut v = self.tone_voice(dest, t, o);
        if let Src::Fm(fm) = src {
            v.source = Source::fm(&fm, midi_to_hz(o.note), t, v.start, self.sr);
        }
        self.voices.add(v);
    }

    /// A note on a patch, held for `hold` if given, plus its layer.
    fn play(&mut self, dest: Bus, t: f64, note: f64, vol: f64, hold: Option<f64>, p: &Patch) {
        self.src_tone(dest, t, p.src, patch_opts(p, note, vol, hold));
        if let Some(l) = p.layer {
            let o = ToneOpts {
                decay: Some(l.decay),
                cutoff: (p.cutoff > 0.0).then_some(p.cutoff),
                cutoff_end: (p.cutoff > 0.0).then_some(p.cutoff_end),
                q: (p.cutoff > 0.0).then_some(p.q),
                ..opts(src_wave(l.src), note + l.semis, vol * l.level)
            };
            self.src_tone(dest, t, l.src, o);
        }
    }

    /// A chord on the keys (an FM electric piano in Classic), each note a little late, held for `hold`.
    #[allow(clippy::too_many_arguments)]
    fn keys(&mut self, dest: Bus, t: f64, notes: &[i32], tr: i32, vol: f64, hold: f64, p: &Patch) {
        let stop = self.sample(t + hold + 2.0);
        for &note in notes {
            let note = note + tr;
            let f = midi_to_hz(note as f64);
            // Players never hit all keys at once.
            let at = t + self.rng.next() * 0.012;
            let mut v = self.tone_voice(dest, at, patch_opts(p, note as f64, vol, Some(hold)));
            if let Src::Fm(fm) = p.src {
                // Bright at the attack, settling into a soft, round tone; the vibrato started with the chord.
                v.source = Source::fm(&fm, f, t, v.start, self.sr);
            }
            v.stop = v.stop.min(stop);
            self.voices.add(v);
        }
    }

    /// Looping white noise from a random point.
    fn noise_source(&mut self) -> Source {
        let duration = self.white_len as f64 / self.sr;
        let offset = self.rng.next() * (duration - 0.1);
        Source::Noise {
            buf: NoiseBuf::White,
            pos: (offset * self.sr) as usize,
            looping: true,
        }
    }

    fn filter(&self, kind: FilterKind, freq: f64, q: f64) -> VoiceFilter {
        VoiceFilter::new(kind, Param::new(freq as f32, self.sr), q as f32)
    }

    /// Noise through a filter with a percussive envelope.
    fn noise_hit(&mut self, dest: Bus, t: f64, vol: f64, o: Hit) {
        let env = Env {
            attack: Some(o.attack.unwrap_or(0.001)),
            decay: Some(o.decay),
            ..Env::default()
        };
        self.noise_env(dest, t, vol, o, env);
    }

    /// Noise through a filter with any envelope.
    fn noise_env(&mut self, dest: Bus, t: f64, vol: f64, o: Hit, env: Env) {
        let first = self.sample(t);
        let mut gain = Param::starting_at(0.0, self.sr, first);
        let stop = Self::envelope(&mut gain, t, vol, env);
        let source = self.noise_source();
        let filter = Some(self.filter(o.kind, o.freq, o.q.unwrap_or(0.7)));
        self.voices.add(Voice {
            start: first,
            stop: self.sample(stop),
            source,
            filter,
            gain,
            dest: Dest::Music(dest),
            send: 0.0,
        });
    }

    /// Sine kick with a pitch drop; `punch` 0 = soft and round (lofi), 1 = tight and clicky (house).
    fn kick(&mut self, dest: Bus, t: f64, vol: f64, punch: f64, k: &Kick) {
        let first = self.sample(t);
        let from = k.from + k.punch * punch;
        let to = k.to + 6.0 * punch;
        let decay = k.decay - 0.12 * punch;
        let mut freq = Param::starting_at(from as f32, self.sr, first);
        freq.set_value_at(from, t);
        freq.exp_ramp_to(to, t + 0.07 + 0.04 * (1.0 - punch));
        let mut gain = Param::starting_at(0.0, self.sr, first);
        gain.set_value_at(0.0, t);
        gain.linear_ramp_to(vol, t + 0.002);
        gain.set_target_at(0.0, t + 0.02, decay / 4.0);
        self.voices.add(Voice {
            start: first,
            stop: self.sample(t + decay * 1.6),
            source: Source::Osc {
                wave: Wave::Sine,
                phase: 0.0,
                freq,
                detune: 1.0,
            },
            filter: None,
            gain,
            dest: Dest::Music(dest),
            send: 0.0,
        });
        // The beater click: what's left of the kick on small phone speakers.
        self.noise_hit(
            dest,
            t,
            vol * (0.1 + 0.25 * punch) * k.click,
            Hit {
                q: Some(0.8),
                ..hit(FilterKind::Bandpass, k.click_freq + 1800.0 * punch, 0.012)
            },
        );
    }

    fn snare(&mut self, dest: Bus, t: f64, vol: f64, decay: f64, s: &Snare) {
        let decay = decay * s.decay;
        let noise = Hit {
            q: Some(s.q),
            ..hit(FilterKind::Bandpass, s.freq, decay)
        };
        match s.kind {
            SnareKind::Noise => self.noise_hit(dest, t, vol, noise),
            SnareKind::Gated => {
                // Held at full size, then shut.
                let env = Env {
                    attack: Some(0.001),
                    decay: Some(0.05),
                    sustain: Some(0.5),
                    hold: Some(0.1 + decay * 0.5),
                    release: Some(0.012),
                };
                self.noise_env(dest, t, vol * 0.9, noise, env);
            }
            SnareKind::Wood => {
                let knock = Hit {
                    decay: decay * 0.5,
                    ..noise
                };
                self.noise_hit(dest, t, vol * 2.2, knock);
            }
            SnareKind::Metal => {
                self.noise_hit(dest, t, vol * 0.8, noise);
                let clank = ToneOpts {
                    attack: Some(0.001),
                    decay: Some(0.08 + decay * 0.5),
                    ..opts(Wave::Sine, s.tone + 12.0, vol * 0.35)
                };
                self.src_tone(dest, t, Src::Fm(ANVIL), clank);
            }
        }
        let (wave, d) = match s.kind {
            SnareKind::Wood => (Wave::Sine, decay * 0.3),
            _ => (Wave::Triangle, decay * 0.5),
        };
        self.tone(
            dest,
            t,
            ToneOpts {
                decay: Some(d),
                ..opts(wave, s.tone, vol * s.tone_level)
            },
        );
    }

    fn clap(&mut self, dest: Bus, t: f64, vol: f64, c: &Clap) {
        let vol = vol * c.level;
        let first = self.sample(t);
        let mut g = Param::starting_at(0.0, self.sr, first);
        g.set_value_at(0.0, t);
        // Three quick slaps and a short tail, like several hands at once.
        for dt in [0.0, 0.011, 0.022] {
            g.set_value_at(vol, t + dt);
            g.set_target_at(vol * 0.15, t + dt, 0.003);
        }
        g.set_value_at(vol * 0.8, t + 0.033);
        g.set_target_at(0.0, t + 0.033, 0.045);
        let source = self.noise_source();
        let filter = Some(self.filter(FilterKind::Bandpass, c.freq, c.q));
        self.voices.add(Voice {
            start: first,
            stop: self.sample(t + 0.3),
            source,
            filter,
            gain: g,
            dest: Dest::Music(dest),
            send: 0.0,
        });
    }

    fn hat(&mut self, dest: Bus, t: f64, vol: f64, decay: f64, freq: f64, h: &Hat) {
        let (vol, decay, freq) = (vol * h.level, decay * h.decay, freq * h.freq);
        match h.kind {
            HatKind::Noise => {
                let o = Hit {
                    q: Some(h.q),
                    attack: Some(h.attack),
                    ..hit(h.filter, freq, decay)
                };
                self.noise_hit(dest, t, vol, o);
            }
            HatKind::Jingle => self.jingle(dest, t, vol, decay, freq),
        }
    }

    /// Sleigh bells: three quick shakes of ringing noise and a short tail.
    fn jingle(&mut self, dest: Bus, t: f64, vol: f64, decay: f64, freq: f64) {
        let first = self.sample(t);
        let mut g = Param::starting_at(0.0, self.sr, first);
        g.set_value_at(0.0, t);
        for (dt, k) in [(0.0, 1.0), (0.022, 0.6), (0.041, 0.8)] {
            g.set_value_at(vol * k, t + dt);
            g.set_target_at(vol * 0.12, t + dt, 0.006);
        }
        g.set_target_at(0.0, t + 0.05, decay.max(0.02));
        let source = self.noise_source();
        let filter = Some(self.filter(FilterKind::Highpass, freq, 2.0));
        self.voices.add(Voice {
            start: first,
            stop: self.sample(t + 0.05 + decay.max(0.02) * 6.0),
            source,
            filter,
            gain: g,
            dest: Dest::Music(dest),
            send: 0.0,
        });
    }

    fn crash(&mut self, dest: Bus, t: f64, vol: f64, freq: f64) {
        self.noise_hit(dest, t, vol, hit(FilterKind::Highpass, freq, 1.6));
    }

    /// Noise that rises in volume and brightness over `dur` seconds, leading into a drop.
    fn riser(&mut self, dest: Bus, t: f64, dur: f64, vol: f64) {
        let first = self.sample(t);
        let mut f = Param::starting_at(400.0, self.sr, first);
        f.set_value_at(400.0, t);
        f.exp_ramp_to(9000.0, t + dur);
        let mut gain = Param::starting_at(0.0, self.sr, first);
        gain.set_value_at(0.0, t);
        gain.linear_ramp_to(vol, t + dur);
        gain.linear_ramp_to(0.0, t + dur + 0.03);
        let source = self.noise_source();
        self.voices.add(Voice {
            start: first,
            stop: self.sample(t + dur + 0.05),
            source,
            filter: Some(VoiceFilter::new(FilterKind::Bandpass, f, 1.2)),
            gain,
            dest: Dest::Music(dest),
            send: 0.0,
        });
    }

    /// Vinyl crackle: a tiny bright click.
    fn crackle(&mut self, dest: Bus, t: f64, vol: f64) {
        let decay = 0.004 + self.rng.next() * 0.012;
        self.noise_hit(dest, t, vol, hit(FilterKind::Highpass, 2500.0, decay));
    }

    /// Rustling paper: a soft band of noise.
    fn rustle(&mut self, dest: Bus, t: f64, vol: f64) {
        let freq = 1600.0 + self.rng.next() * 1600.0;
        let decay = 0.02 + self.rng.next() * 0.05;
        let o = Hit {
            q: Some(0.8),
            attack: Some(0.006),
            ..hit(FilterKind::Bandpass, freq, decay)
        };
        self.noise_hit(dest, t, vol, o);
    }

    /// A bubble popping: a sine blip sweeping up.
    fn bubble(&mut self, dest: Bus, t: f64, vol: f64) {
        let first = self.sample(t);
        let from = 350.0 + self.rng.next() * 450.0;
        let mut freq = Param::starting_at(from as f32, self.sr, first);
        freq.set_value_at(from, t);
        freq.exp_ramp_to(from * 2.8, t + 0.035);
        let mut gain = Param::starting_at(0.0, self.sr, first);
        let stop = Self::envelope(
            &mut gain,
            t,
            vol,
            Env {
                attack: Some(0.002),
                decay: Some(0.04),
                ..Env::default()
            },
        );
        self.voices.add(Voice {
            start: first,
            stop: self.sample(stop),
            source: Source::Osc {
                wave: Wave::Sine,
                phase: 0.0,
                freq,
                detune: 1.0,
            },
            filter: None,
            gain,
            dest: Dest::Music(dest),
            send: 0.0,
        });
    }

    /// A struck bell on a MIDI note.
    fn bell(&mut self, dest: Bus, t: f64, note: f64, vol: f64, decay: f64) {
        let o = ToneOpts {
            attack: Some(0.002),
            decay: Some(decay),
            release: Some(decay * 0.5),
            ..opts(Wave::Sine, note, vol)
        };
        self.src_tone(dest, t, Src::Fm(BELL), o);
    }

    /// A woodblock knock, high or low.
    fn woodblock(&mut self, dest: Bus, t: f64, vol: f64, high: bool) {
        let (note, band) = if high { (84.0, 2100.0) } else { (79.0, 1500.0) };
        self.tone(
            dest,
            t,
            ToneOpts {
                attack: Some(0.001),
                decay: Some(0.045),
                ..opts(Wave::Sine, note, vol)
            },
        );
        let o = Hit {
            q: Some(5.0),
            ..hit(FilterKind::Bandpass, band, 0.025)
        };
        self.noise_hit(dest, t, vol * 1.6, o);
    }

    /// An anvil clank.
    fn anvil(&mut self, dest: Bus, t: f64, vol: f64) {
        let o = ToneOpts {
            attack: Some(0.001),
            decay: Some(0.3),
            release: Some(0.1),
            ..opts(Wave::Sine, 75.0, vol)
        };
        self.src_tone(dest, t, Src::Fm(ANVIL), o);
        self.noise_hit(dest, t, vol * 0.5, hit(FilterKind::Highpass, 5000.0, 0.03));
    }

    /// A timpani stroke on a MIDI note: the head settles onto its pitch, the felt mallet thuds.
    fn timpani(&mut self, dest: Bus, t: f64, note: f64, vol: f64) {
        let first = self.sample(t);
        let f = midi_to_hz(note);
        let mut freq = Param::starting_at((f * 1.04) as f32, self.sr, first);
        freq.set_value_at(f * 1.04, t);
        freq.exp_ramp_to(f, t + 0.08);
        let mut gain = Param::starting_at(0.0, self.sr, first);
        gain.set_value_at(0.0, t);
        gain.linear_ramp_to(vol, t + 0.004);
        gain.set_target_at(0.0, t + 0.01, 0.22);
        self.voices.add(Voice {
            start: first,
            stop: self.sample(t + 1.4),
            source: Source::Osc {
                wave: Wave::Sine,
                phase: 0.0,
                freq,
                detune: 1.0,
            },
            filter: None,
            gain,
            dest: Dest::Music(dest),
            send: 0.0,
        });
        self.noise_hit(dest, t, vol * 0.4, hit(FilterKind::Lowpass, 700.0, 0.05));
    }
}

// ---------------------------------------------------------------------------
// The mixer graph
// ---------------------------------------------------------------------------

/// How a bus is wired: input gain, optional low-pass, panner, sidechain pump, and sends.
struct BusCfg {
    level: f32,
    filter: Option<f32>,
    pan: bool,
    pump: bool,
    reverb: f32,
    delay: f32,
    /// Into the saturation instead of the mix.
    shaper: bool,
}

const fn cfg(
    level: f32,
    filter: Option<f32>,
    pan: bool,
    pump: bool,
    reverb: f32,
    delay: f32,
    shaper: bool,
) -> BusCfg {
    BusCfg {
        level,
        filter,
        pan,
        pump,
        reverb,
        delay,
        shaper,
    }
}

/// In the order of [`Bus`].
const BUS_CFG: [BusCfg; BUSES] = [
    cfg(0.0, Some(1500.0), false, false, 0.45, 0.22, false), // keys
    cfg(1.0, None, true, false, 0.55, 0.35, false),          // melody
    cfg(1.0, Some(1500.0), true, false, 0.12, 0.3, false),   // arp
    cfg(0.0, Some(700.0), false, true, 0.3, 0.1, false),     // pad
    cfg(1.0, None, false, false, 0.0, 0.0, true),            // bass
    cfg(1.0, None, false, false, 0.0, 0.0, true),            // kick
    cfg(1.0, Some(4500.0), false, false, 0.1, 0.0, false),   // drums
    cfg(0.0, Some(5000.0), false, false, 0.0, 0.0, false),   // vinyl
    cfg(1.0, None, false, false, 0.25, 0.0, false),          // fx
];

struct BusState {
    gain: Param,
    filter: Option<(Param, Biquad)>,
    pan: Option<Param>,
    last_freq: f32,
    last_pan: f32,
    gl: f32,
    gr: f32,
}

/// Level of the endless vinyl hiss under the vinyl bus.
const HISS: f32 = 0.025;
const DELAY_FEEDBACK: f32 = 0.38;

/// The style's mix settings, gliding from one style to the next.
struct MixParams {
    reverb: Param,
    drum_reverb: Param,
    delay: Param,
    drive: Param,
    trim: Param,
}

impl MixParams {
    fn new(sr: f64) -> Self {
        MixParams {
            reverb: Param::new(1.0, sr),
            drum_reverb: Param::new(1.0, sr),
            delay: Param::new(1.0, sr),
            drive: Param::new(1.0, sr),
            trim: Param::new(1.0, sr),
        }
    }

    /// Jumps to a style's mix.
    fn reset(&mut self, m: &Mix, frame: u64) {
        self.reverb.reset(m.reverb as f32, frame);
        self.drum_reverb.reset(m.drum_reverb as f32, frame);
        self.delay.reset(m.delay as f32, frame);
        self.drive.reset(m.drive as f32, frame);
        self.trim.reset(m.trim as f32, frame);
    }
}

/// Per-sample values of the mix parameters for one render quantum.
struct MixBufs {
    reverb: [f32; QUANTUM],
    drum_reverb: [f32; QUANTUM],
    delay: [f32; QUANTUM],
    drive: [f32; QUANTUM],
    trim: [f32; QUANTUM],
}

/// All parts of the playing music; stopping fades it out and lets it go.
pub struct MusicGraph {
    sr: f64,
    pub active: bool,
    /// When a fading-out graph goes quiet for good.
    pub release_at: Option<f64>,
    level: f32,
    paused: bool,
    out: Param,
    muffle_freq: Param,
    muffle: [Biquad; 2],
    muffle_last: f32,
    limiter: Compressor,
    delay_time: Param,
    delay: [DelayLine; 2],
    tape: [Biquad; 2],
    reverb: Reverb,
    pump: Param,
    buses: [BusState; BUSES],
    mix_params: MixParams,
    hiss: usize,
    countdown: u32,
    mix: [[f32; QUANTUM]; 2],
    rev: [[f32; QUANTUM]; 2],
    del: [[f32; QUANTUM]; 2],
    shaper: [f32; QUANTUM],
    pump_buf: [f32; QUANTUM],
    mix_bufs: MixBufs,
}

impl MusicGraph {
    pub fn new(sr: f64) -> Self {
        let buses = std::array::from_fn(|i| {
            let c = &BUS_CFG[i];
            BusState {
                gain: Param::new(c.level, sr),
                filter: c.filter.map(|f| (Param::new(f, sr), Biquad::default())),
                pan: c.pan.then(|| Param::new(0.0, sr)),
                last_freq: -1.0,
                last_pan: f32::NAN,
                gl: 1.0,
                gr: 1.0,
            }
        });
        MusicGraph {
            sr,
            active: false,
            release_at: None,
            level: 1.0,
            paused: false,
            out: Param::new(0.0, sr),
            muffle_freq: Param::new(20000.0, sr),
            muffle: [Biquad::default(); 2],
            muffle_last: -1.0,
            limiter: Compressor::new(sr, -12.0, 6.0, 8.0, 0.003, 0.2),
            delay_time: Param::new(0.0, sr),
            delay: [
                DelayLine::new((sr * 2.0) as usize + QUANTUM),
                DelayLine::new((sr * 2.0) as usize + QUANTUM),
            ],
            tape: [Biquad::new(FilterKind::Lowpass, 3200.0, DEFAULT_Q, sr); 2],
            reverb: Reverb::new(sr),
            pump: Param::new(1.0, sr),
            buses,
            mix_params: MixParams::new(sr),
            hiss: 0,
            countdown: 0,
            mix: [[0.0; QUANTUM]; 2],
            rev: [[0.0; QUANTUM]; 2],
            del: [[0.0; QUANTUM]; 2],
            shaper: [0.0; QUANTUM],
            pump_buf: [0.0; QUANTUM],
            mix_bufs: MixBufs {
                reverb: [0.0; QUANTUM],
                drum_reverb: [0.0; QUANTUM],
                delay: [0.0; QUANTUM],
                drive: [0.0; QUANTUM],
                trim: [0.0; QUANTUM],
            },
        }
    }

    fn gain(&self) -> f64 {
        (VOLUME * self.level * self.level) as f64
    }

    pub fn releasing(&self) -> bool {
        self.active && self.release_at.is_some()
    }

    /// Starts playing, fading in over 1.5 s; a fresh graph starts with the mix of `mix`.
    pub fn begin(&mut self, now: f64, frame: u64, level: f32, paused: bool, mix: &Mix) {
        self.level = level;
        self.paused = paused;
        if self.releasing() {
            // Fade back in from wherever the fade-out got to.
            self.out.cancel(now);
            self.out.set_value_at(self.out.value() as f64, now);
        } else {
            // A fresh graph.
            for (b, c) in self.buses.iter_mut().zip(&BUS_CFG) {
                b.gain.reset(c.level, frame);
                if let (Some((p, bq)), Some(f)) = (&mut b.filter, c.filter) {
                    p.reset(f, frame);
                    bq.clear();
                }
                if let Some(p) = &mut b.pan {
                    p.reset(0.0, frame);
                }
                b.last_freq = -1.0;
                b.last_pan = f32::NAN;
            }
            self.out.reset(0.0, frame);
            self.out.set_value_at(0.0, now);
            self.muffle_freq.reset(20000.0, frame);
            self.muffle_last = -1.0;
            for m in &mut self.muffle {
                m.clear();
            }
            self.limiter.clear();
            self.delay_time.reset(0.0, frame);
            for d in &mut self.delay {
                d.clear();
            }
            for t in &mut self.tape {
                t.clear();
            }
            self.reverb.clear();
            self.pump.reset(1.0, frame);
            self.mix_params.reset(mix, frame);
            self.hiss = 0;
            self.countdown = 0;
        }
        self.out.linear_ramp_to(self.gain(), now + 1.5);
        self.active = true;
        self.release_at = None;
        self.apply_pause(now);
    }

    /// Fades out; the engine lets go of everything 0.6 s later.
    pub fn release(&mut self, now: f64) {
        if !self.active || self.release_at.is_some() {
            return;
        }
        self.out.cancel(now);
        self.out.set_target_at(0.0, now, 0.08);
        self.release_at = Some(now + 0.6);
    }

    pub fn deactivate(&mut self) {
        self.active = false;
        self.release_at = None;
    }

    /// Volume 0..1, on a perceptual curve.
    pub fn set_volume(&mut self, now: f64, level: f32) {
        self.level = level;
        if self.active && self.release_at.is_none() {
            self.out.cancel(now);
            self.out.set_target_at(self.gain(), now, 0.05);
        }
    }

    /// Muffles the music while a menu is open.
    pub fn set_paused(&mut self, now: f64, paused: bool) {
        self.paused = paused;
        if self.active {
            self.apply_pause(now);
        }
    }

    fn apply_pause(&mut self, now: f64) {
        self.muffle_freq
            .set_target_at(if self.paused { 700.0 } else { 20000.0 }, now, 0.12);
    }

    /// Mixes the buses (mono voice sums) and adds the result to `out_l`/`out_r`.
    pub fn process(
        &mut self,
        bus: &[[f32; QUANTUM]; BUSES],
        out_l: &mut [f32],
        out_r: &mut [f32],
        pink: &[f32],
    ) {
        let len = out_l.len();
        let sr = self.sr;
        for b in [&mut self.mix, &mut self.rev, &mut self.del] {
            b[0][..len].fill(0.0);
            b[1][..len].fill(0.0);
        }
        self.shaper[..len].fill(0.0);
        self.pump.fill(&mut self.pump_buf[..len]);
        let (mp, mb) = (&mut self.mix_params, &mut self.mix_bufs);
        mp.reverb.fill(&mut mb.reverb[..len]);
        mp.drum_reverb.fill(&mut mb.drum_reverb[..len]);
        mp.delay.fill(&mut mb.delay[..len]);
        mp.drive.fill(&mut mb.drive[..len]);
        mp.trim.fill(&mut mb.trim[..len]);

        for (bi, (st, c)) in self.buses.iter_mut().zip(&BUS_CFG).enumerate() {
            let input = &bus[bi][..len];
            let vinyl = bi == Bus::Vinyl as usize;
            let reverb_scale = if bi == Bus::Drums as usize {
                &mb.drum_reverb
            } else {
                &mb.reverb
            };
            for (i, &x) in input.iter().enumerate() {
                let mut x = x;
                if vinyl {
                    // The vinyl hiss runs all the time; its bus fades it in and out.
                    x += pink[self.hiss] * HISS;
                    self.hiss += 1;
                    if self.hiss == pink.len() {
                        self.hiss = 0;
                    }
                }
                x *= st.gain.tick();
                if let Some((freq, bq)) = &mut st.filter {
                    let f = freq.tick();
                    if i % 32 == 0 && f != st.last_freq {
                        bq.set(FilterKind::Lowpass, f, DEFAULT_Q, sr);
                        st.last_freq = f;
                    }
                    x = bq.process(x);
                }
                let (mut l, mut r) = (x, x);
                if let Some(pan) = &mut st.pan {
                    let p = pan.tick();
                    if p != st.last_pan {
                        // Equal-power panning of a mono input.
                        let a = (p.clamp(-1.0, 1.0) + 1.0) * 0.5 * std::f32::consts::FRAC_PI_2;
                        st.gl = a.cos();
                        st.gr = a.sin();
                        st.last_pan = p;
                    }
                    l = x * st.gl;
                    r = x * st.gr;
                }
                if c.pump {
                    l *= self.pump_buf[i];
                    r *= self.pump_buf[i];
                }
                if c.shaper {
                    self.shaper[i] += l;
                } else {
                    self.mix[0][i] += l;
                    self.mix[1][i] += r;
                }
                if c.reverb > 0.0 {
                    self.rev[0][i] += l * c.reverb * reverb_scale[i];
                    self.rev[1][i] += r * c.reverb * reverb_scale[i];
                }
                if c.delay > 0.0 {
                    self.del[0][i] += l * c.delay * mb.delay[i];
                    self.del[1][i] += r * c.delay * mb.delay[i];
                }
            }
        }

        // Tape delay: echoes on the beat, darker each time round.
        for i in 0..len {
            // A delay inside a feedback loop is at least one render quantum, as in Web Audio.
            let d = (self.delay_time.tick() as f64 * sr).max(QUANTUM as f64) as f32;
            for ch in 0..2 {
                let y = self.tape[ch].process(self.delay[ch].read(d));
                self.delay[ch].write(self.del[ch][i] + DELAY_FEEDBACK * y);
                self.mix[ch][i] += y;
            }
        }

        let [rev_l, rev_r] = &self.rev;
        let [mix_l, mix_r] = &mut self.mix;
        self.reverb.process(
            [&rev_l[..len], &rev_r[..len]],
            [&mut mix_l[..len], &mut mix_r[..len]],
        );

        // Saturation on kick and bass: warmer, and audible on phone speakers; driven harder for grit.
        for i in 0..len {
            let drive = mb.drive[i];
            let s = saturate(self.shaper[i] * drive) / drive;
            mix_l[i] += s;
            mix_r[i] += s;
        }

        // The muffle low-pass while a menu is open.
        for i in 0..len {
            let f = self.muffle_freq.tick();
            if self.countdown == 0 {
                if f != self.muffle_last {
                    self.muffle[0].set(FilterKind::Lowpass, f, DEFAULT_Q, sr);
                    let [a, b] = &mut self.muffle;
                    b.copy_coefs(a);
                    self.muffle_last = f;
                }
                self.countdown = 32;
            }
            self.countdown -= 1;
            mix_l[i] = self.muffle[0].process(mix_l[i]);
            mix_r[i] = self.muffle[1].process(mix_r[i]);
        }

        self.limiter.process(&mut mix_l[..len], &mut mix_r[..len]);

        for i in 0..len {
            let g = self.out.tick();
            out_l[i] += mix_l[i] * g * mb.trim[i];
            out_r[i] += mix_r[i] * g * mb.trim[i];
        }
    }
}

// ---------------------------------------------------------------------------
// The sequencer
// ---------------------------------------------------------------------------

/// Automated parameters, for skipping needless automation.
#[derive(Clone, Copy)]
enum Auto {
    KeysGain,
    KeysFilter,
    PadGain,
    PadFilter,
    ArpFilter,
    DrumsFilter,
    VinylGain,
    DelayTime,
    Reverb,
    DrumReverb,
    DelaySend,
    Drive,
    Trim,
}

const AUTOS: usize = 13;

/// The step sequencer: schedules a 16th step at a time, a little ahead.
pub struct Sequencer {
    step: u64,
    next_time: f64,
    melody: i32,
    /// Last value sent to each automated parameter.
    sent: [Option<f64>; AUTOS],
    pub state: MusicState,
    pub next_tick: f64,
    /// The style the notes are played in...
    style: SoundStyle,
    /// ...the one before it, whose mix holds until `switch_at`...
    prev: SoundStyle,
    switch_at: f64,
    /// ...and the one taking over at the next bar.
    pending: Option<SoundStyle>,
}

impl Default for Sequencer {
    fn default() -> Self {
        Self::new()
    }
}

impl Sequencer {
    pub fn new() -> Self {
        Sequencer {
            step: 0,
            next_time: 0.0,
            melody: 4,
            sent: [None; AUTOS],
            state: MusicState::default(),
            next_tick: 0.0,
            style: SoundStyle::default(),
            prev: SoundStyle::default(),
            switch_at: f64::NEG_INFINITY,
            pending: None,
        }
    }

    pub fn begin(&mut self, now: f64) {
        self.sent = [None; AUTOS];
        self.step = 0;
        self.next_time = now + 0.05;
        self.next_tick = now;
    }

    /// Plays in `style` from now on: right away if the music isn't playing,
    /// otherwise from the next bar.
    pub fn set_style(&mut self, style: SoundStyle, playing: bool) {
        if playing {
            self.pending = (style != self.style).then_some(style);
        } else {
            self.style = style;
            self.prev = style;
            self.switch_at = f64::NEG_INFINITY;
            self.pending = None;
        }
    }

    /// How many 16th steps have been scheduled since the music started.
    pub fn position(&self) -> u64 {
        self.step
    }

    /// The style the notes are played in (not counting a pending switch).
    pub fn style(&self) -> &SoundStyle {
        &self.style
    }

    /// The style taking over at the next bar, if any.
    pub fn pending_style(&self) -> Option<&SoundStyle> {
        self.pending.as_ref()
    }

    /// The style of the mix at time `t`.
    fn style_at(&self, t: f64) -> &SoundStyle {
        if t >= self.switch_at {
            &self.style
        } else {
            &self.prev
        }
    }

    /// Follows the intensity and schedules the notes of the next quarter second.
    pub fn tick(
        &mut self,
        now: f64,
        g: &mut MusicGraph,
        voices: &mut Voices,
        rng: &mut Rng,
        white_len: usize,
        sr: f64,
    ) {
        self.next_tick += TICK;
        if self.next_tick <= now {
            self.next_tick = now + TICK;
        }
        self.apply_levels(g, now);
        let mut ins = Instruments {
            voices,
            rng,
            sr,
            white_len,
        };
        self.schedule(g, &mut ins, now, now + LOOKAHEAD);
    }

    /// Continuous parameters that follow the intensity and the style.
    fn apply_levels(&mut self, g: &mut MusicGraph, t: f64) {
        let e = self.state.e as f64;
        let bpm = self.state.bpm as f64;
        let style = *self.style_at(t);
        let mix = style.timbre().mix;
        let tone = style.tone();
        let sent = &mut self.sent;
        let mut set = |which: Auto, p: &mut Param, v: f64, tau: f64| {
            let last = &mut sent[which as usize];
            if let Some(l) = *last
                && (v - l).abs() <= 0.004 * l.abs().max(1.0)
            {
                return;
            }
            p.set_target_at(v, t, tau);
            *last = Some(v);
        };
        let b = &mut g.buses;
        // The keys' filter drifts slowly, like a hand on a knob.
        let drift = (t / 7.0).sin() * 0.6 + (t / 3.1).sin() * 0.4;
        set(
            Auto::KeysGain,
            &mut b[Bus::Keys as usize].gain,
            1.0 - ramp(e, 0.45, 0.65),
            0.15,
        );
        set(
            Auto::KeysFilter,
            filter_freq(&mut b[Bus::Keys as usize]),
            (1450.0 + 550.0 * drift + 2500.0 * e) * mix.keys_tone * tone,
            0.15,
        );
        set(
            Auto::PadGain,
            &mut b[Bus::Pad as usize].gain,
            ramp(e, 0.5, 0.65),
            0.15,
        );
        set(
            Auto::PadFilter,
            filter_freq(&mut b[Bus::Pad as usize]),
            (700.0 + 3500.0 * ramp(e, 0.5, 1.0)) * tone,
            0.15,
        );
        set(
            Auto::ArpFilter,
            filter_freq(&mut b[Bus::Arp as usize]),
            (1500.0 + 4000.0 * ramp(e, 0.75, 1.0)) * tone,
            0.15,
        );
        set(
            Auto::DrumsFilter,
            filter_freq(&mut b[Bus::Drums as usize]),
            4500.0 + 14000.0 * ramp(e, 0.3, 0.6),
            0.15,
        );
        set(
            Auto::VinylGain,
            &mut b[Bus::Vinyl as usize].gain,
            (1.0 - ramp(e, 0.25, 0.5)) * mix.hiss,
            0.15,
        );
        // Echoes stay on the beat at any tempo.
        set(Auto::DelayTime, &mut g.delay_time, 30.0 / bpm, 0.3);
        // The style's mix glides over when it changes.
        let m = &mut g.mix_params;
        set(Auto::Reverb, &mut m.reverb, mix.reverb, 0.3);
        set(Auto::DrumReverb, &mut m.drum_reverb, mix.drum_reverb, 0.3);
        set(Auto::DelaySend, &mut m.delay, mix.delay, 0.3);
        set(Auto::Drive, &mut m.drive, mix.drive, 0.3);
        set(Auto::Trim, &mut m.trim, mix.trim, 0.3);
    }

    fn schedule(&mut self, g: &mut MusicGraph, ins: &mut Instruments, now: f64, until: f64) {
        // After a stall skip ahead instead of playing catch-up.
        if self.next_time < now - 0.05 {
            self.next_time = now + 0.02;
        }
        while self.next_time < until {
            let dur = 15.0 / self.state.bpm as f64;
            self.play_step(g, ins, self.step, self.next_time, dur);
            self.next_time += dur;
            self.step += 1;
        }
    }

    fn play_step(&mut self, g: &mut MusicGraph, ins: &mut Instruments, n: u64, t: f64, dur: f64) {
        let e = self.state.e as f64;
        let speed = self.state.speed as f64;
        let bar = n / STEPS;
        let s = n % STEPS;

        // A new style takes over on the downbeat.
        if s == 0
            && let Some(next) = self.pending.take()
        {
            self.prev = self.style;
            self.style = next;
            self.switch_at = t;
        }
        let style = self.style;
        let tb = style.timbre();
        let kit = &tb.kit;
        let tone = style.tone();
        let tr = style.tint.transpose as i32;
        let prog = progression(style.mode());

        let chord = chord_at(bar, s);
        let root = (prog.roots[chord] + tr) as f64;
        let last_of_8 = bar % 8 == 7;

        // Stage envelopes.
        let lofi = 1.0 - ramp(e, 0.45, 0.6);
        let lofi_drums = (1.0 - ramp(e, 0.4, 0.55)) * if bar % 16 == 15 { 0.0 } else { 1.0 };
        let swung = t + SWING[(s % 4) as usize] * dur * (1.0 - ramp(e, 0.35, 0.55));

        // ══════════ LOFI ══════════

        // Chords on the keys, held until the next hit.
        let mut key_hits = [0u64; 3];
        let mut k = 0;
        for &h in KEYS_HITS[(bar % 4) as usize] {
            key_hits[k] = h;
            k += 1;
        }
        if last_of_8 && !key_hits[..k].contains(&8) {
            key_hits[k] = 8;
            k += 1;
        }
        let key_hits = &mut key_hits[..k];
        key_hits.sort_unstable();
        if key_hits.contains(&s) && ramp(e, 0.45, 0.65) < 0.999 {
            let next = key_hits.iter().copied().find(|&h| h > s).unwrap_or(STEPS);
            let hold = (next - s) as f64 * dur * 0.95;
            let vol = 0.07 * tb.keys.gain;
            ins.keys(Bus::Keys, swung, &prog.jazz[chord], tr, vol, hold, &tb.keys);
        }

        // Bass on the root, with extra notes once lofi livens up.
        if lofi > 0.001 {
            let b = &tb.bass;
            let filtered = b.cutoff > 0.0;
            let bass = |vol: f64| ToneOpts {
                cutoff: filtered.then_some(b.cutoff * tone),
                q: filtered.then_some(b.q),
                ..opts(b.wave, root, vol * b.gain)
            };
            if LOFI_BASS[(bar % 2) as usize].contains(&s) {
                ins.tone(
                    Bus::Bass,
                    swung,
                    ToneOpts {
                        attack: Some(0.005),
                        decay: Some(0.4),
                        sustain: Some(0.6),
                        hold: Some(dur * 1.8),
                        release: Some(0.2),
                        ..bass(0.5 * lofi)
                    },
                );
            }
            let extra = 0.42 * lofi * ramp(e, 0.15, 0.3);
            if extra > 0.001 && LOFI_BASS_EXTRA.contains(&s) {
                ins.tone(
                    Bus::Bass,
                    swung,
                    ToneOpts {
                        attack: Some(0.005),
                        decay: Some(0.25),
                        sustain: Some(0.4),
                        hold: Some(dur * 1.2),
                        release: Some(0.15),
                        ..bass(extra)
                    },
                );
            }
        }

        // Melody: a random walk over the pentatonic scale, busier with speed and intensity.
        let melody_vol = 0.18 * (1.0 - ramp(e, 0.45, 0.65));
        if melody_vol > 0.001 && chance(ins.rng, 0.12 + 0.35 * speed + 0.15 * e) {
            let walk = [-2, -1, -1, 1, 1, 2][(ins.rng.next() * 6.0).floor() as usize];
            self.melody = (self.melody + walk).clamp(0, PENTA.len() as i32 - 1);
            let mut note = PENTA[self.melody as usize];
            if prog.dominant[chord] && note % 12 == 7 {
                note += 1;
            }
            if chance(ins.rng, 0.15) {
                note += 12;
            }
            let pan = ins.rng.next() * 0.8 - 0.4;
            if let Some(p) = &mut g.buses[Bus::Melody as usize].pan {
                p.set_value_at(pan, swung);
            }
            let note = (note + tr) as f64;
            let vol = melody_vol * tb.melody.gain;
            ins.play(Bus::Melody, swung, note, vol, None, &tb.melody);
        }

        // Swung boom-bap; every 16th bar the drums drop out for a bar.
        if lofi_drums > 0.001 {
            if LOFI_KICK[(bar % 2) as usize].contains(&s) {
                ins.kick(Bus::Kick, swung, 0.75 * lofi_drums, 0.0, &kit.kick);
            }
            if s == 4 || s == 12 {
                ins.snare(Bus::Drums, swung, 0.4 * lofi_drums, 0.2, &kit.snare);
                if chance(ins.rng, 0.1) {
                    ins.snare(Bus::Drums, swung + dur, 0.18 * lofi_drums, 0.12, &kit.snare);
                }
                let clap = 0.22 * ramp(e, 0.25, 0.4) * lofi_drums;
                if clap > 0.001 {
                    ins.clap(Bus::Drums, swung + 0.006, clap, &kit.clap);
                }
            }
            // 8th hats (16ths in the 8th bar), loose and humanised.
            let hats16 = last_of_8;
            if (s.is_multiple_of(2) || hats16) && chance(ins.rng, 0.85) {
                let vol = (0.05 + 0.08 * ins.rng.next()) * lofi_drums;
                ins.hat(Bus::Drums, swung, vol, 0.03, 7500.0, &kit.hat);
            }
            // In-between 16ths once it livens up.
            if !hats16 && s % 2 == 1 && chance(ins.rng, ramp(e, 0.15, 0.3) * (0.3 + 0.6 * speed)) {
                ins.hat(Bus::Drums, swung, 0.06 * lofi_drums, 0.02, 9000.0, &kit.hat);
            }
            // Ghost snares follow the player's speed.
            if s != 4 && s != 12 && chance(ins.rng, 0.3 * ramp(speed, 0.35, 0.6)) {
                ins.snare(Bus::Drums, swung, 0.07 * lofi_drums, 0.06, &kit.snare);
            }
        }

        // Vinyl crackle, or whatever the material crackles with.
        let vinyl = 1.0 - ramp(e, 0.25, 0.5);
        if vinyl > 0.001 {
            match kit.texture {
                Texture::Vinyl => {
                    if chance(ins.rng, 0.15) {
                        let at = t + ins.rng.next() * dur;
                        let vol = (0.02 + 0.06 * ins.rng.next()) * vinyl;
                        ins.crackle(Bus::Vinyl, at, vol);
                    }
                }
                Texture::Paper => {
                    if chance(ins.rng, 0.2) {
                        let at = t + ins.rng.next() * dur;
                        let vol = (0.03 + 0.07 * ins.rng.next()) * vinyl;
                        ins.rustle(Bus::Vinyl, at, vol);
                    }
                }
                Texture::Pops => {
                    if chance(ins.rng, 0.06) {
                        let at = t + ins.rng.next() * dur;
                        let vol = (0.03 + 0.05 * ins.rng.next()) * vinyl;
                        ins.bubble(Bus::Vinyl, at, vol);
                    }
                }
                Texture::Sleigh => {
                    if chance(ins.rng, 0.05) {
                        let at = t + ins.rng.next() * dur;
                        let vol = (0.02 + 0.03 * ins.rng.next()) * vinyl;
                        ins.jingle(Bus::Melody, at, vol, 0.06, 6500.0);
                    }
                }
                Texture::Sparkle => {
                    if chance(ins.rng, 0.05) {
                        let at = t + ins.rng.next() * dur;
                        let note = PENTA[ins.rng.int(PENTA.len())] + 24 + tr;
                        let vol = (0.006 + 0.012 * ins.rng.next()) * vinyl;
                        ins.bell(Bus::Melody, at, note as f64, vol, 0.4);
                    }
                }
                Texture::Tape => {}
            }
        }

        // The material's own percussion.
        match kit.accent {
            Accent::None => {}
            Accent::Woodblock => {
                if matches!(s, 3 | 11 | 14) && chance(ins.rng, 0.35 + 0.4 * speed) {
                    let vol = 0.07 * (0.6 + 0.4 * ramp(e, 0.2, 0.6));
                    ins.woodblock(Bus::Drums, swung, vol, s != 14);
                }
            }
            Accent::Anvil => {
                if s == 14 && bar % 2 == 1 && chance(ins.rng, 0.7) {
                    let vol = 0.06 * (0.4 + 0.6 * ramp(e, 0.3, 0.7));
                    ins.anvil(Bus::Drums, t, vol);
                }
            }
            Accent::Timpani => {
                let build = last_of_8 && s >= 8 && s.is_multiple_of(2);
                if (s == 0 && bar.is_multiple_of(4)) || build {
                    let p = if build { s as f64 / STEPS as f64 } else { 1.0 };
                    let vol = 0.3 * p * (0.5 + 0.5 * ramp(e, 0.3, 0.7));
                    ins.timpani(Bus::Fx, t, root + 12.0, vol);
                }
            }
            Accent::Chime => {
                if (s == 0 && bar.is_multiple_of(2) && chance(ins.rng, 0.7))
                    || (s == 8 && chance(ins.rng, 0.25))
                {
                    let note = prog.triads[chord][2] + 24 + tr;
                    ins.bell(Bus::Melody, t, note as f64, 0.025, 1.2);
                }
            }
        }

        // ══════════ HOUSE → EDM ══════════

        // Four-on-the-floor; in the last of 8 bars the final kick drops out before the downbeat.
        let kick = 0.85 * ramp(e, 0.35, 0.5);
        if kick > 0.001 && s.is_multiple_of(4) && !(last_of_8 && s == 12) {
            ins.kick(Bus::Kick, t, kick, ramp(e, 0.35, 0.8), &kit.kick);
        }

        // Sidechain pump on the chords from every beat, deeper as it gets more intense.
        if s.is_multiple_of(4) {
            let depth = 0.85 * ramp(e, 0.45, 0.7);
            g.pump.set_value_at(1.0 - depth, t);
            g.pump.set_target_at(1.0, t + 0.01, dur * 1.1);
        }

        let clap = 0.5 * ramp(e, 0.5, 0.6);
        if clap > 0.001 && (s == 4 || s == 12) {
            ins.clap(Bus::Drums, t, clap, &kit.clap);
        }

        let off_hat = 0.28 * ramp(e, 0.4, 0.55);
        if off_hat > 0.001 && s % 4 == 2 {
            ins.hat(Bus::Drums, t, off_hat, 0.09, 6000.0, &kit.hat);
        }

        let hats = ramp(e, 0.7, 0.85);
        if hats > 0.001 && s % 4 != 2 && chance(ins.rng, 0.85 * hats) {
            ins.hat(
                Bus::Drums,
                t,
                0.09 + 0.05 * if s.is_multiple_of(2) { 1.0 } else { 0.0 },
                0.025,
                8500.0,
                &kit.hat,
            );
        }

        // Offbeat bass, replaced by a rolling bass at the top.
        let sb = &tb.synth_bass;
        let cutoff = (500.0 + 1500.0 * e) * sb.cutoff * tone;
        let off_bass = 0.3 * ramp(e, 0.45, 0.6) * (1.0 - ramp(e, 0.82, 0.92));
        if off_bass > 0.001 && s % 4 == 2 {
            ins.tone(
                Bus::Bass,
                t,
                ToneOpts {
                    decay: Some(0.16),
                    sustain: Some(0.2),
                    hold: Some(dur * 1.5),
                    release: Some(0.05),
                    cutoff: Some(cutoff * 2.0),
                    cutoff_end: Some(cutoff),
                    q: Some(3.0 * sb.q),
                    ..opts(sb.wave, root, off_bass * sb.gain)
                },
            );
        }
        let roll = 0.26 * ramp(e, 0.82, 0.92);
        if roll > 0.001 && !s.is_multiple_of(4) {
            ins.tone(
                Bus::Bass,
                t,
                ToneOpts {
                    decay: Some(0.09),
                    sustain: Some(0.1),
                    hold: Some(dur * 0.8),
                    release: Some(0.03),
                    cutoff: Some((800.0 + 1800.0 * e) * sb.cutoff * tone * 1.8),
                    cutoff_end: Some(600.0 * sb.cutoff * tone),
                    q: Some(4.0 * sb.q),
                    ..opts(sb.wave, root, roll * sb.gain)
                },
            );
        }

        // Two detuned voices per chord note, held for the chord; the pump makes them breathe.
        if (s == 0 || (last_of_8 && s == 8)) && ramp(e, 0.5, 0.65) > 0.001 {
            let len = (if last_of_8 { 8 } else { STEPS }) as f64 * dur;
            let tri = prog.triads[chord];
            let pad = &tb.pad;
            for note in [prog.roots[chord] + 12, tri[0], tri[1], tri[2]] {
                for detune in [-pad.detune, pad.detune] {
                    ins.tone(
                        Bus::Pad,
                        t,
                        ToneOpts {
                            detune,
                            attack: Some(pad.attack),
                            decay: Some(0.3),
                            sustain: Some(0.8),
                            hold: Some(len),
                            release: Some(0.12),
                            ..opts(pad.wave, (note + tr) as f64, 0.034 * pad.gain)
                        },
                    );
                }
            }
        }

        // Arpeggiated lead over the chord tones, sweeping across the stereo field.
        let arp = 0.1 * ramp(e, 0.75, 0.9);
        if arp > 0.001 {
            let i = ARP[s as usize];
            let tri = prog.triads[chord];
            if let Some(p) = &mut g.buses[Bus::Arp as usize].pan {
                p.set_value_at(
                    0.4 * (s as f64 / STEPS as f64 * std::f64::consts::PI * 4.0).sin(),
                    t,
                );
            }
            let note = tri[i % 3] + 12 * (i / 3) as i32 + tr;
            ins.play(Bus::Arp, t, note as f64, arp * tb.arp.gain, None, &tb.arp);
        }

        // Build every 8 bars: a snare roll and a noise riser into the next downbeat, then a crash.
        let build = ramp(e, 0.65, 0.8);
        if build > 0.001 && last_of_8 {
            let p = s as f64 / STEPS as f64;
            ins.snare(Bus::Fx, t, (0.04 + 0.26 * p * p) * build, 0.08, &kit.snare);
            if s >= 8 {
                let vol = (0.04 + 0.26 * p * p) * build;
                ins.snare(Bus::Fx, t + dur / 2.0, vol, 0.06, &kit.snare);
            }
            if s == 0 {
                ins.riser(Bus::Fx, t, STEPS as f64 * dur, 0.22 * build);
            }
        }
        let crash = 0.12 * ramp(e, 0.6, 0.75);
        if crash > 0.001 && bar > 0 && bar.is_multiple_of(8) && s == 0 {
            ins.crash(Bus::Fx, t, crash, kit.crash);
        }
    }
}

fn chance(rng: &mut Rng, p: f64) -> bool {
    rng.next() < p
}

fn filter_freq(b: &mut BusState) -> &mut Param {
    &mut b.filter.as_mut().expect("bus has a filter").0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plays_the_progression_over_8_bars_with_two_chords_in_the_last_one() {
        assert_eq!(
            (0..8).map(|bar| chord_at(bar, 0)).collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 4, 5, 6, 7]
        );
        assert_eq!(chord_at(7, 8), 8);
        assert_eq!(chord_at(15, 15), 8);
        assert_eq!(chord_at(8, 0), 0);
    }

    #[test]
    fn every_progression_keeps_its_voicings_over_its_roots() {
        for mode in [Mode::Dorian, Mode::Minor, Mode::Major] {
            let p = progression(mode);
            for c in 0..9 {
                // The bass stays low, the voicings in the middle, the triads above.
                assert!((36..=45).contains(&p.roots[c]), "{mode:?} {c}");
                assert!(p.jazz[c].iter().all(|n| (50..=64).contains(n)), "{mode:?} {c}");
                assert!(p.triads[c].iter().all(|n| (62..=72).contains(n)), "{mode:?} {c}");
                // The triad is built on the root.
                assert!(p.triads[c].iter().any(|n| n % 12 == p.roots[c] % 12), "{mode:?} {c}");
                if p.dominant[c] {
                    assert_eq!(p.roots[c] % 12, 4, "{mode:?} {c}: dominant E");
                }
            }
        }
    }
}
