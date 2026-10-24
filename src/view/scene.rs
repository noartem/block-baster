//! Everything on screen that animates, as plain data. The app fills it in and
//! the renderer draws it; effects expire on their own as time passes.

use std::collections::HashSet;

use crate::color::Rgb;
use crate::game::board::{Board, Lines, SIZE, create_board};
use crate::game::game::Piece;
use crate::game::generator::TRAY_SIZE;
use crate::layout::Layout;
use crate::render::sprites::LOOKS;
use crate::styles::{ParticleShape, Style, style_for};

/// The look (see [`LOOKS`]) of the `k`-th cell of a piece whose looks were seeded with `seed`.
/// A block keeps its look from the tray, through the drag, onto the board.
pub fn piece_look(seed: u32, k: usize) -> u8 {
    let mut h = seed ^ (k as u32).wrapping_mul(0x9e37_79b9);
    h ^= h >> 16;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 16;
    (h % LOOKS as u32) as u8
}

pub struct DragView {
    pub piece: Piece,
    pub slot: usize,
    /// Top-left of the piece in logical pixels.
    pub x: f32,
    pub y: f32,
    pub cell: f32,
    /// Snap target on the board, if the piece can be dropped there.
    pub target: Option<(i32, i32, Lines)>,
}

pub struct FlyingPiece {
    pub piece: Piece,
    pub from: (f32, f32, f32),
    pub slot: usize,
    pub start: f64,
    pub duration: f64,
}

#[derive(Clone, Copy)]
pub struct ClearingCell {
    pub r: usize,
    pub c: usize,
    pub color: u8,
    pub look: u8,
    pub delay: f64,
}

/// Special particles: puffs of dust, tumbling block debris, confetti and
/// glowing motes. Plain particles are drawn as their `shape`.
#[derive(Clone, Copy, PartialEq)]
pub enum ParticleKind {
    Plain,
    Dust,
    /// Debris drawn as a block of this colour id and look.
    Block(u8, u8),
    Confetti,
    Mote,
}

#[derive(Clone, Copy)]
pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub size: f32,
    pub color: Rgb,
    pub born: f64,
    pub life: f64,
    pub shape: ParticleShape,
    pub gravity: f32,
    /// Rotation speed in radians per second.
    pub spin: f32,
    pub kind: ParticleKind,
}

/// An expanding shockwave ring.
pub struct Ring {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub color: Rgb,
    pub start: f64,
    pub duration: f64,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Line {
    Row(usize),
    Col(usize),
}

/// A light beam along a cleared row or column.
pub struct Beam {
    pub line: Line,
    pub color: Rgb,
    pub start: f64,
}

/// A bright streak racing along a cleared line, away from where the piece landed.
pub struct Streak {
    pub line: Line,
    /// Position along the line it starts from, in cells.
    pub from: f32,
    pub color: Rgb,
    pub start: f64,
}

pub struct FloatingText {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub size: f32,
    pub color: Rgb,
    pub start: f64,
    pub duration: f64,
    /// Big centred banners pop in; small popups just float up.
    pub banner: bool,
    /// Keeps wobbling after popping in (combo counters).
    pub bounce: bool,
}

pub struct Placed {
    pub cells: HashSet<usize>,
    pub start: f64,
}

pub struct Ripple {
    pub r: f32,
    pub c: f32,
    pub start: f64,
}

pub struct Clearing {
    pub cells: Vec<ClearingCell>,
    pub start: f64,
}

pub struct Shake {
    pub start: f64,
    pub power: f32,
}

/// A burst of light where lines were cleared; `power` 0–1.
pub struct Flash {
    pub x: f32,
    pub y: f32,
    pub power: f32,
    pub color: Rgb,
    pub start: f64,
}

/// Glow pulsing in from the screen edges on combos and board clears; `power` 0–1.
pub struct Pulse {
    pub power: f32,
    pub color: Rgb,
    pub start: f64,
}

pub struct Scene {
    pub layout: Layout,
    pub dpr: f32,
    pub now: f64,
    pub reduced_motion: bool,
    pub style: Style,
    /// Start of the shimmer that sweeps the board when the style changes.
    pub style_wave_at: Option<f64>,
    pub board: Board,
    /// The look of the block in each board cell.
    pub looks: [u8; SIZE * SIZE],
    pub tray: [Option<Piece>; TRAY_SIZE],
    /// Seeds of the looks of the pieces in the tray (see [`piece_look`]).
    pub tray_looks: [u32; TRAY_SIZE],
    /// The tray is a lucky one (see is_lucky_deal) and glows softly.
    pub lucky: bool,
    pub playable: [bool; TRAY_SIZE],
    pub hidden_slot: Option<usize>,
    pub drag: Option<DragView>,
    pub flying: Option<FlyingPiece>,
    pub tray_dealt_at: f64,
    pub placed: Option<Placed>,
    /// Blocks near a placed piece bob in a wave spreading out from its centre (in cells).
    pub ripple: Option<Ripple>,
    pub clearing: Option<Clearing>,
    pub particles: Vec<Particle>,
    pub rings: Vec<Ring>,
    pub beams: Vec<Beam>,
    pub streaks: Vec<Streak>,
    pub texts: Vec<FloatingText>,
    pub shake: Option<Shake>,
    pub flash: Option<Flash>,
    pub pulse: Option<Pulse>,
    pub game_over_at: Option<f64>,
}

impl Scene {
    pub fn new(now: f64) -> Self {
        Scene {
            layout: Layout::default(),
            dpr: 1.0,
            now,
            reduced_motion: false,
            style: style_for(0, 0),
            style_wave_at: None,
            board: create_board(),
            // A saved game doesn't keep looks; mix them up by position.
            looks: std::array::from_fn(|i| piece_look(0x5eed_b10c, i)),
            tray: [None; TRAY_SIZE],
            tray_looks: [0; TRAY_SIZE],
            lucky: false,
            playable: [false; TRAY_SIZE],
            hidden_slot: None,
            drag: None,
            flying: None,
            tray_dealt_at: now,
            placed: None,
            ripple: None,
            clearing: None,
            particles: Vec::new(),
            rings: Vec::new(),
            beams: Vec::new(),
            streaks: Vec::new(),
            texts: Vec::new(),
            shake: None,
            flash: None,
            pulse: None,
            game_over_at: None,
        }
    }

    /// Drops every running effect (new game).
    pub fn reset_effects(&mut self) {
        self.hidden_slot = None;
        self.flying = None;
        self.placed = None;
        self.ripple = None;
        self.clearing = None;
        self.particles.clear();
        self.rings.clear();
        self.beams.clear();
        self.streaks.clear();
        self.texts.clear();
        self.shake = None;
        self.flash = None;
        self.pulse = None;
        self.game_over_at = None;
        self.style_wave_at = None;
    }
}
