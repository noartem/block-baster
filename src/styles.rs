//! Visual styles. The game score unlocks style *tiers* (Classic → Legendary),
//! and each tier comes in a few colour *variants*; clearing the whole board
//! switches to the next variant of the current tier. A tier decides how the
//! blocks are drawn, the board frame, effects, font and sounds; a variant
//! decides the colours.

use crate::color::Rgb;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlockKind {
    Jewel,
    Cardboard,
    Candy,
    Gingerbread,
    Wood,
    Gem,
    Iron,
    Neon,
    Glass,
    Royal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FrameKind {
    Plain,
    Cardboard,
    Soft,
    Gingerbread,
    Wood,
    Metal,
    Iron,
    Neon,
    Glass,
    Gold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParticleShape {
    Square,
    Circle,
    Diamond,
    Spark,
    Star,
}

/// The display fonts of the tiers (all bundled in assets/fonts).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Font {
    Baloo,
    Lilita,
    Righteous,
    Orbitron,
    Cinzel,
}

/// Oscillator waveforms of the synthesised sounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wave {
    Sine,
    Square,
    Sawtooth,
    Triangle,
}

#[derive(Debug)]
pub struct Palette {
    pub name: &'static str,
    /// Block colours, indexed by colour id - 1.
    pub blocks: [Rgb; 7],
    pub board: Rgb,
    pub board_edge: Rgb,
    pub cell: Rgb,
    pub cell_inner: Rgb,
    /// Page background gradient: top, middle, bottom.
    pub bg: [Rgb; 3],
    /// HUD best score, buttons and banners.
    pub accent: Rgb,
    /// Glows, combo text and ambient background effects.
    pub glow: Rgb,
    pub glow2: Rgb,
    pub card: Rgb,
    pub card_edge: Rgb,
}

#[derive(Debug)]
pub struct Effects {
    /// Particles per cleared cell.
    pub particles: u32,
    pub shape: ParticleShape,
    /// Additive blending for particles, rings and beams.
    pub glow: bool,
    pub gravity: f32,
    /// Shockwave ring where lines are cleared.
    pub ring: bool,
    /// Light beam along each cleared row and column.
    pub beam: bool,
    /// Share of particles drawn in the palette's glow colours instead of the block colour.
    pub sparkle: f32,
    /// Colour of the flash on clearing blocks.
    pub flash: Rgb,
    /// Screen shake multiplier.
    pub shake: f32,
    /// Firework bursts when the whole board is cleared.
    pub fireworks: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoundProfile {
    /// Waveform of the line-clear arpeggio.
    pub lead: Wave,
    /// Root note of the arpeggio in Hz.
    pub root: f32,
    /// Arpeggio steps in semitones.
    pub scale: &'static [f32],
    /// Echo send, 0 = dry.
    pub echo: f32,
    /// Adds high bell overtones.
    pub bell: bool,
    /// Adds a soft chord under clears.
    pub pad: bool,
    /// Pitch of the placement thud in Hz.
    pub thud: f32,
}

#[derive(Debug)]
pub struct Tier {
    pub level: usize,
    pub name: &'static str,
    pub min_score: u32,
    pub block: BlockKind,
    pub frame: FrameKind,
    pub font: Font,
    /// Banner text glows in the palette's glow colour.
    pub text_glow: bool,
    /// Banner text is filled with a metallic gradient of the accent colour.
    pub text_gradient: bool,
    pub fx: Effects,
    pub sound: SoundProfile,
    pub variants: [Palette; 3],
}

#[derive(Clone, Copy, Debug)]
pub struct Style {
    pub tier: &'static Tier,
    pub palette: &'static Palette,
    pub variant: usize,
}

impl Style {
    /// Stable id, e.g. "2.1" for the second variant of tier 2.
    pub fn key(&self) -> String {
        format!("{}.{}", self.tier.level, self.variant)
    }

    pub fn same(&self, other: &Style) -> bool {
        self.tier.level == other.tier.level && self.variant == other.variant
    }

    /// Block colour for a 1-based colour id; 0 is the grey used for unplayable pieces and game over.
    pub fn color(&self, id: u8) -> Rgb {
        color_for(id, self.palette)
    }
}

pub const GRAY: Rgb = Rgb::hex("#7a8296");

/// Block colour for a 1-based colour id; 0 is the grey used for unplayable pieces and game over.
pub fn color_for(id: u8, palette: &Palette) -> Rgb {
    if id == 0 { GRAY } else { palette.blocks[(id as usize - 1) % palette.blocks.len()] }
}

/// The highest tier unlocked by the score.
pub fn tier_for(score: u32) -> &'static Tier {
    TIERS.iter().rev().find(|t| score >= t.min_score).unwrap_or(&TIERS[0])
}

/// The style for a game: its tier comes from the score, its variant from the number of board clears.
pub fn style_for(score: u32, board_clears: u32) -> Style {
    let tier = tier_for(score);
    let variant = board_clears as usize % tier.variants.len();
    Style { tier, palette: &tier.variants[variant], variant }
}

/// The next tier to unlock, or None at the top.
pub fn next_tier(tier: &Tier) -> Option<&'static Tier> {
    TIERS.get(tier.level + 1)
}

/// "50k", "1M"… for compact HUD labels.
pub fn format_score(n: u32) -> String {
    let compact = |v: f64, unit: &str| {
        let rounded = (v * 10.0).round() / 10.0;
        if rounded.fract() == 0.0 { format!("{}{unit}", rounded as u64) } else { format!("{rounded:.1}{unit}") }
    };
    if n >= 1_000_000 {
        compact(n as f64 / 1_000_000.0, "M")
    } else if n >= 1000 {
        compact(n as f64 / 1000.0, "k")
    } else {
        n.to_string()
    }
}

/// "1,234,567" (en-US grouping).
pub fn group_digits(n: u32) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

const CLASSIC_DEFAULT: SoundProfile = TIERS[0].sound;

/// The sound profile used before any style is applied.
pub fn default_sound_profile() -> SoundProfile {
    CLASSIC_DEFAULT
}

pub static TIERS: [Tier; 10] = [
    Tier {
        level: 0,
        name: "Classic",
        min_score: 0,
        block: BlockKind::Jewel,
        frame: FrameKind::Plain,
        font: Font::Baloo,
        text_glow: false,
        text_gradient: false,
        fx: Effects { particles: 4, shape: ParticleShape::Square, glow: false, gravity: 900.0, ring: false, beam: false, sparkle: 0.0, flash: Rgb::hex("#ffffff"), shake: 1.0, fireworks: 0 },
        sound: SoundProfile { lead: Wave::Square, root: 392.0, scale: &[0.0, 4.0, 7.0, 12.0, 16.0, 19.0, 24.0], echo: 0.0, bell: false, pad: false, thud: 180.0 },
        variants: [
            Palette {
                name: "Lagoon",
                blocks: [Rgb::hex("#ff6f61"), Rgb::hex("#ff9f1c"), Rgb::hex("#ffd23f"), Rgb::hex("#8bd346"), Rgb::hex("#41a6f6"), Rgb::hex("#8b5cf6"), Rgb::hex("#e85d9f")],
                board: Rgb::hex("#0a2f38"),
                board_edge: Rgb::hex("#061f26"),
                cell: Rgb::hex("#0f3d48"),
                cell_inner: Rgb::hex("#0d3640"),
                bg: [Rgb::hex("#1fa2a0"), Rgb::hex("#137a80"), Rgb::hex("#0b4f5c")],
                accent: Rgb::hex("#ffb547"),
                glow: Rgb::hex("#9ff5e6"),
                glow2: Rgb::hex("#ffffff"),
                card: Rgb::hex("#0f5e66"),
                card_edge: Rgb::hex("#073a40"),
            },
            Palette {
                name: "Ember",
                blocks: [Rgb::hex("#ff6f61"), Rgb::hex("#ff9f1c"), Rgb::hex("#ffd23f"), Rgb::hex("#8bd346"), Rgb::hex("#41a6f6"), Rgb::hex("#8b5cf6"), Rgb::hex("#e85d9f")],
                board: Rgb::hex("#2e130f"),
                board_edge: Rgb::hex("#1e0b08"),
                cell: Rgb::hex("#3d1b15"),
                cell_inner: Rgb::hex("#361712"),
                bg: [Rgb::hex("#e8794a"), Rgb::hex("#b84f33"), Rgb::hex("#6e2a22")],
                accent: Rgb::hex("#ffd84a"),
                glow: Rgb::hex("#ffc59a"),
                glow2: Rgb::hex("#ffffff"),
                card: Rgb::hex("#7a3324"),
                card_edge: Rgb::hex("#3e160f"),
            },
            Palette {
                name: "Sunset",
                blocks: [Rgb::hex("#ff6f61"), Rgb::hex("#ff9f1c"), Rgb::hex("#ffd23f"), Rgb::hex("#8bd346"), Rgb::hex("#41a6f6"), Rgb::hex("#8b5cf6"), Rgb::hex("#e85d9f")],
                board: Rgb::hex("#3a1d4a"),
                board_edge: Rgb::hex("#2a1236"),
                cell: Rgb::hex("#4a2860"),
                cell_inner: Rgb::hex("#422357"),
                bg: [Rgb::hex("#e0679a"), Rgb::hex("#a8467f"), Rgb::hex("#5e2a63")],
                accent: Rgb::hex("#ffc94a"),
                glow: Rgb::hex("#ffb38a"),
                glow2: Rgb::hex("#ffffff"),
                card: Rgb::hex("#6a2f6f"),
                card_edge: Rgb::hex("#3f1844"),
            },
        ],
    },
    Tier {
        level: 1,
        name: "Cardboard",
        min_score: 5000,
        block: BlockKind::Cardboard,
        frame: FrameKind::Cardboard,
        font: Font::Baloo,
        text_glow: false,
        text_gradient: false,
        fx: Effects { particles: 5, shape: ParticleShape::Square, glow: false, gravity: 1100.0, ring: false, beam: false, sparkle: 0.1, flash: Rgb::hex("#fff3dc"), shake: 1.0, fireworks: 2 },
        sound: SoundProfile { lead: Wave::Sine, root: 329.63, scale: &[0.0, 3.0, 5.0, 7.0, 10.0, 12.0, 15.0], echo: 0.0, bell: false, pad: false, thud: 140.0 },
        variants: [
            Palette {
                name: "Moving Day",
                blocks: [Rgb::hex("#e0524c"), Rgb::hex("#ee8a35"), Rgb::hex("#f0c53c"), Rgb::hex("#6db24e"), Rgb::hex("#36a9b8"), Rgb::hex("#4a72c4"), Rgb::hex("#a060b8")],
                board: Rgb::hex("#3b2a1a"),
                board_edge: Rgb::hex("#24180d"),
                cell: Rgb::hex("#4a3622"),
                cell_inner: Rgb::hex("#43301e"),
                bg: [Rgb::hex("#c9a06a"), Rgb::hex("#a37a48"), Rgb::hex("#6b4c2a")],
                accent: Rgb::hex("#ffd166"),
                glow: Rgb::hex("#ffe2b0"),
                glow2: Rgb::hex("#ffffff"),
                card: Rgb::hex("#7a5632"),
                card_edge: Rgb::hex("#3b2a1a"),
            },
            Palette {
                name: "Post Office",
                blocks: [Rgb::hex("#e0524c"), Rgb::hex("#ee8a35"), Rgb::hex("#f0c53c"), Rgb::hex("#6db24e"), Rgb::hex("#36a9b8"), Rgb::hex("#4a72c4"), Rgb::hex("#a060b8")],
                board: Rgb::hex("#2b313c"),
                board_edge: Rgb::hex("#1a1e26"),
                cell: Rgb::hex("#384050"),
                cell_inner: Rgb::hex("#333a49"),
                bg: [Rgb::hex("#8fa3b8"), Rgb::hex("#61758c"), Rgb::hex("#3a4859")],
                accent: Rgb::hex("#ffcf5c"),
                glow: Rgb::hex("#d8e6f5"),
                glow2: Rgb::hex("#ffffff"),
                card: Rgb::hex("#4c5b70"),
                card_edge: Rgb::hex("#2b313c"),
            },
            Palette {
                name: "Recycled",
                blocks: [Rgb::hex("#e0524c"), Rgb::hex("#ee8a35"), Rgb::hex("#f0c53c"), Rgb::hex("#6db24e"), Rgb::hex("#36a9b8"), Rgb::hex("#4a72c4"), Rgb::hex("#a060b8")],
                board: Rgb::hex("#2c3624"),
                board_edge: Rgb::hex("#1b2316"),
                cell: Rgb::hex("#3a4730"),
                cell_inner: Rgb::hex("#34402b"),
                bg: [Rgb::hex("#a3b17a"), Rgb::hex("#76874f"), Rgb::hex("#475530")],
                accent: Rgb::hex("#ffe07a"),
                glow: Rgb::hex("#e6f0c8"),
                glow2: Rgb::hex("#ffffff"),
                card: Rgb::hex("#56653c"),
                card_edge: Rgb::hex("#2c3624"),
            },
        ],
    },
    Tier {
        level: 2,
        name: "Candy",
        min_score: 12000,
        block: BlockKind::Candy,
        frame: FrameKind::Soft,
        font: Font::Lilita,
        text_glow: false,
        text_gradient: false,
        fx: Effects { particles: 6, shape: ParticleShape::Circle, glow: false, gravity: 700.0, ring: true, beam: false, sparkle: 0.25, flash: Rgb::hex("#ffffff"), shake: 1.0, fireworks: 3 },
        sound: SoundProfile { lead: Wave::Triangle, root: 440.0, scale: &[0.0, 2.0, 4.0, 7.0, 9.0, 12.0, 14.0, 16.0], echo: 0.15, bell: true, pad: false, thud: 220.0 },
        variants: [
            Palette {
                name: "Bubblegum",
                blocks: [Rgb::hex("#ff5d8f"), Rgb::hex("#ff9f43"), Rgb::hex("#ffd93d"), Rgb::hex("#5fd97f"), Rgb::hex("#4fd3ff"), Rgb::hex("#7b8cff"), Rgb::hex("#c77dff")],
                board: Rgb::hex("#6b2350"),
                board_edge: Rgb::hex("#4d1539"),
                cell: Rgb::hex("#7d2d60"),
                cell_inner: Rgb::hex("#742858"),
                bg: [Rgb::hex("#ff9ac9"), Rgb::hex("#e86aa8"), Rgb::hex("#b44886")],
                accent: Rgb::hex("#fff06a"),
                glow: Rgb::hex("#ffffff"),
                glow2: Rgb::hex("#ffd1ea"),
                card: Rgb::hex("#a23a78"),
                card_edge: Rgb::hex("#6b2350"),
            },
            Palette {
                name: "Mint Soda",
                blocks: [Rgb::hex("#ff5d8f"), Rgb::hex("#ff9f43"), Rgb::hex("#ffd93d"), Rgb::hex("#5fd97f"), Rgb::hex("#4fd3ff"), Rgb::hex("#7b8cff"), Rgb::hex("#c77dff")],
                board: Rgb::hex("#0f5450"),
                board_edge: Rgb::hex("#0a3b38"),
                cell: Rgb::hex("#146460"),
                cell_inner: Rgb::hex("#125c58"),
                bg: [Rgb::hex("#6ee6c9"), Rgb::hex("#33bfa6"), Rgb::hex("#1d8a7e")],
                accent: Rgb::hex("#fff27a"),
                glow: Rgb::hex("#ffffff"),
                glow2: Rgb::hex("#c8fff1"),
                card: Rgb::hex("#1f8a7e"),
                card_edge: Rgb::hex("#0f5450"),
            },
            Palette {
                name: "Lemon Sky",
                blocks: [Rgb::hex("#ff5d8f"), Rgb::hex("#ff9f43"), Rgb::hex("#ffd93d"), Rgb::hex("#5fd97f"), Rgb::hex("#4fd3ff"), Rgb::hex("#7b8cff"), Rgb::hex("#c77dff")],
                board: Rgb::hex("#1d3f8a"),
                board_edge: Rgb::hex("#142d66"),
                cell: Rgb::hex("#24499a"),
                cell_inner: Rgb::hex("#214492"),
                bg: [Rgb::hex("#7cc8ff"), Rgb::hex("#4d9cf0"), Rgb::hex("#3a6fd1")],
                accent: Rgb::hex("#ffe45c"),
                glow: Rgb::hex("#ffffff"),
                glow2: Rgb::hex("#fff7b0"),
                card: Rgb::hex("#2f5bb8"),
                card_edge: Rgb::hex("#1d3f8a"),
            },
        ],
    },
    Tier {
        level: 3,
        name: "Gingerbread",
        min_score: 25000,
        block: BlockKind::Gingerbread,
        frame: FrameKind::Gingerbread,
        font: Font::Lilita,
        text_glow: false,
        text_gradient: false,
        fx: Effects { particles: 6, shape: ParticleShape::Circle, glow: false, gravity: 650.0, ring: true, beam: false, sparkle: 0.4, flash: Rgb::hex("#fff8e8"), shake: 1.0, fireworks: 4 },
        sound: SoundProfile { lead: Wave::Triangle, root: 523.25, scale: &[0.0, 2.0, 4.0, 7.0, 9.0, 12.0, 16.0, 19.0], echo: 0.2, bell: true, pad: false, thud: 200.0 },
        variants: [
            Palette {
                name: "Christmas Eve",
                blocks: [Rgb::hex("#ff4d6d"), Rgb::hex("#ff9a3c"), Rgb::hex("#ffdb4d"), Rgb::hex("#5ad16a"), Rgb::hex("#4cc9f0"), Rgb::hex("#5b7cfa"), Rgb::hex("#c06cf0")],
                board: Rgb::hex("#3a1f12"),
                board_edge: Rgb::hex("#241208"),
                cell: Rgb::hex("#4a2a18"),
                cell_inner: Rgb::hex("#432515"),
                bg: [Rgb::hex("#c8402f"), Rgb::hex("#8e2219"), Rgb::hex("#4a0f0a")],
                accent: Rgb::hex("#ffe08a"),
                glow: Rgb::hex("#fff4e0"),
                glow2: Rgb::hex("#ff9aa8"),
                card: Rgb::hex("#7a1f17"),
                card_edge: Rgb::hex("#3a0d08"),
            },
            Palette {
                name: "Snowy Cottage",
                blocks: [Rgb::hex("#ff4d6d"), Rgb::hex("#ff9a3c"), Rgb::hex("#ffdb4d"), Rgb::hex("#5ad16a"), Rgb::hex("#4cc9f0"), Rgb::hex("#5b7cfa"), Rgb::hex("#c06cf0")],
                board: Rgb::hex("#3a2214"),
                board_edge: Rgb::hex("#22130a"),
                cell: Rgb::hex("#4a2d1b"),
                cell_inner: Rgb::hex("#432818"),
                bg: [Rgb::hex("#86b9e3"), Rgb::hex("#4f82ba"), Rgb::hex("#2a4d7a")],
                accent: Rgb::hex("#ffd27a"),
                glow: Rgb::hex("#ffffff"),
                glow2: Rgb::hex("#cfe8ff"),
                card: Rgb::hex("#2f5a8a"),
                card_edge: Rgb::hex("#1a3354"),
            },
            Palette {
                name: "Pine Forest",
                blocks: [Rgb::hex("#ff4d6d"), Rgb::hex("#ff9a3c"), Rgb::hex("#ffdb4d"), Rgb::hex("#5ad16a"), Rgb::hex("#4cc9f0"), Rgb::hex("#5b7cfa"), Rgb::hex("#c06cf0")],
                board: Rgb::hex("#352013"),
                board_edge: Rgb::hex("#1f1209"),
                cell: Rgb::hex("#45291a"),
                cell_inner: Rgb::hex("#3e2516"),
                bg: [Rgb::hex("#3f9a5a"), Rgb::hex("#246b3c"), Rgb::hex("#0f3d22")],
                accent: Rgb::hex("#ffcf5a"),
                glow: Rgb::hex("#fff1d0"),
                glow2: Rgb::hex("#ff8080"),
                card: Rgb::hex("#1f5a34"),
                card_edge: Rgb::hex("#0c2c18"),
            },
        ],
    },
    Tier {
        level: 4,
        name: "Wooden",
        min_score: 40000,
        block: BlockKind::Wood,
        frame: FrameKind::Wood,
        font: Font::Righteous,
        text_glow: false,
        text_gradient: false,
        fx: Effects { particles: 6, shape: ParticleShape::Diamond, glow: false, gravity: 850.0, ring: true, beam: false, sparkle: 0.15, flash: Rgb::hex("#fff4dc"), shake: 1.2, fireworks: 4 },
        sound: SoundProfile { lead: Wave::Triangle, root: 392.0, scale: &[0.0, 2.0, 4.0, 7.0, 9.0, 12.0, 14.0, 16.0], echo: 0.1, bell: false, pad: false, thud: 165.0 },
        variants: [
            Palette {
                name: "Oak Workshop",
                blocks: [Rgb::hex("#d23c2e"), Rgb::hex("#f08a2c"), Rgb::hex("#f5c829"), Rgb::hex("#3fb05a"), Rgb::hex("#1fb0b8"), Rgb::hex("#3b78dc"), Rgb::hex("#9a4fc0")],
                board: Rgb::hex("#2b1a0e"),
                board_edge: Rgb::hex("#1a0f07"),
                cell: Rgb::hex("#3a2414"),
                cell_inner: Rgb::hex("#331f11"),
                bg: [Rgb::hex("#c58b4f"), Rgb::hex("#8f5a2c"), Rgb::hex("#4f2f14")],
                accent: Rgb::hex("#ffd27a"),
                glow: Rgb::hex("#ffe0b0"),
                glow2: Rgb::hex("#ffffff"),
                card: Rgb::hex("#6b4122"),
                card_edge: Rgb::hex("#2b1a0e"),
            },
            Palette {
                name: "Walnut Study",
                blocks: [Rgb::hex("#d23c2e"), Rgb::hex("#f08a2c"), Rgb::hex("#f5c829"), Rgb::hex("#3fb05a"), Rgb::hex("#1fb0b8"), Rgb::hex("#3b78dc"), Rgb::hex("#9a4fc0")],
                board: Rgb::hex("#1f140d"),
                board_edge: Rgb::hex("#120b06"),
                cell: Rgb::hex("#2c1d13"),
                cell_inner: Rgb::hex("#271a11"),
                bg: [Rgb::hex("#7a5238"), Rgb::hex("#4e3322"), Rgb::hex("#24160d")],
                accent: Rgb::hex("#f5c46a"),
                glow: Rgb::hex("#ffcf9a"),
                glow2: Rgb::hex("#ffffff"),
                card: Rgb::hex("#4e3322"),
                card_edge: Rgb::hex("#1f140d"),
            },
            Palette {
                name: "Birch Grove",
                blocks: [Rgb::hex("#d23c2e"), Rgb::hex("#f08a2c"), Rgb::hex("#f5c829"), Rgb::hex("#3fb05a"), Rgb::hex("#1fb0b8"), Rgb::hex("#3b78dc"), Rgb::hex("#9a4fc0")],
                board: Rgb::hex("#2a2a20"),
                board_edge: Rgb::hex("#191912"),
                cell: Rgb::hex("#38382b"),
                cell_inner: Rgb::hex("#323226"),
                bg: [Rgb::hex("#c9c3a0"), Rgb::hex("#9a9472"), Rgb::hex("#5c5942")],
                accent: Rgb::hex("#ffe27a"),
                glow: Rgb::hex("#fff6d8"),
                glow2: Rgb::hex("#ffffff"),
                card: Rgb::hex("#6a664c"),
                card_edge: Rgb::hex("#2a2a20"),
            },
        ],
    },
    Tier {
        level: 5,
        name: "Gems",
        min_score: 60000,
        block: BlockKind::Gem,
        frame: FrameKind::Metal,
        font: Font::Lilita,
        text_glow: true,
        text_gradient: false,
        fx: Effects { particles: 6, shape: ParticleShape::Diamond, glow: true, gravity: 600.0, ring: true, beam: true, sparkle: 0.3, flash: Rgb::hex("#ffffff"), shake: 1.1, fireworks: 4 },
        sound: SoundProfile { lead: Wave::Sine, root: 523.25, scale: &[0.0, 4.0, 7.0, 11.0, 12.0, 16.0, 19.0, 23.0], echo: 0.25, bell: true, pad: false, thud: 200.0 },
        variants: [
            Palette {
                name: "Ruby Cave",
                blocks: [Rgb::hex("#e0115f"), Rgb::hex("#ff8c1a"), Rgb::hex("#ffd21f"), Rgb::hex("#14c46b"), Rgb::hex("#1fd1e0"), Rgb::hex("#2456e8"), Rgb::hex("#9b30ff")],
                board: Rgb::hex("#2a0a18"),
                board_edge: Rgb::hex("#1a0510"),
                cell: Rgb::hex("#3a1022"),
                cell_inner: Rgb::hex("#330d1e"),
                bg: [Rgb::hex("#9a1a42"), Rgb::hex("#5c0d2a"), Rgb::hex("#2e0618")],
                accent: Rgb::hex("#ffd86b"),
                glow: Rgb::hex("#ff6b9a"),
                glow2: Rgb::hex("#ffffff"),
                card: Rgb::hex("#5c0d2a"),
                card_edge: Rgb::hex("#2e0618"),
            },
            Palette {
                name: "Emerald Vault",
                blocks: [Rgb::hex("#e0115f"), Rgb::hex("#ff8c1a"), Rgb::hex("#ffd21f"), Rgb::hex("#14c46b"), Rgb::hex("#1fd1e0"), Rgb::hex("#2456e8"), Rgb::hex("#9b30ff")],
                board: Rgb::hex("#06241c"),
                board_edge: Rgb::hex("#031510"),
                cell: Rgb::hex("#0b3428"),
                cell_inner: Rgb::hex("#092d23"),
                bg: [Rgb::hex("#14855f"), Rgb::hex("#0a4f3c"), Rgb::hex("#052a20")],
                accent: Rgb::hex("#ffe27a"),
                glow: Rgb::hex("#6bffc8"),
                glow2: Rgb::hex("#ffffff"),
                card: Rgb::hex("#0a4f3c"),
                card_edge: Rgb::hex("#052a20"),
            },
            Palette {
                name: "Amethyst Grotto",
                blocks: [Rgb::hex("#e0115f"), Rgb::hex("#ff8c1a"), Rgb::hex("#ffd21f"), Rgb::hex("#14c46b"), Rgb::hex("#1fd1e0"), Rgb::hex("#2456e8"), Rgb::hex("#9b30ff")],
                board: Rgb::hex("#1e0a3a"),
                board_edge: Rgb::hex("#12052a"),
                cell: Rgb::hex("#2a1150"),
                cell_inner: Rgb::hex("#250e47"),
                bg: [Rgb::hex("#7330c4"), Rgb::hex("#44187e"), Rgb::hex("#220a44")],
                accent: Rgb::hex("#ffd86b"),
                glow: Rgb::hex("#d59bff"),
                glow2: Rgb::hex("#ffffff"),
                card: Rgb::hex("#44187e"),
                card_edge: Rgb::hex("#220a44"),
            },
        ],
    },
    Tier {
        level: 6,
        name: "Iron",
        min_score: 90000,
        block: BlockKind::Iron,
        frame: FrameKind::Iron,
        font: Font::Orbitron,
        text_glow: true,
        text_gradient: false,
        fx: Effects { particles: 7, shape: ParticleShape::Spark, glow: true, gravity: 900.0, ring: true, beam: false, sparkle: 0.55, flash: Rgb::hex("#ffe6c2"), shake: 1.5, fireworks: 5 },
        sound: SoundProfile { lead: Wave::Sine, root: 440.0, scale: &[0.0, 5.0, 7.0, 12.0, 17.0, 19.0, 24.0, 29.0], echo: 0.3, bell: true, pad: false, thud: 95.0 },
        variants: [
            Palette {
                name: "Foundry",
                blocks: [Rgb::hex("#e23b3b"), Rgb::hex("#f08c25"), Rgb::hex("#f2c62e"), Rgb::hex("#3dbb4c"), Rgb::hex("#22b5c8"), Rgb::hex("#3c6fe6"), Rgb::hex("#9c50dc")],
                board: Rgb::hex("#141618"),
                board_edge: Rgb::hex("#0a0b0c"),
                cell: Rgb::hex("#1f2225"),
                cell_inner: Rgb::hex("#1b1d20"),
                bg: [Rgb::hex("#5a5f66"), Rgb::hex("#33373d"), Rgb::hex("#16181b")],
                accent: Rgb::hex("#ff9a3c"),
                glow: Rgb::hex("#ffa54c"),
                glow2: Rgb::hex("#fff0c0"),
                card: Rgb::hex("#2a2e33"),
                card_edge: Rgb::hex("#0a0b0c"),
            },
            Palette {
                name: "Rust Belt",
                blocks: [Rgb::hex("#e23b3b"), Rgb::hex("#f08c25"), Rgb::hex("#f2c62e"), Rgb::hex("#3dbb4c"), Rgb::hex("#22b5c8"), Rgb::hex("#3c6fe6"), Rgb::hex("#9c50dc")],
                board: Rgb::hex("#1c120d"),
                board_edge: Rgb::hex("#0e0805"),
                cell: Rgb::hex("#291b13"),
                cell_inner: Rgb::hex("#241710"),
                bg: [Rgb::hex("#8a4b2a"), Rgb::hex("#5a2e18"), Rgb::hex("#2a140a")],
                accent: Rgb::hex("#ffc46b"),
                glow: Rgb::hex("#ff8a4c"),
                glow2: Rgb::hex("#ffd9a0"),
                card: Rgb::hex("#4a2615"),
                card_edge: Rgb::hex("#0e0805"),
            },
            Palette {
                name: "Blue Steel",
                blocks: [Rgb::hex("#e23b3b"), Rgb::hex("#f08c25"), Rgb::hex("#f2c62e"), Rgb::hex("#3dbb4c"), Rgb::hex("#22b5c8"), Rgb::hex("#3c6fe6"), Rgb::hex("#9c50dc")],
                board: Rgb::hex("#0f1720"),
                board_edge: Rgb::hex("#070b10"),
                cell: Rgb::hex("#18222e"),
                cell_inner: Rgb::hex("#151e29"),
                bg: [Rgb::hex("#4a6a8a"), Rgb::hex("#2a4058"), Rgb::hex("#121e2b")],
                accent: Rgb::hex("#9fd8ff"),
                glow: Rgb::hex("#8fcfff"),
                glow2: Rgb::hex("#ffffff"),
                card: Rgb::hex("#22344a"),
                card_edge: Rgb::hex("#070b10"),
            },
        ],
    },
    Tier {
        level: 7,
        name: "Neon",
        min_score: 130000,
        block: BlockKind::Neon,
        frame: FrameKind::Neon,
        font: Font::Orbitron,
        text_glow: true,
        text_gradient: false,
        fx: Effects { particles: 7, shape: ParticleShape::Spark, glow: true, gravity: 300.0, ring: true, beam: true, sparkle: 0.3, flash: Rgb::hex("#ffffff"), shake: 1.4, fireworks: 5 },
        sound: SoundProfile { lead: Wave::Sawtooth, root: 220.0, scale: &[0.0, 3.0, 5.0, 7.0, 10.0, 12.0, 15.0, 17.0], echo: 0.35, bell: false, pad: true, thud: 120.0 },
        variants: [
            Palette {
                name: "Synthwave",
                blocks: [Rgb::hex("#ff2a6d"), Rgb::hex("#ff8a00"), Rgb::hex("#f9f871"), Rgb::hex("#39ff14"), Rgb::hex("#05d9e8"), Rgb::hex("#4d6bff"), Rgb::hex("#e100d4")],
                board: Rgb::hex("#0b0420"),
                board_edge: Rgb::hex("#05010f"),
                cell: Rgb::hex("#120830"),
                cell_inner: Rgb::hex("#0f0628"),
                bg: [Rgb::hex("#3a0d5e"), Rgb::hex("#160633"), Rgb::hex("#05010f")],
                accent: Rgb::hex("#ff2a6d"),
                glow: Rgb::hex("#05d9e8"),
                glow2: Rgb::hex("#ff2a6d"),
                card: Rgb::hex("#1a0a3a"),
                card_edge: Rgb::hex("#ff2a6d"),
            },
            Palette {
                name: "Toxic",
                blocks: [Rgb::hex("#ff2a6d"), Rgb::hex("#ff8a00"), Rgb::hex("#f9f871"), Rgb::hex("#39ff14"), Rgb::hex("#05d9e8"), Rgb::hex("#4d6bff"), Rgb::hex("#e100d4")],
                board: Rgb::hex("#021006"),
                board_edge: Rgb::hex("#010803"),
                cell: Rgb::hex("#06190c"),
                cell_inner: Rgb::hex("#051509"),
                bg: [Rgb::hex("#0e4a20"), Rgb::hex("#05210f"), Rgb::hex("#010a04")],
                accent: Rgb::hex("#39ff14"),
                glow: Rgb::hex("#39ff14"),
                glow2: Rgb::hex("#f9f871"),
                card: Rgb::hex("#06250f"),
                card_edge: Rgb::hex("#39ff14"),
            },
            Palette {
                name: "Ultraviolet",
                blocks: [Rgb::hex("#ff2a6d"), Rgb::hex("#ff8a00"), Rgb::hex("#f9f871"), Rgb::hex("#39ff14"), Rgb::hex("#05d9e8"), Rgb::hex("#4d6bff"), Rgb::hex("#e100d4")],
                board: Rgb::hex("#070320"),
                board_edge: Rgb::hex("#030112"),
                cell: Rgb::hex("#0d0838"),
                cell_inner: Rgb::hex("#0b0630"),
                bg: [Rgb::hex("#24107a"), Rgb::hex("#0c0533"), Rgb::hex("#030112")],
                accent: Rgb::hex("#00f0ff"),
                glow: Rgb::hex("#b026ff"),
                glow2: Rgb::hex("#00f0ff"),
                card: Rgb::hex("#120a4a"),
                card_edge: Rgb::hex("#b026ff"),
            },
        ],
    },
    Tier {
        level: 8,
        name: "Aurora",
        min_score: 200000,
        block: BlockKind::Glass,
        frame: FrameKind::Glass,
        font: Font::Righteous,
        text_glow: true,
        text_gradient: false,
        fx: Effects { particles: 8, shape: ParticleShape::Star, glow: true, gravity: 180.0, ring: true, beam: true, sparkle: 0.45, flash: Rgb::hex("#ffffff"), shake: 1.4, fireworks: 6 },
        sound: SoundProfile { lead: Wave::Sine, root: 349.23, scale: &[0.0, 2.0, 4.0, 6.0, 7.0, 9.0, 11.0, 12.0, 14.0], echo: 0.45, bell: true, pad: true, thud: 160.0 },
        variants: [
            Palette {
                name: "Borealis",
                blocks: [Rgb::hex("#ff6b8b"), Rgb::hex("#ffa36b"), Rgb::hex("#ffe66b"), Rgb::hex("#6bffb0"), Rgb::hex("#6be4ff"), Rgb::hex("#7b93ff"), Rgb::hex("#c56bff")],
                board: Rgb::hex("#06223a"),
                board_edge: Rgb::hex("#031526"),
                cell: Rgb::hex("#0a2f4d"),
                cell_inner: Rgb::hex("#082944"),
                bg: [Rgb::hex("#0e4a6e"), Rgb::hex("#072a45"), Rgb::hex("#031526")],
                accent: Rgb::hex("#7dffcf"),
                glow: Rgb::hex("#4dffb5"),
                glow2: Rgb::hex("#b35dff"),
                card: Rgb::hex("#0a2f4d"),
                card_edge: Rgb::hex("#4dffb5"),
            },
            Palette {
                name: "Ice Crystal",
                blocks: [Rgb::hex("#ff6b8b"), Rgb::hex("#ffa36b"), Rgb::hex("#ffe66b"), Rgb::hex("#6bffb0"), Rgb::hex("#6be4ff"), Rgb::hex("#7b93ff"), Rgb::hex("#c56bff")],
                board: Rgb::hex("#102a52"),
                board_edge: Rgb::hex("#0a1c3a"),
                cell: Rgb::hex("#163566"),
                cell_inner: Rgb::hex("#13305d"),
                bg: [Rgb::hex("#4a8fd4"), Rgb::hex("#285a99"), Rgb::hex("#173a6b")],
                accent: Rgb::hex("#e6f6ff"),
                glow: Rgb::hex("#9fe8ff"),
                glow2: Rgb::hex("#ffffff"),
                card: Rgb::hex("#1d4580"),
                card_edge: Rgb::hex("#9fe8ff"),
            },
            Palette {
                name: "Nebula",
                blocks: [Rgb::hex("#ff6b8b"), Rgb::hex("#ffa36b"), Rgb::hex("#ffe66b"), Rgb::hex("#6bffb0"), Rgb::hex("#6be4ff"), Rgb::hex("#7b93ff"), Rgb::hex("#c56bff")],
                board: Rgb::hex("#1c0a2e"),
                board_edge: Rgb::hex("#0f0519"),
                cell: Rgb::hex("#26103d"),
                cell_inner: Rgb::hex("#210d36"),
                bg: [Rgb::hex("#5a2080"), Rgb::hex("#2c0f45"), Rgb::hex("#140620")],
                accent: Rgb::hex("#ff9ff3"),
                glow: Rgb::hex("#ff6bd6"),
                glow2: Rgb::hex("#6bd6ff"),
                card: Rgb::hex("#2c0f45"),
                card_edge: Rgb::hex("#ff6bd6"),
            },
        ],
    },
    Tier {
        level: 9,
        name: "Legendary",
        min_score: 350000,
        block: BlockKind::Royal,
        frame: FrameKind::Gold,
        font: Font::Cinzel,
        text_glow: true,
        text_gradient: true,
        fx: Effects { particles: 10, shape: ParticleShape::Star, glow: true, gravity: 500.0, ring: true, beam: true, sparkle: 0.5, flash: Rgb::hex("#fff1a8"), shake: 1.6, fireworks: 9 },
        sound: SoundProfile { lead: Wave::Triangle, root: 261.63, scale: &[0.0, 4.0, 7.0, 12.0, 16.0, 19.0, 24.0, 28.0], echo: 0.4, bell: true, pad: true, thud: 140.0 },
        variants: [
            Palette {
                name: "Golden Throne",
                blocks: [Rgb::hex("#e8173c"), Rgb::hex("#ff7a1a"), Rgb::hex("#ffcc00"), Rgb::hex("#1fbf5a"), Rgb::hex("#14b8d4"), Rgb::hex("#2f5bff"), Rgb::hex("#a020f0")],
                board: Rgb::hex("#1a0f02"),
                board_edge: Rgb::hex("#0c0700"),
                cell: Rgb::hex("#261705"),
                cell_inner: Rgb::hex("#211304"),
                bg: [Rgb::hex("#5a360c"), Rgb::hex("#2a1804"), Rgb::hex("#100800")],
                accent: Rgb::hex("#ffd700"),
                glow: Rgb::hex("#ffb300"),
                glow2: Rgb::hex("#fff1a8"),
                card: Rgb::hex("#2a1804"),
                card_edge: Rgb::hex("#ffd700"),
            },
            Palette {
                name: "Celestial",
                blocks: [Rgb::hex("#e8173c"), Rgb::hex("#ff7a1a"), Rgb::hex("#ffcc00"), Rgb::hex("#1fbf5a"), Rgb::hex("#14b8d4"), Rgb::hex("#2f5bff"), Rgb::hex("#a020f0")],
                board: Rgb::hex("#0a0a24"),
                board_edge: Rgb::hex("#04040f"),
                cell: Rgb::hex("#12123a"),
                cell_inner: Rgb::hex("#0f0f32"),
                bg: [Rgb::hex("#22225c"), Rgb::hex("#0d0d2e"), Rgb::hex("#04040f")],
                accent: Rgb::hex("#ffe27a"),
                glow: Rgb::hex("#8fb8ff"),
                glow2: Rgb::hex("#ffd700"),
                card: Rgb::hex("#14143f"),
                card_edge: Rgb::hex("#ffd700"),
            },
            Palette {
                name: "Phoenix",
                blocks: [Rgb::hex("#e8173c"), Rgb::hex("#ff7a1a"), Rgb::hex("#ffcc00"), Rgb::hex("#1fbf5a"), Rgb::hex("#14b8d4"), Rgb::hex("#2f5bff"), Rgb::hex("#a020f0")],
                board: Rgb::hex("#240502"),
                board_edge: Rgb::hex("#120201"),
                cell: Rgb::hex("#330903"),
                cell_inner: Rgb::hex("#2c0702"),
                bg: [Rgb::hex("#8a1a06"), Rgb::hex("#420903"), Rgb::hex("#160200")],
                accent: Rgb::hex("#ffb000"),
                glow: Rgb::hex("#ff5a00"),
                glow2: Rgb::hex("#ffd700"),
                card: Rgb::hex("#420903"),
                card_edge: Rgb::hex("#ffb000"),
            },
        ],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn unlocks_tiers_from_classic_to_legendary_at_rising_scores() {
        let names: Vec<_> = TIERS.iter().map(|t| t.name).collect();
        assert_eq!(names, ["Classic", "Cardboard", "Candy", "Gingerbread", "Wooden", "Gems", "Iron", "Neon", "Aurora", "Legendary"]);
        let mins: Vec<_> = TIERS.iter().map(|t| t.min_score).collect();
        assert_eq!(mins, [0, 5_000, 12_000, 25_000, 40_000, 60_000, 90_000, 130_000, 200_000, 350_000]);
        assert_eq!(tier_for(0).name, "Classic");
        assert_eq!(tier_for(4_999).level, 0);
        assert_eq!(tier_for(5_000).name, "Cardboard");
        assert_eq!(tier_for(39_999).name, "Gingerbread");
        assert_eq!(tier_for(99_999).name, "Iron");
        assert_eq!(tier_for(5_000_000).name, "Legendary");
        assert!(next_tier(tier_for(5_000_000)).is_none());
    }

    #[test]
    fn numbers_tiers_by_position() {
        for (i, tier) in TIERS.iter().enumerate() {
            assert_eq!(tier.level, i);
            if i > 0 {
                assert!(tier.min_score > TIERS[i - 1].min_score);
            }
            assert_eq!(next_tier(tier).map(|t| t.level), TIERS.get(i + 1).map(|t| t.level));
        }
    }

    #[test]
    fn gives_every_tier_its_own_blocks_and_frame() {
        assert_eq!(TIERS.iter().map(|t| t.block).collect::<HashSet<_>>().len(), TIERS.len());
        assert_eq!(TIERS.iter().map(|t| t.frame).collect::<HashSet<_>>().len(), TIERS.len());
    }

    #[test]
    fn cycles_through_the_variants_of_a_tier_on_board_clears() {
        let n = TIERS[0].variants.len() as u32;
        assert_eq!(style_for(0, 0).key(), "0.0");
        assert_eq!(style_for(0, 1).key(), "0.1");
        assert_eq!(style_for(0, n).key(), "0.0");
        assert_eq!(style_for(60_000, 2).key(), "5.2");
        assert_eq!(style_for(60_000, 2).tier.name, "Gems");
    }

    #[test]
    fn formats_compact_scores() {
        assert_eq!(format_score(50_000), "50k");
        assert_eq!(format_score(1_000_000), "1M");
        assert_eq!(format_score(12_500), "12.5k");
        assert_eq!(format_score(950), "950");
        assert_eq!(group_digits(1_234_567), "1,234,567");
        assert_eq!(group_digits(999), "999");
    }
}
