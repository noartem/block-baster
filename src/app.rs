//! The game app: input, moves, effects, styles, menus and saving. Platform
//! shells feed it events and a canvas; it never touches a window itself.

use std::collections::HashSet;
use std::f32::consts::PI;

use crate::audio::intensity::{ActionMeter, score_progress};
use crate::audio::{Audio, Sfx};
use crate::color::Rgb;
use crate::game::board::{SIZE, idx, preview_lines};
use crate::game::game::{Game, PlaceResult};
use crate::game::generator::TRAY_SIZE;
use crate::game::rng::Rng;
use crate::game::shapes::Shape;
use crate::input::{TOUCH_GAIN, nearest_fit};
use crate::layout::{Layout, compute_layout_in};
use crate::storage::{self, KeyValue, SettingKey, Settings};
use crate::styles::{GRAY, ParticleShape, Style, format_score, next_tier, style_for};
use crate::view::Canvas;
use crate::view::background::{self, BgColors};
use crate::view::renderer::{CLEAR_DURATION, GAME_OVER_FILL, LUCKY_SHINE_DELAY, Renderer, tray_piece_position};
use crate::view::scene::*;
use crate::view::ui::{GameOverInfo, HudInfo, Insets, Overlay, REPO_URL, Ui, UiContext, UiId, hud_height};

/// Vibration strengths, as in the web version's haptics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strength {
    Light,
    Medium,
    Heavy,
}

/// Services the app needs from the platform it runs on.
pub trait Platform {
    /// Whether the device can vibrate (the vibration slider is hidden otherwise).
    fn can_vibrate(&self) -> bool;
    /// Short vibration feedback; `intensity` 0..1 scales it: 0.5 is the normal strength, 0 is off.
    fn vibrate(&mut self, strength: Strength, intensity: f32);
    /// Inside an installed app (shows "Exit" in the menu and lets Back leave the game over screen).
    fn is_app(&self) -> bool;
    fn exit(&mut self);
    fn open_url(&mut self, url: &str);
    /// Players who ask for less motion get calmer screen-wide effects.
    fn reduced_motion(&self) -> bool {
        false
    }
    /// The style changed (the web shell updates the theme colour and test hooks).
    fn style_changed(&mut self, _style: &Style) {}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerKind {
    Mouse,
    Touch,
}

struct DragState {
    slot: usize,
    pointer: u64,
    /// Last pointer position.
    px: f32,
    py: f32,
    /// Top-left corner of the piece at full size.
    x: f32,
    y: f32,
    /// How far the piece moves per pixel of pointer movement.
    gain: (f32, f32),
    /// Where the piece started (in the tray), for the pick-up animation.
    from: (f32, f32, f32),
    start: f64,
}

/// A pointer held down on the menus.
enum UiPress {
    Button(UiId),
    Slider(SettingKey),
    /// Dragging the card to scroll it; last y.
    Scroll(f32),
    Backdrop,
}

#[derive(Clone, Copy, PartialEq)]
enum GameOverStage {
    /// The board greys out.
    Fill { record: bool },
    /// The game over card appears.
    Card { record: bool },
}

const PICKUP_MS: f64 = 110.0;
/// Hard caps on effect particles, so even a huge clear stays cheap on phones.
const MAX_DEBRIS: usize = 40;
const MAX_PARTICLES: usize = 700;
const PRAISE: [&str; 7] = ["", "", "Good!", "Great!", "Excellent!", "Amazing!", "Unbelievable!"];
/// How long the page background takes to fade to a new style's colours.
const BG_FADE_MS: f64 = 900.0;

pub struct App {
    pub game: Game,
    pub layout: Layout,
    pub best: u32,
    pub settings: Settings,
    pub audio: Audio,
    pub ui: Ui,
    pub scene: Scene,
    pub shown_score: u32,
    pub best_at_start: u32,
    store: Box<dyn KeyValue>,
    platform: Box<dyn Platform>,
    renderer: Option<Renderer>,
    drag: Option<DragState>,
    ui_press: Option<(u64, UiPress)>,
    locked: bool,
    /// A style earned by the last move, applied on the first frame at or after the time.
    pending_style: Option<(Style, f64)>,
    game_over_timer: Option<(f64, GameOverStage)>,
    /// How fast the player is moving, for the music.
    meter: ActionMeter,
    /// Where the dragged piece last left trail sparkles.
    trail: Option<(f32, f32)>,
    /// Score bump: start and strength.
    bump: Option<(f64, f32)>,
    /// Shown progress of the tier bar (eases towards the real one).
    tier_bar: f32,
    bg_from: BgColors,
    bg_at: f64,
    insets: Insets,
    fx: Rng,
    dirty: bool,
    now: f64,
    /// Set when the renderer's caches must be dropped on the next frame.
    flush_caches: bool,
}

fn bg_colors(style: &Style) -> BgColors {
    let p = style.palette;
    BgColors { bg: p.bg, glow: p.glow, glow2: p.glow2, accent: p.accent }
}

impl App {
    /// Creates the app; `seed` starts a reproducible game (and ignores the saved one).
    pub fn new(store: Box<dyn KeyValue>, platform: Box<dyn Platform>, seed: Option<u32>, now: f64) -> Self {
        let settings = storage::load_settings(store.as_ref());
        let mut audio = Audio::new();
        audio.set_sound_volume(settings.sound);
        audio.set_music_volume(settings.music);
        let best = storage::load_best(store.as_ref());
        let game = Self::restore_or_create(store.as_ref(), seed);
        let style = style_for(game.score, game.board_clears);
        let mut app = App {
            best_at_start: if game.moves > 0 { storage::load_best_at_start(store.as_ref()).unwrap_or(best) } else { best },
            shown_score: game.score,
            game,
            layout: Layout::default(),
            best,
            settings,
            audio,
            ui: Ui::new(),
            scene: Scene::new(now),
            store,
            platform,
            renderer: None,
            drag: None,
            ui_press: None,
            locked: false,
            pending_style: None,
            game_over_timer: None,
            meter: ActionMeter::new(),
            trail: None,
            bump: None,
            tier_bar: 0.0,
            bg_from: bg_colors(&style),
            bg_at: f64::NEG_INFINITY,
            insets: Insets::default(),
            fx: Rng::from_time(),
            dirty: true,
            now,
            flush_caches: false,
        };
        storage::save_best_at_start(app.store.as_mut(), app.best_at_start);
        app.scene.reduced_motion = app.platform.reduced_motion();
        app.apply_style(style);
        app.sync_scene();
        app.tier_bar = app.tier_progress();
        app
    }

    fn restore_or_create(store: &dyn KeyValue, seed: Option<u32>) -> Game {
        if seed.is_none()
            && let Some(saved) = storage::load_game(store)
            && !saved.over
            && let Ok(game) = Game::deserialize(&saved)
        {
            return game;
        }
        // Start fresh when there's no save, or it is corrupt or outdated.
        Game::new(seed)
    }

    fn rand(&mut self) -> f32 {
        self.fx.next() as f32
    }

    // -----------------------------------------------------------------------
    // Frame loop
    // -----------------------------------------------------------------------

    /// Requests a redraw.
    pub fn invalidate(&mut self) {
        self.dirty = true;
    }

    pub fn resize(&mut self, width: f32, height: f32, dpr: f32, insets: Insets) {
        let dpr = dpr.clamp(1.0, 3.0);
        self.insets = insets;
        let top = insets.top + hud_height(width, self.scene.style.tier.level);
        let mut layout = compute_layout_in(top, width, height - insets.bottom);
        // Hit areas and drags reach the real screen bottom.
        layout.height = height;
        for s in &mut layout.slots {
            s.h = (height - s.y).max(s.h);
        }
        if dpr != self.scene.dpr || layout.cell != self.scene.layout.cell {
            self.flush_caches = true;
        }
        self.layout = layout;
        self.scene.layout = layout;
        self.scene.dpr = dpr;
        self.dirty = true;
    }

    /// Timers and the soundtrack; call before every frame and whenever `next_wakeup` comes due.
    pub fn tick(&mut self, now: f64) {
        self.now = now;
        if let Some((at, stage)) = self.game_over_timer
            && now >= at
        {
            self.game_over_timer = None;
            match stage {
                GameOverStage::Fill { record } => {
                    self.scene.game_over_at = Some(now);
                    self.game_over_dust(now);
                    self.audio.play(Sfx::GameOver);
                    self.vibrate(Strength::Heavy);
                    self.game_over_timer = Some((now + GAME_OVER_FILL + 500.0, GameOverStage::Card { record }));
                }
                GameOverStage::Card { record } => self.show_game_over(record),
            }
            self.dirty = true;
        }
        let (score, speed) = self.music_input(now);
        self.audio.update(now, score, speed);
    }

    /// Whether the app needs a new frame now.
    pub fn needs_frame(&self) -> bool {
        self.dirty || self.is_animating()
    }

    fn is_animating(&self) -> bool {
        let now = self.now;
        Renderer::is_animating(&self.scene)
            || self.drag.is_some()
            || self.pending_style.is_some()
            || self.shown_score != self.game.score
            || self.bump.is_some()
            || (self.tier_bar - self.tier_progress()).abs() > 0.002
            || now - self.bg_at < BG_FADE_MS
            || self.ui.is_animating(now)
    }

    /// The animated tier backgrounds keep moving even when nothing else does.
    pub fn background_animates(&self) -> bool {
        background::is_animated(self.scene.style.tier.level) && !self.scene.reduced_motion
    }

    /// When the app next needs `tick` (ms on the frame clock), if nothing animates.
    pub fn next_wakeup(&self) -> Option<f64> {
        let mut at: Option<f64> = self.game_over_timer.map(|t| t.0);
        if self.audio.music_playing() {
            // The soundtrack follows the game a few times a second.
            let t = self.now + 50.0;
            at = Some(at.map_or(t, |a| a.min(t)));
        }
        at
    }

    /// Draws a frame of the whole screen.
    pub fn frame(&mut self, c: &mut Canvas, now: f64) {
        self.tick(now);
        self.dirty = false;
        let renderer = match &mut self.renderer {
            Some(r) => r,
            None => self.renderer.insert(Renderer::new(c, self.scene.style)),
        };
        if self.flush_caches {
            renderer.clear_caches(c);
            self.flush_caches = false;
        }
        self.scene.now = now;
        self.scene.drag = self.drag_view(now);
        if let Some(view) = &self.scene.drag {
            let (x, y, cell, piece) = (view.x, view.y, view.cell, view.piece);
            self.drag_trail(x, y, cell, piece.shape, piece.color, now);
        }
        if let Some((style, at)) = self.pending_style
            && now >= at
        {
            self.pending_style = None;
            self.switch_style(style, now);
        }
        self.update_hud(now);

        let (w, h) = (self.layout.width, self.layout.height);
        let fade = ((now - self.bg_at) / BG_FADE_MS).clamp(0.0, 1.0) as f32;
        let colors = self.bg_from.lerp(&bg_colors(&self.scene.style), 0.5 - 0.5 * (fade * PI).cos());
        background::draw(c, w, h, self.scene.style.tier.level, &colors, (now / 1000.0) as f32, self.scene.reduced_motion);

        let bump = self.bump_scale(now);
        let style = self.scene.style;
        let settings = self.settings;
        let fonts = self.renderer.as_ref().expect("renderer").fonts;
        let ctx = UiContext {
            fonts: &fonts,
            style: &style,
            width: w,
            height: h,
            insets: self.insets,
            settings: &settings,
            can_vibrate: self.platform.can_vibrate(),
            native: self.platform.is_app(),
            now,
            dpr: self.scene.dpr,
        };
        let hud = HudInfo {
            best: self.best.max(self.game.score),
            score: self.shown_score,
            bump,
            style: &style,
            progress: self.tier_bar,
            next_label: next_tier(style.tier).map_or("MAX".to_owned(), |t| format_score(t.min_score)),
            now,
        };
        self.ui.draw_hud(c, &ctx, &hud);
        self.renderer.as_mut().expect("renderer").render(c, &mut self.scene);
        self.ui.draw_overlay(c, &ctx);
    }

    fn bump_scale(&mut self, now: f64) -> f32 {
        let Some((start, k)) = self.bump else { return 1.0 };
        let duration = 240.0 + 220.0 * k as f64;
        let t = ((now - start) / duration) as f32;
        if t >= 1.0 {
            self.bump = None;
            return 1.0;
        }
        // Keyframes 1 → peak at 30% → 1, eased out.
        let peak = 1.08 + 0.22 * k;
        let e = |x: f32| 1.0 - (1.0 - x).powi(2);
        if t < 0.3 { 1.0 + (peak - 1.0) * e(t / 0.3) } else { peak + (1.0 - peak) * e((t - 0.3) / 0.7) }
    }

    fn sync_scene(&mut self) {
        self.scene.board = self.game.board;
        self.scene.tray = self.game.tray;
        self.scene.lucky = self.game.lucky;
        for i in 0..TRAY_SIZE {
            self.scene.playable[i] = self.game.is_playable(i);
        }
    }

    fn update_hud(&mut self, _now: f64) {
        let target = self.game.score;
        if self.shown_score != target {
            let diff = target as i64 - self.shown_score as i64;
            self.shown_score = if diff.abs() <= 1 { target } else { (self.shown_score as i64 + ((diff.abs() as f64 * 0.18).ceil() as i64) * diff.signum()) as u32 };
        }
        // The bar eases towards its value (a 0.3 s CSS transition before).
        let goal = self.tier_progress();
        self.tier_bar += (goal - self.tier_bar) * 0.2;
        if (goal - self.tier_bar).abs() < 0.002 {
            self.tier_bar = goal;
        }
    }

    /// Progress towards the next style tier.
    fn tier_progress(&self) -> f32 {
        let tier = self.scene.style.tier;
        match next_tier(tier) {
            Some(next) => ((self.shown_score as f32 - tier.min_score as f32) / (next.min_score - tier.min_score) as f32).clamp(0.0, 1.0),
            None => 1.0,
        }
    }

    /// Skips the HUD's count-up to the current score (for tools).
    pub fn settle_hud(&mut self) {
        self.shown_score = self.game.score;
        self.tier_bar = self.tier_progress();
        self.dirty = true;
    }

    // -----------------------------------------------------------------------
    // Styles
    // -----------------------------------------------------------------------

    /// Switches the board, blocks, page colours, fonts and sounds to a style.
    pub fn apply_style(&mut self, style: Style) {
        let fade = ((self.now - self.bg_at) / BG_FADE_MS).clamp(0.0, 1.0) as f32;
        self.bg_from = self.bg_from.lerp(&bg_colors(&self.scene.style), 0.5 - 0.5 * (fade * PI).cos());
        self.bg_at = self.now;
        let tier_changed = style.tier.level != self.scene.style.tier.level;
        self.scene.style = style;
        self.flush_caches = true;
        self.audio.set_style(style.tier.level, style.variant);
        self.platform.style_changed(&style);
        if tier_changed && self.layout.width > 0.0 {
            // Wider display fonts make the HUD a little shorter.
            let (w, h, dpr, insets) = (self.layout.width, self.layout.height, self.scene.dpr, self.insets);
            self.resize(w, h, dpr, insets);
        }
        self.dirty = true;
    }

    /// Schedules the style earned by the current game to appear after the line-clear animation.
    /// It runs on the animation clock rather than a timer, so it lands on the same frame every time.
    pub fn update_style(&mut self, delay: f64) {
        let style = style_for(self.game.score, self.game.board_clears);
        self.pending_style = if style.same(&self.scene.style) { None } else { Some((style, self.now + delay)) };
        self.dirty = true;
    }

    /// Applies a newly earned style with its reveal: a shimmer, a banner, sounds and, for a new tier, fireworks.
    fn switch_style(&mut self, style: Style, now: f64) {
        let prev = self.scene.style;
        self.apply_style(style);
        let l = self.layout;
        let (cell, width) = (l.cell, l.width);
        let mid = l.board_y + l.board_size / 2.0;
        self.scene.style_wave_at = Some(now);
        if style.tier.level > prev.tier.level {
            let at = now + 700.0;
            let p = style.palette;
            self.scene.texts.push(FloatingText { text: "New style!".into(), x: width / 2.0, y: mid - cell * 0.75, size: cell * 0.6, color: p.glow, start: at, duration: 2200.0, banner: true, bounce: false });
            self.scene.texts.push(FloatingText { text: style.tier.name.into(), x: width / 2.0, y: mid + cell * 0.35, size: cell * 1.25, color: p.accent, start: at + 120.0, duration: 2100.0, banner: true, bounce: false });
            self.audio.play(Sfx::TierUp(style.tier.level));
            self.vibrate(Strength::Heavy);
            self.fireworks(style.tier.fx.fireworks.max(3), now + 600.0);
        } else {
            self.scene.texts.push(FloatingText { text: style.palette.name.into(), x: width / 2.0, y: mid + cell * 2.1, size: cell * 0.6, color: style.palette.glow, start: now + 500.0, duration: 1500.0, banner: true, bounce: false });
            self.audio.play(Sfx::StyleShift);
        }
    }

    // -----------------------------------------------------------------------
    // Pointer input
    // -----------------------------------------------------------------------

    pub fn pointer_down(&mut self, id: u64, kind: PointerKind, x: f32, y: f32) {
        self.unlock_audio();
        if self.ui_press.is_some() {
            return;
        }
        if let Some(hit) = self.ui.hit_at(x, y) {
            let press = match hit {
                UiId::Slider(key) => {
                    self.set_level(key, x);
                    UiPress::Slider(key)
                }
                UiId::Card => UiPress::Scroll(y),
                UiId::Backdrop => UiPress::Backdrop,
                id => {
                    self.ui.pressed = Some(id);
                    UiPress::Button(id)
                }
            };
            self.ui_press = Some((id, press));
            self.dirty = true;
            return;
        }
        if self.ui.overlay != Overlay::None || self.drag.is_some() || self.locked || self.game.over {
            return;
        }
        let Some(slot) = self.layout.slot_at(x, y) else { return };
        let Some(piece) = self.game.tray[slot] else { return };
        let cell = self.layout.cell;
        let w = piece.shape.cols as f32 * cell;
        let h = piece.shape.rows as f32 * cell;
        // Touch: lift the piece above the finger so it stays visible. Mouse: grab the centre.
        let mouse = kind == PointerKind::Mouse;
        self.drag = Some(DragState {
            slot,
            pointer: id,
            px: x,
            py: y,
            x: x - w / 2.0,
            y: if mouse { y - h / 2.0 } else { y - h - cell * 1.1 },
            gain: if mouse { (1.0, 1.0) } else { TOUCH_GAIN },
            from: tray_piece_position(&self.layout, slot, piece.shape),
            start: self.now,
        });
        self.scene.hidden_slot = Some(slot);
        self.scene.flying = None;
        self.audio.play(Sfx::Pickup);
        self.dirty = true;
    }

    pub fn pointer_move(&mut self, id: u64, x: f32, y: f32) {
        if let Some((pid, press)) = &mut self.ui_press {
            if *pid != id {
                return;
            }
            match press {
                UiPress::Slider(key) => {
                    let key = *key;
                    self.set_level(key, x);
                }
                UiPress::Scroll(last) => {
                    let dy = *last - y;
                    *last = y;
                    self.ui.scroll_by(dy);
                }
                UiPress::Button(b) => {
                    // The button stays pressed only while the pointer is over it.
                    let over = self.ui.hit_at(x, y) == Some(*b);
                    self.ui.pressed = if over { Some(*b) } else { None };
                }
                UiPress::Backdrop => {}
            }
            self.dirty = true;
            return;
        }
        let Some(d) = &mut self.drag else { return };
        if d.pointer != id {
            return;
        }
        let Some(piece) = self.game.tray[d.slot] else { return };
        let w = piece.shape.cols as f32 * self.layout.cell;
        let h = piece.shape.rows as f32 * self.layout.cell;
        // Move by the scaled delta and keep the piece's centre on screen, so it
        // follows the finger back at once after overshooting an edge.
        d.x = (d.x + (x - d.px) * d.gain.0).max(-w / 2.0).min(self.layout.width - w / 2.0);
        d.y = (d.y + (y - d.py) * d.gain.1).max(-h / 2.0).min(self.layout.height - h / 2.0);
        d.px = x;
        d.py = y;
        self.dirty = true;
    }

    /// The drop uses the last move: fingers often slide a little while lifting off.
    pub fn pointer_up(&mut self, id: u64, x: f32, y: f32) {
        if let Some((pid, _)) = &self.ui_press {
            if *pid != id {
                return;
            }
            let (_, press) = self.ui_press.take().expect("press");
            self.ui.pressed = None;
            match press {
                UiPress::Button(b) => {
                    if self.ui.hit_at(x, y) == Some(b) {
                        self.activate(b);
                    }
                }
                UiPress::Slider(key) => self.commit_level(key),
                UiPress::Backdrop => {
                    // Tapping outside the menu closes it.
                    if self.ui.overlay == Overlay::Menu && self.ui.hit_at(x, y) == Some(UiId::Backdrop) {
                        self.close_menu();
                    }
                }
                UiPress::Scroll(_) => {}
            }
            self.dirty = true;
            return;
        }
        if self.drag.as_ref().is_some_and(|d| d.pointer == id) {
            self.drop_piece(false);
        }
    }

    pub fn pointer_cancel(&mut self, id: u64) {
        if self.ui_press.as_ref().is_some_and(|(pid, _)| *pid == id) {
            if let Some((_, UiPress::Slider(key))) = self.ui_press.take() {
                self.commit_level(key);
            }
            self.ui.pressed = None;
            self.dirty = true;
        }
        if self.drag.as_ref().is_some_and(|d| d.pointer == id) {
            self.drop_piece(true);
        }
    }

    /// Mouse wheel / trackpad scroll over a card.
    pub fn wheel(&mut self, dy: f32) {
        if self.ui.overlay != Overlay::None {
            self.ui.scroll_by(dy);
            self.dirty = true;
        }
    }

    /// Where the dragged piece would land: the closest spot near it where it fits.
    fn snap_target(&self, d: &DragState) -> Option<(i32, i32)> {
        self.game.tray[d.slot]?;
        let l = &self.layout;
        nearest_fit((d.y - l.board_y) / l.cell, (d.x - l.board_x) / l.cell, |r, c| self.game.can_place(d.slot, r, c))
    }

    fn drag_view(&self, now: f64) -> Option<DragView> {
        let d = self.drag.as_ref()?;
        let piece = self.game.tray[d.slot]?;
        let t = ((now - d.start) / PICKUP_MS).clamp(0.0, 1.0) as f32;
        let lerp = |a: f32, b: f32| a + (b - a) * t;
        let target = self.snap_target(d).map(|(r, c)| (r, c, preview_lines(&self.game.board, piece.shape, r, c)));
        Some(DragView { piece, x: lerp(d.from.0, d.x), y: lerp(d.from.1, d.y), cell: lerp(d.from.2, self.layout.cell), target })
    }

    fn drop_piece(&mut self, cancelled: bool) {
        let view = self.drag_view(self.now);
        let Some(d) = self.drag.take() else { return };
        self.trail = None;
        self.scene.hidden_slot = None;
        let target = if cancelled { None } else { self.snap_target(&d) };
        if let Some((r, c)) = target {
            self.commit(d.slot, r, c);
        } else if let Some(view) = view {
            // Fly back to the tray.
            self.scene.flying = Some(FlyingPiece { piece: view.piece, from: (view.x, view.y, view.cell), slot: d.slot, start: self.now, duration: 200.0 });
            let l = &self.layout;
            let over_board = d.x + view.piece.shape.cols as f32 * l.cell > l.board_x && d.x < l.board_x + l.board_size && d.y < l.board_y + l.board_size;
            if over_board {
                self.audio.play(Sfx::Invalid);
            }
        }
        self.dirty = true;
    }

    // -----------------------------------------------------------------------
    // Moves & effects
    // -----------------------------------------------------------------------

    /// Places the piece in `slot` at (r, c). Public so tests and tools can drive the game.
    pub fn commit(&mut self, slot: usize, r: i32, c: i32) -> Option<PlaceResult> {
        let res = self.game.place(slot, r, c)?;
        let now = self.now;
        let l = self.layout;
        let cell = l.cell;
        let shape = res.piece.shape;

        self.scene.placed = Some(Placed { cells: shape.cells.iter().map(|&(dr, dc)| idx((r + dr) as usize, (c + dc) as usize)).collect(), start: now });
        let cx = l.board_x + (c as f32 + shape.cols as f32 / 2.0) * cell;
        let cy = l.board_y + (r as f32 + shape.rows as f32 / 2.0) * cell;
        let lines = res.lines.count() as u32;
        let pr = r as f32 + shape.rows as f32 / 2.0;
        let pc = c as f32 + shape.cols as f32 / 2.0;
        self.scene.ripple = Some(Ripple { r: pr, c: pc, start: now });
        let style = self.scene.style;
        self.landing_dust(shape, r, c, style.color(res.piece.color), now);
        self.bump_score(res.points, now);

        self.audio.play(Sfx::Place);
        self.meter.record(now, self.game.combo);
        self.vibrate(if lines > 0 { Strength::Medium } else { Strength::Light });

        if lines > 0 {
            let cells: Vec<ClearingCell> = res
                .cleared
                .iter()
                .map(|cc| ClearingCell { r: cc.r, c: cc.c, color: cc.color, delay: ((cc.r as f32 + 0.5 - pr).hypot(cc.c as f32 + 0.5 - pc) * 28.0) as f64 })
                .collect();
            let palette = style.palette;
            let fx = &style.tier.fx;
            let piece_color = style.color(res.piece.color);
            for cc in &res.cleared {
                self.burst(l.board_x + (cc.c as f32 + 0.5) * cell, l.board_y + (cc.r as f32 + 0.5) * cell, style.color(cc.color), now, None, 1.0);
            }
            self.debris(&cells, pr, pc, now);
            self.scene.clearing = Some(Clearing { cells, start: now });
            for &row in &res.lines.rows {
                self.scene.streaks.push(Streak { line: Line::Row(row), from: pc, color: piece_color, start: now });
            }
            for &col in &res.lines.cols {
                self.scene.streaks.push(Streak { line: Line::Col(col), from: pr, color: piece_color, start: now });
            }
            let board_bonus = if res.board_cleared { 0.3 } else { 0.0 };
            self.scene.flash = Some(Flash { x: cx, y: cy, power: (0.25 + 0.2 * lines as f32 + board_bonus).min(1.0), color: palette.glow, start: now });
            if res.combo >= 2 || lines >= 3 || res.board_cleared {
                let power = (0.2 + 0.12 * res.combo as f32 + 0.1 * lines as f32 + if res.board_cleared { 0.4 } else { 0.0 }).min(1.0);
                self.scene.pulse = Some(Pulse { power, color: palette.glow, start: now });
            }
            if fx.beam {
                for &row in &res.lines.rows {
                    self.scene.beams.push(Beam { line: Line::Row(row), color: piece_color, start: now });
                }
                for &col in &res.lines.cols {
                    self.scene.beams.push(Beam { line: Line::Col(col), color: piece_color, start: now });
                }
            }
            if fx.ring {
                self.scene.rings.push(Ring { x: cx, y: cy, radius: cell * (2.0 + lines as f32), color: palette.glow, start: now, duration: 520.0 });
                if lines >= 2 {
                    self.scene.rings.push(Ring { x: cx, y: cy, radius: cell * (3.0 + lines as f32), color: piece_color, start: now + 90.0, duration: 620.0 });
                }
            }
            if lines >= 3 || (lines >= 2 && fx.shake > 1.0) {
                let calm = if self.scene.reduced_motion { 0.4 } else { 1.0 };
                self.scene.shake = Some(Shake { start: now, power: (lines as f32 * 2.5 * fx.shake).min(14.0) * calm });
            }
            self.audio.play(Sfx::Clear { lines, combo: res.combo });
            if lines >= 3 {
                self.vibrate(Strength::Heavy);
            }

            let mid = l.board_y + l.board_size / 2.0;
            // Bigger scores get bigger numbers.
            self.scene.texts.push(FloatingText {
                text: format!("+{}", res.points),
                x: cx,
                y: cy,
                size: cell * (0.7 + (res.points as f32 / 600.0).min(0.45)),
                color: Rgb::WHITE,
                start: now,
                duration: 900.0,
                banner: false,
                bounce: false,
            });
            let banner = palette.accent;
            if res.board_cleared {
                self.scene.texts.push(FloatingText { text: "Board Clear!".into(), x: l.width / 2.0, y: mid, size: cell * 0.95, color: banner, start: now + 150.0, duration: 1400.0, banner: true, bounce: false });
                self.audio.play(Sfx::BoardClear);
                self.fireworks(fx.fireworks, now + 200.0);
                self.confetti(now + 150.0);
            } else if lines >= 2 {
                let word = PRAISE[(lines as usize).min(PRAISE.len() - 1)];
                self.scene.texts.push(FloatingText { text: word.into(), x: l.width / 2.0, y: mid, size: cell * 0.95, color: banner, start: now + 80.0, duration: 1100.0, banner: true, bounce: false });
            }
            if res.combo >= 2 {
                // The combo counter grows, wobbles and throws more sparkles the longer the streak runs.
                let y = mid + cell * 1.05;
                let size = cell * (0.62 + 0.06 * (res.combo - 2).min(8) as f32);
                self.scene.texts.push(FloatingText {
                    text: format!("Combo {}", res.combo),
                    x: l.width / 2.0,
                    y,
                    size,
                    color: if res.combo >= 5 { palette.accent } else { palette.glow },
                    start: now + 80.0,
                    duration: 1100.0 + (res.combo as f64 * 60.0).min(500.0),
                    banner: true,
                    bounce: true,
                });
                if res.combo >= 3 {
                    self.burst(l.width / 2.0, y, palette.glow, now + 120.0, Some((4 + res.combo * 3).min(24)), 1.3);
                }
                self.motes(res.combo, now);
            }
        }

        if res.refilled {
            self.scene.tray_dealt_at = now + if lines > 0 { 120.0 } else { 0.0 };
            if self.game.lucky {
                self.lucky_sparkles(self.scene.tray_dealt_at);
            }
        }
        self.sync_scene();

        if self.game.score > self.best {
            self.best = self.game.score;
            storage::save_best(self.store.as_mut(), self.best);
        }
        let saved = (!self.game.over).then(|| self.game.serialize());
        storage::save_game(self.store.as_mut(), saved.as_ref());
        self.update_style(if lines > 0 { CLEAR_DURATION + 80.0 } else { 0.0 });

        if res.game_over {
            self.start_game_over(if lines > 0 { CLEAR_DURATION + 250.0 } else { 300.0 });
        }
        self.dirty = true;
        Some(res)
    }

    fn vibrate(&mut self, strength: Strength) {
        let level = self.settings.haptics;
        if level > 0.0 && self.platform.can_vibrate() {
            self.platform.vibrate(strength, level);
        }
    }

    /// Adds an effect particle, dropping it once a frame would get too busy.
    fn spawn(&mut self, p: Particle) {
        if self.scene.particles.len() < MAX_PARTICLES {
            self.scene.particles.push(p);
        }
    }

    /// Bumps the HUD score, harder for bigger gains.
    fn bump_score(&mut self, points: u32, now: f64) {
        if self.scene.reduced_motion {
            return;
        }
        self.bump = Some((now, (points as f32 / 300.0).min(1.0)));
    }

    /// Puffs of dust along the outer edges of a piece that just landed, pushed outwards.
    fn landing_dust(&mut self, shape: &Shape, r: i32, c: i32, color: Rgb, now: f64) {
        let l = self.layout;
        let cell = l.cell;
        let own: HashSet<(i32, i32)> = shape.cells.iter().copied().collect();
        let puff = color.shade(0.7);
        let mut n = 0;
        for &(dr, dc) in &shape.cells {
            for (nr, nc) in [(0, 1), (0, -1), (1, 0), (-1, 0)] {
                let (br, bc) = (r + dr + nr, c + dc + nc);
                // Only edges facing open space puff; blocks next door keep the dust in.
                let blocked = br >= 0 && br < SIZE as i32 && bc >= 0 && bc < SIZE as i32 && self.game.board[idx(br as usize, bc as usize)] != 0;
                if own.contains(&(dr + nr, dc + nc)) || blocked || self.rand() < 0.35 || n >= 14 {
                    continue;
                }
                n += 1;
                // The middle of the outer edge, nudged along it a little.
                let along = (self.rand() - 0.5) * 0.7;
                let x = l.board_x + ((c + dc) as f32 + 0.5 + nc as f32 * 0.5 + if nr != 0 { along } else { 0.0 }) * cell;
                let y = l.board_y + ((r + dr) as f32 + 0.5 + nr as f32 * 0.5 + if nc != 0 { along } else { 0.0 }) * cell;
                let v = cell * (0.8 + self.rand() * 1.2);
                let p = Particle {
                    x,
                    y,
                    vx: nc as f32 * v + (self.rand() - 0.5) * cell * 0.4,
                    vy: nr as f32 * v - cell * 0.3,
                    size: cell * (0.22 + self.rand() * 0.18),
                    color: puff,
                    born: now,
                    life: 320.0 + self.rand() as f64 * 180.0,
                    shape: ParticleShape::Circle,
                    gravity: -cell * 0.6,
                    spin: 0.0,
                    kind: ParticleKind::Dust,
                };
                self.spawn(p);
            }
        }
    }

    /// Chunks of the cleared blocks thrown away from the drop point as each cell pops.
    fn debris(&mut self, cells: &[ClearingCell], pr: f32, pc: f32, now: f64) {
        let l = self.layout;
        let cell = l.cell;
        let step = cells.len().div_ceil(MAX_DEBRIS).max(1);
        for cc in cells.iter().step_by(step) {
            let dx = cc.c as f32 + 0.5 - pc;
            let dy = cc.r as f32 + 0.5 - pr;
            let d = dx.hypot(dy);
            let d = if d == 0.0 { 1.0 } else { d };
            let v = cell * (2.0 + self.rand() * 3.0);
            let p = Particle {
                x: l.board_x + (cc.c as f32 + 0.5) * cell,
                y: l.board_y + (cc.r as f32 + 0.5) * cell,
                vx: dx / d * v + (self.rand() - 0.5) * cell * 2.0,
                vy: dy / d * v * 0.5 - cell * (4.0 + self.rand() * 3.0),
                size: cell * (0.4 + self.rand() * 0.15),
                color: Rgb::WHITE,
                // Leaves as its cell starts to shrink (see the clear animation in the renderer).
                born: now + cc.delay + CLEAR_DURATION * 0.25,
                life: 650.0 + self.rand() as f64 * 250.0,
                shape: ParticleShape::Square,
                gravity: cell * 30.0,
                spin: (self.rand() - 0.5) * 14.0,
                kind: ParticleKind::Block(cc.color),
            };
            self.spawn(p);
        }
    }

    /// Glowing motes drifting up over the board while a combo streak runs, more for longer streaks.
    fn motes(&mut self, combo: u32, now: f64) {
        let l = self.layout;
        let palette = self.scene.style.palette;
        let n = (4 + combo * 3).min(26);
        for i in 0..n {
            let p = Particle {
                x: l.board_x + self.rand() * l.board_size,
                y: l.board_y + l.board_size * (0.35 + self.rand() * 0.7),
                vx: (self.rand() - 0.5) * l.cell * 0.4,
                vy: -l.cell * (0.5 + self.rand() * 1.1),
                size: l.cell * (0.05 + self.rand() * 0.06),
                color: if i % 3 != 0 { palette.glow } else { palette.glow2 },
                born: now + self.rand() as f64 * 700.0,
                life: 1500.0 + self.rand() as f64 * 1100.0,
                shape: ParticleShape::Circle,
                gravity: 0.0,
                spin: self.rand() * PI * 2.0,
                kind: ParticleKind::Mote,
            };
            self.spawn(p);
        }
    }

    /// Twinkling stars around the pieces of a lucky tray as the shine sweeps over them.
    fn lucky_sparkles(&mut self, dealt_at: f64) {
        let palette = self.scene.style.palette;
        let colors = [palette.glow, palette.glow2, Rgb::WHITE];
        for slot in 0..TRAY_SIZE {
            let Some(piece) = self.game.tray[slot] else { continue };
            let (px, py, pcell) = tray_piece_position(&self.layout, slot, piece.shape);
            for i in 0..5 {
                let (dr, dc) = piece.shape.cells[(self.rand() * piece.shape.cells.len() as f32) as usize % piece.shape.cells.len()];
                let p = Particle {
                    x: px + (dc as f32 + self.rand()) * pcell,
                    y: py + (dr as f32 + self.rand()) * pcell,
                    vx: (self.rand() - 0.5) * pcell,
                    vy: -pcell * (0.6 + self.rand()),
                    size: pcell * (0.35 + self.rand() * 0.25),
                    color: colors[i % colors.len()],
                    born: dealt_at + LUCKY_SHINE_DELAY + slot as f64 * 110.0 + self.rand() as f64 * 400.0,
                    life: 450.0 + self.rand() as f64 * 250.0,
                    shape: ParticleShape::Star,
                    gravity: 0.0,
                    spin: (self.rand() - 0.5) * 6.0,
                    kind: ParticleKind::Plain,
                };
                self.spawn(p);
            }
        }
    }

    /// Small sparkles left behind by the dragged piece while it moves.
    fn drag_trail(&mut self, vx: f32, vy: f32, vcell: f32, shape: &Shape, color_id: u8, now: f64) {
        let cx = vx + shape.cols as f32 * vcell / 2.0;
        let cy = vy + shape.rows as f32 * vcell / 2.0;
        let last = self.trail;
        if let Some((lx, ly)) = last
            && (cx - lx).hypot(cy - ly) < vcell * 0.15
        {
            return;
        }
        self.trail = Some((cx, cy));
        let Some((lx, ly)) = last else { return };
        let moved = (cx - lx).hypot(cy - ly);
        let style = self.scene.style;
        let color = style.color(color_id);
        let n = ((moved / (vcell * 0.3)).floor() as usize + 1).min(3);
        for _ in 0..n {
            let (dr, dc) = shape.cells[(self.rand() * shape.cells.len() as f32) as usize % shape.cells.len()];
            let p = Particle {
                x: vx + (dc as f32 + 0.2 + self.rand() * 0.6) * vcell,
                y: vy + (dr as f32 + 0.2 + self.rand() * 0.6) * vcell,
                vx: (lx - cx) * 2.0 + (self.rand() - 0.5) * 40.0,
                vy: (ly - cy) * 2.0 + (self.rand() - 0.5) * 40.0,
                size: vcell * (0.1 + self.rand() * 0.1),
                color: if self.rand() < 0.5 { style.palette.glow } else { color },
                born: now,
                life: 260.0 + self.rand() as f64 * 160.0,
                shape: style.tier.fx.shape,
                gravity: 0.0,
                spin: (self.rand() - 0.5) * 10.0,
                kind: ParticleKind::Plain,
            };
            self.spawn(p);
        }
    }

    /// Confetti raining over the whole screen.
    fn confetti(&mut self, start: f64) {
        let (width, cell) = (self.layout.width, self.layout.cell);
        let palette = self.scene.style.palette;
        let colors: Vec<Rgb> = palette.blocks.iter().copied().chain([palette.glow, palette.accent]).collect();
        let n = if self.scene.reduced_motion { 24 } else { 70 };
        for i in 0..n {
            let p = Particle {
                x: self.rand() * width,
                y: -cell * (0.5 + self.rand() * 2.0),
                vx: (self.rand() - 0.5) * cell * 1.5,
                vy: cell * (2.0 + self.rand() * 3.0),
                size: cell * (0.2 + self.rand() * 0.14),
                color: colors[i % colors.len()],
                born: start + self.rand() as f64 * 900.0,
                life: 2300.0 + self.rand() as f64 * 900.0,
                shape: ParticleShape::Square,
                gravity: cell * 1.2,
                spin: (self.rand() - 0.5) * 8.0,
                kind: ParticleKind::Confetti,
            };
            self.spawn(p);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn burst(&mut self, x: f32, y: f32, color: Rgb, now: f64, count: Option<u32>, speed: f32) {
        let style = self.scene.style;
        let fx = &style.tier.fx;
        let palette = style.palette;
        let sparkles = [palette.glow, palette.glow2, palette.accent];
        let n = count.unwrap_or(fx.particles);
        let big = if matches!(fx.shape, ParticleShape::Star | ParticleShape::Diamond) { 1.4 } else { 1.0 };
        for i in 0..n as usize {
            let a = self.rand() * PI * 2.0;
            let v = (80.0 + self.rand() * 220.0) * speed;
            let p = Particle {
                x,
                y,
                vx: a.cos() * v,
                vy: a.sin() * v - 220.0 * (fx.gravity / 900.0).min(1.0),
                size: self.layout.cell * (0.14 + self.rand() * 0.14) * big,
                color: if self.rand() < fx.sparkle { sparkles[i % sparkles.len()] } else { color },
                born: now + self.rand() as f64 * 120.0,
                life: 550.0 + self.rand() as f64 * 300.0 + if fx.gravity < 400.0 { 250.0 } else { 0.0 },
                shape: fx.shape,
                gravity: fx.gravity,
                spin: (self.rand() - 0.5) * 12.0,
                kind: ParticleKind::Plain,
            };
            self.spawn(p);
        }
    }

    /// Celebration bursts scattered over the board.
    fn fireworks(&mut self, count: u32, start: f64) {
        let l = self.layout;
        let palette = self.scene.style.palette;
        let colors: Vec<Rgb> = palette.blocks.iter().copied().chain([palette.glow, palette.accent]).collect();
        for i in 0..count {
            let x = l.board_x + l.board_size * (0.15 + self.rand() * 0.7);
            let y = l.board_y + l.board_size * (0.15 + self.rand() * 0.6);
            let at = start + i as f64 * 140.0 + self.rand() as f64 * 80.0;
            let color = colors[(self.rand() * colors.len() as f32) as usize % colors.len()];
            self.burst(x, y, color, at, Some(14), 1.6);
            if self.scene.style.tier.fx.ring {
                self.scene.rings.push(Ring { x, y, radius: l.cell * 1.8, color, start: at, duration: 600.0 });
            }
        }
    }

    fn start_game_over(&mut self, delay: f64) {
        self.locked = true;
        let record = self.game.score > self.best_at_start;
        self.game_over_timer = Some((self.now + delay, GameOverStage::Fill { record }));
    }

    /// Grey puffs under the blocks of each row as it turns grey and hops (see the renderer).
    fn game_over_dust(&mut self, start: f64) {
        let l = self.layout;
        let cell = l.cell;
        let puff = GRAY.shade(0.4);
        for r in 0..SIZE {
            let at = start + ((SIZE - r) as f64 / (SIZE as f64 + 1.0)) * GAME_OVER_FILL + 120.0;
            for c in 0..SIZE {
                if self.game.board[idx(r, c)] == 0 || self.rand() < 0.6 {
                    continue;
                }
                let p = Particle {
                    x: l.board_x + (c as f32 + 0.2 + self.rand() * 0.6) * cell,
                    y: l.board_y + (r as f32 + 1.0) * cell,
                    vx: (self.rand() - 0.5) * cell * 1.6,
                    vy: -cell * 0.3,
                    size: cell * (0.2 + self.rand() * 0.15),
                    color: puff,
                    born: at,
                    life: 380.0 + self.rand() as f64 * 160.0,
                    shape: ParticleShape::Circle,
                    gravity: -cell * 0.5,
                    spin: 0.0,
                    kind: ParticleKind::Dust,
                };
                self.spawn(p);
            }
        }
    }

    fn show_game_over(&mut self, record: bool) {
        self.ui.game_over = Some(GameOverInfo { score: self.game.score, best: self.best, record });
        self.ui.open(Overlay::GameOver, self.now);
        self.sync_music_pause();
    }

    pub fn new_game(&mut self, seed: Option<u32>) {
        self.game_over_timer = None;
        self.pending_style = None;
        self.game = Game::new(seed);
        self.best_at_start = self.best;
        storage::save_best_at_start(self.store.as_mut(), self.best);
        self.meter.reset();
        self.locked = false;
        self.drag = None;
        self.scene.reset_effects();
        self.scene.tray_dealt_at = self.now;
        self.shown_score = 0;
        let style = style_for(0, 0);
        if !style.same(&self.scene.style) {
            self.apply_style(style);
            self.scene.style_wave_at = Some(self.now);
        }
        self.sync_scene();
        if self.game.lucky {
            self.lucky_sparkles(self.scene.tray_dealt_at);
        }
        storage::save_game(self.store.as_mut(), Some(&self.game.serialize()));
        self.ui.game_over = None;
        self.ui.close();
        self.sync_music_pause();
        self.dirty = true;
    }

    /// Saves the game in progress (when the app goes to the background).
    pub fn save(&mut self) {
        if !self.game.over {
            storage::save_game(self.store.as_mut(), Some(&self.game.serialize()));
        }
    }

    /// Board, tray and score changed from outside (tests and tools): show them.
    pub fn sync(&mut self) {
        self.sync_scene();
        self.dirty = true;
    }

    /// Sizes of everything that accumulates while playing (for leak tests).
    pub fn debug_stats(&self) -> serde_json::Value {
        serde_json::json!({
            "sprites": self.renderer.as_ref().map_or(0, |r| r.sprite_count()),
            "particles": self.scene.particles.len(),
            "rings": self.scene.rings.len(),
            "beams": self.scene.beams.len(),
            "streaks": self.scene.streaks.len(),
            "texts": self.scene.texts.len(),
            "animating": self.is_animating(),
            "now": self.now,
            "wasmMemory": wasm_memory(),
        })
    }

    // -----------------------------------------------------------------------
    // Audio
    // -----------------------------------------------------------------------

    /// Unlocks audio (browsers need a user gesture) and starts the music when it's on.
    fn unlock_audio(&mut self) {
        self.audio.unlock();
        if self.settings.music > 0.0 && !self.audio.music_playing() {
            self.audio.start_music();
            self.sync_music_pause();
        }
    }

    /// What the soundtrack follows: progress towards the best score and the pace of play.
    fn music_input(&mut self, now: f64) -> (f32, f32) {
        if self.game.over {
            return (0.0, 0.0);
        }
        (score_progress(self.game.score, self.best_at_start), self.meter.speed(now))
    }

    /// Muffles the music while a menu or the game over screen is up.
    fn sync_music_pause(&mut self) {
        self.audio.set_music_paused(self.ui.overlay != Overlay::None);
    }

    // -----------------------------------------------------------------------
    // Menus
    // -----------------------------------------------------------------------

    pub fn open_menu(&mut self) {
        if self.ui.overlay == Overlay::None {
            self.ui.open(Overlay::Menu, self.now);
            self.sync_music_pause();
            self.dirty = true;
        }
    }

    pub fn close_menu(&mut self) {
        if self.ui.overlay == Overlay::Menu {
            self.ui.close();
            self.sync_music_pause();
            self.dirty = true;
        }
    }

    fn activate(&mut self, id: UiId) {
        match id {
            UiId::MenuButton => self.open_menu(),
            UiId::Resume => self.close_menu(),
            UiId::Restart | UiId::Again => self.new_game(None),
            UiId::Exit => {
                self.save();
                self.platform.exit();
            }
            UiId::Help => self.ui.help_open = !self.ui.help_open,
            UiId::Link => self.platform.open_url(REPO_URL),
            UiId::Slider(_) | UiId::Card | UiId::Backdrop => {}
        }
        self.dirty = true;
    }

    /// Sliders apply as they move.
    fn set_level(&mut self, key: SettingKey, x: f32) {
        if let Some(v) = self.ui.slider_value(key, x) {
            self.set_setting(key, v);
        }
    }

    /// Changes a setting and applies it at once (sliders, tests).
    pub fn set_setting(&mut self, key: SettingKey, v: f32) {
        if self.settings.get(key) == v {
            return;
        }
        self.settings.set(key, v);
        match key {
            SettingKey::Sound => self.audio.set_sound_volume(v),
            SettingKey::Music => {
                self.audio.set_music_volume(v);
                if v > 0.0 {
                    self.unlock_audio();
                } else {
                    self.audio.stop_music();
                }
            }
            SettingKey::Haptics => {}
        }
        self.dirty = true;
    }

    /// The sound and vibration preview once a slider is let go, and the settings are saved.
    pub fn commit_level(&mut self, key: SettingKey) {
        storage::save_settings(self.store.as_mut(), &self.settings);
        if self.settings.get(key) > 0.0 {
            match key {
                SettingKey::Sound => {
                    self.audio.unlock();
                    self.audio.play(Sfx::Pickup);
                }
                SettingKey::Haptics => self.vibrate(Strength::Medium),
                SettingKey::Music => {}
            }
        }
    }

    /// Escape toggles the menu.
    pub fn key_escape(&mut self) {
        match self.ui.overlay {
            Overlay::None => self.open_menu(),
            Overlay::Menu => self.close_menu(),
            Overlay::GameOver => {}
        }
    }

    /// The Android back button.
    pub fn back_button(&mut self) {
        match self.ui.overlay {
            Overlay::Menu => self.close_menu(),
            Overlay::GameOver => {
                self.save();
                self.platform.exit();
            }
            Overlay::None => self.open_menu(),
        }
    }

    /// The app went to the background (`false`) or came back (`true`).
    pub fn set_visible(&mut self, visible: bool) {
        if visible {
            self.audio.resume();
            self.dirty = true;
        } else {
            self.save();
            self.audio.suspend();
        }
    }

    /// The frame clock as of the last tick, in ms.
    pub fn now(&self) -> f64 {
        self.now
    }

    pub fn overlay(&self) -> Overlay {
        self.ui.overlay
    }

    pub fn store(&mut self) -> &mut dyn KeyValue {
        self.store.as_mut()
    }
}

/// Size of the WebAssembly memory in bytes (0 natively), for leak tests.
fn wasm_memory() -> usize {
    #[cfg(target_arch = "wasm32")]
    {
        core::arch::wasm32::memory_size(0) * 65536
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::MemoryStore;

    struct TestPlatform;

    impl Platform for TestPlatform {
        fn can_vibrate(&self) -> bool {
            false
        }
        fn vibrate(&mut self, _: Strength, _: f32) {}
        fn is_app(&self) -> bool {
            false
        }
        fn exit(&mut self) {}
        fn open_url(&mut self, _: &str) {}
    }

    fn app() -> App {
        let mut a = App::new(Box::new(MemoryStore::default()), Box::new(TestPlatform), Some(12345), 0.0);
        a.resize(390.0, 844.0, 2.0, Insets::default());
        a
    }

    #[test]
    fn places_a_piece_by_dragging_with_the_mouse() {
        let mut a = app();
        let (slot, r, c) = (0..3)
            .find_map(|s| (0..8).find_map(|r| (0..8).find(|&c| a.game.can_place(s, r, c)).map(|c| (s, r, c))))
            .unwrap();
        let piece = a.game.tray[slot].unwrap();
        let l = a.layout;
        a.pointer_down(1, PointerKind::Mouse, l.slots[slot].cx, l.slots[slot].cy);
        let tx = l.board_x + (c as f32 + piece.shape.cols as f32 / 2.0) * l.cell;
        let ty = l.board_y + (r as f32 + piece.shape.rows as f32 / 2.0) * l.cell;
        a.pointer_move(1, tx, ty);
        a.pointer_up(1, tx, ty);
        assert_eq!(a.game.moves, 1);
        assert_eq!(a.game.score, piece.shape.cells.len() as u32);
        assert!(a.game.tray[slot].is_none());
    }

    #[test]
    fn a_piece_dropped_outside_the_board_flies_back() {
        let mut a = app();
        let l = a.layout;
        a.pointer_down(1, PointerKind::Touch, l.slots[0].cx, l.slots[0].cy);
        a.pointer_move(1, 5.0, 5.0);
        a.pointer_up(1, 5.0, 5.0);
        assert_eq!(a.game.moves, 0);
        assert!(a.scene.flying.is_some());
    }

    #[test]
    fn game_over_shows_the_card_after_the_animation() {
        let mut a = app();
        for i in 0..64 {
            a.game.board[i] = if ((i / 8) + i) % 2 == 0 { 0 } else { 3 };
        }
        a.game.set_tray(&[Some("dot-0"), Some("O2-0"), Some("O2-0")]);
        let res = a.commit(0, 0, 0).unwrap();
        assert!(res.game_over);
        let mut t = 0.0;
        while a.overlay() != Overlay::GameOver && t < 5000.0 {
            t += 16.0;
            a.tick(t);
        }
        assert_eq!(a.overlay(), Overlay::GameOver);
        assert!(a.ui.game_over.as_ref().unwrap().record);
        a.new_game(None);
        assert_eq!(a.overlay(), Overlay::None);
        assert_eq!(a.game.score, 0);
    }

    #[test]
    fn saves_progress_and_restores_it() {
        let mut a = app();
        let (slot, r, c) = (0..3).find_map(|s| (0..8).find_map(|r| (0..8).find(|&c| a.game.can_place(s, r, c)).map(|c| (s, r, c)))).unwrap();
        a.commit(slot, r, c);
        let saved = storage::load_game(a.store()).unwrap();
        assert_eq!(saved, a.game.serialize());
        assert_eq!(storage::load_best(a.store()), a.game.score);
    }

    #[test]
    fn clearing_the_board_switches_the_style_after_the_clear() {
        let mut a = app();
        for c in 0..7 {
            a.game.board[c] = 2;
        }
        a.game.set_tray(&[Some("dot-0"), Some("dot-0"), Some("dot-0")]);
        a.commit(0, 0, 7);
        assert_eq!(a.scene.style.key(), "0.0");
        a.tick(1000.0);
        a.scene.now = 1000.0;
        if let Some((style, at)) = a.pending_style {
            assert!(at <= 1000.0);
            a.switch_style(style, 1000.0);
        }
        assert_eq!(a.scene.style.key(), "0.1");
    }
}
