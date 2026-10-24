//! Building blocks of the software synth. Together they emulate the parts of
//! the Web Audio API the original sounds were made with: automated parameters,
//! band-limited oscillators, biquad filters, noise buffers, delays, a reverb,
//! the waveshaper and the dynamics compressor. Nothing here allocates once
//! constructed, so it is all safe to run on the audio thread.

use crate::game::rng::Rng;
use crate::styles::Wave;

/// Web Audio renders in blocks of 128 frames, and so does the engine.
pub const QUANTUM: usize = 128;

pub fn midi_to_hz(m: f64) -> f64 {
    440.0 * 2f64.powf((m - 69.0) / 12.0)
}

// ---------------------------------------------------------------------------
// Automation
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Set,
    Linear,
    Exp,
    Target,
}

#[derive(Clone, Copy, Debug)]
struct Event {
    kind: Kind,
    /// In samples.
    time: f64,
    value: f64,
    /// Time constant of a setTarget, in samples.
    tau: f64,
}

const NO_EVENT: Event = Event {
    kind: Kind::Set,
    time: 0.0,
    value: 0.0,
    tau: 0.0,
};

/// More pending events than any sound schedules on one parameter (the clap has 9).
pub const MAX_EVENTS: usize = 12;

#[derive(Clone, Copy, Debug)]
enum Mode {
    Hold,
    /// Added per sample.
    Linear(f64),
    /// Multiplied per sample.
    Exp(f64),
    /// Exponential approach: `value = target + (value - target) * k` per sample.
    Target {
        target: f64,
        k: f64,
    },
}

/// An `AudioParam`: a value with a timeline of automation events, evaluated
/// sample by sample (a-rate). Times are given in seconds of engine time.
///
/// It follows the Web Audio timeline rules, including Chrome's habit of
/// replacing an event of the same type at the same time. The one deliberate
/// difference: `cancel` holds the current value instead of jumping back to the
/// last remaining event (like `cancelAndHoldAtTime`).
#[derive(Clone, Debug)]
pub struct Param {
    sr: f64,
    value: f64,
    /// The next sample `tick` returns.
    pos: u64,
    events: [Event; MAX_EVENTS],
    len: usize,
    mode: Mode,
    /// Time (samples) and value of the last event that has passed, where a
    /// following ramp starts. None after a setTarget: a ramp then starts from
    /// the current value.
    anchor: Option<(f64, f64)>,
    /// Time of the first pending event, in samples.
    next: f64,
    dirty: bool,
}

impl Param {
    pub fn new(value: f32, sr: f64) -> Self {
        Param {
            sr,
            value: value as f64,
            pos: 0,
            events: [NO_EVENT; MAX_EVENTS],
            len: 0,
            mode: Mode::Hold,
            anchor: None,
            next: f64::INFINITY,
            dirty: false,
        }
    }

    /// A fresh parameter whose first `tick` is sample `pos`.
    pub fn starting_at(value: f32, sr: f64, pos: u64) -> Self {
        let mut p = Param::new(value, sr);
        p.pos = pos;
        p.anchor = Some((pos as f64, value as f64));
        p
    }

    /// Drops all events and jumps to `value`; the next `tick` is sample `pos`.
    pub fn reset(&mut self, value: f32, pos: u64) {
        *self = Param::starting_at(value, self.sr, pos);
    }

    /// The value at the last sample returned by `tick`.
    pub fn value(&self) -> f32 {
        self.value as f32
    }

    /// True while no automation is running or pending, so the value stays put.
    pub fn is_constant(&self) -> bool {
        self.len == 0 && !self.dirty && matches!(self.mode, Mode::Hold)
    }

    pub fn set_value_at(&mut self, value: f64, time: f64) {
        self.insert(Event {
            kind: Kind::Set,
            time: time * self.sr,
            value,
            tau: 0.0,
        });
    }

    pub fn linear_ramp_to(&mut self, value: f64, time: f64) {
        self.insert(Event {
            kind: Kind::Linear,
            time: time * self.sr,
            value,
            tau: 0.0,
        });
    }

    pub fn exp_ramp_to(&mut self, value: f64, time: f64) {
        self.insert(Event {
            kind: Kind::Exp,
            time: time * self.sr,
            value,
            tau: 0.0,
        });
    }

    pub fn set_target_at(&mut self, value: f64, time: f64, tau: f64) {
        if tau <= 0.0 {
            self.set_value_at(value, time);
        } else {
            self.insert(Event {
                kind: Kind::Target,
                time: time * self.sr,
                value,
                tau: tau * self.sr,
            });
        }
    }

    /// Removes the events at or after `time`, holding the current value.
    pub fn cancel(&mut self, time: f64) {
        let t = time * self.sr;
        while self.len > 0 && self.events[self.len - 1].time >= t {
            self.len -= 1;
        }
        self.dirty = true;
    }

    fn insert(&mut self, ev: Event) {
        let mut i = 0;
        while i < self.len {
            let e = self.events[i];
            if e.time == ev.time && e.kind == ev.kind {
                self.events[i] = ev;
                self.dirty = true;
                return;
            }
            if e.time > ev.time {
                break;
            }
            i += 1;
        }
        if self.len == MAX_EVENTS {
            debug_assert!(false, "too many automation events");
            return;
        }
        self.events.copy_within(i..self.len, i + 1);
        self.events[i] = ev;
        self.len += 1;
        self.dirty = true;
    }

    /// Returns the value at the next sample and advances by one.
    #[inline]
    pub fn tick(&mut self) -> f32 {
        let n = self.pos;
        if self.dirty || n as f64 >= self.next {
            self.recompute(n);
        } else {
            match self.mode {
                Mode::Hold => {}
                Mode::Linear(inc) => self.value += inc,
                Mode::Exp(mul) => self.value *= mul,
                Mode::Target { target, k } => {
                    self.value = target + (self.value - target) * k;
                    if (self.value - target).abs() <= 1e-9 * target.abs().max(1e-3) {
                        self.value = target;
                        self.mode = Mode::Hold;
                    }
                }
            }
        }
        self.pos = n + 1;
        self.value as f32
    }

    /// Fills `out` with the next values.
    pub fn fill(&mut self, out: &mut [f32]) {
        if self.is_constant() {
            out.fill(self.value as f32);
            self.pos += out.len() as u64;
        } else {
            for o in out {
                *o = self.tick();
            }
        }
    }

    /// Evaluates the timeline at sample `n` from the current value.
    fn recompute(&mut self, n: u64) {
        self.dirty = false;
        let nf = n as f64;
        // Start the events whose time has come, in order.
        let mut done = 0;
        while done < self.len && self.events[done].time <= nf {
            let ev = self.events[done];
            match ev.kind {
                Kind::Set | Kind::Linear | Kind::Exp => {
                    self.value = ev.value;
                    self.anchor = Some((ev.time, ev.value));
                    self.mode = Mode::Hold;
                }
                Kind::Target => {
                    // Runs until the next started event, or until now.
                    let until = if done + 1 < self.len && self.events[done + 1].time <= nf {
                        self.events[done + 1].time
                    } else {
                        nf
                    };
                    self.value = ev.value
                        + (self.value - ev.value) * (-(until - ev.time).max(0.0) / ev.tau).exp();
                    self.mode = Mode::Target {
                        target: ev.value,
                        k: (-1.0 / ev.tau).exp(),
                    };
                    self.anchor = None;
                }
            }
            done += 1;
        }
        if done > 0 {
            self.events.copy_within(done..self.len, 0);
            self.len -= done;
        }
        // A ramp runs from the previous event towards the next one.
        if self.len > 0 && matches!(self.events[0].kind, Kind::Linear | Kind::Exp) {
            let ev = self.events[0];
            let (t0, v0) = self.anchor.unwrap_or((nf, self.value));
            let span = (ev.time - t0).max(1e-9);
            let x = ((nf - t0) / span).clamp(0.0, 1.0);
            if ev.kind == Kind::Linear {
                self.value = v0 + (ev.value - v0) * x;
                self.mode = Mode::Linear((ev.value - v0) / span);
            } else if v0 * ev.value > 0.0 {
                let ratio = ev.value / v0;
                self.value = v0 * ratio.powf(x);
                self.mode = Mode::Exp(ratio.powf(1.0 / span));
            } else {
                // An exponential ramp can't cross or touch zero: hold until the event.
                self.value = v0;
                self.mode = Mode::Hold;
            }
        } else if matches!(self.mode, Mode::Linear(_) | Mode::Exp(_)) {
            // The ramp's target was cancelled.
            self.mode = Mode::Hold;
        }
        self.next = if self.len > 0 {
            self.events[0].time
        } else {
            f64::INFINITY
        };
    }
}

// ---------------------------------------------------------------------------
// Oscillators
// ---------------------------------------------------------------------------

/// sin(2πx), accurate to ~1e-7, much faster than `f32::sin` with its range reduction.
#[inline]
pub fn sin_turns(x: f64) -> f32 {
    let mut y = (x - x.round()) as f32; // -0.5..0.5
    if y > 0.25 {
        y = 0.5 - y;
    } else if y < -0.25 {
        y = -0.5 - y;
    }
    let z = y * std::f32::consts::TAU; // -π/2..π/2
    let z2 = z * z;
    z * (1.0
        + z2 * (-1.0 / 6.0
            + z2 * (1.0 / 120.0
                + z2 * (-1.0 / 5040.0 + z2 * (1.0 / 362_880.0 + z2 * (-1.0 / 39_916_800.0))))))
}

/// PolyBLEP residual for a step at phase 0 (`t` in 0..1, `dt` the phase increment).
#[inline]
fn blep(t: f64, dt: f64) -> f64 {
    if t < dt {
        let t = t / dt;
        t + t - t * t - 1.0
    } else if t > 1.0 - dt {
        let t = (t - 1.0) / dt;
        t * t + t + t + 1.0
    } else {
        0.0
    }
}

/// Web Audio normalises its built-in square and sawtooth waves so the peak of
/// the band-limited table (with its Gibbs overshoot of ~1.18) is 1.
const BUILTIN_NORM: f64 = 0.848;

/// One sample of a waveform at `phase` (0..1); `dt` is the phase increment per
/// sample. Phases match Web Audio's: every wave starts at 0 going up.
#[inline]
pub fn wave_sample(wave: Wave, phase: f64, dt: f64) -> f32 {
    match wave {
        Wave::Sine => sin_turns(phase),
        Wave::Square => {
            let dt = dt.abs().min(0.5);
            let half = (phase + 0.5).fract();
            let naive = if phase < 0.5 { 1.0 } else { -1.0 };
            ((naive + blep(phase, dt) - blep(half, dt)) * BUILTIN_NORM) as f32
        }
        Wave::Sawtooth => {
            let dt = dt.abs().min(0.5);
            let t = (phase + 0.5).fract();
            ((2.0 * t - 1.0 - blep(t, dt)) * BUILTIN_NORM) as f32
        }
        Wave::Triangle => {
            // Harmonics fall at 12 dB/octave, so the plain shape aliases very little.
            let v = if phase < 0.25 {
                4.0 * phase
            } else if phase < 0.75 {
                2.0 - 4.0 * phase
            } else {
                4.0 * phase - 4.0
            };
            v as f32
        }
    }
}

/// Advances a phase in 0..1 by `dt` (which may be negative).
#[inline]
pub fn advance(phase: &mut f64, dt: f64) {
    *phase += dt;
    if !(0.0..1.0).contains(phase) {
        *phase -= phase.floor();
    }
}

// ---------------------------------------------------------------------------
// Filters
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterKind {
    Lowpass,
    Highpass,
    Bandpass,
}

/// A `BiquadFilterNode` (RBJ cookbook, with the Web Audio conventions: Q of
/// the low- and high-pass is in dB, the band-pass has a 0 dB peak).
#[derive(Clone, Copy, Debug)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Default for Biquad {
    fn default() -> Self {
        Biquad {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            z1: 0.0,
            z2: 0.0,
        }
    }
}

impl Biquad {
    pub fn new(kind: FilterKind, freq: f32, q: f32, sr: f64) -> Self {
        let mut b = Biquad::default();
        b.set(kind, freq, q, sr);
        b
    }

    pub fn set(&mut self, kind: FilterKind, freq: f32, q: f32, sr: f64) {
        let nyquist = sr * 0.5;
        let f = (freq as f64 / nyquist).clamp(0.0, 1.0);
        let q = q as f64;
        let (b0, b1, b2, a0, a1, a2);
        match kind {
            FilterKind::Lowpass | FilterKind::Highpass => {
                let low = kind == FilterKind::Lowpass;
                if f >= 1.0 {
                    // At or above Nyquist: low-pass lets everything through, high-pass nothing.
                    self.coefs(if low { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0, 0.0);
                    return;
                }
                if f <= 0.0 {
                    self.coefs(if low { 0.0 } else { 1.0 }, 0.0, 0.0, 0.0, 0.0);
                    return;
                }
                let w0 = std::f64::consts::PI * f;
                let alpha = w0.sin() / (2.0 * 10f64.powf(q / 20.0));
                let cos = w0.cos();
                if low {
                    b0 = (1.0 - cos) / 2.0;
                    b1 = 1.0 - cos;
                } else {
                    b0 = (1.0 + cos) / 2.0;
                    b1 = -(1.0 + cos);
                }
                b2 = b0;
                a0 = 1.0 + alpha;
                a1 = -2.0 * cos;
                a2 = 1.0 - alpha;
            }
            FilterKind::Bandpass => {
                if f <= 0.0 || f >= 1.0 {
                    self.coefs(0.0, 0.0, 0.0, 0.0, 0.0);
                    return;
                }
                if q <= 0.0 {
                    self.coefs(1.0, 0.0, 0.0, 0.0, 0.0);
                    return;
                }
                let w0 = std::f64::consts::PI * f;
                let alpha = w0.sin() / (2.0 * q);
                b0 = alpha;
                b1 = 0.0;
                b2 = -alpha;
                a0 = 1.0 + alpha;
                a1 = -2.0 * w0.cos();
                a2 = 1.0 - alpha;
            }
        }
        self.coefs(b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0);
    }

    fn coefs(&mut self, b0: f64, b1: f64, b2: f64, a1: f64, a2: f64) {
        self.b0 = b0 as f32;
        self.b1 = b1 as f32;
        self.b2 = b2 as f32;
        self.a1 = a1 as f32;
        self.a2 = a2 as f32;
    }

    /// Copies the coefficients (not the state) of another filter.
    pub fn copy_coefs(&mut self, o: &Biquad) {
        self.b0 = o.b0;
        self.b1 = o.b1;
        self.b2 = o.b2;
        self.a1 = o.a1;
        self.a2 = o.a2;
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        if self.z1.abs() < 1e-25 {
            // Keep denormals out of decaying tails.
            self.z1 = 0.0;
            self.z2 = 0.0;
        }
        y
    }

    pub fn clear(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}

// ---------------------------------------------------------------------------
// Noise and shaping
// ---------------------------------------------------------------------------

/// White noise, uniform in -1..1.
pub fn white_noise(len: usize, rng: &mut Rng) -> Vec<f32> {
    (0..len).map(|_| (rng.next() * 2.0 - 1.0) as f32).collect()
}

/// Pink noise through Paul Kellet's economy filter.
pub fn pink_noise(len: usize, rng: &mut Rng) -> Vec<f32> {
    let (mut b0, mut b1, mut b2) = (0.0f64, 0.0f64, 0.0f64);
    (0..len)
        .map(|_| {
            let w = rng.next() * 2.0 - 1.0;
            b0 = 0.99765 * b0 + w * 0.099046;
            b1 = 0.963 * b1 + w * 0.2965164;
            b2 = 0.57 * b2 + w * 1.0526913;
            ((b0 + b1 + b2 + w * 0.1848) * 0.2) as f32
        })
        .collect()
}

/// The waveshaper curve on kick and bass: tanh(2x) / tanh(2), flat beyond ±1.
#[inline]
pub fn saturate(x: f32) -> f32 {
    const NORM: f32 = 1.037_314_7; // 1 / tanh(2)
    if x >= 1.0 {
        1.0
    } else if x <= -1.0 {
        -1.0
    } else {
        (2.0 * x).tanh() * NORM
    }
}

// ---------------------------------------------------------------------------
// Delay
// ---------------------------------------------------------------------------

/// A delay line with a fractional, linearly interpolated read.
#[derive(Clone, Debug)]
pub struct DelayLine {
    buf: Vec<f32>,
    mask: usize,
    w: usize,
}

impl DelayLine {
    /// Holds at least `max` samples.
    pub fn new(max: usize) -> Self {
        let size = (max + 2).next_power_of_two();
        DelayLine {
            buf: vec![0.0; size],
            mask: size - 1,
            w: 0,
        }
    }

    /// Reads the sample written `delay` samples ago (at least 1).
    #[inline]
    pub fn read(&self, delay: f32) -> f32 {
        let d = delay.clamp(1.0, (self.mask - 1) as f32);
        let i = d as usize;
        let frac = d - i as f32;
        let a = self.buf[(self.w + self.mask + 1 - i) & self.mask];
        let b = self.buf[(self.w + self.mask - i) & self.mask];
        a + (b - a) * frac
    }

    #[inline]
    pub fn write(&mut self, x: f32) {
        self.buf[self.w] = if x.abs() < 1e-25 { 0.0 } else { x };
        self.w = (self.w + 1) & self.mask;
    }

    pub fn clear(&mut self) {
        self.buf.fill(0.0);
    }
}

// ---------------------------------------------------------------------------
// Reverb
// ---------------------------------------------------------------------------

/// Freeverb's comb filter: a delay with a damped (low-passed) feedback.
#[derive(Clone, Debug)]
struct Comb {
    buf: Vec<f32>,
    i: usize,
    store: f32,
}

impl Comb {
    #[inline]
    fn process(&mut self, x: f32, feedback: f32, damp: f32) -> f32 {
        let y = self.buf[self.i];
        self.store = y + (self.store - y) * damp;
        if self.store.abs() < 1e-25 {
            self.store = 0.0;
        }
        self.buf[self.i] = x + self.store * feedback;
        self.i += 1;
        if self.i == self.buf.len() {
            self.i = 0;
        }
        y
    }
}

#[derive(Clone, Debug)]
struct Allpass {
    buf: Vec<f32>,
    i: usize,
}

impl Allpass {
    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        let b = self.buf[self.i];
        let w = x + b * 0.5;
        self.buf[self.i] = if w.abs() < 1e-25 { 0.0 } else { w };
        self.i += 1;
        if self.i == self.buf.len() {
            self.i = 0;
        }
        b - x
    }
}

const COMBS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASSES: [usize; 4] = [556, 441, 341, 225];
const SPREAD: usize = 23;

/// Stereo algorithmic reverb standing in for the original's convolution with
/// a 2.2 s noise impulse response (too expensive to convolve on a phone).
/// A Freeverb (8 damped combs and 4 all-passes per side, the right side
/// slightly longer so the two sides are decorrelated like the two channels of
/// the impulse response), tuned to its decay (about -20 dB after 1.2 s and
/// -30 dB after 1.5 s), its darker tail and its overall gain (Chrome
/// normalises convolver impulse responses, which leaves about -3 dB in the
/// mids, less in the highs). Each side reverberates its own input channel, as
/// the convolver does with a stereo input.
#[derive(Clone, Debug)]
pub struct Reverb {
    combs: [Vec<Comb>; 2],
    allpasses: [Vec<Allpass>; 2],
    /// Darkens the input like the start of the impulse response.
    tone: [f32; 2],
    tone_k: f32,
    feedback: f32,
    damp: f32,
}

/// Freeverb's fixed input gain.
const REVERB_IN: f32 = 0.015;
/// Output gain that matches the convolver's level.
const REVERB_WET: f32 = 1.24;
const REVERB_FEEDBACK: f32 = 0.93;
/// Damping of the comb feedback at 44.1 kHz.
const REVERB_DAMP: f64 = 0.05;

impl Reverb {
    pub fn new(sr: f64) -> Self {
        let scale = sr / 44100.0;
        let len = |n: usize| ((n as f64 * scale) as usize).max(1);
        let side = |extra: usize| -> (Vec<Comb>, Vec<Allpass>) {
            (
                COMBS
                    .iter()
                    .map(|&n| Comb {
                        buf: vec![0.0; len(n + extra)],
                        i: 0,
                        store: 0.0,
                    })
                    .collect(),
                ALLPASSES
                    .iter()
                    .map(|&n| Allpass {
                        buf: vec![0.0; len(n + extra)],
                        i: 0,
                    })
                    .collect(),
            )
        };
        let (cl, al) = side(0);
        let (cr, ar) = side(SPREAD);
        // Comb feedback for a ~3 s decay; the comb lengths (in seconds) don't depend on the sample rate.
        let feedback = REVERB_FEEDBACK;
        Reverb {
            combs: [cl, cr],
            allpasses: [al, ar],
            tone: [0.0; 2],
            tone_k: (-2.0 * std::f64::consts::PI * 5000.0 / sr).exp() as f32,
            feedback,
            damp: REVERB_DAMP.powf(1.0 / scale.max(0.25)) as f32,
        }
    }

    /// Adds the reverberated `inp` (left, right) to `out`.
    pub fn process(&mut self, inp: [&[f32]; 2], out: [&mut [f32]; 2]) {
        let [out_l, out_r] = out;
        for (ch, o) in [out_l, out_r].into_iter().enumerate() {
            let combs = &mut self.combs[ch];
            let allpasses = &mut self.allpasses[ch];
            let mut tone = self.tone[ch];
            for (i, &x) in inp[ch].iter().enumerate() {
                tone = x + (tone - x) * self.tone_k;
                if tone.abs() < 1e-25 {
                    tone = 0.0;
                }
                let x = tone * REVERB_IN;
                let mut y = 0.0;
                for c in combs.iter_mut() {
                    y += c.process(x, self.feedback, self.damp);
                }
                for a in allpasses.iter_mut() {
                    y = a.process(y);
                }
                o[i] += y * REVERB_WET;
            }
            self.tone[ch] = tone;
        }
    }

    pub fn clear(&mut self) {
        for side in &mut self.combs {
            for c in side {
                c.buf.fill(0.0);
                c.store = 0.0;
            }
        }
        for side in &mut self.allpasses {
            for a in side {
                a.buf.fill(0.0);
            }
        }
        self.tone = [0.0; 2];
    }
}

// ---------------------------------------------------------------------------
// Dynamics compressor
// ---------------------------------------------------------------------------

/// A port of Chromium's `DynamicsCompressorKernel` (which the Web Audio spec
/// describes): a soft-knee curve, a 6 ms look-ahead, an adaptive release, and
/// the automatic make-up gain that makes a compressed mix louder.
#[derive(Clone, Debug)]
pub struct Compressor {
    pre_delay: [Vec<f32>; 2],
    read: usize,
    write: usize,
    detector_average: f32,
    compressor_gain: f32,
    max_attack_compression_diff_db: f32,
    /// Frames left until the envelope rate is recomputed (Chromium does it every 32).
    division: usize,
    envelope_rate: f32,
    scaled_desired_gain: f32,
    // Static curve.
    linear_threshold: f32,
    knee_threshold: f32,
    knee_threshold_db: f32,
    yknee_threshold_db: f32,
    slope: f32,
    k: f32,
    master_linear_gain: f32,
    attack_frames: f32,
    sat_release_frames: f32,
    /// The detector's release rate at the 2 dB floor.
    sat_release_floor: f32,
    release: [f32; 5],
}

const PRE_DELAY_SIZE: usize = 1024;

fn db_to_lin(db: f32) -> f32 {
    10f32.powf(0.05 * db)
}

fn lin_to_db(lin: f32) -> f32 {
    20.0 * lin.log10()
}

impl Compressor {
    pub fn new(
        sr: f64,
        threshold_db: f32,
        knee_db: f32,
        ratio: f32,
        attack: f32,
        release: f32,
    ) -> Self {
        let sr = sr as f32;
        let mut c = Compressor {
            pre_delay: [vec![0.0; PRE_DELAY_SIZE], vec![0.0; PRE_DELAY_SIZE]],
            read: 0,
            write: 0,
            detector_average: 0.0,
            compressor_gain: 1.0,
            max_attack_compression_diff_db: -1.0,
            division: 0,
            envelope_rate: 0.0,
            scaled_desired_gain: 0.0,
            linear_threshold: db_to_lin(threshold_db),
            knee_threshold: 0.0,
            knee_threshold_db: threshold_db + knee_db,
            yknee_threshold_db: 0.0,
            slope: 1.0 / ratio,
            k: 0.0,
            master_linear_gain: 1.0,
            attack_frames: attack.max(0.001) * sr,
            sat_release_frames: 0.0025 * sr,
            sat_release_floor: db_to_lin(2.0 / (0.0025 * sr)) - 1.0,
            release: [0.0; 5],
        };
        // Knee: find the k whose slope at the end of the knee is 1/ratio.
        let x = db_to_lin(threshold_db + knee_db);
        let (mut min_k, mut max_k, mut k) = (0.1f32, 10000.0f32, 5.0f32);
        for _ in 0..15 {
            if c.slope_at(x, k) < c.slope {
                max_k = k;
            } else {
                min_k = k;
            }
            k = (min_k * max_k).sqrt();
        }
        c.k = k;
        c.knee_threshold = db_to_lin(c.knee_threshold_db);
        c.yknee_threshold_db = lin_to_db(c.knee_curve(c.knee_threshold, k));
        // Make-up gain, with Chromium's empirical/perceptual tuning.
        let full_range_gain = c.saturate(1.0, k);
        c.master_linear_gain = (1.0 / full_range_gain).powf(0.6);
        // Adaptive release: a polynomial through four release times (Chromium's coefficients, in f64).
        let frames = sr as f64 * release as f64;
        let [y1, y2, y3, y4] = [0.09, 0.16, 0.42, 0.98].map(|z| frames * z);
        c.release = [
            0.9999999999999998 * y1 + 1.8432219684323923e-16 * y2 - 1.9373394351676423e-16 * y3
                + 8.824516011816245e-18 * y4,
            -1.5788320352845888 * y1 + 2.3305837032074286 * y2 - 0.9141194204840429 * y3
                + 0.1623677525612032 * y4,
            0.5334142869106424 * y1 - 1.272736789213631 * y2 + 0.9258856042207512 * y3
                - 0.18656310191776226 * y4,
            0.08783463138207234 * y1 - 0.1694162967925622 * y2 + 0.08588057951595272 * y3
                - 0.00429891410546283 * y4,
            -0.042416883008123074 * y1 + 0.1115693827987602 * y2 - 0.09764676325265872 * y3
                + 0.028494263462021576 * y4,
        ]
        .map(|v| v as f32);
        // Chromium's default pre-delay.
        let delay = ((0.006 * sr) as usize).min(PRE_DELAY_SIZE - 1);
        c.read = (PRE_DELAY_SIZE - delay) & (PRE_DELAY_SIZE - 1);
        c
    }

    fn knee_curve(&self, x: f32, k: f32) -> f32 {
        if x < self.linear_threshold {
            x
        } else {
            self.linear_threshold + (1.0 - (-k * (x - self.linear_threshold)).exp()) / k
        }
    }

    fn saturate(&self, x: f32, k: f32) -> f32 {
        if x < self.knee_threshold {
            self.knee_curve(x, k)
        } else {
            db_to_lin(
                self.yknee_threshold_db + self.slope * (lin_to_db(x) - self.knee_threshold_db),
            )
        }
    }

    fn slope_at(&self, x: f32, k: f32) -> f32 {
        if x < self.linear_threshold {
            return 1.0;
        }
        let x2 = x * 1.001;
        let (x_db, x2_db) = (lin_to_db(x), lin_to_db(x2));
        let (y_db, y2_db) = (
            lin_to_db(self.knee_curve(x, k)),
            lin_to_db(self.knee_curve(x2, k)),
        );
        (y2_db - y_db) / (x2_db - x_db)
    }

    /// The envelope rate for the next 32 frames.
    fn update_envelope(&mut self) {
        if !self.detector_average.is_finite() {
            self.detector_average = 1.0;
        }
        let desired_gain = self.detector_average;
        // Pre-warp so we get the desired gain after the sin() warp below.
        let scaled = desired_gain.asin() / std::f32::consts::FRAC_PI_2;
        self.scaled_desired_gain = scaled;
        let releasing = scaled > self.compressor_gain;
        let mut diff_db = lin_to_db(self.compressor_gain / scaled);
        if releasing {
            self.max_attack_compression_diff_db = -1.0;
            if !diff_db.is_finite() {
                diff_db = -1.0;
            }
            // Adaptive release: more compression releases faster.
            let x = 0.25 * (diff_db.clamp(-12.0, 0.0) + 12.0);
            let [a, b, c, d, e] = self.release;
            let (x2, x3, x4) = (x * x, x * x * x, x * x * x * x);
            let release_frames = a + b * x + c * x2 + d * x3 + e * x4;
            self.envelope_rate = db_to_lin(5.0 / release_frames);
        } else {
            if !diff_db.is_finite() {
                diff_db = 1.0;
            }
            if self.max_attack_compression_diff_db == -1.0
                || self.max_attack_compression_diff_db < diff_db
            {
                self.max_attack_compression_diff_db = diff_db;
            }
            let eff = self.max_attack_compression_diff_db.max(0.5);
            self.envelope_rate = 1.0 - (0.25 / eff).powf(1.0 / self.attack_frames);
        }
    }

    /// Compresses the stereo signal in place.
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let mask = PRE_DELAY_SIZE - 1;
        for i in 0..l.len() {
            if self.division == 0 {
                self.update_envelope();
                self.division = 32;
            }
            self.division -= 1;
            let (xl, xr) = (l[i], r[i]);
            self.pre_delay[0][self.write] = xl;
            self.pre_delay[1][self.write] = xr;
            let abs_input = xl.abs().max(xr.abs());
            // Below the threshold the curve is linear: no attenuation, and the rate for the 2 dB floor.
            let (attenuation, sat_release_rate) = if abs_input < self.linear_threshold {
                (1.0, self.sat_release_floor)
            } else {
                let attenuation = self.saturate(abs_input, self.k) / abs_input;
                let attenuation_db = (-lin_to_db(attenuation)).max(2.0);
                (
                    attenuation,
                    db_to_lin(attenuation_db / self.sat_release_frames) - 1.0,
                )
            };
            let rate = if attenuation > self.detector_average {
                sat_release_rate
            } else {
                1.0
            };
            self.detector_average += (attenuation - self.detector_average) * rate;
            self.detector_average = self.detector_average.min(1.0);
            if !self.detector_average.is_finite() {
                self.detector_average = 1.0;
            }
            if self.envelope_rate < 1.0 {
                // Attack: reduce the gain towards the desired one.
                self.compressor_gain +=
                    (self.scaled_desired_gain - self.compressor_gain) * self.envelope_rate;
            } else {
                // Release: exponentially back up to 1.
                self.compressor_gain = (self.compressor_gain * self.envelope_rate).min(1.0);
            }
            // Warp the gain to smooth out sharp transitions.
            let warped = if self.compressor_gain >= 1.0 {
                1.0
            } else {
                (std::f32::consts::FRAC_PI_2 * self.compressor_gain).sin()
            };
            let gain = self.master_linear_gain * warped;
            l[i] = self.pre_delay[0][self.read] * gain;
            r[i] = self.pre_delay[1][self.read] * gain;
            self.read = (self.read + 1) & mask;
            self.write = (self.write + 1) & mask;
        }
    }

    pub fn clear(&mut self) {
        for d in &mut self.pre_delay {
            d.fill(0.0);
        }
        self.detector_average = 0.0;
        self.compressor_gain = 1.0;
        self.max_attack_compression_diff_db = -1.0;
        self.division = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f64 = 1000.0;

    fn run(p: &mut Param, n: usize) -> Vec<f32> {
        (0..n).map(|_| p.tick()).collect()
    }

    #[test]
    fn set_value_and_linear_ramp() {
        let mut p = Param::new(5.0, SR);
        p.set_value_at(0.0, 0.0);
        p.linear_ramp_to(1.0, 0.1);
        let v = run(&mut p, 200);
        assert_eq!(v[0], 0.0);
        assert!((v[50] - 0.5).abs() < 1e-6);
        assert!((v[100] - 1.0).abs() < 1e-6);
        assert_eq!(v[150], 1.0);
    }

    #[test]
    fn exponential_ramp_and_target() {
        let mut p = Param::new(0.0, SR);
        p.set_value_at(1.0, 0.0);
        p.exp_ramp_to(100.0, 0.2);
        p.set_target_at(0.0, 0.2, 0.05);
        let v = run(&mut p, 400);
        assert!((v[100] - 10.0).abs() < 1e-3);
        assert!((v[200] - 100.0).abs() < 1e-2);
        // One time constant later: 1/e of the way left.
        assert!((v[250] - 100.0 * (-1.0f32).exp()).abs() < 0.05);
    }

    #[test]
    fn same_type_at_the_same_time_replaces() {
        let mut p = Param::new(0.0, SR);
        p.set_target_at(0.5, 0.0, 0.01);
        p.set_target_at(0.0, 0.0, 0.01);
        let v = run(&mut p, 100);
        assert_eq!(v[99], 0.0);
    }

    #[test]
    fn cancel_holds_the_current_value() {
        let mut p = Param::new(0.0, SR);
        p.linear_ramp_to(1.0, 1.0);
        run(&mut p, 500);
        p.cancel(0.5);
        let v = run(&mut p, 10);
        assert!((v[9] - 0.5).abs() < 0.01);
    }

    #[test]
    fn oscillators_are_bounded_and_start_at_zero() {
        for wave in [Wave::Sine, Wave::Square, Wave::Sawtooth, Wave::Triangle] {
            let dt = 440.0 / 44100.0;
            let mut phase = 0.0;
            let mut peak = 0.0f32;
            for _ in 0..44100 {
                let s = wave_sample(wave, phase, dt);
                peak = peak.max(s.abs());
                advance(&mut phase, dt);
            }
            assert!(peak <= 1.0 && peak > 0.8, "{wave:?} peak {peak}");
            assert!(wave_sample(wave, 0.0, dt).abs() < 0.01 || wave == Wave::Square);
        }
        assert!((sin_turns(0.25) - 1.0).abs() < 1e-6);
        assert!((sin_turns(0.1) - (0.2 * std::f64::consts::PI).sin() as f32).abs() < 1e-6);
    }

    #[test]
    fn lowpass_passes_lows_and_cuts_highs() {
        let gain = |freq: f64| {
            let mut f = Biquad::new(FilterKind::Lowpass, 1000.0, 1.0, 44100.0);
            let mut peak = 0.0f32;
            for i in 0..44100 {
                let y = f.process((i as f64 * freq / 44100.0 * std::f64::consts::TAU).sin() as f32);
                if i > 22050 {
                    peak = peak.max(y.abs());
                }
            }
            peak
        };
        assert!((gain(100.0) - 1.0).abs() < 0.02);
        assert!(gain(8000.0) < 0.03);
    }
}
