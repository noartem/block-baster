//! The sound of each style. A tier's [`Material`] picks the instruments: those
//! of its sound effects and those the soundtrack plays its (shared) parts on,
//! from the lofi keys to the EDM arpeggio, plus the drum kit and the mix. A
//! palette's [`Tint`] then transposes it, picks the harmony and shades its
//! brightness, so the palettes of a tier are siblings.
//!
//! The music's structure (what plays when, following the intensity) never
//! depends on the style: only the timbres here do. Classic keeps the original
//! instruments exactly.

use super::dsp::FilterKind;
use crate::styles::{Material, Mode, SoundProfile, TIERS, Tint, Wave};

/// A tier and palette, resolved to everything the synth needs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoundStyle {
    pub tier: usize,
    pub variant: usize,
    pub profile: SoundProfile,
    pub tint: Tint,
}

impl Default for SoundStyle {
    fn default() -> Self {
        SoundStyle::new(0, 0)
    }
}

impl SoundStyle {
    /// The style of a tier level and palette variant (out-of-range values are clamped / wrapped).
    pub fn new(tier: usize, variant: usize) -> Self {
        let tier = tier.min(TIERS.len() - 1);
        let profile = TIERS[tier].sound;
        let variant = variant % profile.tints.len();
        SoundStyle {
            tier,
            variant,
            profile,
            tint: profile.tints[variant],
        }
    }

    pub fn material(&self) -> Material {
        self.profile.material
    }

    pub fn timbre(&self) -> &'static Timbre {
        timbre(self.profile.material)
    }

    /// Frequency ratio of the palette's transposition.
    pub fn transposition(&self) -> f64 {
        2f64.powf(self.tint.transpose as f64 / 12.0)
    }

    /// Brightness factor for filter cutoffs.
    pub fn tone(&self) -> f64 {
        self.tint.tone as f64
    }

    pub fn mode(&self) -> Mode {
        self.tint.mode
    }
}

// ---------------------------------------------------------------------------
// Instruments
// ---------------------------------------------------------------------------

/// Two-operator FM: a sine carrier modulated by a sine at `ratio` times its
/// frequency. The depth starts at `index` times the carrier frequency and
/// settles to `settle_to` of that with the time constant `settle` (a bright
/// attack that mellows), plus an optional 5 Hz vibrato of `vibrato` cents.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fm {
    pub ratio: f64,
    pub index: f64,
    pub settle_to: f64,
    pub settle: f64,
    pub vibrato: f64,
}

/// The sound source of a note.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Src {
    Osc(Wave),
    Fm(Fm),
}

const fn fm(ratio: f64, index: f64, settle_to: f64, settle: f64, vibrato: f64) -> Src {
    Src::Fm(Fm {
        ratio,
        index,
        settle_to,
        settle,
        vibrato,
    })
}

/// A second, quieter voice on each note: an overtone, a sparkle, a detuned double.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layer {
    pub src: Src,
    /// Interval above the note, in (fractional) semitones.
    pub semis: f64,
    /// Level relative to the note.
    pub level: f64,
    pub decay: f64,
}

/// A melodic instrument: a source, an ADSR envelope, an optional low-pass
/// with its own envelope and an optional layer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Patch {
    pub src: Src,
    pub attack: f64,
    pub decay: f64,
    /// Relative to the peak, 0..1.
    pub sustain: f64,
    pub release: f64,
    /// Low-pass cutoff at the attack, gliding to `cutoff_end` over the decay; 0 = unfiltered.
    pub cutoff: f64,
    pub cutoff_end: f64,
    pub q: f64,
    pub layer: Option<Layer>,
    /// Level relative to the classic instrument in the same part, so the parts stay balanced.
    pub gain: f64,
}

const fn patch(src: Src, attack: f64, decay: f64, sustain: f64, release: f64) -> Patch {
    Patch {
        src,
        attack,
        decay,
        sustain,
        release,
        cutoff: 0.0,
        cutoff_end: 0.0,
        q: 0.7,
        layer: None,
        gain: 1.0,
    }
}

impl Patch {
    const fn lp(mut self, cutoff: f64, cutoff_end: f64, q: f64) -> Self {
        self.cutoff = cutoff;
        self.cutoff_end = cutoff_end;
        self.q = q;
        self
    }

    const fn layer(mut self, src: Src, semis: f64, level: f64, decay: f64) -> Self {
        self.layer = Some(Layer {
            src,
            semis,
            level,
            decay,
        });
        self
    }

    const fn gain(mut self, gain: f64) -> Self {
        self.gain = gain;
        self
    }
}

/// A bass voice; the envelopes come from the part that plays it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bass {
    pub wave: Wave,
    /// Low-pass in Hz (lofi bass, 0 = none), or a factor on the part's own cutoff (house bass).
    pub cutoff: f64,
    /// Filter Q (lofi bass), or a factor on the part's own Q (house bass).
    pub q: f64,
    pub gain: f64,
}

const fn bass(wave: Wave, cutoff: f64, q: f64, gain: f64) -> Bass {
    Bass {
        wave,
        cutoff,
        q,
        gain,
    }
}

/// The held chords of house and EDM: two detuned voices per note.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pad {
    pub wave: Wave,
    /// Each voice is this many cents off, one up, one down.
    pub detune: f64,
    pub attack: f64,
    pub gain: f64,
}

const fn pad(wave: Wave, detune: f64, attack: f64, gain: f64) -> Pad {
    Pad {
        wave,
        detune,
        attack,
        gain,
    }
}

// ---------------------------------------------------------------------------
// Drums
// ---------------------------------------------------------------------------

/// Sine kick with a pitch drop: from `from` (+ `punch` Hz at full punch) down to `to`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Kick {
    pub from: f64,
    pub punch: f64,
    pub to: f64,
    pub decay: f64,
    /// Level of the beater click relative to the classic one, and its band.
    pub click: f64,
    pub click_freq: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnareKind {
    /// Band-passed noise and a low tone.
    Noise,
    /// Noise held, then cut: the gated reverb snare of the eighties.
    Gated,
    /// A woodblock / rim knock: a narrow band of noise and a short high tone.
    Wood,
    /// Noise and an inharmonic FM clank.
    Metal,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snare {
    pub kind: SnareKind,
    pub freq: f64,
    pub q: f64,
    /// MIDI note of the tone.
    pub tone: f64,
    pub tone_level: f64,
    /// Factor on the part's decay.
    pub decay: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clap {
    pub freq: f64,
    pub q: f64,
    pub level: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HatKind {
    Noise,
    /// Sleigh bells: a few quick shakes.
    Jingle,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hat {
    pub kind: HatKind,
    pub filter: FilterKind,
    /// Factors on the part's frequency and decay.
    pub freq: f64,
    pub q: f64,
    pub decay: f64,
    pub level: f64,
    pub attack: f64,
}

/// What crackles under the lofi stage (in place of the vinyl crackle).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Texture {
    Vinyl,
    /// Rustling paper.
    Paper,
    /// Little bubbles popping.
    Pops,
    /// Distant sleigh bells.
    Sleigh,
    /// Faint glassy glints.
    Sparkle,
    /// Tape hiss only.
    Tape,
}

/// Extra percussion that marks the material.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Accent {
    None,
    /// High and low woodblocks on the offbeats.
    Woodblock,
    /// An anvil clank late in the bar.
    Anvil,
    /// Timpani on the downbeats of the phrase and in the builds.
    Timpani,
    /// A high chime on chord changes.
    Chime,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Kit {
    pub kick: Kick,
    pub snare: Snare,
    pub clap: Clap,
    pub hat: Hat,
    /// High-pass of the crash.
    pub crash: f64,
    pub texture: Texture,
    pub accent: Accent,
}

/// The material's mix: factors on the classic settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mix {
    /// Reverb sends of the melodic parts and the effects bus.
    pub reverb: f64,
    /// Reverb send of the drums.
    pub drum_reverb: f64,
    /// Tape-delay sends.
    pub delay: f64,
    /// Level of the lofi hiss and crackle.
    pub hiss: f64,
    /// Drive of the saturation on kick and bass.
    pub drive: f64,
    /// Brightness of the lofi keys' filter.
    pub keys_tone: f64,
    /// Output level, matching the loudness of the other materials.
    pub trim: f64,
}

/// Level trims of the material's effects in dB, so each style is about as loud as the others.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SfxLevels {
    pub pickup: f64,
    pub place: f64,
    pub invalid: f64,
    /// Line clears.
    pub clear: f64,
    /// Board clear, style shift and game over.
    pub lead: f64,
    /// The tier-up fanfare.
    pub fanfare: f64,
}

const fn levels(pickup: f64, place: f64, invalid: f64, clear: f64, lead: f64, fanfare: f64) -> SfxLevels {
    SfxLevels {
        pickup,
        place,
        invalid,
        clear,
        lead,
        fanfare,
    }
}

/// Everything a material sounds like.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timbre {
    /// Lofi chords.
    pub keys: Patch,
    /// The lofi melody.
    pub melody: Patch,
    /// The EDM arpeggio.
    pub arp: Patch,
    /// Lofi bass.
    pub bass: Bass,
    /// House and EDM bass.
    pub synth_bass: Bass,
    /// House and EDM chords.
    pub pad: Pad,
    pub kit: Kit,
    pub mix: Mix,
    /// Line clears and the other melodic effects.
    pub lead: Patch,
    /// The tier-up fanfare.
    pub fanfare: Patch,
    pub levels: SfxLevels,
}

pub fn timbre(m: Material) -> &'static Timbre {
    match m {
        Material::Classic => &CLASSIC,
        Material::Cardboard => &CARDBOARD,
        Material::Candy => &CANDY,
        Material::Gingerbread => &GINGERBREAD,
        Material::Wood => &WOOD,
        Material::Gem => &GEM,
        Material::Iron => &IRON,
        Material::Neon => &NEON,
        Material::Glass => &GLASS,
        Material::Royal => &ROYAL,
    }
}

const fn kick(from: f64, punch: f64, to: f64, decay: f64, click: f64, click_freq: f64) -> Kick {
    Kick {
        from,
        punch,
        to,
        decay,
        click,
        click_freq,
    }
}

const fn snare(kind: SnareKind, freq: f64, q: f64, tone: f64, tone_level: f64, decay: f64) -> Snare {
    Snare {
        kind,
        freq,
        q,
        tone,
        tone_level,
        decay,
    }
}

const fn clap(freq: f64, q: f64, level: f64) -> Clap {
    Clap { freq, q, level }
}

const fn hat(kind: HatKind, filter: FilterKind, freq: f64, q: f64, decay: f64, level: f64) -> Hat {
    Hat {
        kind,
        filter,
        freq,
        q,
        decay,
        level,
        attack: 0.001,
    }
}

const fn mix(reverb: f64, drum_reverb: f64, delay: f64, hiss: f64, drive: f64, keys_tone: f64, trim: f64) -> Mix {
    Mix {
        reverb,
        drum_reverb,
        delay,
        hiss,
        drive,
        keys_tone,
        trim,
    }
}

use FilterKind::{Bandpass, Highpass};
use HatKind::{Jingle, Noise as HatNoise};
use SnareKind::{Gated, Metal, Noise, Wood};
use Wave::{Sawtooth, Sine, Square, Triangle};

/// The original sound: FM e-piano, sine melody, square arpeggio, saw chords.
pub const CLASSIC: Timbre = Timbre {
    keys: patch(fm(1.0, 1.8, 0.15, 0.25, 8.0), 0.01, 1.6, 0.35, 1.2),
    melody: patch(Src::Osc(Sine), 0.005, 0.25, 0.0, 0.3).layer(Src::Osc(Triangle), 12.0, 0.12, 0.08),
    arp: patch(Src::Osc(Square), 0.003, 0.13, 0.0, 0.04),
    bass: bass(Triangle, 0.0, 0.7, 1.0),
    synth_bass: bass(Sawtooth, 1.0, 1.0, 1.0),
    pad: pad(Sawtooth, 9.0, 0.02, 1.0),
    kit: Kit {
        kick: kick(120.0, 110.0, 46.0, 0.42, 1.0, 2200.0),
        snare: snare(Noise, 1900.0, 0.6, 54.0, 0.6, 1.0),
        clap: clap(1150.0, 1.1, 1.0),
        hat: hat(HatNoise, Highpass, 1.0, 0.7, 1.0, 1.0),
        crash: 4500.0,
        texture: Texture::Vinyl,
        accent: Accent::None,
    },
    mix: mix(1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0),
    lead: patch(Src::Osc(Square), 0.008, 0.25, 0.0, 0.1),
    fanfare: patch(Src::Osc(Square), 0.008, 0.3, 0.0, 0.1),
    levels: levels(0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
};

/// Muffled and papery: a felt piano, soft triangle tones, taped-box drums, lots of crackle.
const CARDBOARD: Timbre = Timbre {
    keys: patch(fm(1.0, 0.9, 0.12, 0.12, 0.0), 0.006, 1.1, 0.18, 0.5).gain(1.15),
    melody: patch(Src::Osc(Triangle), 0.008, 0.3, 0.0, 0.25).lp(1300.0, 900.0, 0.7).gain(1.35),
    arp: patch(Src::Osc(Triangle), 0.003, 0.12, 0.0, 0.05).lp(2200.0, 900.0, 0.7).gain(2.2),
    bass: bass(Sine, 0.0, 0.7, 1.15),
    synth_bass: bass(Triangle, 0.8, 0.5, 1.5),
    pad: pad(Triangle, 7.0, 0.04, 1.6),
    kit: Kit {
        kick: kick(95.0, 70.0, 44.0, 0.36, 0.5, 1100.0),
        snare: snare(Noise, 850.0, 0.7, 50.0, 0.7, 1.1),
        clap: clap(850.0, 0.9, 0.9),
        hat: hat(HatNoise, Highpass, 0.55, 0.7, 1.3, 0.9),
        crash: 3000.0,
        texture: Texture::Paper,
        accent: Accent::None,
    },
    mix: mix(0.7, 0.6, 0.9, 1.3, 0.9, 0.7, 1.0),
    lead: patch(Src::Osc(Triangle), 0.006, 0.25, 0.0, 0.15).lp(1600.0, 1000.0, 0.7).gain(1.4),
    fanfare: patch(Src::Osc(Triangle), 0.01, 0.35, 0.0, 0.2).lp(1800.0, 1100.0, 0.7).gain(1.4),
    levels: levels(-0.7, -5.0, -7.0, -2.1, -2.7, 0.0),
};

/// Bright and bubbly: vibraphone chords, xylophone and marimba, bouncy drums, little pops.
const CANDY: Timbre = Timbre {
    keys: patch(fm(4.0, 0.7, 0.08, 0.1, 10.0), 0.003, 1.2, 0.2, 0.7),
    melody: patch(fm(3.0, 1.6, 0.05, 0.05, 0.0), 0.002, 0.22, 0.0, 0.15).gain(0.95),
    arp: patch(fm(4.0, 1.4, 0.05, 0.04, 0.0), 0.002, 0.14, 0.0, 0.05).gain(1.6),
    bass: bass(Square, 520.0, 1.0, 0.75),
    synth_bass: bass(Square, 1.0, 1.0, 0.85),
    pad: pad(Square, 8.0, 0.02, 0.75),
    kit: Kit {
        kick: kick(160.0, 90.0, 55.0, 0.3, 1.2, 3000.0),
        snare: snare(Noise, 2900.0, 1.0, 66.0, 0.45, 0.6),
        clap: clap(1600.0, 1.2, 1.0),
        hat: hat(HatNoise, Highpass, 1.15, 0.7, 0.8, 1.0),
        crash: 5500.0,
        texture: Texture::Pops,
        accent: Accent::None,
    },
    mix: mix(0.8, 0.8, 1.15, 0.6, 1.0, 1.25, 1.0),
    lead: patch(fm(3.0, 1.8, 0.05, 0.05, 0.0), 0.002, 0.3, 0.0, 0.2),
    fanfare: patch(fm(4.0, 1.4, 0.05, 0.06, 0.0), 0.002, 0.45, 0.0, 0.25),
    levels: levels(-3.7, -0.2, -1.6, -3.5, -2.7, -2.3),
};

/// Warm and festive: a reed organ, a music box and a celesta, brushes and sleigh bells.
const GINGERBREAD: Timbre = Timbre {
    keys: patch(fm(3.0, 0.8, 0.6, 0.3, 5.0), 0.03, 1.5, 0.6, 0.6).gain(0.9),
    melody: patch(fm(5.0, 1.0, 0.1, 0.15, 0.0), 0.002, 0.5, 0.0, 0.35)
        .layer(Src::Osc(Sine), 24.0, 0.2, 0.25)
        .gain(0.9),
    arp: patch(fm(4.0, 0.9, 0.1, 0.1, 0.0), 0.002, 0.18, 0.0, 0.1).gain(1.4),
    bass: bass(Triangle, 0.0, 0.7, 1.0),
    synth_bass: bass(Sawtooth, 0.7, 1.0, 1.0),
    pad: pad(Triangle, 6.0, 0.05, 1.6),
    kit: Kit {
        kick: kick(110.0, 80.0, 48.0, 0.4, 0.6, 1800.0),
        snare: snare(Noise, 1500.0, 0.5, 52.0, 0.4, 1.3),
        clap: clap(1100.0, 1.0, 0.9),
        hat: hat(Jingle, Highpass, 0.9, 0.7, 1.0, 1.0),
        crash: 6000.0,
        texture: Texture::Sleigh,
        accent: Accent::None,
    },
    mix: mix(1.15, 1.0, 0.9, 0.7, 0.9, 0.85, 1.0),
    lead: patch(fm(4.0, 1.2, 0.1, 0.15, 0.0), 0.002, 0.5, 0.0, 0.4).layer(Src::Osc(Sine), 24.0, 0.2, 0.2),
    fanfare: patch(fm(5.0, 1.0, 0.1, 0.15, 0.0), 0.002, 0.7, 0.0, 0.5).layer(Src::Osc(Sine), 24.0, 0.2, 0.25),
    levels: levels(-6.5, -0.3, -2.8, -5.6, -4.7, -3.5),
};

/// Knocks and resonant bars: kalimba chords, marimba, woodblocks and a shaker.
const WOOD: Timbre = Timbre {
    keys: patch(fm(6.0, 1.2, 0.0, 0.03, 0.0), 0.002, 1.0, 0.12, 0.5).gain(1.05),
    melody: patch(fm(4.0, 1.6, 0.0, 0.05, 0.0), 0.002, 0.35, 0.0, 0.25),
    arp: patch(fm(4.0, 1.3, 0.0, 0.04, 0.0), 0.002, 0.15, 0.0, 0.06).gain(1.6),
    bass: bass(Triangle, 900.0, 0.7, 1.05),
    synth_bass: bass(Triangle, 1.0, 1.0, 1.4),
    pad: pad(Triangle, 5.0, 0.03, 1.6),
    kit: Kit {
        kick: kick(140.0, 70.0, 58.0, 0.3, 0.8, 1500.0),
        snare: snare(Wood, 1800.0, 5.0, 79.0, 0.5, 0.5),
        clap: clap(1300.0, 1.6, 0.9),
        hat: Hat {
            attack: 0.008,
            ..hat(HatNoise, Bandpass, 0.65, 1.5, 1.6, 1.2)
        },
        crash: 4000.0,
        texture: Texture::Vinyl,
        accent: Accent::Woodblock,
    },
    mix: mix(0.75, 0.7, 0.85, 0.6, 1.0, 1.0, 1.0),
    lead: patch(fm(4.0, 1.8, 0.0, 0.05, 0.0), 0.002, 0.35, 0.0, 0.2),
    fanfare: patch(fm(6.0, 1.2, 0.0, 0.04, 0.0), 0.002, 0.6, 0.0, 0.3),
    levels: levels(1.7, 1.6, 0.7, -4.5, -3.1, -2.9),
};

/// Crystalline: FM bells, glass pings, ringing hats, glints in the air.
const GEM: Timbre = Timbre {
    keys: patch(fm(3.5, 1.4, 0.35, 0.6, 0.0), 0.002, 2.2, 0.25, 1.0),
    melody: patch(fm(3.5, 2.2, 0.15, 0.3, 0.0), 0.002, 0.6, 0.0, 0.45).gain(0.9),
    arp: patch(fm(2.0, 1.0, 0.1, 0.08, 0.0), 0.002, 0.16, 0.0, 0.08).gain(1.4),
    bass: bass(Sine, 0.0, 0.7, 1.15),
    synth_bass: bass(Sawtooth, 1.0, 1.0, 1.0),
    pad: pad(Triangle, 7.0, 0.06, 1.6),
    kit: Kit {
        kick: kick(130.0, 100.0, 48.0, 0.38, 0.8, 2600.0),
        snare: snare(Noise, 3000.0, 0.8, 60.0, 0.45, 0.8),
        clap: clap(1500.0, 1.2, 1.0),
        hat: hat(HatNoise, Bandpass, 1.15, 3.0, 1.4, 1.3),
        crash: 6000.0,
        texture: Texture::Sparkle,
        accent: Accent::Chime,
    },
    mix: mix(1.35, 1.2, 1.2, 0.4, 1.0, 1.2, 1.0),
    lead: patch(fm(3.5, 2.5, 0.15, 0.35, 0.0), 0.002, 0.6, 0.0, 0.5),
    fanfare: patch(fm(3.5, 2.0, 0.2, 0.4, 0.0), 0.002, 0.9, 0.0, 0.6),
    levels: levels(-2.5, 1.2, 1.5, -4.7, -5.5, -2.4),
};

/// Dark and industrial: hollow keys, metallic plucks, a gritty bass, clanking drums.
const IRON: Timbre = Timbre {
    keys: patch(fm(2.0, 1.5, 0.25, 0.3, 0.0), 0.005, 1.6, 0.3, 1.0),
    melody: patch(fm(1.41, 1.8, 0.25, 0.2, 0.0), 0.003, 0.35, 0.0, 0.3).gain(0.95),
    arp: patch(Src::Osc(Square), 0.003, 0.13, 0.0, 0.04).lp(1600.0, 700.0, 2.0).gain(1.0),
    bass: bass(Sawtooth, 420.0, 2.0, 1.0),
    synth_bass: bass(Sawtooth, 0.75, 1.4, 1.0),
    pad: pad(Square, 12.0, 0.02, 0.75),
    kit: Kit {
        kick: kick(150.0, 120.0, 42.0, 0.36, 1.6, 3400.0),
        snare: snare(Metal, 2600.0, 1.2, 64.0, 0.5, 0.8),
        clap: clap(1000.0, 1.4, 1.0),
        hat: hat(HatNoise, Bandpass, 0.95, 6.0, 1.2, 1.6),
        crash: 3500.0,
        texture: Texture::Vinyl,
        accent: Accent::Anvil,
    },
    mix: mix(0.9, 1.4, 0.8, 0.8, 2.2, 0.7, 1.0),
    lead: patch(fm(1.41, 2.5, 0.3, 0.25, 0.0), 0.002, 0.45, 0.0, 0.3),
    fanfare: patch(fm(1.41, 2.0, 0.35, 0.3, 0.0), 0.002, 0.6, 0.0, 0.35),
    levels: levels(-1.2, -6.2, -1.3, -3.8, -4.3, -0.8),
};

/// Synthwave: brassy poly synth, detuned saw leads, wide saw chords, gated snares.
const NEON: Timbre = Timbre {
    keys: patch(Src::Osc(Sawtooth), 0.02, 1.4, 0.5, 0.6).lp(3200.0, 1300.0, 1.0).gain(0.8),
    melody: patch(Src::Osc(Sawtooth), 0.01, 0.3, 0.3, 0.25)
        .lp(2800.0, 1800.0, 1.5)
        .layer(Src::Osc(Sawtooth), 0.12, 0.8, 0.3),
    arp: patch(Src::Osc(Sawtooth), 0.003, 0.13, 0.0, 0.04).lp(2200.0, 700.0, 4.0).gain(1.2),
    bass: bass(Sawtooth, 600.0, 3.0, 0.95),
    synth_bass: bass(Sawtooth, 1.2, 1.2, 1.0),
    pad: pad(Sawtooth, 15.0, 0.03, 1.0),
    kit: Kit {
        kick: kick(170.0, 90.0, 50.0, 0.34, 1.0, 2400.0),
        snare: snare(Gated, 1700.0, 0.55, 55.0, 0.5, 1.0),
        clap: clap(1200.0, 1.0, 0.8),
        hat: hat(HatNoise, Highpass, 1.0, 0.7, 0.8, 1.0),
        crash: 4500.0,
        texture: Texture::Tape,
        accent: Accent::None,
    },
    mix: mix(1.0, 3.0, 1.3, 0.5, 1.2, 1.1, 1.07),
    lead: patch(Src::Osc(Sawtooth), 0.004, 0.25, 0.0, 0.2)
        .lp(2600.0, 1400.0, 1.0)
        .layer(Src::Osc(Sawtooth), 0.15, 0.8, 0.25),
    fanfare: patch(Src::Osc(Sawtooth), 0.01, 0.4, 0.3, 0.25)
        .lp(1500.0, 3200.0, 1.5)
        .layer(Src::Osc(Sawtooth), 0.15, 0.8, 0.4),
    levels: levels(2.0, -3.6, 0.2, -1.1, 3.5, -1.2),
};

/// Airy: glass harmonica chords, glassy pings, slow pads, soft drums, lots of reverb.
const GLASS: Timbre = Timbre {
    keys: patch(fm(2.0, 0.45, 0.4, 0.5, 4.0), 0.15, 2.4, 0.65, 1.2),
    melody: patch(Src::Osc(Sine), 0.02, 0.5, 0.0, 0.5).layer(Src::Osc(Sine), 14.57, 0.22, 0.25),
    arp: patch(fm(2.0, 0.7, 0.1, 0.1, 0.0), 0.004, 0.16, 0.0, 0.12).gain(1.4),
    bass: bass(Sine, 0.0, 0.7, 1.15),
    synth_bass: bass(Triangle, 1.0, 1.0, 1.4),
    pad: pad(Triangle, 6.0, 0.25, 1.6),
    kit: Kit {
        kick: kick(100.0, 80.0, 45.0, 0.45, 0.4, 1500.0),
        snare: snare(Noise, 2600.0, 0.5, 58.0, 0.3, 1.5),
        clap: clap(1800.0, 0.9, 0.7),
        hat: hat(HatNoise, Highpass, 1.2, 0.7, 1.2, 0.75),
        crash: 6500.0,
        texture: Texture::Sparkle,
        accent: Accent::Chime,
    },
    mix: mix(1.9, 1.5, 1.35, 0.3, 0.9, 1.25, 0.95),
    lead: patch(Src::Osc(Sine), 0.03, 0.5, 0.0, 0.5).layer(Src::Osc(Sine), 12.0, 0.3, 0.3),
    fanfare: patch(fm(2.0, 0.6, 0.4, 0.4, 4.0), 0.04, 0.8, 0.3, 0.6),
    levels: levels(-3.6, 0.7, -0.9, -4.5, -5.4, -2.0),
};

/// Regal: harpsichord, brass, timpani, an orchestral snare.
const ROYAL: Timbre = Timbre {
    keys: patch(fm(3.0, 2.4, 0.15, 0.12, 0.0), 0.002, 1.0, 0.08, 0.3).gain(0.9),
    melody: patch(Src::Osc(Sawtooth), 0.04, 0.4, 0.6, 0.2).lp(700.0, 2600.0, 1.0).gain(1.2),
    arp: patch(fm(3.0, 2.2, 0.15, 0.08, 0.0), 0.002, 0.15, 0.0, 0.08).gain(1.4),
    bass: bass(Sawtooth, 550.0, 1.0, 1.0),
    synth_bass: bass(Sawtooth, 1.0, 1.0, 1.0),
    pad: pad(Sawtooth, 6.0, 0.1, 1.0),
    kit: Kit {
        kick: kick(100.0, 60.0, 52.0, 0.5, 0.3, 900.0),
        snare: snare(Noise, 2200.0, 0.7, 56.0, 0.55, 1.0),
        clap: clap(1100.0, 1.1, 0.8),
        hat: hat(HatNoise, Highpass, 1.0, 0.7, 1.0, 0.9),
        crash: 4000.0,
        texture: Texture::Tape,
        accent: Accent::Timpani,
    },
    mix: mix(1.4, 1.2, 0.9, 0.5, 1.0, 1.3, 1.0),
    lead: patch(fm(3.0, 2.4, 0.15, 0.1, 0.0), 0.002, 0.4, 0.0, 0.2),
    fanfare: patch(Src::Osc(Sawtooth), 0.03, 0.35, 0.55, 0.2).lp(900.0, 3000.0, 1.0),
    levels: levels(-9.8, -10.1, 1.5, -4.1, -4.0, -2.7),
};
