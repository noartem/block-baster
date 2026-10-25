//! All sound effects are synthesised at runtime, so the game ships without any
//! audio files (and without licensing questions). Each style tier has its own
//! sound profile and material: placing a block sounds like the material
//! (cardboard thumps, wood knocks, iron clanks, gems clink…), and the melodic
//! effects play on the material's lead instrument, transposed by the palette.

use super::Sfx;
use super::dsp::{FilterKind, Param};
use super::engine::{DEFAULT_Q, Dest, Engine, NoiseBuf, Source, Voice, VoiceFilter};
use super::timbre::{Fm, Patch, SoundStyle, Src};
use crate::styles::{Material, SoundProfile, Wave};

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
    /// Low-pass from the first cutoff to the second over the tone, and its Q.
    lp: Option<(f64, f64, f32)>,
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
            lp: None,
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

/// A sine tone, gliding to `to`, with no echo.
fn glide(vol: f64, to: f64) -> ToneOpts {
    ToneOpts {
        to: Some(to),
        echo: Some(0.0),
        ..tone_opts(Wave::Sine, vol)
    }
}

/// A filtered noise burst.
#[derive(Clone, Copy, Debug)]
struct NoiseOpts {
    kind: FilterKind,
    freq: f64,
    /// Sweeps exponentially to this cutoff over the burst.
    to: Option<f64>,
    q: f32,
    attack: f64,
    echo: f32,
}

fn noise(kind: FilterKind, freq: f64) -> NoiseOpts {
    NoiseOpts {
        kind,
        freq,
        to: None,
        q: DEFAULT_Q,
        attack: 0.0,
        echo: 0.0,
    }
}

fn band(freq: f64, q: f32) -> NoiseOpts {
    NoiseOpts {
        q,
        ..noise(FilterKind::Bandpass, freq)
    }
}

/// Volume that keeps the lead waveform roughly equally loud across profiles.
fn wave_vol(w: Wave) -> f64 {
    match w {
        Wave::Square => 0.07,
        Wave::Sawtooth => 0.08,
        Wave::Triangle => 0.14,
        Wave::Sine => 0.16,
    }
}

/// The classic profiles' lead waveform: square (the original sound).
const CLASSIC_LEAD: Wave = Wave::Square;

/// Lead volume of a patch: a sine-like instrument at 0.16, scaled by its gain.
fn patch_vol(p: &Patch) -> f64 {
    let base = match p.src {
        Src::Osc(w) => wave_vol(w),
        Src::Fm(_) => 0.16,
    };
    base * p.gain
}

fn semis(step: f64) -> f64 {
    2f64.powf(step / 12.0)
}

impl Engine {
    /// Plays a sound effect now, in the current style.
    pub fn play(&mut self, sfx: Sfx) {
        let style = self.style;
        self.play_in(sfx, &style);
    }

    /// Plays a sound effect now, in the style `s`.
    pub fn play_in(&mut self, sfx: Sfx, s: &SoundStyle) {
        let t = self.now();
        let tr = s.transposition();
        let lv = s.timbre().levels;
        let db = match sfx {
            Sfx::Pickup => lv.pickup,
            Sfx::Place => lv.place,
            Sfx::Invalid => lv.invalid,
            Sfx::Clear { .. } => lv.clear,
            Sfx::BoardClear | Sfx::StyleShift | Sfx::GameOver => lv.lead,
            Sfx::TierUp(_) => lv.fanfare,
        };
        self.sfx_gain = 10f64.powf(db / 20.0);
        if s.material() == Material::Classic {
            self.play_classic(sfx, &s.profile, t, tr);
            return;
        }
        let lead = s.timbre().lead;
        let p = &s.profile;
        match sfx {
            Sfx::Pickup => self.pickup(s, t),
            Sfx::Place => self.place(s, t),
            Sfx::Invalid => self.invalid(s, t),
            Sfx::Clear { lines, combo } => self.clear(s, t, lines, combo),
            Sfx::BoardClear => {
                let t = t + 0.25;
                for (i, f) in [523.25, 659.25, 783.99, 1046.5].into_iter().enumerate() {
                    let at = t + i as f64 * 0.08;
                    self.note(p, &lead, f * tr, at, patch_vol(&lead) * 1.2, Some(0.25), None);
                }
                self.sparkle(s, t + 0.3, 0.6);
            }
            Sfx::StyleShift => {
                // A sparkly upward sweep when the board clear switches the style.
                let t = t + 0.7;
                let root = p.root as f64 * 2.0 * tr;
                for (i, &step) in p.scale.iter().enumerate() {
                    let f = root * semis(step as f64);
                    let at = t + i as f64 * 0.035;
                    self.note(p, &lead, f, at, patch_vol(&lead) * 0.4, None, Some(0.3));
                }
                self.sparkle(s, t + 0.15, 0.5);
            }
            Sfx::TierUp(level) => self.tier_up(s, t, level),
            Sfx::GameOver => {
                for (i, f) in [392.0, 330.0, 262.0, 196.0].into_iter().enumerate() {
                    let at = t + i as f64 * 0.16;
                    self.note(p, &lead, f * tr, at, patch_vol(&lead), Some(0.15), None);
                }
            }
        }
    }

    /// The original effects, kept as they were for the Classic tier (transposed by `tr`).
    fn play_classic(&mut self, sfx: Sfx, profile: &SoundProfile, t: f64, tr: f64) {
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
                220.0 * tr,
                t,
                0.12,
                ToneOpts {
                    to: Some(160.0 * tr),
                    ..tone_opts(Wave::Sine, 0.15)
                },
            ),
            Sfx::Clear { lines, combo } => self.clear_classic(profile, t, lines, combo, tr),
            Sfx::BoardClear => {
                let t = t + 0.25;
                for (i, f) in [523.25, 659.25, 783.99, 1046.5].into_iter().enumerate() {
                    self.tone(
                        profile,
                        f * tr,
                        t + i as f64 * 0.08,
                        0.6,
                        tone_opts(Wave::Triangle, 0.18),
                    );
                }
            }
            Sfx::StyleShift => {
                let t = t + 0.7;
                let root = profile.root as f64 * 2.0 * tr;
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
            Sfx::TierUp(level) => self.tier_up_classic(profile, t, level, tr),
            Sfx::GameOver => {
                for (i, f) in [392.0, 330.0, 262.0, 196.0].into_iter().enumerate() {
                    self.tone(
                        profile,
                        f * tr,
                        t + i as f64 * 0.16,
                        0.3,
                        tone_opts(Wave::Triangle, 0.2),
                    );
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Materials
    // -----------------------------------------------------------------------

    /// Picking up a piece.
    fn pickup(&mut self, s: &SoundStyle, t: f64) {
        let p = &s.profile;
        let tr = s.transposition();
        let lp = |f: f64| s.tone() * f;
        match s.material() {
            Material::Classic => self.play_classic(Sfx::Pickup, p, t, tr),
            Material::Cardboard => {
                // A papery rustle and a soft, muffled blip.
                self.noise(
                    t,
                    0.08,
                    0.3,
                    NoiseOpts {
                        to: Some(lp(2600.0)),
                        attack: 0.01,
                        ..band(lp(900.0), 1.2)
                    },
                );
                self.tone(
                    p,
                    330.0 * tr,
                    t,
                    0.06,
                    ToneOpts {
                        to: Some(440.0 * tr),
                        lp: Some((lp(1200.0), lp(1200.0), 0.7)),
                        ..tone_opts(Wave::Triangle, 0.1)
                    },
                );
            }
            Material::Candy => {
                // Two bubbly blips.
                self.tone(p, 600.0 * tr, t, 0.06, glide(0.14, 1300.0 * tr));
                self.tone(p, 900.0 * tr, t + 0.035, 0.05, glide(0.08, 1800.0 * tr));
            }
            Material::Gingerbread => {
                // A celesta tick and a whisper of sleigh bells.
                self.fm(p, 1568.0 * tr, t, 0.3, 0.08, 4.0, 1.2, Some(0.2));
                self.noise(t, 0.03, 0.035, noise(FilterKind::Highpass, 7000.0));
            }
            Material::Wood => {
                // A light woodblock tick.
                self.noise(t, 0.03, 0.55, band(2400.0, 5.0));
                self.tone(p, 1200.0, t, 0.03, glide(0.08, 1100.0));
            }
            Material::Gem => {
                // Three glints.
                for (i, f) in [2093.0, 2637.0, 3136.0].into_iter().enumerate() {
                    let o = ToneOpts {
                        attack: 0.002,
                        echo: Some(0.3),
                        ..tone_opts(Wave::Sine, 0.04)
                    };
                    self.tone(p, f * tr, t + i as f64 * 0.025, 0.12, o);
                }
            }
            Material::Iron => {
                // A small metallic tick.
                self.fm(p, 880.0 * tr, t, 0.14, 0.07, 2.76, 3.0, Some(0.05));
                self.noise(t, 0.02, 0.05, noise(FilterKind::Highpass, 5000.0));
            }
            Material::Neon => {
                // A laser zap.
                let o = ToneOpts {
                    to: Some(1400.0 * tr),
                    lp: Some((lp(3000.0), lp(3000.0), 1.0)),
                    ..tone_opts(Wave::Square, 0.06)
                };
                self.tone(p, 400.0 * tr, t, 0.07, o);
            }
            Material::Glass => {
                // An airy glass ping.
                for (f, to, vol) in [(1318.5, 1568.0, 0.07), (2637.0, 3136.0, 0.02)] {
                    let o = ToneOpts {
                        to: Some(to * tr),
                        attack: 0.02,
                        echo: Some(0.4),
                        ..tone_opts(Wave::Sine, vol)
                    };
                    self.tone(p, f * tr, t, 0.2, o);
                }
            }
            Material::Royal => {
                // A harpsichord pluck.
                let lead = s.timbre().lead;
                self.note(p, &lead, 523.25 * tr, t, 0.1, None, Some(0.1));
            }
        }
    }

    /// Setting a piece down: the material itself.
    fn place(&mut self, s: &SoundStyle, t: f64) {
        let p = &s.profile;
        // The palettes shift the material's pitch a little (half their transposition).
        let tr = semis(s.tint.transpose as f64 * 0.5);
        let f = p.thud as f64 * tr;
        let lp = |f: f64| s.tone() * f;
        match s.material() {
            Material::Classic => self.play_classic(Sfx::Place, p, t, tr),
            Material::Cardboard => {
                // A muffled box thump and a papery scuff.
                self.tone(p, f, t, 0.14, glide(0.55, f * 0.55));
                self.noise(t, 0.09, 0.4, noise(FilterKind::Lowpass, lp(700.0)));
                self.noise(t, 0.05, 0.12, band(lp(2200.0), 0.8));
            }
            Material::Candy => {
                // A pop.
                let o = ToneOpts {
                    attack: 0.002,
                    ..glide(0.28, f * 3.3)
                };
                self.tone(p, f * 1.2, t, 0.06, o);
                self.tone(p, f * 0.8, t, 0.09, glide(0.3, f * 0.5));
                self.noise(t, 0.012, 0.1, noise(FilterKind::Highpass, 4000.0));
            }
            Material::Gingerbread => {
                // A crumbly cookie thock with a tiny bell.
                let o = ToneOpts {
                    to: Some(f * 0.55),
                    echo: Some(0.0),
                    ..tone_opts(Wave::Triangle, 0.42)
                };
                self.tone(p, f, t, 0.11, o);
                self.noise(t, 0.07, 0.3, band(lp(1100.0), 0.7));
                self.fm(p, 1568.0 * tr, t + 0.01, 0.3, 0.025, 4.0, 1.0, Some(0.2));
            }
            Material::Wood => {
                // A wood knock: the body, the click and the block's ring.
                self.tone(p, f * 1.1, t, 0.08, glide(0.3, f * 0.85));
                self.noise(t, 0.045, 0.9, band(lp(f * 12.0), 8.0));
                let o = ToneOpts {
                    attack: 0.001,
                    ..glide(0.16, f * 6.8)
                };
                self.tone(p, f * 7.0, t, 0.05, o);
            }
            Material::Gem => {
                // A crystalline clink over a soft thud.
                self.tone(p, f, t, 0.08, glide(0.32, f * 0.6));
                for (ratio, vol, dur) in [(1.0, 0.06, 0.2), (2.76, 0.03, 0.12)] {
                    let o = ToneOpts {
                        attack: 0.002,
                        echo: Some(0.3),
                        ..tone_opts(Wave::Sine, vol)
                    };
                    self.tone(p, 1760.0 * ratio * tr, t, dur, o);
                }
                self.noise(t, 0.015, 0.08, noise(FilterKind::Highpass, 6000.0));
            }
            Material::Iron => {
                // An anvil clank.
                self.tone(p, f, t, 0.16, glide(0.5, f * 0.65));
                self.fm(p, f * 2.3, t, 0.3, 0.16, 1.41, 5.0, Some(0.15));
                self.noise(t, 0.05, 0.3, band(3200.0, 2.0));
            }
            Material::Neon => {
                // A synth thump: a filter-swept saw over a sub.
                let o = ToneOpts {
                    to: Some(f * 0.5),
                    echo: Some(0.0),
                    lp: Some((lp(1800.0), 200.0, 2.0)),
                    ..tone_opts(Wave::Sawtooth, 0.3)
                };
                self.tone(p, f, t, 0.12, o);
                self.tone(p, f, t, 0.12, glide(0.32, f * 0.42));
                self.noise(t, 0.05, 0.15, noise(FilterKind::Lowpass, 4000.0));
            }
            Material::Glass => {
                // A soft glass tap.
                self.tone(p, f, t, 0.09, glide(0.32, f * 0.7));
                for (ratio, vol, dur) in [(1.0, 0.05, 0.3), (2.32, 0.02, 0.15)] {
                    let o = ToneOpts {
                        attack: 0.003,
                        echo: Some(0.45),
                        ..tone_opts(Wave::Sine, vol)
                    };
                    self.tone(p, 1318.5 * ratio * tr, t, dur, o);
                }
                self.noise(t, 0.012, 0.05, noise(FilterKind::Highpass, 7000.0));
            }
            Material::Royal => {
                // A timpani stroke.
                let o = ToneOpts {
                    attack: 0.004,
                    ..glide(0.5, f * 0.7)
                };
                self.tone(p, f * 0.8, t, 0.5, o);
                let o = ToneOpts {
                    to: Some(f * 1.4),
                    echo: Some(0.0),
                    ..tone_opts(Wave::Triangle, 0.08)
                };
                self.tone(p, f * 1.6, t, 0.25, o);
                self.noise(t, 0.06, 0.25, noise(FilterKind::Lowpass, 900.0));
            }
        }
    }

    /// A piece that doesn't fit.
    fn invalid(&mut self, s: &SoundStyle, t: f64) {
        let p = &s.profile;
        let tr = s.transposition();
        let lp = |f: f64| s.tone() * f;
        match s.material() {
            Material::Classic => self.play_classic(Sfx::Invalid, p, t, tr),
            Material::Cardboard => {
                // Two dull thumps.
                for at in [t, t + 0.09] {
                    self.tone(p, 150.0, at, 0.07, glide(0.32, 100.0));
                    self.noise(at, 0.05, 0.2, noise(FilterKind::Lowpass, lp(400.0)));
                }
            }
            Material::Candy => {
                // A deflating double boing.
                self.tone(p, 500.0 * tr, t, 0.15, glide(0.13, 250.0 * tr));
                self.tone(p, 375.0 * tr, t + 0.07, 0.13, glide(0.1, 190.0 * tr));
            }
            Material::Gingerbread => {
                // Two muted celesta notes, falling.
                self.fm(p, 392.0 * tr, t, 0.25, 0.1, 4.0, 0.8, Some(0.1));
                self.fm(p, 330.0 * tr, t + 0.1, 0.25, 0.1, 4.0, 0.8, Some(0.1));
            }
            Material::Wood => {
                // Two low knocks.
                for at in [t, t + 0.08] {
                    self.noise(at, 0.04, 0.75, band(lp(700.0), 4.0));
                    self.tone(p, 300.0, at, 0.05, glide(0.15, 280.0));
                }
            }
            Material::Gem => {
                // A dissonant chime.
                for f in [880.0, 932.33] {
                    let o = ToneOpts {
                        attack: 0.002,
                        ..tone_opts(Wave::Sine, 0.055)
                    };
                    self.tone(p, f * tr, t, 0.25, o);
                }
            }
            Material::Iron => {
                // A dull clang.
                self.fm(p, 150.0 * tr, t, 0.22, 0.14, 1.41, 3.0, Some(0.1));
                self.noise(t, 0.06, 0.2, noise(FilterKind::Lowpass, 600.0));
            }
            Material::Neon => {
                // An error buzz.
                for (f, to) in [(160.0, 120.0), (166.0, 124.0)] {
                    let o = ToneOpts {
                        to: Some(to * tr),
                        lp: Some((lp(1200.0), lp(800.0), 1.0)),
                        ..tone_opts(Wave::Square, 0.055)
                    };
                    self.tone(p, f * tr, t, 0.16, o);
                }
            }
            Material::Glass => {
                // A soft, rubbed glass discord.
                for (f, vol) in [(466.16, 0.07), (493.88, 0.06)] {
                    let o = ToneOpts {
                        attack: 0.03,
                        echo: Some(0.3),
                        ..tone_opts(Wave::Sine, vol)
                    };
                    self.tone(p, f * tr, t, 0.3, o);
                }
            }
            Material::Royal => {
                // A low brass "bwomp".
                let o = ToneOpts {
                    to: Some(110.0 * tr),
                    lp: Some((lp(900.0), lp(500.0), 1.0)),
                    ..tone_opts(Wave::Sawtooth, 0.13)
                };
                self.tone(p, 147.0 * tr, t, 0.22, o);
            }
        }
    }

    /// Rising arpeggio on the lead; higher and longer with more lines and bigger combos.
    fn clear(&mut self, s: &SoundStyle, t: f64, lines: u32, combo: u32) {
        let p = &s.profile;
        let lead = s.timbre().lead;
        let root = p.root as f64 * s.transposition() * semis((combo as f64 - 1.0).min(7.0));
        let notes = p.scale.len().min(2 + lines as usize);
        let vol = patch_vol(&lead);
        for i in 0..notes {
            let f = root * semis(p.scale[i] as f64);
            let at = t + i as f64 * 0.055;
            self.note(p, &lead, f, at, vol, None, None);
            self.tone(p, f * 2.0, at, 0.2, tone_opts(Wave::Sine, 0.06));
            if p.bell {
                self.tone(p, f * 4.0, at + 0.02, 0.35, tone_opts(Wave::Sine, 0.03));
            }
        }
        if p.pad {
            // A soft chord under the arpeggio.
            for step in [0.0, 7.0, 12.0] {
                let o = ToneOpts {
                    attack: 0.08,
                    ..tone_opts(Wave::Triangle, 0.06)
                };
                self.tone(p, root / 2.0 * semis(step), t, 0.9, o);
            }
        }
        self.clear_texture(s, t);
    }

    /// The noise that goes with a line clear, in the material.
    fn clear_texture(&mut self, s: &SoundStyle, t: f64) {
        let tone = s.tone();
        match s.material() {
            Material::Cardboard => {
                self.noise(t, 0.25, 0.14, band(1500.0 * tone, 0.8));
            }
            Material::Gingerbread => self.jingle(t, 0.1, 0.25),
            Material::Gem | Material::Glass => {
                self.burst(t, 0.2, 0.06, 6000.0);
                self.sparkle(s, t + 0.05, 0.35);
            }
            Material::Iron => self.noise(t, 0.25, 0.16, band(4000.0, 3.0)),
            Material::Neon => {
                // Gated: held, then cut.
                let o = NoiseOpts {
                    attack: 0.005,
                    ..noise(FilterKind::Lowpass, 7000.0 * tone)
                };
                self.noise(t, 0.2, 0.06, o);
            }
            Material::Wood => self.noise(t, 0.12, 0.25, band(2000.0, 3.0)),
            _ => self.burst(t, 0.25, 0.12, (6000.0 * tone) as f32),
        }
    }

    /// Material glitter on the bigger moments: sleigh bells, glints, sparkles.
    fn sparkle(&mut self, s: &SoundStyle, t: f64, vol: f64) {
        let p = &s.profile;
        let tr = s.transposition();
        match s.material() {
            Material::Gingerbread => self.jingle(t, 0.08 * vol, 0.4),
            Material::Gem | Material::Glass | Material::Candy => {
                for (i, step) in [24.0, 28.0, 31.0, 36.0].into_iter().enumerate() {
                    let o = ToneOpts {
                        attack: 0.002,
                        echo: Some(0.45),
                        ..tone_opts(Wave::Sine, 0.025 * vol)
                    };
                    let f = p.root as f64 * tr * semis(step - 12.0);
                    self.tone(p, f, t + i as f64 * 0.045, 0.18, o);
                }
            }
            _ => {}
        }
    }

    /// A fanfare when a new style tier is unlocked; grander for higher tiers.
    fn tier_up(&mut self, s: &SoundStyle, t: f64, level: usize) {
        let p = &s.profile;
        let fanfare = s.timbre().fanfare;
        let t = t + 0.2;
        let root = 261.63 * s.transposition();
        let chords = tier_up_chords(level);
        let beat = 0.16;
        let vol = patch_vol(&fanfare) * 0.9;
        for (i, chord) in chords.iter().enumerate() {
            let last = i == chords.len() - 1;
            let at = t + i as f64 * beat;
            let hold = if last { 0.9 } else { 0.12 };
            for &step in chord.iter() {
                let f = root * semis(step);
                self.note(p, &fanfare, f, at, vol, Some(hold), Some(0.3));
                let o = ToneOpts {
                    echo: Some(0.3),
                    ..tone_opts(Wave::Sine, 0.04)
                };
                self.tone(p, f * 2.0, at, if last { 0.9 } else { 0.25 }, o);
                if level >= 8 {
                    let o = ToneOpts {
                        attack: 0.04,
                        ..tone_opts(Wave::Triangle, 0.08)
                    };
                    self.tone(p, f / 2.0, at, if last { 1.4 } else { 0.35 }, o);
                }
            }
            if s.material() == Material::Royal {
                // Timpani under every chord.
                let f = root / 2.0 * semis(chord[0]);
                let o = ToneOpts {
                    attack: 0.004,
                    echo: Some(0.2),
                    ..glide(0.35, f * 0.92)
                };
                self.tone(p, f, at, if last { 0.9 } else { 0.4 }, o);
            }
        }
        if level >= 9 {
            self.cascade(p, t + chords.len() as f64 * beat);
        }
        self.sparkle(s, t + chords.len() as f64 * beat, 1.0);
    }

    /// Shimmering cascade on top of the final tier's fanfare.
    fn cascade(&mut self, p: &SoundProfile, t: f64) {
        for i in 0..8 {
            let step = [0.0, 4.0, 7.0, 12.0][i % 4] / 12.0 + (i / 4) as f64;
            let o = ToneOpts {
                echo: Some(0.5),
                ..tone_opts(Wave::Sine, 0.03)
            };
            self.tone(p, 1046.5 * 2f64.powf(step), t + i as f64 * 0.05, 0.4, o);
        }
    }

    // -----------------------------------------------------------------------
    // The original Classic effects
    // -----------------------------------------------------------------------

    /// Rising arpeggio; higher and longer with more lines and bigger combos.
    fn clear_classic(&mut self, p: &SoundProfile, t: f64, lines: u32, combo: u32, tr: f64) {
        let lead_vol = wave_vol(CLASSIC_LEAD);
        let root = p.root as f64 * tr * semis((combo as f64 - 1.0).min(7.0));
        let notes = p.scale.len().min(2 + lines as usize);
        for i in 0..notes {
            let f = root * semis(p.scale[i] as f64);
            let at = t + i as f64 * 0.055;
            self.tone(p, f, at, 0.25, tone_opts(CLASSIC_LEAD, lead_vol));
            self.tone(p, f * 2.0, at, 0.2, tone_opts(Wave::Sine, 0.1));
            if p.bell {
                self.tone(p, f * 4.0, at + 0.02, 0.35, tone_opts(Wave::Sine, 0.035));
            }
        }
        if p.pad {
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

    fn tier_up_classic(&mut self, p: &SoundProfile, t: f64, level: usize, tr: f64) {
        let t = t + 0.2;
        let root = 261.63 * tr;
        let lead_vol = wave_vol(CLASSIC_LEAD);
        let chords = tier_up_chords(level);
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
                        ..tone_opts(CLASSIC_LEAD, lead_vol * 0.9)
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
            self.cascade(p, t + chords.len() as f64 * beat);
        }
    }

    // -----------------------------------------------------------------------
    // Voices
    // -----------------------------------------------------------------------

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
        gain.exp_ramp_to(o.vol * self.sfx_gain, start + o.attack);
        gain.exp_ramp_to(0.0001, start + dur);
        let filter = match o.lp {
            Some((from, to, q)) => {
                let mut c = Param::starting_at(from as f32, sr, first);
                c.set_value_at(from, start);
                if to != from {
                    c.exp_ramp_to(to, start + dur);
                }
                Some(VoiceFilter::new(FilterKind::Lowpass, c, q))
            }
            // Raw saws are harsh; soften them like an analogue synth.
            None => (o.wave == Wave::Sawtooth)
                .then(|| VoiceFilter::new(FilterKind::Lowpass, Param::new(2400.0, sr), DEFAULT_Q)),
        };
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

    /// A struck FM tone: bright at the attack (`index`), mellowing quickly, decaying over `dur`.
    #[allow(clippy::too_many_arguments)]
    fn fm(
        &mut self,
        p: &SoundProfile,
        freq: f64,
        start: f64,
        dur: f64,
        vol: f64,
        ratio: f64,
        index: f64,
        echo: Option<f32>,
    ) {
        let sr = self.sr;
        let first = (start * sr).ceil() as u64;
        let mut gain = Param::starting_at(0.0001, sr, first);
        gain.set_value_at(0.0001, start);
        gain.exp_ramp_to(vol * self.sfx_gain, start + 0.002);
        gain.exp_ramp_to(0.0001, start + dur);
        let fm = Fm {
            ratio,
            index,
            settle_to: 0.1,
            settle: dur * 0.2,
            vibrato: 0.0,
        };
        self.voices.add(Voice {
            start: first,
            stop: ((start + dur + 0.02) * sr).ceil() as u64,
            source: Source::fm(&fm, freq, start, first, sr),
            filter: None,
            gain,
            dest: Dest::Sfx,
            send: echo.unwrap_or(p.echo).max(0.0),
        });
    }

    /// A note on a patch: its envelope (held for `hold`, if given), filter and layer.
    #[allow(clippy::too_many_arguments)]
    fn note(
        &mut self,
        p: &SoundProfile,
        patch: &Patch,
        freq: f64,
        start: f64,
        vol: f64,
        hold: Option<f64>,
        echo: Option<f32>,
    ) {
        let send = echo.unwrap_or(p.echo).max(0.0);
        let v = self.patch_voice(patch.src, patch, freq, start, vol, hold, patch.decay, send);
        self.voices.add(v);
        if let Some(l) = patch.layer {
            let f = freq * semis(l.semis);
            let v = self.patch_voice(l.src, patch, f, start, vol * l.level, None, l.decay, send);
            self.voices.add(v);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn patch_voice(
        &self,
        src: Src,
        patch: &Patch,
        freq: f64,
        start: f64,
        vol: f64,
        hold: Option<f64>,
        decay: f64,
        send: f32,
    ) -> Voice {
        let sr = self.sr;
        let first = (start * sr).ceil() as u64;
        let mut gain = Param::starting_at(0.0, sr, first);
        let a = patch.attack;
        let vol = vol * self.sfx_gain;
        let end = start + a.max(hold.unwrap_or(a + decay));
        gain.set_value_at(0.0, start);
        gain.linear_ramp_to(vol, start + a);
        gain.set_target_at(vol * patch.sustain, start + a, decay / 4.0);
        gain.set_target_at(0.0, end, patch.release / 4.0);
        let stop = end + patch.release * 1.5;
        let filter = (patch.cutoff > 0.0).then(|| {
            let mut c = Param::starting_at(patch.cutoff as f32, sr, first);
            c.set_value_at(patch.cutoff, start);
            c.set_target_at(patch.cutoff_end, start, decay / 3.0);
            VoiceFilter::new(FilterKind::Lowpass, c, patch.q as f32)
        });
        let source = match src {
            Src::Osc(wave) => Source::Osc {
                wave,
                phase: 0.0,
                freq: Param::new(freq as f32, sr),
                detune: 1.0,
            },
            Src::Fm(fm) => Source::fm(&fm, freq, start, first, sr),
        };
        Voice {
            start: first,
            stop: (stop * sr).ceil() as u64,
            source,
            filter,
            gain,
            dest: Dest::Sfx,
            send,
        }
    }

    fn burst(&mut self, start: f64, dur: f64, vol: f64, cutoff: f32) {
        let vol = vol * self.sfx_gain;
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

    /// A filtered noise burst with an optional attack and filter sweep.
    fn noise(&mut self, start: f64, dur: f64, vol: f64, o: NoiseOpts) {
        let vol = vol * self.sfx_gain;
        let sr = self.sr;
        let first = (start * sr).ceil() as u64;
        let mut gain = Param::starting_at(0.0001, sr, first);
        if o.attack > 0.0 {
            gain.set_value_at(0.0001, start);
            gain.exp_ramp_to(vol, start + o.attack);
        } else {
            gain.set_value_at(vol, start);
        }
        gain.exp_ramp_to(0.0001, start + dur);
        let mut f = Param::starting_at(o.freq as f32, sr, first);
        f.set_value_at(o.freq, start);
        if let Some(to) = o.to {
            f.exp_ramp_to(to, start + dur);
        }
        // Start somewhere in the buffer, so repeated bursts differ.
        let pos = (first.wrapping_mul(7919) % (self.noise.burst.len() as u64 / 2)) as usize;
        self.voices.add(Voice {
            start: first,
            stop: ((start + dur) * sr).ceil() as u64,
            source: Source::Noise {
                buf: NoiseBuf::Burst,
                pos,
                looping: false,
            },
            filter: Some(VoiceFilter::new(o.kind, f, o.q)),
            gain,
            dest: Dest::Sfx,
            send: o.echo,
        });
    }

    /// Sleigh bells: a few quick shakes of bright noise, then a short ring.
    fn jingle(&mut self, start: f64, vol: f64, dur: f64) {
        let vol = vol * self.sfx_gain;
        let sr = self.sr;
        let first = (start * sr).ceil() as u64;
        let mut g = Param::starting_at(0.0, sr, first);
        g.set_value_at(0.0, start);
        for (i, dt) in [0.0, 0.03, 0.055, 0.085].into_iter().enumerate() {
            g.set_value_at(vol * [1.0, 0.7, 0.85, 0.6][i], start + dt);
            g.set_target_at(vol * 0.1, start + dt, 0.008);
        }
        g.set_target_at(0.0, start + 0.1, dur / 4.0);
        let pos = (first.wrapping_mul(104_729) % (self.noise.burst.len() as u64 / 2)) as usize;
        self.voices.add(Voice {
            start: first,
            stop: ((start + 0.1 + dur * 1.5) * sr).ceil() as u64,
            source: Source::Noise {
                buf: NoiseBuf::Burst,
                pos,
                looping: false,
            },
            filter: Some(VoiceFilter::new(
                FilterKind::Highpass,
                Param::new(6500.0, sr),
                3.0,
            )),
            gain: g,
            dest: Dest::Sfx,
            send: 0.2,
        });
    }
}

/// I - IV - V - I, adding voices and length as the tiers rise (Neon is level 7, Aurora 8, Legendary 9).
fn tier_up_chords(level: usize) -> &'static [&'static [f64]] {
    const ALL: [&[f64]; 4] = [
        &[0.0, 4.0, 7.0],
        &[5.0, 9.0, 12.0],
        &[7.0, 11.0, 14.0],
        &[12.0, 16.0, 19.0, 24.0],
    ];
    &ALL[if level >= 7 { 0 } else { 2 }..]
}
