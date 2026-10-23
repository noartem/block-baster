//! The software synth: one engine renders the sound effects and the music into
//! an interleaved buffer, at whatever sample rate the device runs. Notes are
//! voices in a fixed pool; each one is an oscillator or a noise buffer, an
//! optional filter and a gain envelope, all with Web Audio style automation,
//! scheduled at exact sample times (also in the future, like `osc.start(t)`).

use super::dsp::{
    Biquad, DelayLine, FilterKind, Param, QUANTUM, advance, pink_noise, sin_turns, wave_sample,
    white_noise,
};
use super::intensity::MusicState;
use super::music::{MusicGraph, Sequencer};
use crate::game::rng::Rng;
use crate::styles::Wave;

/// At most this many notes sound (or wait to sound) at once; more steal the quietest.
pub const MAX_VOICES: usize = 128;

/// Gain of all sound effects, before the volume.
pub const MASTER: f32 = 0.5;

/// The mixer channels of the music, as in the original Web Audio graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bus {
    Keys,
    Melody,
    Arp,
    Pad,
    Bass,
    Kick,
    Drums,
    Vinyl,
    Fx,
}

pub const BUSES: usize = 9;

/// Where a voice plays into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dest {
    /// The sound effects' master gain, plus `send` into their echo.
    Sfx,
    Music(Bus),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoiseBuf {
    /// Half a second of white noise for the effects' bursts.
    Burst,
    /// One second of white noise for the drums.
    White,
    /// Two seconds of pink noise for the vinyl hiss.
    Pink,
}

/// The noise buffers, made once.
pub struct Noise {
    pub burst: Vec<f32>,
    pub white: Vec<f32>,
    pub pink: Vec<f32>,
}

impl Noise {
    pub fn get(&self, b: NoiseBuf) -> &[f32] {
        match b {
            NoiseBuf::Burst => &self.burst,
            NoiseBuf::White => &self.white,
            NoiseBuf::Pink => &self.pink,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Source {
    Osc {
        wave: Wave,
        phase: f64,
        /// Hz.
        freq: Param,
        /// Frequency ratio from `detune`.
        detune: f64,
    },
    /// Two-operator FM: a sine carrier whose frequency is modulated by a sine
    /// at the same frequency, `depth` Hz deep, plus a shared 5 Hz vibrato.
    Fm {
        phase: f64,
        mod_phase: f64,
        freq: f64,
        depth: Param,
        lfo_phase: f64,
    },
    Noise {
        buf: NoiseBuf,
        pos: usize,
        looping: bool,
    },
}

#[derive(Clone, Debug)]
pub struct VoiceFilter {
    pub kind: FilterKind,
    pub freq: Param,
    pub q: f32,
    bq: Biquad,
    last: f32,
    countdown: u32,
}

impl VoiceFilter {
    pub fn new(kind: FilterKind, freq: Param, q: f32) -> Self {
        VoiceFilter {
            kind,
            freq,
            q,
            bq: Biquad::default(),
            last: -1.0,
            countdown: 0,
        }
    }
}

/// Vibrato depth of the e-piano, in cents.
const VIBRATO_CENTS: f64 = 8.0;
const VIBRATO_HZ: f64 = 5.0;

/// One scheduled note: a source through an optional filter and a gain.
#[derive(Clone, Debug)]
pub struct Voice {
    /// First sample.
    pub start: u64,
    /// The source stops (and the voice is freed) at this sample.
    pub stop: u64,
    pub source: Source,
    pub filter: Option<VoiceFilter>,
    pub gain: Param,
    pub dest: Dest,
    /// Echo send of a sound effect.
    pub send: f32,
}

impl Voice {
    /// Renders into `out` (and `send_out`) for the quantum starting at `frame0`; true once finished.
    fn render(
        &mut self,
        out: &mut [f32],
        send_out: &mut [f32],
        frame0: u64,
        sr: f64,
        noise: &Noise,
    ) -> bool {
        let end = frame0 + out.len() as u64;
        if self.start >= end {
            return false;
        }
        let from = self.start.saturating_sub(frame0) as usize;
        let to = (self.stop.min(end).max(frame0) - frame0) as usize;
        let inv_sr = 1.0 / sr;
        for i in from..to {
            let s = match &mut self.source {
                Source::Osc {
                    wave,
                    phase,
                    freq,
                    detune,
                } => {
                    let dt = freq.tick() as f64 * *detune * inv_sr;
                    let s = wave_sample(*wave, *phase, dt);
                    advance(phase, dt);
                    s
                }
                Source::Fm {
                    phase,
                    mod_phase,
                    freq,
                    depth,
                    lfo_phase,
                } => {
                    let m = sin_turns(*mod_phase) as f64;
                    advance(mod_phase, *freq * inv_sr);
                    let cents = VIBRATO_CENTS * sin_turns(*lfo_phase) as f64;
                    advance(lfo_phase, VIBRATO_HZ * inv_sr);
                    // 2^(cents/1200), to second order (the vibrato is only a few cents).
                    let x = cents * (std::f64::consts::LN_2 / 1200.0);
                    let f = (*freq + depth.tick() as f64 * m) * (1.0 + x + 0.5 * x * x);
                    let s = sin_turns(*phase);
                    advance(phase, f * inv_sr);
                    s
                }
                Source::Noise { buf, pos, looping } => {
                    let data = noise.get(*buf);
                    if *pos >= data.len() {
                        if *looping {
                            *pos = 0;
                        } else {
                            // Played to the end: silence.
                            self.stop = frame0 + i as u64;
                            break;
                        }
                    }
                    let s = data[*pos];
                    *pos += 1;
                    s
                }
            };
            let s = match &mut self.filter {
                Some(f) => {
                    let freq = f.freq.tick();
                    if f.countdown == 0 {
                        if freq != f.last {
                            f.bq.set(f.kind, freq, f.q, sr);
                            f.last = freq;
                        }
                        f.countdown = 16;
                    }
                    f.countdown -= 1;
                    f.bq.process(s)
                }
                None => s,
            };
            let y = s * self.gain.tick();
            out[i] += y;
            if self.send != 0.0 {
                send_out[i] += y * self.send;
            }
        }
        self.stop <= end
    }
}

/// The pre-allocated voice pool.
pub struct Voices {
    list: Vec<Voice>,
    /// The current frame, for stealing.
    now: u64,
}

impl Voices {
    fn new() -> Self {
        Voices {
            list: Vec::with_capacity(MAX_VOICES),
            now: 0,
        }
    }

    /// Adds a voice; when the pool is full, the quietest sounding voice makes room.
    pub fn add(&mut self, v: Voice) {
        if v.stop <= v.start {
            return;
        }
        if self.list.len() < MAX_VOICES {
            self.list.push(v);
            return;
        }
        let now = self.now;
        let victim = self
            .list
            .iter()
            .enumerate()
            .filter(|(_, v)| v.start <= now)
            .min_by(|(_, a), (_, b)| a.gain.value().abs().total_cmp(&b.gain.value().abs()))
            .map(|(i, _)| i);
        if let Some(i) = victim {
            self.list[i] = v;
        }
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// Drops the voices that match.
    pub fn remove_where(&mut self, f: impl Fn(&Voice) -> bool) {
        self.list.retain(|v| !f(v));
    }
}

/// The sound effects' output: master gain and the echo of higher style tiers.
struct SfxOut {
    master: Param,
    dry: [f32; QUANTUM],
    send: [f32; QUANTUM],
    /// Feedback delay: 0.16 s, 0.38 feedback, damped at 3.2 kHz.
    delay: DelayLine,
    damp: Biquad,
    delay_samples: f32,
}

const ECHO_TIME: f64 = 0.16;
const ECHO_FEEDBACK: f32 = 0.38;
const ECHO_DAMP: f32 = 3200.0;
/// Default Q of a BiquadFilterNode.
pub const DEFAULT_Q: f32 = 1.0;

pub struct Engine {
    pub(crate) sr: f64,
    pub(crate) frame: u64,
    pub(crate) voices: Voices,
    pub(crate) noise: Noise,
    pub(crate) rng: Rng,
    sfx: SfxOut,
    pub(crate) music: MusicGraph,
    pub(crate) seq: Sequencer,
    bus: [[f32; QUANTUM]; BUSES],
    out: [[f32; QUANTUM]; 2],
}

impl Engine {
    /// An engine rendering at `sample_rate`; `seed` drives all its random choices.
    pub fn new(sample_rate: f32, seed: u32) -> Self {
        let sr = sample_rate as f64;
        let mut rng = Rng::new(seed);
        let noise = Noise {
            burst: white_noise((sr * 0.5) as usize, &mut rng),
            white: white_noise(sr as usize, &mut rng),
            pink: pink_noise((sr * 2.0) as usize, &mut rng),
        };
        Engine {
            sr,
            frame: 0,
            voices: Voices::new(),
            noise,
            rng,
            sfx: SfxOut {
                master: Param::new(MASTER, sr),
                dry: [0.0; QUANTUM],
                send: [0.0; QUANTUM],
                delay: DelayLine::new(sr as usize),
                damp: Biquad::new(FilterKind::Lowpass, ECHO_DAMP, DEFAULT_Q, sr),
                delay_samples: (ECHO_TIME * sr) as f32,
            },
            music: MusicGraph::new(sr),
            seq: Sequencer::new(),
            bus: [[0.0; QUANTUM]; BUSES],
            out: [[0.0; QUANTUM]; 2],
        }
    }

    pub fn sample_rate(&self) -> f64 {
        self.sr
    }

    /// Engine time in seconds: when the next rendered sample plays.
    pub fn now(&self) -> f64 {
        self.frame as f64 / self.sr
    }

    /// Voices playing or waiting to play.
    pub fn active_voices(&self) -> usize {
        self.voices.len()
    }

    /// Volume of the effects, 0..1 on a perceptual curve.
    pub fn set_sound_volume(&mut self, level: f32) {
        let now = self.now();
        self.sfx
            .master
            .set_target_at((MASTER * level * level) as f64, now, 0.02);
    }

    /// Sets the volume right away, without the short glide.
    pub fn init_sound_volume(&mut self, level: f32) {
        self.sfx.master.reset(MASTER * level * level, self.frame);
    }

    /// Starts the music, fading in over 1.5 s.
    pub fn start_music(&mut self, level: f32, paused: bool) {
        if self.music.active && !self.music.releasing() {
            return;
        }
        let now = self.now();
        self.seq.begin(now);
        if self.music.releasing() {
            // Still fading out from a stop: drop what was scheduled and fade back in.
            self.voices.remove_where(|v| {
                matches!(v.dest, Dest::Music(_)) && v.start as f64 > now * self.sr
            });
        }
        self.music.begin(now, self.frame, level, paused);
    }

    /// Fades the music out and then lets it go.
    pub fn stop_music(&mut self) {
        let now = self.now();
        self.music.release(now);
    }

    pub fn set_music_volume(&mut self, level: f32) {
        let now = self.now();
        self.music.set_volume(now, level);
    }

    pub fn set_music_paused(&mut self, paused: bool) {
        let now = self.now();
        self.music.set_paused(now, paused);
    }

    /// The state the music follows (the app tracks it from the game).
    pub fn set_music_state(&mut self, s: MusicState) {
        self.seq.state = s;
    }

    pub fn music_state(&self) -> MusicState {
        self.seq.state
    }

    /// True while the music plays or fades out.
    pub fn music_active(&self) -> bool {
        self.music.active
    }

    /// Renders interleaved frames with `channels` channels (stereo, mono down-mixed, extra channels silent).
    pub fn render(&mut self, out: &mut [f32], channels: usize) {
        let channels = channels.max(1);
        let frames = out.len() / channels;
        let mut done = 0;
        while done < frames {
            let len = (frames - done).min(QUANTUM);
            self.quantum(len);
            for i in 0..len {
                let (l, r) = (
                    self.out[0][i].clamp(-1.0, 1.0),
                    self.out[1][i].clamp(-1.0, 1.0),
                );
                let frame = &mut out[(done + i) * channels..(done + i + 1) * channels];
                if channels == 1 {
                    frame[0] = 0.5 * (l + r);
                } else {
                    frame[0] = l;
                    frame[1] = r;
                    frame[2..].fill(0.0);
                }
            }
            done += len;
        }
    }

    /// Renders one block of up to 128 frames into `self.out`.
    fn quantum(&mut self, len: usize) {
        let now = self.now();
        self.voices.now = self.frame;
        // The music's own clock: like the original's timer, it looks ahead and schedules notes.
        if self.music.active {
            if let Some(at) = self.music.release_at {
                if now >= at {
                    self.music.deactivate();
                    self.voices
                        .remove_where(|v| matches!(v.dest, Dest::Music(_)));
                }
            } else if now >= self.seq.next_tick {
                self.seq.tick(
                    now,
                    &mut self.music,
                    &mut self.voices,
                    &mut self.rng,
                    self.noise.white.len(),
                    self.sr,
                );
            }
        }

        for b in &mut self.bus {
            b[..len].fill(0.0);
        }
        self.sfx.dry[..len].fill(0.0);
        self.sfx.send[..len].fill(0.0);
        for o in &mut self.out {
            o[..len].fill(0.0);
        }

        let mut i = 0;
        while i < self.voices.list.len() {
            let v = &mut self.voices.list[i];
            let finished = match v.dest {
                Dest::Sfx => v.render(
                    &mut self.sfx.dry[..len],
                    &mut self.sfx.send[..len],
                    self.frame,
                    self.sr,
                    &self.noise,
                ),
                Dest::Music(b) => v.render(
                    &mut self.bus[b as usize][..len],
                    &mut self.sfx.send[..len],
                    self.frame,
                    self.sr,
                    &self.noise,
                ),
            };
            if finished {
                self.voices.list.swap_remove(i);
            } else {
                i += 1;
            }
        }

        // Effects: echo, then the master gain.
        let sfx = &mut self.sfx;
        for i in 0..len {
            let echo = sfx.damp.process(sfx.delay.read(sfx.delay_samples));
            sfx.delay.write(sfx.send[i] + ECHO_FEEDBACK * echo);
            let y = (sfx.dry[i] + echo) * sfx.master.tick();
            self.out[0][i] = y;
            self.out[1][i] = y;
        }

        if self.music.active {
            let [l, r] = &mut self.out;
            self.music
                .process(&self.bus, &mut l[..len], &mut r[..len], &self.noise.pink);
        }
        self.frame += len as u64;
    }
}
