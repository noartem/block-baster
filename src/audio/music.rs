//! Adaptive soundtrack, synthesised live. It drifts from relaxed lofi through
//! chill house and house to fast EDM as the score grows and the player speeds
//! up:
//!
//!   e 0    – 0.25  lofi: jazz chords on an FM e-piano, triangle bass, swung boom-bap, a sparse melody, vinyl
//!   e 0.15 – 0.4   livelier lofi: more bass notes, 16th hats, a clap; the vinyl fades
//!   e 0.35 – 0.6   chill house: four-on-the-floor kick and offbeat hats replace the swung drums
//!   e 0.5  – 0.75  house: clap on 2 and 4, pumping saw chords, the e-piano leaves
//!   e 0.65 – 0.85  builds every 8 bars: snare roll, noise riser, the kick drops out before the downbeat
//!   e 0.75 – 1     EDM: arpeggiated lead, 16th hats, rolling bass
//!
//! Layers fade in and out between two thresholds of `e` instead of switching
//! abruptly. Lofi and EDM layers share one progression (jazz voicings vs. plain
//! triads over the same roots), so they never clash during a crossfade.

use super::dsp::{
    Biquad, Compressor, DelayLine, FilterKind, Param, QUANTUM, Reverb, midi_to_hz, saturate,
};
use super::engine::{BUSES, Bus, DEFAULT_Q, Dest, NoiseBuf, Source, Voice, VoiceFilter, Voices};
use super::intensity::MusicState;
use crate::game::rng::Rng;
use crate::styles::Wave;

/// Gain of the music, before the volume.
pub const VOLUME: f32 = 0.4;
/// Notes are scheduled this far ahead...
const LOOKAHEAD: f64 = 0.25;
/// ...every this many seconds.
const TICK: f64 = 0.04;
const STEPS: u64 = 16;

// One progression, 8 bars; the last bar holds two chords (Gm9 → C9):
// Dm9, G13, C^9, Am9, F^9, E7b9, Am9, Gm9, C9.
const ROOTS: [i32; 9] = [38, 43, 36, 45, 41, 40, 45, 43, 36];
/// Jazz voicings for the lofi e-piano.
const JAZZ: [[i32; 4]; 9] = [
    [53, 57, 60, 64], // Dm9
    [53, 57, 59, 64], // G13
    [52, 55, 59, 62], // C^9
    [52, 55, 59, 60], // Am9
    [52, 55, 57, 60], // F^9
    [50, 53, 56, 59], // E7b9
    [52, 55, 59, 60], // Am9
    [53, 57, 58, 62], // Gm9
    [52, 55, 58, 62], // C9
];
/// Plain triads over the same roots for house and EDM.
const TRIADS: [[i32; 3]; 9] = [
    [62, 65, 69], // Dm
    [62, 67, 71], // G
    [64, 67, 72], // C
    [64, 69, 72], // Am
    [65, 69, 72], // F
    [64, 68, 71], // E
    [64, 69, 72], // Am
    [62, 67, 70], // Gm
    [64, 67, 72], // C
];
const E_CHORD: usize = 5;

/// C major pentatonic from C4; over E7b9 the G is raised to G#.
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

    /// A chord on a two-operator FM electric piano, with a gentle shared vibrato.
    fn epiano(&mut self, dest: Bus, t: f64, notes: &[i32], vol: f64, hold: f64) {
        let stop = self.sample(t + hold + 2.0);
        for &note in notes {
            let f = midi_to_hz(note as f64);
            // Players never hit all keys at once.
            let at = t + self.rng.next() * 0.012;
            let mut v = self.tone_voice(
                dest,
                at,
                ToneOpts {
                    attack: Some(0.01),
                    decay: Some(1.6),
                    sustain: Some(0.35),
                    hold: Some(hold),
                    release: Some(1.2),
                    ..opts(Wave::Sine, note as f64, vol)
                },
            );
            // Bright tine at the attack that settles into a soft, round tone.
            let mut depth = Param::starting_at((f * 1.8) as f32, self.sr, v.start);
            depth.set_value_at(f * 1.8, t);
            depth.set_target_at(f * 1.8 * 0.15, t, 0.25);
            // The vibrato LFO started with the chord.
            let lfo_phase = (5.0 * (v.start as f64 / self.sr - t)).rem_euclid(1.0);
            v.source = Source::Fm {
                phase: 0.0,
                mod_phase: 0.0,
                freq: f,
                depth,
                lfo_phase,
            };
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
        let first = self.sample(t);
        let mut gain = Param::starting_at(0.0, self.sr, first);
        let stop = Self::envelope(
            &mut gain,
            t,
            vol,
            Env {
                attack: Some(o.attack.unwrap_or(0.001)),
                decay: Some(o.decay),
                ..Env::default()
            },
        );
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
    fn kick(&mut self, dest: Bus, t: f64, vol: f64, punch: f64) {
        let first = self.sample(t);
        let from = 120.0 + 110.0 * punch;
        let to = 46.0 + 6.0 * punch;
        let decay = 0.42 - 0.12 * punch;
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
            vol * (0.1 + 0.25 * punch),
            Hit {
                q: Some(0.8),
                ..hit(FilterKind::Bandpass, 2200.0 + 1800.0 * punch, 0.012)
            },
        );
    }

    fn snare(&mut self, dest: Bus, t: f64, vol: f64, decay: f64) {
        self.noise_hit(
            dest,
            t,
            vol,
            Hit {
                q: Some(0.6),
                ..hit(FilterKind::Bandpass, 1900.0, decay)
            },
        );
        self.tone(
            dest,
            t,
            ToneOpts {
                decay: Some(decay * 0.5),
                ..opts(Wave::Triangle, 54.0, vol * 0.6)
            },
        );
    }

    fn clap(&mut self, dest: Bus, t: f64, vol: f64) {
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
        let filter = Some(self.filter(FilterKind::Bandpass, 1150.0, 1.1));
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

    fn hat(&mut self, dest: Bus, t: f64, vol: f64, decay: f64, freq: f64) {
        self.noise_hit(dest, t, vol, hit(FilterKind::Highpass, freq, decay));
    }

    fn crash(&mut self, dest: Bus, t: f64, vol: f64) {
        self.noise_hit(dest, t, vol, hit(FilterKind::Highpass, 4500.0, 1.6));
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
    hiss: usize,
    countdown: u32,
    mix: [[f32; QUANTUM]; 2],
    rev: [[f32; QUANTUM]; 2],
    del: [[f32; QUANTUM]; 2],
    shaper: [f32; QUANTUM],
    pump_buf: [f32; QUANTUM],
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
            hiss: 0,
            countdown: 0,
            mix: [[0.0; QUANTUM]; 2],
            rev: [[0.0; QUANTUM]; 2],
            del: [[0.0; QUANTUM]; 2],
            shaper: [0.0; QUANTUM],
            pump_buf: [0.0; QUANTUM],
        }
    }

    fn gain(&self) -> f64 {
        (VOLUME * self.level * self.level) as f64
    }

    pub fn releasing(&self) -> bool {
        self.active && self.release_at.is_some()
    }

    /// Starts playing, fading in over 1.5 s.
    pub fn begin(&mut self, now: f64, frame: u64, level: f32, paused: bool) {
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

        for (bi, (st, c)) in self.buses.iter_mut().zip(&BUS_CFG).enumerate() {
            let input = &bus[bi][..len];
            let vinyl = bi == Bus::Vinyl as usize;
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
                    self.rev[0][i] += l * c.reverb;
                    self.rev[1][i] += r * c.reverb;
                }
                if c.delay > 0.0 {
                    self.del[0][i] += l * c.delay;
                    self.del[1][i] += r * c.delay;
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

        // Gentle saturation on kick and bass: warmer, and audible on phone speakers.
        for i in 0..len {
            let s = saturate(self.shaper[i]);
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
            out_l[i] += mix_l[i] * g;
            out_r[i] += mix_r[i] * g;
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
}

/// The step sequencer: schedules a 16th step at a time, a little ahead.
pub struct Sequencer {
    step: u64,
    next_time: f64,
    melody: i32,
    /// Last value sent to each automated parameter.
    sent: [Option<f64>; 8],
    pub state: MusicState,
    pub next_tick: f64,
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
            sent: [None; 8],
            state: MusicState::default(),
            next_tick: 0.0,
        }
    }

    pub fn begin(&mut self, now: f64) {
        self.sent = [None; 8];
        self.step = 0;
        self.next_time = now + 0.05;
        self.next_tick = now;
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

    /// Continuous parameters that follow the intensity.
    fn apply_levels(&mut self, g: &mut MusicGraph, t: f64) {
        let e = self.state.e as f64;
        let bpm = self.state.bpm as f64;
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
        // The e-piano filter drifts slowly, like a hand on a knob.
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
            1450.0 + 550.0 * drift + 2500.0 * e,
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
            700.0 + 3500.0 * ramp(e, 0.5, 1.0),
            0.15,
        );
        set(
            Auto::ArpFilter,
            filter_freq(&mut b[Bus::Arp as usize]),
            1500.0 + 4000.0 * ramp(e, 0.75, 1.0),
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
            1.0 - ramp(e, 0.25, 0.5),
            0.15,
        );
        // Echoes stay on the beat at any tempo.
        set(Auto::DelayTime, &mut g.delay_time, 30.0 / bpm, 0.3);
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
        let chord = chord_at(bar, s);
        let root = ROOTS[chord] as f64;
        let last_of_8 = bar % 8 == 7;

        // Stage envelopes.
        let lofi = 1.0 - ramp(e, 0.45, 0.6);
        let lofi_drums = (1.0 - ramp(e, 0.4, 0.55)) * if bar % 16 == 15 { 0.0 } else { 1.0 };
        let swung = t + SWING[(s % 4) as usize] * dur * (1.0 - ramp(e, 0.35, 0.55));

        // ══════════ LOFI ══════════

        // E-piano chords, held until the next hit.
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
            ins.epiano(Bus::Keys, swung, &JAZZ[chord], 0.07, hold);
        }

        // Triangle bass on the root, with extra notes once lofi livens up.
        if lofi > 0.001 {
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
                        ..opts(Wave::Triangle, root, 0.5 * lofi)
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
                        ..opts(Wave::Triangle, root, extra)
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
            if chord == E_CHORD && note % 12 == 7 {
                note += 1;
            }
            if chance(ins.rng, 0.15) {
                note += 12;
            }
            let pan = ins.rng.next() * 0.8 - 0.4;
            if let Some(p) = &mut g.buses[Bus::Melody as usize].pan {
                p.set_value_at(pan, swung);
            }
            let note = note as f64;
            ins.tone(
                Bus::Melody,
                swung,
                ToneOpts {
                    attack: Some(0.005),
                    decay: Some(0.25),
                    release: Some(0.3),
                    ..opts(Wave::Sine, note, melody_vol)
                },
            );
            ins.tone(
                Bus::Melody,
                swung,
                ToneOpts {
                    decay: Some(0.08),
                    ..opts(Wave::Triangle, note + 12.0, melody_vol * 0.12)
                },
            );
        }

        // Swung boom-bap; every 16th bar the drums drop out for a bar.
        if lofi_drums > 0.001 {
            if LOFI_KICK[(bar % 2) as usize].contains(&s) {
                ins.kick(Bus::Kick, swung, 0.75 * lofi_drums, 0.0);
            }
            if s == 4 || s == 12 {
                ins.snare(Bus::Drums, swung, 0.4 * lofi_drums, 0.2);
                if chance(ins.rng, 0.1) {
                    ins.snare(Bus::Drums, swung + dur, 0.18 * lofi_drums, 0.12);
                }
                let clap = 0.22 * ramp(e, 0.25, 0.4) * lofi_drums;
                if clap > 0.001 {
                    ins.clap(Bus::Drums, swung + 0.006, clap);
                }
            }
            // 8th hats (16ths in the 8th bar), loose and humanised.
            let hats16 = last_of_8;
            if (s.is_multiple_of(2) || hats16) && chance(ins.rng, 0.85) {
                let vol = (0.05 + 0.08 * ins.rng.next()) * lofi_drums;
                ins.hat(Bus::Drums, swung, vol, 0.03, 7500.0);
            }
            // In-between 16ths once it livens up.
            if !hats16 && s % 2 == 1 && chance(ins.rng, ramp(e, 0.15, 0.3) * (0.3 + 0.6 * speed)) {
                ins.hat(Bus::Drums, swung, 0.06 * lofi_drums, 0.02, 9000.0);
            }
            // Ghost snares follow the player's speed.
            if s != 4 && s != 12 && chance(ins.rng, 0.3 * ramp(speed, 0.35, 0.6)) {
                ins.snare(Bus::Drums, swung, 0.07 * lofi_drums, 0.06);
            }
        }

        // Vinyl crackle.
        let vinyl = 1.0 - ramp(e, 0.25, 0.5);
        if vinyl > 0.001 && chance(ins.rng, 0.15) {
            let at = t + ins.rng.next() * dur;
            let vol = (0.02 + 0.06 * ins.rng.next()) * vinyl;
            ins.crackle(Bus::Vinyl, at, vol);
        }

        // ══════════ HOUSE → EDM ══════════

        // Four-on-the-floor; in the last of 8 bars the final kick drops out before the downbeat.
        let kick = 0.85 * ramp(e, 0.35, 0.5);
        if kick > 0.001 && s.is_multiple_of(4) && !(last_of_8 && s == 12) {
            ins.kick(Bus::Kick, t, kick, ramp(e, 0.35, 0.8));
        }

        // Sidechain pump on the chords from every beat, deeper as it gets more intense.
        if s.is_multiple_of(4) {
            let depth = 0.85 * ramp(e, 0.45, 0.7);
            g.pump.set_value_at(1.0 - depth, t);
            g.pump.set_target_at(1.0, t + 0.01, dur * 1.1);
        }

        let clap = 0.5 * ramp(e, 0.5, 0.6);
        if clap > 0.001 && (s == 4 || s == 12) {
            ins.clap(Bus::Drums, t, clap);
        }

        let off_hat = 0.28 * ramp(e, 0.4, 0.55);
        if off_hat > 0.001 && s % 4 == 2 {
            ins.hat(Bus::Drums, t, off_hat, 0.09, 6000.0);
        }

        let hats = ramp(e, 0.7, 0.85);
        if hats > 0.001 && s % 4 != 2 && chance(ins.rng, 0.85 * hats) {
            ins.hat(
                Bus::Drums,
                t,
                0.09 + 0.05 * if s.is_multiple_of(2) { 1.0 } else { 0.0 },
                0.025,
                8500.0,
            );
        }

        // Offbeat saw bass, replaced by a rolling bass at the top.
        let cutoff = 500.0 + 1500.0 * e;
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
                    q: Some(3.0),
                    ..opts(Wave::Sawtooth, root, off_bass)
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
                    cutoff: Some((800.0 + 1800.0 * e) * 1.8),
                    cutoff_end: Some(600.0),
                    q: Some(4.0),
                    ..opts(Wave::Sawtooth, root, roll)
                },
            );
        }

        // Two detuned saws per chord note, held for the chord; the pump makes them breathe.
        if (s == 0 || (last_of_8 && s == 8)) && ramp(e, 0.5, 0.65) > 0.001 {
            let len = (if last_of_8 { 8 } else { STEPS }) as f64 * dur;
            let tri = TRIADS[chord];
            for note in [ROOTS[chord] + 12, tri[0], tri[1], tri[2]] {
                for detune in [-9.0, 9.0] {
                    ins.tone(
                        Bus::Pad,
                        t,
                        ToneOpts {
                            detune,
                            attack: Some(0.02),
                            decay: Some(0.3),
                            sustain: Some(0.8),
                            hold: Some(len),
                            release: Some(0.12),
                            ..opts(Wave::Sawtooth, note as f64, 0.034)
                        },
                    );
                }
            }
        }

        // Arpeggiated square lead over the chord tones, sweeping across the stereo field.
        let arp = 0.1 * ramp(e, 0.75, 0.9);
        if arp > 0.001 {
            let i = ARP[s as usize];
            let tri = TRIADS[chord];
            if let Some(p) = &mut g.buses[Bus::Arp as usize].pan {
                p.set_value_at(
                    0.4 * (s as f64 / STEPS as f64 * std::f64::consts::PI * 4.0).sin(),
                    t,
                );
            }
            let note = tri[i % 3] + 12 * (i / 3) as i32;
            ins.tone(
                Bus::Arp,
                t,
                ToneOpts {
                    decay: Some(0.13),
                    release: Some(0.04),
                    ..opts(Wave::Square, note as f64, arp)
                },
            );
        }

        // Build every 8 bars: a snare roll and a noise riser into the next downbeat, then a crash.
        let build = ramp(e, 0.65, 0.8);
        if build > 0.001 && last_of_8 {
            let p = s as f64 / STEPS as f64;
            ins.snare(Bus::Fx, t, (0.04 + 0.26 * p * p) * build, 0.08);
            if s >= 8 {
                ins.snare(Bus::Fx, t + dur / 2.0, (0.04 + 0.26 * p * p) * build, 0.06);
            }
            if s == 0 {
                ins.riser(Bus::Fx, t, STEPS as f64 * dur, 0.22 * build);
            }
        }
        let crash = 0.12 * ramp(e, 0.6, 0.75);
        if crash > 0.001 && bar > 0 && bar.is_multiple_of(8) && s == 0 {
            ins.crash(Bus::Fx, t, crash);
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
}
