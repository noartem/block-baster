//! Draws the board, the tray and every effect of a frame (port of the Canvas 2D renderer).

use std::collections::HashMap;
use std::f32::consts::PI;

use femtovg::{Align, Color, CompositeOperation, ImageId, LineCap, LineJoin, Paint, Path};

use super::draw::{self, circle, clamp01, ease_out_back, ease_out_cubic, fill_rect, lerp, linear, radial, rect, round_rect};
use super::scene::*;
use super::text::{self, Fonts};
use super::text_cache::TextCache;
use super::{Canvas, upload};
use crate::color::{Rgb, black, white};
use crate::game::board::{SIZE, idx};
use crate::game::shapes::Shape;
use crate::layout::Layout;
use crate::render::{board as board_art, sprites};
use crate::styles::{ParticleShape, Style};

pub const CLEAR_DURATION: f64 = 420.0;
pub const PLACE_DURATION: f64 = 260.0;
pub const DEAL_DURATION: f64 = 320.0;
pub const GAME_OVER_FILL: f64 = 900.0;
pub const BEAM_DURATION: f64 = 520.0;
pub const STYLE_WAVE_DURATION: f64 = 1100.0;
pub const STREAK_DURATION: f64 = 380.0;
pub const FLASH_DURATION: f64 = 280.0;
pub const PULSE_DURATION: f64 = 750.0;
/// How long each block bobs, and how much later each cell of distance starts.
const RIPPLE_BOB: f64 = 240.0;
const RIPPLE_STEP: f64 = 45.0;
const RIPPLE_REACH: f32 = 4.0;
/// A row of blocks hops as it turns grey at game over.
const GAME_OVER_HOP: f64 = 260.0;
/// The shine sweeping over a lucky tray once its pieces have slid in.
pub const LUCKY_SHINE_DELAY: f64 = DEAL_DURATION + 80.0;
const LUCKY_SHINE_DURATION: f64 = 650.0;
const LUCKY_SHINE_STAGGER: f64 = 110.0;

/// Top-left corner and block size of a piece resting in a tray slot.
pub fn tray_piece_position(layout: &Layout, slot: usize, shape: &Shape) -> (f32, f32, f32) {
    layout.tray_piece_position(slot, shape.rows, shape.cols)
}

pub struct Renderer {
    pub fonts: Fonts,
    /// Blocks of every size are drawn from one sprite per colour and look at board-cell size.
    sprites: HashMap<(u8, u8), ImageId>,
    sprite_key: (String, u32),
    board: Option<(String, ImageId)>,
    sprite_size: f32,
    dpr: f32,
    style: Style,
    /// Floating texts and banners, rendered once each.
    texts: TextCache,
}

impl Renderer {
    pub fn new(c: &mut Canvas, style: Style) -> Self {
        Renderer {
            fonts: Fonts::load(c),
            sprites: HashMap::new(),
            sprite_key: (String::new(), 0),
            board: None,
            sprite_size: 1.0,
            dpr: 1.0,
            style,
            texts: TextCache::default(),
        }
    }

    /// Number of cached block sprites (for leak tests).
    pub fn sprite_count(&self) -> usize {
        self.sprites.len()
    }

    /// Frees every cached image (style or size changed).
    pub fn clear_caches(&mut self, c: &mut Canvas) {
        for (_, id) in self.sprites.drain() {
            c.delete_image(id);
        }
        if let Some((_, id)) = self.board.take() {
            c.delete_image(id);
        }
        self.texts.clear(c);
    }

    /// True while any time-based effect still needs frames.
    pub fn is_animating(scene: &Scene) -> bool {
        let t = scene.now;
        scene.flying.is_some()
            || scene.placed.is_some()
            || scene.ripple.is_some()
            || scene.clearing.is_some()
            || !scene.particles.is_empty()
            || !scene.rings.is_empty()
            || !scene.beams.is_empty()
            || !scene.streaks.is_empty()
            || !scene.texts.is_empty()
            || scene.style_wave_at.is_some()
            || scene.shake.is_some()
            || scene.flash.is_some()
            || scene.pulse.is_some()
            || t - scene.tray_dealt_at < DEAL_DURATION + 200.0
            || (scene.lucky && t - scene.tray_dealt_at < LUCKY_SHINE_DELAY + LUCKY_SHINE_DURATION + LUCKY_SHINE_STAGGER * 2.0 + 50.0)
            || scene.game_over_at.is_some_and(|at| t - at < GAME_OVER_FILL + GAME_OVER_HOP + 100.0)
    }

    /// Removes effects that have run their course.
    pub fn expire(scene: &mut Scene) {
        let t = scene.now;
        if scene.placed.as_ref().is_some_and(|p| t - p.start > PLACE_DURATION) {
            scene.placed = None;
        }
        if scene.ripple.as_ref().is_some_and(|r| t - r.start > RIPPLE_BOB + RIPPLE_STEP * RIPPLE_REACH as f64) {
            scene.ripple = None;
        }
        if scene.flash.as_ref().is_some_and(|f| t - f.start > FLASH_DURATION) {
            scene.flash = None;
        }
        if scene.pulse.as_ref().is_some_and(|p| t - p.start > PULSE_DURATION) {
            scene.pulse = None;
        }
        if let Some(cl) = &scene.clearing {
            let max_delay = cl.cells.iter().map(|c| c.delay).fold(0.0, f64::max);
            if t - cl.start > CLEAR_DURATION + max_delay {
                scene.clearing = None;
            }
        }
        if scene.shake.as_ref().is_some_and(|s| t - s.start > 350.0) {
            scene.shake = None;
        }
        if scene.flying.as_ref().is_some_and(|f| t - f.start > f.duration) {
            scene.flying = None;
        }
        if scene.style_wave_at.is_some_and(|at| t - at > STYLE_WAVE_DURATION) {
            scene.style_wave_at = None;
        }
        scene.particles.retain(|p| t - p.born < p.life);
        scene.rings.retain(|r| t - r.start < r.duration);
        scene.beams.retain(|b| t - b.start < BEAM_DURATION);
        scene.streaks.retain(|b| t - b.start < STREAK_DURATION);
        scene.texts.retain(|p| t - p.start < p.duration);
    }

    /// Draws the play area (everything but the page background and the HUD).
    pub fn render(&mut self, c: &mut Canvas, scene: &mut Scene) {
        let layout = scene.layout;
        let now = scene.now;
        self.dpr = scene.dpr;
        self.style = scene.style;
        let px = ((layout.cell * scene.dpr).round() as u32).max(4);
        let key = (scene.style.key(), px);
        if self.sprite_key != key {
            for (_, id) in self.sprites.drain() {
                c.delete_image(id);
            }
            self.sprite_key = key;
        }
        self.sprite_size = layout.cell;

        Self::expire(scene);
        self.texts.begin_frame(c);

        c.save();
        if let Some(shake) = &scene.shake {
            let t = ((now - shake.start) / 350.0) as f32;
            let p = shake.power * (1.0 - t);
            c.translate((t * 45.0).sin() * p, (t * 38.0).cos() * p * 0.6);
        }
        self.draw_board(c, scene);
        self.draw_clear_preview(c, scene);
        self.draw_beams(c, scene);
        self.draw_streaks(c, scene);
        self.draw_style_wave(c, scene);
        c.restore();

        self.draw_flash(c, scene);
        self.draw_tray(c, scene);
        self.draw_rings(c, scene);
        self.draw_particles(c, scene);
        if let Some(f) = &scene.flying {
            self.draw_flying(c, scene, f);
        }
        if let Some(d) = &scene.drag {
            self.draw_drag_halo(c, scene, d);
            self.draw_piece(c, d.piece.shape, d.piece.color, scene.tray_looks[d.slot], d.x, d.y, d.cell, 1.0, true);
        }
        self.draw_game_over_shade(c, scene);
        self.draw_texts(c, scene);
        self.draw_pulse(c, scene);
    }

    fn sprite(&mut self, c: &mut Canvas, color: u8, look: u8) -> Option<ImageId> {
        if let Some(id) = self.sprites.get(&(color, look)) {
            return Some(*id);
        }
        let pm = sprites::block_sprite(self.style.tier.block, self.style.color(color), self.sprite_key.1, look);
        let id = upload(c, &pm)?;
        self.sprites.insert((color, look), id);
        Some(id)
    }

    fn board_image(&mut self, c: &mut Canvas, layout: &Layout, dpr: f32, style: &Style) -> Option<ImageId> {
        let key = format!("{}:{}:{}", layout.board_size, dpr, style.key());
        if let Some((k, id)) = &self.board {
            if *k == key {
                return Some(*id);
            }
            c.delete_image(*id);
            self.board = None;
        }
        let pm = board_art::board_background(style, layout.cell, layout.board_size, dpr);
        let id = upload(c, &pm)?;
        self.board = Some((key, id));
        Some(id)
    }

    fn draw_board(&mut self, c: &mut Canvas, scene: &Scene) {
        let layout = &scene.layout;
        let now = scene.now;
        let cell = layout.cell;
        if let Some(bg) = self.board_image(c, layout, scene.dpr, &scene.style) {
            let pad = board_art::board_pad(cell) + board_art::board_margin(cell);
            let (x, y, s) = (layout.board_x - pad, layout.board_y - pad, layout.board_size + pad * 2.0);
            c.fill_path(&rect(x, y, s, s), &Paint::image(bg, x, y, s, s, 0.0, 1.0));
        }

        let drag = scene.drag.as_ref();
        let target = drag.and_then(|d| d.target.as_ref());
        let mut highlight = [false; SIZE * SIZE];
        let mut ghost = [false; SIZE * SIZE];
        let mut ghost_look = [0u8; SIZE * SIZE];
        if let (Some(d), Some((tr, tc, lines))) = (drag, target) {
            for &r in &lines.rows {
                for col in 0..SIZE {
                    highlight[idx(r, col)] = true;
                }
            }
            for &col in &lines.cols {
                for r in 0..SIZE {
                    highlight[idx(r, col)] = true;
                }
            }
            for (k, &(dr, dc)) in d.piece.shape.cells.iter().enumerate() {
                let i = idx((tr + dr) as usize, (tc + dc) as usize);
                ghost[i] = true;
                ghost_look[i] = piece_look(scene.tray_looks[d.slot], k);
            }
        }

        let gray_rows = scene.game_over_at.map_or(-1, |at| (clamp01(((now - at) / GAME_OVER_FILL) as f32) * (SIZE as f32 + 1.0)).floor() as i32);
        let breath = 0.5 + 0.5 * (now as f32 / 130.0).sin();

        for r in 0..SIZE {
            for col in 0..SIZE {
                let i = idx(r, col);
                let mut color = scene.board[i];
                let x = layout.board_x + col as f32 * cell;
                let y = layout.board_y + r as f32 * cell;
                if let Some(d) = drag
                    && highlight[i]
                    && (color != 0 || ghost[i])
                {
                    // Blocks in lines about to be cleared take the colour of the dragged piece.
                    let look = if ghost[i] { ghost_look[i] } else { scene.looks[i] };
                    self.blit(c, d.piece.color, look, x, y, cell, 1.0, 1.0, 1.0);
                    continue;
                }
                if color == 0 {
                    // The ghost of the dragged piece breathes softly.
                    if let Some(d) = drag
                        && ghost[i]
                    {
                        self.blit(c, d.piece.color, ghost_look[i], x, y, cell, 0.32 + 0.14 * breath, 1.0, 1.0);
                    }
                    continue;
                }
                let (mut sx, mut sy, mut dy, mut impact) = (1.0f32, 1.0f32, 0.0f32, 0.0f32);
                if gray_rows > (SIZE - 1 - r) as i32 {
                    color = 0;
                    // Each row hops as it turns grey, so the game over runs up the board in a wave.
                    let at = scene.game_over_at.unwrap_or(now);
                    let hop = ((now - at - ((SIZE - r) as f64 / (SIZE as f64 + 1.0)) * GAME_OVER_FILL) / GAME_OVER_HOP) as f32;
                    if hop > 0.0 && hop < 1.0 {
                        let h = (hop * PI).sin();
                        dy = -cell * 0.16 * h;
                        sx = 1.0 - 0.06 * h;
                        sy = 1.0 + 0.06 * h;
                    }
                }
                if let Some(placed) = scene.placed.as_ref().filter(|p| p.cells.contains(&i)) {
                    // Lands squashed, springs up a little tall, then settles on its bottom edge.
                    let t = clamp01(((now - placed.start) / PLACE_DURATION) as f32);
                    let k = (t * PI * 2.5).cos() * (1.0 - t) * (1.0 - t);
                    let grow = 0.9 + 0.1 * ease_out_cubic((t * 3.0).min(1.0));
                    sx = grow * (1.0 + 0.16 * k);
                    sy = grow * (1.0 - 0.16 * k);
                    dy = cell * (1.0 - sy) / 2.0;
                    impact = 1.0 - t / 0.35;
                } else if let Some(ripple) = &scene.ripple {
                    // Neighbours dip as the wave from the placed piece passes them.
                    let d = (r as f32 + 0.5 - ripple.r).hypot(col as f32 + 0.5 - ripple.c);
                    let t = ((now - ripple.start - d as f64 * RIPPLE_STEP) / RIPPLE_BOB) as f32;
                    if d < RIPPLE_REACH && t > 0.0 && t < 1.0 {
                        sx = 1.0 - 0.1 * (1.0 - d / RIPPLE_REACH) * (t * PI).sin();
                        sy = sx;
                    }
                }
                self.blit(c, color, scene.looks[i], x, y + dy, cell, 1.0, sx, sy);
                if impact > 0.0 {
                    draw::fill_color(c, &round_rect(x + cell * 0.1, y + dy + cell * 0.1, cell * 0.8, cell * 0.8, cell * 0.14), white(0.5 * impact));
                }
            }
        }

        if let Some(cl) = &scene.clearing {
            let flash = scene.style.tier.fx.flash;
            for cc in &cl.cells {
                let t = clamp01(((now - cl.start - cc.delay) / CLEAR_DURATION) as f32);
                if t >= 1.0 {
                    continue;
                }
                let x = layout.board_x + cc.c as f32 * cell;
                let y = layout.board_y + cc.r as f32 * cell;
                let scale = if t < 0.25 { 1.0 + t * 0.6 } else { 1.15 * (1.0 - ease_out_cubic((t - 0.25) / 0.75)) };
                self.blit(c, cc.color, cc.look, x, y, cell, 1.0 - t * 0.3, scale, scale);
                if t < 0.3 {
                    fill_rect(c, x + cell * 0.08, y + cell * 0.08, cell * 0.84, cell * 0.84, flash.alpha(0.7 * (1.0 - t / 0.3)));
                }
            }
        }
    }

    /// Draws one block. Animated pieces change size every frame; caching a sprite per size
    /// would create hundreds of images, so they all scale the board-sized one.
    #[allow(clippy::too_many_arguments)]
    fn blit(&mut self, c: &mut Canvas, color: u8, look: u8, x: f32, y: f32, size: f32, alpha: f32, scale: f32, scale_y: f32) {
        let Some(id) = self.sprite(c, color, look) else { return };
        let w = size * scale;
        let h = size * scale_y;
        if w <= 0.0 || h <= 0.0 || alpha <= 0.0 {
            return;
        }
        let (bx, by) = (x + (size - w) / 2.0, y + (size - h) / 2.0);
        c.fill_path(&rect(bx, by, w, h), &Paint::image(id, bx, by, w, h, 0.0, alpha));
    }

    #[allow(clippy::too_many_arguments)]
    /// Draws a piece whose block looks were seeded with `looks` (see [`piece_look`]).
    pub fn draw_piece(&mut self, c: &mut Canvas, shape: &Shape, color: u8, looks: u32, x: f32, y: f32, cell: f32, alpha: f32, shadow: bool) {
        if shadow {
            let mut p = Path::new();
            for &(r, col) in &shape.cells {
                p.rect(x + col as f32 * cell + cell * 0.12, y + r as f32 * cell + cell * 0.2, cell * 0.9, cell * 0.9);
            }
            c.fill_path(&p, &Paint::color(black(0.18)).with_anti_alias(false));
        }
        for (k, &(r, col)) in shape.cells.iter().enumerate() {
            self.blit(c, color, piece_look(looks, k), x + col as f32 * cell, y + r as f32 * cell, cell, alpha, 1.0, 1.0);
        }
    }

    fn draw_tray(&mut self, c: &mut Canvas, scene: &Scene) {
        let layout = &scene.layout;
        let now = scene.now;
        let mut shown = Vec::new();
        for (i, piece) in scene.tray.iter().enumerate() {
            let Some(piece) = piece else { continue };
            if scene.hidden_slot == Some(i) || scene.flying.as_ref().is_some_and(|f| f.slot == i) {
                continue;
            }
            let t = clamp01(((now - scene.tray_dealt_at - i as f64 * 60.0) / DEAL_DURATION) as f32);
            let offset = (1.0 - ease_out_cubic(t)) * layout.width * 0.6;
            let alpha = (if scene.playable[i] { 1.0 } else { 0.35 }) * clamp01(t * 2.0);
            if alpha > 0.0 {
                shown.push((*piece, i, offset, alpha));
            }
        }
        // Halos go first so a neighbour's halo never covers a piece.
        if scene.lucky {
            for &(_, i, offset, alpha) in &shown {
                self.draw_lucky_glow(c, scene, i, offset, alpha);
            }
        }
        for &(piece, i, offset, alpha) in &shown {
            let (x, y, cell) = tray_piece_position(layout, i, piece.shape);
            self.draw_piece(c, piece.shape, if scene.playable[i] { piece.color } else { 0 }, scene.tray_looks[i], x + offset, y, cell, alpha, false);
            if scene.lucky && scene.playable[i] {
                self.draw_lucky_shine(c, scene, piece.shape, i, x + offset, y, cell);
            }
        }
    }

    /// A shine sweeping diagonally over each piece of a lucky tray once it has slid in.
    #[allow(clippy::too_many_arguments)]
    fn draw_lucky_shine(&mut self, c: &mut Canvas, scene: &Scene, shape: &Shape, slot: usize, x: f32, y: f32, cell: f32) {
        let t = ((scene.now - scene.tray_dealt_at - LUCKY_SHINE_DELAY - slot as f64 * LUCKY_SHINE_STAGGER) / LUCKY_SHINE_DURATION) as f32;
        if t <= 0.0 || t >= 1.0 {
            return;
        }
        let w = shape.cols as f32 * cell;
        let h = shape.rows as f32 * cell;
        // Filling just the blocks with the gradient clips it to them; it slides along the
        // diagonal and stays clear beyond its ends.
        let mut p = Path::new();
        for &(r, col) in &shape.cells {
            p.rect(x + col as f32 * cell + cell * 0.06, y + r as f32 * cell + cell * 0.06, cell * 0.88, cell * 0.88);
        }
        let band = 0.22;
        let mid = -band + ease_out_cubic(t) * (1.0 + band * 2.0);
        let grad = linear(
            x + w * (mid - band),
            y + h * (mid - band),
            x + w * (mid + band),
            y + h * (mid + band),
            &[(0.0, white(0.0)), (0.5, white(0.8)), (1.0, white(0.0))],
        );
        draw::lighter(c, true, |c| c.fill_path(&p, &grad));
    }

    /// Soft halo behind a piece of a lucky tray, so help from the dealer is visible.
    fn draw_lucky_glow(&mut self, c: &mut Canvas, scene: &Scene, slot: usize, offset: f32, alpha: f32) {
        let s = scene.layout.slots[slot];
        let radius = scene.layout.tray_cell * 3.0;
        let glow = scene.style.palette.glow;
        let paint = radial(s.cx + offset, s.cy, 0.0, radius, &[(0.0, glow.alpha(0.42 * alpha)), (1.0, glow.alpha(0.0))]);
        c.fill_path(&rect(s.cx + offset - radius, s.cy - radius, radius * 2.0, radius * 2.0), &paint);
    }

    fn draw_flying(&mut self, c: &mut Canvas, scene: &Scene, f: &FlyingPiece) {
        let (tx, ty, tc) = tray_piece_position(&scene.layout, f.slot, f.piece.shape);
        let t = ease_out_cubic(clamp01(((scene.now - f.start) / f.duration) as f32));
        self.draw_piece(c, f.piece.shape, f.piece.color, scene.tray_looks[f.slot], lerp(f.from.0, tx, t), lerp(f.from.1, ty, t), lerp(f.from.2, tc, t), 1.0, false);
    }

    fn draw_particles(&mut self, c: &mut Canvas, scene: &Scene) {
        let glow = scene.style.tier.fx.glow;
        // Block debris, confetti and dust are solid; sparks and motes add light.
        let mut additive = false;
        let mut set_blend = |c: &mut Canvas, on: bool| {
            if on != additive {
                additive = on;
                c.global_composite_operation(if on { CompositeOperation::Lighter } else { CompositeOperation::SourceOver });
            }
        };
        for p in &scene.particles {
            if scene.now < p.born {
                continue;
            }
            let age = ((scene.now - p.born) / 1000.0) as f32;
            let k = 1.0 - ((scene.now - p.born) / p.life) as f32;
            let x = p.x + p.vx * age;
            let y = p.y + p.vy * age + p.gravity * age * age;
            if p.kind != ParticleKind::Plain {
                self.draw_special_particle(c, p, x, y, age, k.max(0.0), &mut set_blend);
                continue;
            }
            set_blend(c, glow);
            let s = p.size * (0.4 + 0.6 * k);
            let color = p.color.alpha(k.max(0.0));
            match p.shape {
                ParticleShape::Square => fill_rect(c, x - s / 2.0, y - s / 2.0, s, s, color),
                ParticleShape::Circle => draw::fill_color(c, &circle(x, y, s / 2.0), color),
                ParticleShape::Spark => {
                    // A streak pointing along the direction of travel.
                    let vx = p.vx;
                    let vy = p.vy + 2.0 * p.gravity * age;
                    let len = vx.hypot(vy).max(1e-3);
                    let tail = s * 2.2;
                    let mut path = Path::new();
                    path.move_to(x, y);
                    path.line_to(x - vx / len * tail, y - vy / len * tail);
                    let mut paint = Paint::color(color);
                    paint.set_line_width((s * 0.35).max(1.0));
                    paint.set_line_cap(LineCap::Round);
                    c.stroke_path(&path, &paint);
                }
                ParticleShape::Diamond | ParticleShape::Star => {
                    c.save();
                    c.translate(x, y);
                    c.rotate(p.spin * age);
                    let mut path = Path::new();
                    if p.shape == ParticleShape::Diamond {
                        path.move_to(0.0, -s * 0.7);
                        path.line_to(s * 0.45, 0.0);
                        path.line_to(0.0, s * 0.7);
                        path.line_to(-s * 0.45, 0.0);
                    } else {
                        // Five-pointed star.
                        for i in 0..10 {
                            let r = if i % 2 == 1 { s * 0.28 } else { s * 0.7 };
                            let a = i as f32 * PI / 5.0 - PI / 2.0;
                            if i == 0 {
                                path.move_to(a.cos() * r, a.sin() * r);
                            } else {
                                path.line_to(a.cos() * r, a.sin() * r);
                            }
                        }
                    }
                    path.close();
                    c.fill_path(&path, &Paint::color(color));
                    c.restore();
                }
            }
        }
        set_blend(c, false);
    }

    /// Draws a particle with a kind; `k` runs from 1 at birth to 0 at the end of its life.
    #[allow(clippy::too_many_arguments)]
    fn draw_special_particle(&mut self, c: &mut Canvas, p: &Particle, x: f32, y: f32, age: f32, k: f32, set_blend: &mut impl FnMut(&mut Canvas, bool)) {
        match p.kind {
            ParticleKind::Dust => {
                // A soft puff that swells and thins out: a faint outer cloud around a denser core.
                set_blend(c, false);
                let r = p.size * (1.8 - k) / 2.0;
                let color = p.color.alpha(0.22 * k);
                draw::fill_color(c, &circle(x, y, r), color);
                draw::fill_color(c, &circle(x, y, r * 0.55), color);
            }
            ParticleKind::Block(block, look) => {
                // A chunk of a cleared block, tumbling away.
                set_blend(c, false);
                let s = p.size * (0.5 + 0.5 * k);
                let Some(id) = self.sprite(c, block, look) else { return };
                c.save();
                c.translate(x, y);
                c.rotate(p.spin * age);
                c.fill_path(&rect(-s / 2.0, -s / 2.0, s, s), &Paint::image(id, -s / 2.0, -s / 2.0, s, s, 0.0, (k * 2.5).min(1.0)));
                c.restore();
            }
            ParticleKind::Confetti => {
                // A paper strip swaying as it falls and flipping over, so its width flickers.
                set_blend(c, false);
                let sx = x + (age * 3.0 + p.spin).sin() * p.size * 1.6;
                let flip = (age * (5.0 + p.spin.abs())).cos();
                c.save();
                c.translate(sx, y);
                c.rotate(p.spin * 0.35 + age * p.spin * 0.4);
                c.scale(1.0, if flip.abs() < 0.01 { 0.01 } else { flip });
                fill_rect(c, -p.size / 2.0, -p.size * 0.3, p.size, p.size * 0.6, p.color.alpha((k * 4.0).min(1.0)));
                c.restore();
            }
            ParticleKind::Mote => {
                // A soft glint drifting upwards, twinkling as it goes.
                set_blend(c, true);
                let twinkle = 0.65 + 0.35 * (age * 9.0 + p.spin * 3.0).sin();
                let s = p.size * (0.6 + 0.4 * twinkle);
                let mx = x + (age * 2.2 + p.spin).sin() * self.sprite_size * 0.25;
                let life = (k * PI).sin();
                draw::fill_color(c, &circle(mx, y, s), p.color.alpha(life * twinkle * 0.9));
                draw::fill_color(c, &circle(mx, y, s * 2.6), p.color.alpha(life * twinkle * 0.25));
            }
            ParticleKind::Plain => {}
        }
    }

    fn draw_rings(&mut self, c: &mut Canvas, scene: &Scene) {
        if scene.rings.is_empty() {
            return;
        }
        draw::lighter(c, scene.style.tier.fx.glow, |c| {
            for ring in &scene.rings {
                let t = clamp01(((scene.now - ring.start) / ring.duration) as f32);
                if t <= 0.0 {
                    continue;
                }
                draw::stroke(c, &circle(ring.x, ring.y, ring.radius * ease_out_cubic(t)), ring.color.alpha(0.85 * (1.0 - t)), (scene.layout.cell * 0.22 * (1.0 - t)).max(1.0));
            }
        });
    }

    fn draw_beams(&mut self, c: &mut Canvas, scene: &Scene) {
        if scene.beams.is_empty() {
            return;
        }
        let l = &scene.layout;
        let (cell, bx, by, bs) = (l.cell, l.board_x, l.board_y, l.board_size);
        draw::lighter(c, true, |c| {
            for b in &scene.beams {
                let t = clamp01(((scene.now - b.start) / BEAM_DURATION) as f32);
                if t <= 0.0 {
                    continue;
                }
                // The beam flares open and then thins out.
                let width = cell * (if t < 0.2 { t / 0.2 } else { 1.0 - ease_out_cubic((t - 0.2) / 0.8) }) * 1.1;
                if width <= 0.01 {
                    continue;
                }
                let alpha = 0.9 * (1.0 - t);
                let stops = [(0.0, b.color.alpha(0.0)), (0.5, b.color.alpha(alpha)), (1.0, b.color.alpha(0.0))];
                let over = cell * 0.6 * (1.0 - t);
                let core = (width * 0.12).max(1.0);
                match b.line {
                    Line::Row(row) => {
                        let mid = by + (row as f32 + 0.5) * cell;
                        c.fill_path(&rect(bx - over, mid - width, bs + over * 2.0, width * 2.0), &linear(0.0, mid - width, 0.0, mid + width, &stops));
                        // Bright core.
                        fill_rect(c, bx, mid - core, bs, core * 2.0, white(alpha * 0.8));
                    }
                    Line::Col(col) => {
                        let mid = bx + (col as f32 + 0.5) * cell;
                        c.fill_path(&rect(mid - width, by - over, width * 2.0, bs + over * 2.0), &linear(mid - width, 0.0, mid + width, 0.0, &stops));
                        fill_rect(c, mid - core, by, core * 2.0, bs, white(alpha * 0.8));
                    }
                }
            }
        });
    }

    /// A diagonal shimmer across the board, revealing the new style.
    fn draw_style_wave(&mut self, c: &mut Canvas, scene: &Scene) {
        let Some(at) = scene.style_wave_at else { return };
        let t = ((scene.now - at) / STYLE_WAVE_DURATION) as f32;
        if !(0.0..=1.0).contains(&t) {
            return;
        }
        let l = &scene.layout;
        let cell = l.cell;
        let glow = scene.style.palette.glow;
        draw::lighter(c, true, |c| {
            for r in 0..SIZE {
                for col in 0..SIZE {
                    // Each cell lights up as the wave front passes it.
                    let local = (t * 1.6 - (r + col) as f32 / (2 * SIZE - 2) as f32 * 0.9) / 0.7;
                    if local <= 0.0 || local >= 1.0 {
                        continue;
                    }
                    let a = (local * PI).sin();
                    let inset = cell * (0.5 - 0.42 * a);
                    let color = if local < 0.5 { Rgb::WHITE } else { glow };
                    draw::fill_color(
                        c,
                        &round_rect(l.board_x + col as f32 * cell + inset, l.board_y + r as f32 * cell + inset, cell - inset * 2.0, cell - inset * 2.0, cell * 0.12),
                        color.alpha(0.55 * a),
                    );
                }
            }
        });
    }

    /// Rows and columns the dragged piece would clear pulse with light, framed in the glow colour.
    fn draw_clear_preview(&mut self, c: &mut Canvas, scene: &Scene) {
        let Some((_, _, lines)) = scene.drag.as_ref().and_then(|d| d.target.as_ref()) else { return };
        if lines.count() == 0 {
            return;
        }
        let l = &scene.layout;
        let (cell, bx, by, bs) = (l.cell, l.board_x, l.board_y, l.board_size);
        let pulse = 0.5 + 0.5 * (scene.now as f32 / 110.0).sin();
        let rects: Vec<[f32; 4]> = lines
            .rows
            .iter()
            .map(|&r| [bx, by + r as f32 * cell, bs, cell])
            .chain(lines.cols.iter().map(|&col| [bx + col as f32 * cell, by, cell, bs]))
            .collect();
        draw::lighter(c, true, |c| {
            for [x, y, w, h] in &rects {
                fill_rect(c, *x, *y, *w, *h, white(0.06 + 0.12 * pulse));
            }
        });
        let color = scene.style.palette.glow.alpha(0.45 + 0.4 * pulse);
        let width = (cell * (0.05 + 0.03 * pulse)).max(1.5);
        for [x, y, w, h] in &rects {
            draw::stroke(c, &round_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, cell * 0.16), color, width);
        }
    }

    /// Bright streaks racing from where the piece landed to both ends of each cleared line.
    fn draw_streaks(&mut self, c: &mut Canvas, scene: &Scene) {
        if scene.streaks.is_empty() {
            return;
        }
        let l = &scene.layout;
        let cell = l.cell;
        draw::lighter(c, true, |c| {
            for st in &scene.streaks {
                let t = clamp01(((scene.now - st.start) / STREAK_DURATION) as f32);
                if t <= 0.0 {
                    continue;
                }
                let (horizontal, across, origin) = match st.line {
                    Line::Row(row) => (true, l.board_y + (row as f32 + 0.5) * cell, l.board_x),
                    Line::Col(col) => (false, l.board_x + (col as f32 + 0.5) * cell, l.board_y),
                };
                let width = cell * 0.26 * (1.0 - t * 0.6);
                let length = cell * (1.0 + 2.5 * t);
                for dir in [-1.0f32, 1.0] {
                    // Both halves reach just past their end of the line at the same moment.
                    let reach = (if dir > 0.0 { SIZE as f32 - st.from } else { st.from }) + 0.8;
                    let head = origin + (st.from + dir * ease_out_cubic(t) * reach) * cell;
                    let tail = head - dir * length;
                    let stops = [(0.0, st.color.alpha(0.0)), (0.7, st.color.alpha(0.7 * (1.0 - t))), (1.0, white(1.0 - t))];
                    let a = head.min(tail);
                    if horizontal {
                        c.fill_path(&rect(a, across - width / 2.0, length, width), &linear(tail, 0.0, head, 0.0, &stops));
                    } else {
                        c.fill_path(&rect(across - width / 2.0, a, width, length), &linear(0.0, tail, 0.0, head, &stops));
                    }
                }
            }
        });
    }

    /// A burst of light from the cleared lines; big clears also tint the whole screen for a moment.
    fn draw_flash(&mut self, c: &mut Canvas, scene: &Scene) {
        let Some(f) = &scene.flash else { return };
        let t = clamp01(((scene.now - f.start) / FLASH_DURATION) as f32);
        if t <= 0.0 {
            return;
        }
        let l = &scene.layout;
        let fade = (1.0 - t) * (1.0 - t);
        draw::lighter(c, true, |c| {
            if f.power > 0.5 && !scene.reduced_motion {
                fill_rect(c, 0.0, 0.0, l.width, l.height, f.color.alpha(0.18 * (f.power - 0.4) * fade));
            }
            let radius = l.cell * (2.5 + 4.0 * f.power) * (0.5 + 0.5 * ease_out_cubic(t));
            let grad = radial(f.x, f.y, 0.0, radius, &[(0.0, white(0.6 * f.power * fade)), (0.35, f.color.alpha(0.45 * f.power * fade)), (1.0, f.color.alpha(0.0))]);
            c.fill_path(&rect(f.x - radius, f.y - radius, radius * 2.0, radius * 2.0), &grad);
        });
    }

    /// Soft glow under the dragged piece, brighter and pulsing when the drop would clear lines.
    fn draw_drag_halo(&mut self, c: &mut Canvas, scene: &Scene, d: &DragView) {
        let w = d.piece.shape.cols as f32 * d.cell;
        let h = d.piece.shape.rows as f32 * d.cell;
        let (cx, cy) = (d.x + w / 2.0, d.y + h / 2.0);
        let lines = d.target.as_ref().map_or(0, |t| t.2.count());
        let strength = if lines > 0 { 0.4 + 0.2 * (scene.now as f32 / 110.0).sin() } else { 0.22 };
        let radius = w.max(h) * 0.6 + d.cell * 0.9;
        let glow = scene.style.palette.glow;
        let grad = radial(cx, cy, 0.0, radius, &[(0.0, glow.alpha(strength)), (1.0, glow.alpha(0.0))]);
        draw::lighter(c, scene.style.tier.fx.glow, |c| c.fill_path(&rect(cx - radius, cy - radius, radius * 2.0, radius * 2.0), &grad));
    }

    /// Light pulsing in from the screen edges on combos and board clears.
    fn draw_pulse(&mut self, c: &mut Canvas, scene: &Scene) {
        let Some(p) = &scene.pulse else { return };
        let t = clamp01(((scene.now - p.start) / PULSE_DURATION) as f32);
        if t <= 0.0 {
            return;
        }
        let (width, height) = (scene.layout.width, scene.layout.height);
        let envelope = if t < 0.18 { t / 0.18 } else { 1.0 - ease_out_cubic((t - 0.18) / 0.82) };
        let alpha = p.power * envelope * if scene.reduced_motion { 0.35 } else { 0.7 };
        let depth = width.min(height) * (0.08 + 0.08 * p.power);
        let edge = |x0: f32, y0: f32, x1: f32, y1: f32| linear(x0, y0, x1, y1, &[(0.0, p.color.alpha(alpha)), (1.0, p.color.alpha(0.0))]);
        draw::lighter(c, scene.style.tier.fx.glow, |c| {
            c.fill_path(&rect(0.0, 0.0, width, depth), &edge(0.0, 0.0, 0.0, depth));
            c.fill_path(&rect(0.0, height - depth, width, depth), &edge(0.0, height, 0.0, height - depth));
            c.fill_path(&rect(0.0, 0.0, depth, height), &edge(0.0, 0.0, depth, 0.0));
            c.fill_path(&rect(width - depth, 0.0, depth, height), &edge(width, 0.0, width - depth, 0.0));
        });
    }

    /// The screen dims towards the edges once no piece fits any more.
    fn draw_game_over_shade(&mut self, c: &mut Canvas, scene: &Scene) {
        let Some(at) = scene.game_over_at else { return };
        let t = clamp01(((scene.now - at) / GAME_OVER_FILL) as f32);
        if t <= 0.0 {
            return;
        }
        let l = &scene.layout;
        let (cx, cy) = (l.board_x + l.board_size / 2.0, l.board_y + l.board_size / 2.0);
        let grad = radial(cx, cy, l.board_size * 0.45, l.width.hypot(l.height) * 0.6, &[(0.0, black(0.0)), (1.0, black(0.45 * ease_out_cubic(t)))]);
        c.fill_path(&rect(0.0, 0.0, l.width, l.height), &grad);
    }

    fn draw_texts(&mut self, c: &mut Canvas, scene: &Scene) {
        let tier = scene.style.tier;
        let palette = scene.style.palette;
        let fonts = self.fonts.display(tier.font);
        for ft in &scene.texts {
            if scene.now < ft.start {
                continue;
            }
            let t = clamp01(((scene.now - ft.start) / ft.duration) as f32);
            let (mut scale, dy, alpha);
            let mut tilt = 0.0;
            if ft.banner {
                scale = if t < 0.2 { ease_out_back(t / 0.2) } else { 1.0 };
                alpha = if t > 0.75 { 1.0 - (t - 0.75) / 0.25 } else { 1.0 };
                dy = -t * 20.0;
            } else {
                // Small popups punch in, then float up.
                scale = if t < 0.12 { 0.4 + 0.6 * ease_out_back(t / 0.12) } else { 1.0 };
                dy = -ease_out_cubic(t) * 50.0;
                alpha = if t > 0.6 { 1.0 - (t - 0.6) / 0.4 } else { 1.0 };
            }
            if ft.bounce && t >= 0.2 {
                // Combo counters keep wobbling, dying down as they fade.
                let w = ((t - 0.2) * PI * 7.0).sin() * (1.0 - t);
                scale *= 1.0 + 0.12 * w;
                tilt = 0.06 * w;
            }
            // Scale the text down if it would not fit the screen in this style's font.
            let fill = text::paint(&fonts, ft.size, Align::Center, ft.color.fv());
            let text_width = text::width(c, &ft.text, &fill).max(1.0);
            let fit = (scene.layout.width * 0.92 / text_width).min(1.0);
            // Popups near the screen edges move in so they stay readable.
            let half = text_width * fit / 2.0 + ft.size * 0.15;
            let x = if ft.banner { ft.x } else { ft.x.min(scene.layout.width - half).max(half) };
            c.save();
            c.translate(x, ft.y + dy);
            if tilt != 0.0 {
                c.rotate(tilt);
            }
            c.scale(scale.max(0.01) * fit, scale.max(0.01) * fit);
            let glow = tier.text_glow;
            let gradient = tier.text_gradient && ft.banner;
            let key = format!("{}|{}|{}|{}|{}", ft.text, scene.style.key(), ft.size, ft.color.to_hex(), gradient);
            let size = ft.size;
            let label = ft.text.as_str();
            let color = ft.color;
            let mut outline = fill.clone();
            outline.set_color(palette.board_edge.alpha(0.9));
            outline.set_line_width((size * 0.14).max(3.0));
            outline.set_line_join(LineJoin::Round);
            let (w, h) = (text_width + size * 0.5, size * 2.0);
            if glow {
                // The glow is soft, so a low-resolution texture does; it is blurred by the
                // original's device pixels (shadowBlur ignores the transform), in logical pixels here.
                let blur = size * 0.45 / scene.dpr;
                let pad = blur * 2.0;
                let outline = outline.clone();
                self.texts.draw(c, &format!("glow|{key}"), w + pad * 2.0, h + pad * 2.0, 1.0, alpha.max(0.0), |c| {
                    c.set_shadow_color(palette.glow.fv());
                    c.set_shadow_blur(blur);
                    text::stroke(c, 0.0, 0.0, label, &outline);
                    c.set_shadow_color(Color::rgbaf(0.0, 0.0, 0.0, 0.0));
                    c.set_shadow_blur(0.0);
                });
            }
            // Crisp outline and fill, a little larger than they ever appear.
            if gradient {
                // The gradient fill needs a texture of its own (see text::fill_gradient).
                self.texts.draw(c, &format!("outline|{key}"), w, h, scene.dpr * 1.25, alpha.max(0.0), |c| text::stroke(c, 0.0, 0.0, label, &outline));
                let grad = linear(0.0, -size / 2.0, 0.0, size / 2.0, &[(0.0, Rgb::hex("#fffbe6").fv()), (0.45, color.fv()), (0.55, color.fv()), (1.0, Rgb::hex("#b07a10").fv())]);
                self.texts.draw(c, &key, w, h, scene.dpr * 1.25, alpha.max(0.0), |c| text::fill_gradient(c, label, &fill, &grad, w, h));
            } else {
                self.texts.draw(c, &key, w, h, scene.dpr * 1.25, alpha.max(0.0), |c| {
                    text::stroke(c, 0.0, 0.0, label, &outline);
                    text::fill(c, 0.0, 0.0, label, &fill);
                });
            }
            c.restore();
        }
    }
}
