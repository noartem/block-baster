//! All sound effects are synthesised at runtime, so the game ships without any
//! audio files (and without licensing questions). Each style tier has its own
//! sound profile: instrument, scale, echo and extras.

use super::Sfx;
use super::dsp::{FilterKind, Param};
use super::engine::{DEFAULT_Q, Dest, Engine, NoiseBuf, Source, Voice, VoiceFilter};
use crate::styles::{SoundProfile, Wave};

#[derive(Clone, Copy, Debug)]
struct ToneOpts {
    wave: Wave,
    vol: f64,
    /// Glides exponentially to this frequency over the tone.
    to: Option<f64>,
    attack: f64,
    /// Echo send; the profile's when None.
    echo: Option<f32>,
    detune: f64,
}

impl Default for ToneOpts {
    fn default() -> Self {
        ToneOpts {
            wave: Wave::Sine,
            vol: 0.3,
            to: None,
            attack: 0.008,
            echo: None,
            detune: 0.0,
        }
    }
}

fn tone_opts(wave: Wave, vol: f64) -> ToneOpts {
    ToneOpts {
        wave,
        vol,
        ..ToneOpts::default()
    }
}

/// Volume that keeps the lead waveform roughly equally loud across profiles.
fn lead_vol(p: &SoundProfile) -> f64 {
    match p.lead {
        Wave::Square => 0.07,
        Wave::Sawtooth => 0.08,
        Wave::Triangle => 0.14,
        Wave::Sine => 0.16,
    }
}

fn semis(step: f64) -> f64 {
    2f64.powf(step / 12.0)
}

impl Engine {
    /// Plays a sound effect now, in the style of `profile`.
    pub fn play(&mut self, sfx: Sfx, profile: &SoundProfile) {
        let t = self.now();
        match sfx {
            Sfx::Pickup => self.tone(
                profile,
                520.0,
                t,
                0.07,
                ToneOpts {
                    to: Some(700.0),
                    ..tone_opts(Wave::Triangle, 0.12)
                },
            ),
            Sfx::Place => {
                let f = profile.thud as f64;
                self.tone(
                    profile,
                    f,
                    t,
                    0.12,
                    ToneOpts {
                        to: Some(f / 2.0),
                        echo: Some(0.0),
                        ..tone_opts(Wave::Triangle, 0.4)
                    },
                );
                self.burst(t, 0.06, 0.25, 1800.0);
            }
            Sfx::Invalid => self.tone(
                profile,
                220.0,
                t,
                0.12,
                ToneOpts {
                    to: Some(160.0),
                    ..tone_opts(Wave::Sine, 0.15)
                },
            ),
            Sfx::Clear { lines, combo } => self.clear(profile, t, lines, combo),
            Sfx::BoardClear => {
                let t = t + 0.25;
                for (i, f) in [523.25, 659.25, 783.99, 1046.5].into_iter().enumerate() {
                    self.tone(
                        profile,
                        f,
                        t + i as f64 * 0.08,
                        0.6,
                        tone_opts(Wave::Triangle, 0.18),
                    );
                }
            }
            Sfx::StyleShift => {
                // A sparkly upward sweep when the board clear switches the style.
                let t = t + 0.7;
                let root = profile.root as f64 * 2.0;
                for (i, &step) in profile.scale.iter().enumerate() {
                    self.tone(
                        profile,
                        root * semis(step as f64),
                        t + i as f64 * 0.035,
                        0.3,
                        ToneOpts {
                            echo: Some(0.3),
                            ..tone_opts(Wave::Sine, 0.06)
                        },
                    );
                }
            }
            Sfx::TierUp(level) => self.tier_up(profile, t, level),
            Sfx::GameOver => {
                for (i, f) in [392.0, 330.0, 262.0, 196.0].into_iter().enumerate() {
                    self.tone(
                        profile,
                        f,
                        t + i as f64 * 0.16,
                        0.3,
                        tone_opts(Wave::Triangle, 0.2),
                    );
                }
            }
        }
    }

    /// Rising arpeggio; higher and longer with more lines and bigger combos.
    fn clear(&mut self, p: &SoundProfile, t: f64, lines: u32, combo: u32) {
        let root = p.root as f64 * semis((combo as f64 - 1.0).min(7.0));
        let notes = p.scale.len().min(2 + lines as usize);
        for i in 0..notes {
            let f = root * semis(p.scale[i] as f64);
            let at = t + i as f64 * 0.055;
            self.tone(p, f, at, 0.25, tone_opts(p.lead, lead_vol(p)));
            self.tone(p, f * 2.0, at, 0.2, tone_opts(Wave::Sine, 0.1));
            if p.bell {
                self.tone(p, f * 4.0, at + 0.02, 0.35, tone_opts(Wave::Sine, 0.035));
            }
        }
        if p.pad {
            // A soft chord under the arpeggio.
            for step in [0.0, 7.0, 12.0] {
                self.tone(
                    p,
                    root / 2.0 * semis(step),
                    t,
                    0.9,
                    ToneOpts {
                        attack: 0.08,
                        ..tone_opts(Wave::Triangle, 0.06)
                    },
                );
            }
        }
        self.burst(t, 0.25, 0.12, 6000.0);
    }

    /// A fanfare when a new style tier is unlocked; grander for higher tiers.
    fn tier_up(&mut self, p: &SoundProfile, t: f64, level: usize) {
        let t = t + 0.2;
        let root = 261.63;
        // I - IV - V - I, adding voices and length as the tiers rise (Neon is level 7, Aurora 8, Legendary 9).
        let all: [&[f64]; 4] = [
            &[0.0, 4.0, 7.0],
            &[5.0, 9.0, 12.0],
            &[7.0, 11.0, 14.0],
            &[12.0, 16.0, 19.0, 24.0],
        ];
        let chords = &all[if level >= 7 { 0 } else { 2 }..];
        let beat = 0.16;
        for (i, chord) in chords.iter().enumerate() {
            let last = i == chords.len() - 1;
            let at = t + i as f64 * beat;
            for &step in chord.iter() {
                let f = root * semis(step);
                self.tone(
                    p,
                    f,
                    at,
                    if last { 1.2 } else { 0.3 },
                    ToneOpts {
                        echo: Some(0.3),
                        ..tone_opts(p.lead, lead_vol(p) * 0.9)
                    },
                );
                self.tone(
                    p,
                    f * 2.0,
                    at,
                    if last { 0.9 } else { 0.25 },
                    ToneOpts {
                        echo: Some(0.3),
                        ..tone_opts(Wave::Sine, 0.05)
                    },
                );
                if level >= 8 {
                    self.tone(
                        p,
                        f / 2.0,
                        at,
                        if last { 1.4 } else { 0.35 },
                        ToneOpts {
                            attack: 0.04,
                            ..tone_opts(Wave::Triangle, 0.08)
                        },
                    );
                }
            }
        }
        if level >= 9 {
            // Shimmering cascade on top for the final tier.
            for i in 0..8 {
                let step = [0.0, 4.0, 7.0, 12.0][i % 4] / 12.0 + (i / 4) as f64;
                let at = t + chords.len() as f64 * beat + i as f64 * 0.05;
                self.tone(
                    p,
                    1046.5 * 2f64.powf(step),
                    at,
                    0.4,
                    ToneOpts {
                        echo: Some(0.5),
                        ..tone_opts(Wave::Sine, 0.03)
                    },
                );
            }
        }
    }

    fn tone(&mut self, p: &SoundProfile, freq: f64, start: f64, dur: f64, o: ToneOpts) {
        let sr = self.sr;
        let first = (start * sr).ceil() as u64;
        let mut f = Param::starting_at(freq as f32, sr, first);
        f.set_value_at(freq, start);
        if let Some(to) = o.to {
            f.exp_ramp_to(to, start + dur);
        }
        let mut gain = Param::starting_at(0.0001, sr, first);
        gain.set_value_at(0.0001, start);
        gain.exp_ramp_to(o.vol, start + o.attack);
        gain.exp_ramp_to(0.0001, start + dur);
        // Raw saws are harsh; soften them like an analogue synth.
        let filter = (o.wave == Wave::Sawtooth)
            .then(|| VoiceFilter::new(FilterKind::Lowpass, Param::new(2400.0, sr), DEFAULT_Q));
        let echo = o.echo.unwrap_or(p.echo);
        self.voices.add(Voice {
            start: first,
            stop: ((start + dur + 0.02) * sr).ceil() as u64,
            source: Source::Osc {
                wave: o.wave,
                phase: 0.0,
                freq: f,
                detune: 2f64.powf(o.detune / 1200.0),
            },
            filter,
            gain,
            dest: Dest::Sfx,
            send: echo.max(0.0),
        });
    }

    fn burst(&mut self, start: f64, dur: f64, vol: f64, cutoff: f32) {
        let sr = self.sr;
        let first = (start * sr).ceil() as u64;
        let mut gain = Param::starting_at(vol as f32, sr, first);
        gain.set_value_at(vol, start);
        gain.exp_ramp_to(0.0001, start + dur);
        self.voices.add(Voice {
            start: first,
            stop: ((start + dur) * sr).ceil() as u64,
            source: Source::Noise {
                buf: NoiseBuf::Burst,
                pos: 0,
                looping: false,
            },
            filter: Some(VoiceFilter::new(
                FilterKind::Lowpass,
                Param::new(cutoff, sr),
                DEFAULT_Q,
            )),
            gain,
            dest: Dest::Sfx,
            send: 0.0,
        });
    }
}
