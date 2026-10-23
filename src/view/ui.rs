//! The HUD (best score, score, style progress, menu button) and the two cards
//! that cover the game: the pause menu and the game over screen. Everything is
//! drawn on the canvas; this module also hit-tests pointers against it.

use std::f32::consts::PI;

use femtovg::{Align, Color, LineCap, Paint, Path};

use super::Canvas;
use super::draw::{self, clamp01, ease_out_cubic, fill_rect, linear, round_rect};
use super::text::{self, Fonts};
use crate::color::{Rgb, black, white};
use crate::storage::{SettingKey, Settings};
use crate::styles::{Style, TIERS, format_score};

/// Something on screen a pointer can hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiId {
    MenuButton,
    Resume,
    Restart,
    Again,
    Exit,
    Help,
    Link,
    Slider(SettingKey),
    /// The card itself (taps on it do nothing, drags scroll it).
    Card,
    /// The dimmed area around the card.
    Backdrop,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub id: UiId,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Hit {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// Safe-area insets (status bar, notch, navigation bar) in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Overlay {
    None,
    Menu,
    GameOver,
}

pub struct GameOverInfo {
    pub score: u32,
    pub best: u32,
    pub record: bool,
}

/// What the HUD shows this frame.
pub struct HudInfo<'a> {
    pub best: u32,
    pub score: u32,
    /// Scale of the score bump animation (1 = at rest).
    pub bump: f32,
    pub style: &'a Style,
    /// Progress bar towards the next tier, 0..1.
    pub progress: f32,
    /// "10k" or "MAX".
    pub next_label: String,
    pub now: f64,
}

pub const HUD_PAD_TOP: f32 = 10.0;

/// Font size of the HUD score for a screen width (CSS `clamp(40px, 11vw, 60px)`; wide fonts get less).
fn score_size(width: f32, tier: usize) -> (f32, f32, f32) {
    if matches!(tier, 6 | 7 | 9) { ((width * 0.085).clamp(32.0, 50.0), 8.0, 4.0) } else { ((width * 0.11).clamp(40.0, 60.0), 6.0, 0.0) }
}

/// Height of the HUD below the top inset.
pub fn hud_height(width: f32, tier: usize) -> f32 {
    let (size, pad_top, pad_bottom) = score_size(width, tier);
    // Score line, then the style name and the tier bar.
    HUD_PAD_TOP + pad_top + size + pad_bottom + 2.0 + 13.0 + 3.0 + 12.0 + 6.0
}

/// The crown icon (24×24 viewbox) at (x, y) with size `s`.
fn crown(x: f32, y: f32, s: f32) -> Path {
    let k = s / 24.0;
    let pts = [(3.0, 7.0), (7.5, 11.0), (12.0, 4.0), (16.5, 11.0), (21.0, 7.0), (19.0, 18.0), (5.0, 18.0)];
    let mut p = Path::new();
    for (i, (px, py)) in pts.iter().enumerate() {
        if i == 0 {
            p.move_to(x + px * k, y + py * k);
        } else {
            p.line_to(x + px * k, y + py * k);
        }
    }
    p.close();
    p
}

/// A button's look: the accent "primary" or the plain one.
#[derive(Clone, Copy, PartialEq)]
enum ButtonKind {
    Primary,
    Plain,
}

/// One laid-out element of a card, positioned relative to the card's content top.
enum Item {
    Title(String, f32),
    Button(UiId, &'static str, ButtonKind),
    Slider(SettingKey, &'static str),
    HelpToggle,
    Paragraph(String),
    About,
    Record,
    Label(String, f32, Color),
    BigScore(String),
    Gap(f32),
}

pub struct Ui {
    pub overlay: Overlay,
    /// When the current overlay opened (for its pop-in).
    opened_at: f64,
    pub help_open: bool,
    pub game_over: Option<GameOverInfo>,
    /// The button held down, highlighted until the pointer is released.
    pub pressed: Option<UiId>,
    /// Card scroll offset when it is taller than the screen.
    pub scroll: f32,
    max_scroll: f32,
    hits: Vec<Hit>,
    /// Slider tracks from the last frame: (key, x, width).
    tracks: Vec<(SettingKey, f32, f32)>,
}

pub struct UiContext<'a> {
    pub fonts: &'a Fonts,
    pub style: &'a Style,
    pub width: f32,
    pub height: f32,
    pub insets: Insets,
    pub settings: &'a Settings,
    pub can_vibrate: bool,
    pub native: bool,
    pub now: f64,
}

pub const HELP_1: &str = "Drag the pieces onto the 8×8 board. Fill a whole row or column to clear it. Clear several lines at once, or keep clearing on consecutive turns, for combo bonuses. The game ends when none of your pieces fit.";
pub const HELP_2: &str = "Pieces glowing in the tray are a lucky set, picked to clear lines and free up room. At first every set is lucky, then every 2nd from 3k points and every 3rd from 40k. When the board is 45% full, the next set is always lucky and clears a line whenever it can.";

/// The third help paragraph, listing the tier thresholds.
pub fn help_3() -> String {
    let scores: Vec<String> = TIERS[1..].iter().map(|t| format_score(t.min_score)).collect();
    let (last, rest) = scores.split_last().expect("tiers");
    format!(
        "Clear the whole board to switch to a new look. Score {} and {} points in one game to unlock fancier styles, with new blocks, effects, fonts and sounds.",
        rest.join(", "),
        last
    )
}

pub const REPO_URL: &str = "https://github.com/noartem/block-baster";

impl Default for Ui {
    fn default() -> Self {
        Self::new()
    }
}

impl Ui {
    pub fn new() -> Self {
        Ui { overlay: Overlay::None, opened_at: 0.0, help_open: false, game_over: None, pressed: None, scroll: 0.0, max_scroll: 0.0, hits: Vec::new(), tracks: Vec::new() }
    }

    pub fn open(&mut self, overlay: Overlay, now: f64) {
        if self.overlay != overlay {
            self.overlay = overlay;
            self.opened_at = now;
            self.scroll = 0.0;
            self.pressed = None;
        }
    }

    pub fn close(&mut self) {
        self.overlay = Overlay::None;
        self.pressed = None;
    }

    /// True while an overlay is popping in.
    pub fn is_animating(&self, now: f64) -> bool {
        self.overlay != Overlay::None && (now - self.opened_at < 300.0 || self.game_over.as_ref().is_some_and(|g| g.record))
    }

    /// Everything a pointer could hit in the last frame, topmost first.
    pub fn hits(&self) -> &[Hit] {
        &self.hits
    }

    pub fn hit_at(&self, x: f32, y: f32) -> Option<UiId> {
        self.hits.iter().find(|h| h.contains(x, y)).map(|h| h.id)
    }

    pub fn rect_of(&self, id: UiId) -> Option<Hit> {
        self.hits.iter().find(|h| h.id == id).copied()
    }

    /// The slider value (0..1, in 5% steps) for a pointer at `x`.
    pub fn slider_value(&self, key: SettingKey, x: f32) -> Option<f32> {
        let &(_, tx, tw) = self.tracks.iter().find(|t| t.0 == key)?;
        let v = clamp01((x - tx) / tw.max(1.0));
        Some((v * 20.0).round() / 20.0)
    }

    pub fn scroll_by(&mut self, dy: f32) {
        self.scroll = (self.scroll + dy).clamp(0.0, self.max_scroll);
    }

    // -----------------------------------------------------------------------
    // HUD
    // -----------------------------------------------------------------------

    /// Draws the HUD and records the menu button's hit area.
    pub fn draw_hud(&mut self, c: &mut Canvas, ctx: &UiContext, hud: &HudInfo) {
        self.hits.clear();
        self.tracks.clear();
        let p = ctx.style.palette;
        let tier = ctx.style.tier;
        let inner_w = (ctx.width - ctx.insets.left - ctx.insets.right).min(640.0);
        let left = ctx.insets.left + ((ctx.width - ctx.insets.left - ctx.insets.right) - inner_w) / 2.0 + 14.0;
        let right = left + inner_w - 28.0;
        let top = ctx.insets.top + HUD_PAD_TOP;
        let (size, pad_top, _) = score_size(ctx.width, tier.level);
        let row_mid = top + pad_top + size / 2.0;

        // Best score with a crown.
        let best = text::paint(&[ctx.fonts.bold], 22.0, Align::Left, p.accent.fv());
        let crown_path = crown(left, row_mid - 13.0, 26.0);
        c.save();
        c.translate(0.0, 2.0);
        draw::fill_color(c, &crown_path, black(0.25));
        c.restore();
        draw::fill_color(c, &crown_path, p.accent.fv());
        text::fill_shadowed(c, left + 32.0, row_mid + 1.0, &hud.best.to_string(), &best, 2.0, 0.25);

        // Menu button.
        let (bx, by) = (right - 44.0, row_mid - 22.0);
        let pressed = self.pressed == Some(UiId::MenuButton);
        c.save();
        if pressed {
            c.translate(bx + 22.0, by + 22.0);
            c.scale(0.94, 0.94);
            c.translate(-(bx + 22.0), -(by + 22.0));
        }
        draw::fill_color(c, &round_rect(bx, by, 44.0, 44.0, 12.0), white(0.12));
        let mut lines = Path::new();
        let k = 26.0 / 24.0;
        let (ix, iy) = (bx + 9.0, by + 9.0);
        for y in [6.0, 12.0, 18.0] {
            lines.move_to(ix + 4.0 * k, iy + y * k);
            lines.line_to(ix + 20.0 * k, iy + y * k);
        }
        let mut stroke = Paint::color(Color::white());
        stroke.set_line_width(2.6);
        stroke.set_line_cap(LineCap::Round);
        c.stroke_path(&lines, &stroke);
        c.restore();
        self.hits.push(Hit { id: UiId::MenuButton, x: bx - 6.0, y: by - 6.0, w: 56.0, h: 56.0 });

        // Score, in the style's display font.
        let cx = ctx.width / 2.0;
        let fonts = ctx.fonts.display(tier.font);
        let score_text = hud.score.to_string();
        let fill = text::paint(&fonts, size, Align::Center, Color::white());
        // Long scores shrink to stay clear of the best score and the menu button.
        let best_w = 32.0 + text::width(c, &hud.best.to_string(), &best);
        let room = (right - left) - 2.0 * best_w.max(44.0) - 16.0;
        let fit = (room / text::width(c, &score_text, &fill).max(1.0)).min(1.0);
        c.save();
        c.translate(cx, row_mid);
        c.scale(hud.bump * fit, hud.bump * fit);
        match tier.level {
            7 => {
                // Neon tubes: layered glows.
                c.set_shadow_color(p.glow2.fv());
                c.set_shadow_blur(36.0);
                text::fill(c, 0.0, 0.0, &score_text, &fill);
                c.set_shadow_color(p.glow.fv());
                c.set_shadow_blur(18.0);
                text::fill(c, 0.0, 0.0, &score_text, &fill);
                c.set_shadow_blur(6.0);
                text::fill(c, 0.0, 0.0, &score_text, &fill);
            }
            9 => {
                // Gold lettering with a dark drop and a glow.
                c.set_shadow_color(p.glow.fv());
                c.set_shadow_blur(10.0);
                text::fill(c, 0.0, 2.0, &score_text, &text::paint(&fonts, size, Align::Center, black(0.45)));
                c.set_shadow_color(Color::rgbaf(0.0, 0.0, 0.0, 0.0));
                let grad = linear(0.0, -size / 2.0, 0.0, size / 2.0, &[(0.0, Rgb::hex("#fffbe6").fv()), (0.48, p.accent.fv()), (0.55, p.accent.fv()), (1.0, Rgb::hex("#b07a10").fv())]);
                text::fill(c, 0.0, 0.0, &score_text, &text::styled(grad, &fonts, size, Align::Center));
            }
            _ => {
                if matches!(tier.level, 5 | 6 | 8) {
                    c.set_shadow_color(p.glow.fv());
                    c.set_shadow_blur(18.0);
                    text::fill(c, 0.0, 3.0, &score_text, &text::paint(&fonts, size, Align::Center, black(0.28)));
                    c.set_shadow_color(Color::rgbaf(0.0, 0.0, 0.0, 0.0));
                    text::fill(c, 0.0, 0.0, &score_text, &fill);
                } else {
                    text::fill_shadowed(c, 0.0, 0.0, &score_text, &fill, 3.0, 0.28);
                }
            }
        }
        c.set_shadow_color(Color::rgbaf(0.0, 0.0, 0.0, 0.0));
        c.set_shadow_blur(0.0);
        c.restore();

        // Style name and progress towards the next tier.
        let info_y = top + pad_top + size + score_size(ctx.width, tier.level).2 + 2.0 + 6.5;
        let mut name = text::paint(&[ctx.fonts.regular], 12.0, Align::Center, white(0.78));
        name.set_letter_spacing(0.48);
        text::fill(c, cx, info_y, &format!("{} · {}", tier.name, p.name).to_uppercase(), &name);
        let bar_y = info_y + 6.5 + 3.0 + 6.0;
        let label = text::paint(&[ctx.fonts.regular], 11.0, Align::Left, white(0.6));
        let label_w = text::width(c, &hud.next_label, &label);
        let total = 84.0 + 6.0 + label_w;
        let tx = cx - total / 2.0;
        draw::fill_color(c, &round_rect(tx, bar_y - 2.5, 84.0, 5.0, 2.5), black(0.25));
        let filled = 84.0 * clamp01(hud.progress);
        if filled > 0.5 {
            let paint = if tier.level == 9 {
                // A golden shimmer running along the bar.
                let shift = ((hud.now / 2400.0).fract() as f32) * 168.0;
                let gold = [Rgb::hex("#b07a10"), Rgb::hex("#fff6c8"), p.accent, Rgb::hex("#fff6c8"), Rgb::hex("#b07a10")];
                let stops: Vec<(f32, Color)> = gold.iter().enumerate().map(|(i, c)| (i as f32 / 4.0, c.fv())).collect();
                let mut paint = linear(tx - shift, 0.0, tx - shift + 168.0, 0.0, &stops);
                paint = paint.with_anti_alias(true);
                paint
            } else {
                linear(tx, 0.0, tx + 84.0, 0.0, &[(0.0, p.accent.fv()), (1.0, p.glow.fv())])
            };
            c.fill_path(&round_rect(tx, bar_y - 2.5, filled, 5.0, 2.5), &paint);
        }
        text::fill(c, tx + 90.0, bar_y + 0.5, &hud.next_label, &label);
    }

    // -----------------------------------------------------------------------
    // Cards
    // -----------------------------------------------------------------------

    fn items(&self, ctx: &UiContext) -> Vec<Item> {
        let mut items = Vec::new();
        match self.overlay {
            Overlay::Menu => {
                items.push(Item::Title("Paused".into(), 30.0));
                items.push(Item::Button(UiId::Resume, "Resume", ButtonKind::Primary));
                items.push(Item::Button(UiId::Restart, "New game", ButtonKind::Plain));
                items.push(Item::Gap(6.0));
                items.push(Item::Slider(SettingKey::Music, "Music"));
                items.push(Item::Slider(SettingKey::Sound, "Sounds"));
                if ctx.can_vibrate {
                    items.push(Item::Slider(SettingKey::Haptics, "Vibration"));
                }
                items.push(Item::HelpToggle);
                if self.help_open {
                    items.push(Item::Paragraph(HELP_1.into()));
                    items.push(Item::Paragraph(HELP_2.into()));
                    items.push(Item::Paragraph(help_3()));
                }
                if ctx.native {
                    items.push(Item::Button(UiId::Exit, "Exit", ButtonKind::Plain));
                }
                items.push(Item::About);
            }
            Overlay::GameOver => {
                let Some(info) = &self.game_over else { return items };
                items.push(Item::Title("No more space!".into(), 30.0));
                if info.record {
                    items.push(Item::Record);
                }
                items.push(Item::Label("Score".into(), 18.0, white(0.72)));
                items.push(Item::BigScore(info.score.to_string()));
                items.push(Item::Label(format!("Best {}", info.best), 20.0, ctx.style.palette.accent.fv()));
                items.push(Item::Gap(10.0));
                items.push(Item::Button(UiId::Again, "Play again", ButtonKind::Primary));
            }
            Overlay::None => {}
        }
        items
    }

    fn item_height(&self, c: &Canvas, ctx: &UiContext, item: &Item, width: f32) -> f32 {
        match item {
            Item::Title(_, size) => size * 1.1 + 14.0,
            Item::Button(..) => 48.0 + 20.0,
            Item::Slider(..) => 44.0 + 8.0,
            Item::HelpToggle => 36.0,
            Item::Paragraph(t) => {
                let paint = text::paint(&[ctx.fonts.regular], 16.0, Align::Left, Color::white());
                text::wrap(c, t, width - 8.0, &paint).len() as f32 * 16.0 * 1.35 + 12.0
            }
            Item::About => 12.0 + 13.0 * 1.5 * 2.0,
            Item::Record => 20.0 * 1.2 + 2.0,
            Item::Label(_, size, _) => size * 1.3,
            Item::BigScore(_) => 54.0 * 1.05,
            Item::Gap(g) => *g,
        }
    }

    /// Draws the open card (if any) over the game and records its hit areas.
    pub fn draw_overlay(&mut self, c: &mut Canvas, ctx: &UiContext) {
        if self.overlay == Overlay::None {
            return;
        }
        let p = ctx.style.palette;
        let t = ((ctx.now - self.opened_at) / 200.0).clamp(0.0, 1.0) as f32;
        // Backdrop fades in.
        fill_rect(c, 0.0, 0.0, ctx.width, ctx.height, black(0.5 * t));

        let avail_w = ctx.width - ctx.insets.left - ctx.insets.right - 40.0;
        let card_w = avail_w.min(360.0);
        let content_w = card_w - 40.0;
        let items = self.items(ctx);
        let content_h: f32 = items.iter().map(|i| self.item_height(c, ctx, i, content_w)).sum();
        let card_h_full = 22.0 + content_h + 16.0;
        let avail_h = ctx.height - ctx.insets.top - ctx.insets.bottom - 40.0;
        let card_h = card_h_full.min(avail_h);
        self.max_scroll = (card_h_full - card_h).max(0.0);
        self.scroll = self.scroll.clamp(0.0, self.max_scroll);
        let card_x = ctx.insets.left + (ctx.width - ctx.insets.left - ctx.insets.right - card_w) / 2.0;
        let card_y = ctx.insets.top + (ctx.height - ctx.insets.top - ctx.insets.bottom - card_h) / 2.0;

        // The card pops in (cubic-bezier(0.2, 1.4, 0.4, 1) over 280 ms, roughly).
        let pt = ((ctx.now - self.opened_at) / 280.0).clamp(0.0, 1.0) as f32;
        let pop = 0.85 + 0.15 * back_out(pt);
        c.save();
        c.set_global_alpha(clamp01(pt * 1.6));
        let (ccx, ccy) = (card_x + card_w / 2.0, card_y + card_h / 2.0);
        c.translate(ccx, ccy);
        c.scale(pop, pop);
        c.translate(-ccx, -ccy);

        let card = round_rect(card_x, card_y, card_w, card_h, 22.0);
        c.set_shadow_color(black(0.4));
        c.set_shadow_blur(40.0);
        c.set_shadow_offset(0.0, 16.0);
        draw::fill_color(c, &card, p.card.fv());
        c.set_shadow_offset(0.0, 0.0);
        if ctx.style.tier.level >= 7 {
            c.set_shadow_color(p.card_edge.fv());
            c.set_shadow_blur(22.0);
            draw::fill_color(c, &card, p.card.fv());
        }
        c.set_shadow_color(Color::rgbaf(0.0, 0.0, 0.0, 0.0));
        c.set_shadow_blur(0.0);
        draw::stroke(c, &round_rect(card_x + 1.5, card_y + 1.5, card_w - 3.0, card_h - 3.0, 20.5), p.card_edge.fv(), 3.0);

        // Hits are recorded topmost first: card contents, the card, then the backdrop.
        let mut hits = Vec::new();
        c.save();
        c.scissor(card_x + 3.0, card_y + 3.0, card_w - 6.0, card_h - 6.0);
        let x = card_x + 20.0;
        let mut y = card_y + 22.0 - self.scroll;
        for item in &items {
            let h = self.item_height(c, ctx, item, content_w);
            self.draw_item(c, ctx, item, x, y, content_w, h, &mut hits);
            y += h;
        }
        c.restore();
        // Only what is visible inside the card can be hit.
        hits.retain(|h: &Hit| h.y + h.h > card_y && h.y < card_y + card_h);
        for h in &mut hits {
            let bottom = (h.y + h.h).min(card_y + card_h);
            h.y = h.y.max(card_y);
            h.h = bottom - h.y;
        }
        if self.max_scroll > 0.0 {
            // A thin scroll indicator.
            let track = card_h - 24.0;
            let thumb = track * card_h / card_h_full;
            let ty = card_y + 12.0 + (track - thumb) * self.scroll / self.max_scroll;
            draw::fill_color(c, &round_rect(card_x + card_w - 8.0, ty, 4.0, thumb, 2.0), white(0.3));
        }
        c.restore();

        hits.push(Hit { id: UiId::Card, x: card_x, y: card_y, w: card_w, h: card_h });
        hits.push(Hit { id: UiId::Backdrop, x: 0.0, y: 0.0, w: ctx.width, h: ctx.height });
        // The overlay covers the HUD, so its hits replace the menu button's.
        self.hits = hits;
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_item(&mut self, c: &mut Canvas, ctx: &UiContext, item: &Item, x: f32, y: f32, w: f32, h: f32, hits: &mut Vec<Hit>) {
        let p = ctx.style.palette;
        let cx = x + w / 2.0;
        match item {
            Item::Title(title, size) => {
                let fonts = ctx.fonts.display(ctx.style.tier.font);
                let paint = text::paint(&fonts, *size, Align::Center, Color::white());
                text::fill(c, cx, y + size * 0.55, title, &paint);
            }
            Item::Button(id, label, kind) => {
                let by = y + 10.0;
                let pressed = self.pressed == Some(*id);
                let (bg, edge, fg) = match kind {
                    ButtonKind::Primary => (p.accent, p.accent.shade(-0.28), Rgb::hex("#4a2a00")),
                    ButtonKind::Plain => (p.card.shade(0.14), p.board_edge, Rgb::WHITE),
                };
                let dy = if pressed { 3.0 } else { 0.0 };
                let drop = if pressed { 1.0 } else { 4.0 };
                draw::fill_color(c, &round_rect(x, by + dy + drop, w, 48.0, 14.0), edge.fv());
                draw::fill_color(c, &round_rect(x, by + dy, w, 48.0, 14.0), bg.fv());
                let paint = text::paint(&[ctx.fonts.bold], 20.0, Align::Center, fg.fv());
                text::fill(c, cx, by + dy + 25.0, label, &paint);
                hits.push(Hit { id: *id, x, y: by, w, h: 52.0 });
            }
            Item::Slider(key, label) => {
                let ry = y;
                draw::fill_color(c, &round_rect(x, ry, w, 44.0, 12.0), white(0.08));
                let mid = ry + 22.0;
                let lp = text::paint(&[ctx.fonts.regular], 17.0, Align::Left, Color::white());
                text::fill(c, x + 12.0, mid + 1.0, label, &lp);
                let value = ctx.settings.get(*key);
                let vtext = if value > 0.0 { format!("{}%", (value * 100.0).round()) } else { "Off".to_owned() };
                let vp = text::paint(&[ctx.fonts.regular], 17.0, Align::Right, white(0.72));
                text::fill(c, x + w - 12.0, mid + 1.0, &vtext, &vp);
                // 5.2em label column and 3.2em value column, 10px gaps.
                let tx = x + 12.0 + 17.0 * 5.2 + 10.0;
                let tw = (x + w - 12.0 - 17.0 * 3.2 - 10.0) - tx;
                draw::fill_color(c, &round_rect(tx, mid - 3.0, tw, 6.0, 3.0), white(0.25));
                draw::fill_color(c, &round_rect(tx, mid - 3.0, tw * value, 6.0, 3.0), p.accent.fv());
                let kx = tx + tw * value;
                c.set_shadow_color(black(0.35));
                c.set_shadow_blur(4.0);
                draw::fill_color(c, &draw::circle(kx, mid, 10.0), p.accent.fv());
                c.set_shadow_color(Color::rgbaf(0.0, 0.0, 0.0, 0.0));
                c.set_shadow_blur(0.0);
                draw::fill_color(c, &draw::circle(kx, mid, 4.0), white(0.85));
                self.tracks.push((*key, tx, tw));
                hits.push(Hit { id: UiId::Slider(*key), x: tx - 14.0, y: ry, w: tw + 28.0, h: 44.0 });
            }
            Item::HelpToggle => {
                let paint = text::paint(&[ctx.fonts.regular], 16.0, Align::Center, Color::white());
                let label = "How to play";
                text::fill(c, cx, y + h / 2.0, label, &paint);
                // A disclosure triangle, like <details>.
                let tw = text::width(c, label, &paint);
                let (ax, ay) = (cx - tw / 2.0 - 14.0, y + h / 2.0);
                let mut tri = Path::new();
                if self.help_open {
                    tri.move_to(ax - 5.0, ay - 3.0);
                    tri.line_to(ax + 5.0, ay - 3.0);
                    tri.line_to(ax, ay + 4.0);
                } else {
                    tri.move_to(ax - 3.0, ay - 5.0);
                    tri.line_to(ax + 4.0, ay);
                    tri.line_to(ax - 3.0, ay + 5.0);
                }
                tri.close();
                draw::fill_color(c, &tri, Color::white());
                hits.push(Hit { id: UiId::Help, x, y, w, h });
            }
            Item::Paragraph(t) => {
                let paint = text::paint(&[ctx.fonts.regular], 16.0, Align::Left, white(0.72));
                let mut ly = y + 6.0 + 16.0 * 1.35 / 2.0;
                for line in text::wrap(c, t, w - 8.0, &paint) {
                    text::fill(c, x + 4.0, ly, &line, &paint);
                    ly += 16.0 * 1.35;
                }
            }
            Item::About => {
                let paint = text::paint(&[ctx.fonts.regular], 13.0, Align::Center, white(0.72));
                let l1 = y + 12.0 + 13.0 * 0.75;
                text::fill(c, cx, l1, "Free & open source · no ads · no tracking", &paint);
                let link = "github.com/noartem/block-baster";
                let version = format!(" · v{}", env!("CARGO_PKG_VERSION"));
                let lw = text::width(c, link, &paint);
                let vw = text::width(c, &version, &paint);
                let lx = cx - (lw + vw) / 2.0;
                let l2 = l1 + 13.0 * 1.5;
                let left = text::paint(&[ctx.fonts.regular], 13.0, Align::Left, white(0.72));
                text::fill(c, lx, l2, link, &left);
                text::fill(c, lx + lw, l2, &version, &left);
                fill_rect(c, lx, l2 + 7.0, lw, 1.0, white(0.6));
                hits.push(Hit { id: UiId::Link, x: lx - 4.0, y: l2 - 11.0, w: lw + 8.0, h: 22.0 });
            }
            Item::Record => {
                // Pulses gently, like the CSS animation.
                let k = 1.0 + 0.08 * (0.5 - 0.5 * ((ctx.now / 1000.0) as f32 * PI).cos());
                c.save();
                c.translate(cx, y + 12.0);
                c.scale(k, k);
                let paint = text::paint(&[ctx.fonts.bold], 20.0, Align::Center, p.accent.fv());
                text::fill(c, 0.0, 0.0, "New best score!", &paint);
                c.restore();
            }
            Item::Label(t, size, color) => {
                let paint = text::paint(&[ctx.fonts.regular], *size, Align::Center, *color);
                text::fill(c, cx, y + h / 2.0, t, &paint);
            }
            Item::BigScore(t) => {
                let fonts = ctx.fonts.display(ctx.style.tier.font);
                let paint = text::paint(&fonts, 54.0, Align::Center, Color::white());
                text::fill(c, cx, y + h / 2.0, t, &paint);
            }
            Item::Gap(_) => {}
        }
    }
}

/// An overshooting ease-out for the card pop.
fn back_out(t: f32) -> f32 {
    let s = 1.7;
    let u = t - 1.0;
    1.0 + (s + 1.0) * u * u * u + s * u * u
}

#[allow(dead_code)]
fn ease(t: f32) -> f32 {
    ease_out_cubic(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_tier_thresholds_in_the_help() {
        let h = help_3();
        assert!(h.contains("5k, 12k, 25k, 40k, 60k, 90k, 130k, 200k and 350k points"), "{h}");
    }
}
