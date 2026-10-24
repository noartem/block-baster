//! The web version: the game runs as WebAssembly and draws with WebGL2 on one
//! full-page canvas. This shell wires DOM events, the animation frame loop,
//! localStorage and a small JS API (`window.blockBaster`) for tests and tools.

use std::cell::RefCell;
use std::rc::Rc;

use femtovg::renderer::OpenGl;
use serde_json::{Value, json};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, KeyboardEvent, PointerEvent, WheelEvent};

use crate::app::{App, PointerKind, Platform, Strength};
use crate::game::board::SIZE;
use crate::game::shapes::shape_by_id;
use crate::storage::{KeyValue, SettingKey};
use crate::styles::{Style, style_for};
use crate::view::Canvas;
use crate::view::ui::{Insets, Overlay, UiId};

fn window() -> web_sys::Window {
    web_sys::window().expect("window")
}

fn now() -> f64 {
    window().performance().map_or(0.0, |p| p.now())
}

fn media(query: &str) -> bool {
    window().match_media(query).ok().flatten().is_some_and(|m| m.matches())
}

/// localStorage, keeping the keys and JSON of the earlier web version.
struct LocalStore(Option<web_sys::Storage>);

impl KeyValue for LocalStore {
    fn get(&self, key: &str) -> Option<String> {
        self.0.as_ref()?.get_item(key).ok().flatten()
    }

    fn set(&mut self, key: &str, value: Option<&str>) {
        // Storage may be unavailable (private mode, quota); the game still works.
        if let Some(s) = &self.0 {
            let _ = match value {
                Some(v) => s.set_item(key, v),
                None => s.remove_item(key),
            };
        }
    }

    fn clear(&mut self) {
        if let Some(s) = &self.0 {
            let _ = s.clear();
        }
    }
}

struct Web {
    can_vibrate: bool,
}

impl Platform for Web {
    fn can_vibrate(&self) -> bool {
        self.can_vibrate
    }

    fn vibrate(&mut self, strength: Strength, intensity: f32) {
        let base = match strength {
            Strength::Light => 8.0,
            Strength::Medium => 18.0,
            Strength::Heavy => 35.0,
        };
        // Some browsers block vibration without user activation; that's fine.
        let _ = window().navigator().vibrate_with_duration((base * intensity * 2.0).round().max(4.0) as u32);
    }

    fn is_app(&self) -> bool {
        false
    }

    fn exit(&mut self) {}

    fn open_url(&mut self, url: &str) {
        let _ = window().open_with_url_and_target_and_features(url, "_blank", "noopener");
    }

    fn reduced_motion(&self) -> bool {
        media("(prefers-reduced-motion: reduce)")
    }

    fn style_changed(&mut self, style: &Style) {
        if let Some(doc) = window().document() {
            if let Some(body) = doc.body() {
                let _ = body.dataset().set("style", &style.key());
                let _ = body.dataset().set("tier", &style.tier.level.to_string());
            }
            if let Ok(Some(meta)) = doc.query_selector("meta[name=theme-color]") {
                let _ = meta.set_attribute("content", &style.palette.bg[1].to_hex());
            }
        }
    }
}

struct WebShell {
    app: App,
    canvas: Canvas,
    element: HtmlCanvasElement,
    dpr: f32,
    frame_requested: bool,
    timer: Option<i32>,
    last_frame: f64,
    overlay: Option<Overlay>,
}

thread_local! {
    static SHELL: RefCell<Option<Rc<RefCell<WebShell>>>> = const { RefCell::new(None) };
    static FRAME: RefCell<Option<Closure<dyn FnMut(f64)>>> = const { RefCell::new(None) };
    static TIMER: RefCell<Option<Closure<dyn FnMut()>>> = const { RefCell::new(None) };
}

fn shell() -> Option<Rc<RefCell<WebShell>>> {
    SHELL.with(|s| s.borrow().clone())
}

/// Runs `f` on the shell, then makes sure a frame or wake-up is scheduled.
fn with_shell<R>(f: impl FnOnce(&mut WebShell) -> R) -> Option<R> {
    let shell = shell()?;
    let r = f(&mut shell.borrow_mut());
    schedule(&shell);
    Some(r)
}

/// Requests an animation frame when the app needs one, otherwise a timer for its next wake-up.
fn schedule(shell: &Rc<RefCell<WebShell>>) {
    let mut s = shell.borrow_mut();
    if s.frame_requested {
        return;
    }
    let t = now();
    s.app.tick(t);
    if s.app.needs_frame() {
        s.frame_requested = true;
        FRAME.with(|f| {
            if let Some(cb) = f.borrow().as_ref() {
                let _ = window().request_animation_frame(cb.as_ref().unchecked_ref());
            }
        });
        return;
    }
    let mut wake = s.app.next_wakeup();
    if s.app.background_animates() {
        // Animated backgrounds run at 30 fps while nothing else moves.
        let next = s.last_frame + 33.0;
        wake = Some(wake.map_or(next, |w| w.min(next)));
    }
    if let Some(old) = s.timer.take() {
        window().clear_timeout_with_handle(old);
    }
    if let Some(at) = wake {
        let delay = (at - t).max(0.0).ceil() as i32;
        TIMER.with(|cb| {
            if let Some(cb) = cb.borrow().as_ref() {
                s.timer = window().set_timeout_with_callback_and_timeout_and_arguments_0(cb.as_ref().unchecked_ref(), delay).ok();
            }
        });
    }
}

impl WebShell {
    fn resize(&mut self) {
        let rect = self.element.get_bounding_client_rect();
        let (w, h) = (rect.width().max(1.0) as f32, rect.height().max(1.0) as f32);
        let dpr = (window().device_pixel_ratio() as f32).clamp(1.0, 3.0);
        let (pw, ph) = ((w * dpr).round() as u32, (h * dpr).round() as u32);
        if self.element.width() != pw || self.element.height() != ph {
            self.element.set_width(pw);
            self.element.set_height(ph);
        }
        self.dpr = dpr;
        self.canvas.set_size(pw, ph, 1.0);
        self.app.resize(w, h, dpr, safe_area());
    }

    fn frame(&mut self, t: f64) {
        let (pw, ph) = (self.element.width(), self.element.height());
        self.canvas.reset();
        self.canvas.clear_rect(0, 0, pw, ph, femtovg::Color::black());
        self.canvas.reset_transform();
        self.canvas.scale(self.dpr, self.dpr);
        self.app.frame(&mut self.canvas, t);
        self.canvas.flush();
        self.last_frame = t;
        // Test hooks watch the open card through a data attribute.
        let overlay = self.app.overlay();
        if self.overlay != Some(overlay)
            && let Some(body) = window().document().and_then(|d| d.body())
        {
            let name = match overlay {
                Overlay::None => "none",
                Overlay::Menu => "menu",
                Overlay::GameOver => "gameover",
            };
            let _ = body.dataset().set("overlay", name);
            self.overlay = Some(overlay);
        }
    }

    fn point(&self, e: &PointerEvent) -> (f32, f32) {
        let rect = self.element.get_bounding_client_rect();
        ((e.client_x() as f64 - rect.left()) as f32, (e.client_y() as f64 - rect.top()) as f32)
    }
}

/// `env(safe-area-inset-*)`, read from a probe element styled with them.
fn safe_area() -> Insets {
    let Some(doc) = window().document() else { return Insets::default() };
    let Some(el) = doc.get_element_by_id("safe-area") else { return Insets::default() };
    let Ok(Some(style)) = window().get_computed_style(&el) else { return Insets::default() };
    let px = |prop: &str| style.get_property_value(prop).ok().and_then(|v| v.trim_end_matches("px").parse::<f32>().ok()).unwrap_or(0.0);
    Insets { top: px("padding-top"), right: px("padding-right"), bottom: px("padding-bottom"), left: px("padding-left") }
}

fn listen<E: JsCast + wasm_bindgen::convert::FromWasmAbi + 'static>(target: &web_sys::EventTarget, name: &str, f: impl FnMut(E) + 'static) {
    let cb = Closure::<dyn FnMut(E)>::new(f);
    let _ = target.add_event_listener_with_callback(name, cb.as_ref().unchecked_ref());
    cb.forget();
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    let doc = window().document().ok_or("no document")?;
    let element: HtmlCanvasElement = doc.get_element_by_id("game").ok_or("no #game canvas")?.dyn_into()?;
    let renderer = OpenGl::new_from_html_canvas(&element).map_err(|e| JsValue::from_str(&format!("WebGL2: {e:?}")))?;
    let canvas = Canvas::new(renderer).map_err(|e| JsValue::from_str(&format!("canvas: {e:?}")))?;

    let params = web_sys::UrlSearchParams::new_with_str(&window().location().search().unwrap_or_default())?;
    let seed = params.get("seed").and_then(|s| s.parse::<u32>().ok());
    let mut store = LocalStore(window().local_storage().ok().flatten());
    if params.has("reset") {
        store.clear();
    }
    // Desktop Chrome exposes `navigator.vibrate` without any motor behind it, so only touch-first devices count.
    let can_vibrate = js_sys::Reflect::has(&window().navigator(), &"vibrate".into()).unwrap_or(false) && media("(pointer: coarse)");
    let mut platform = Web { can_vibrate };
    let t = now();
    let app = App::new(Box::new(store), Box::new(Web { can_vibrate }), seed, t);
    platform.style_changed(&app.scene.style);

    let shell = Rc::new(RefCell::new(WebShell { app, canvas, element: element.clone(), dpr: 1.0, frame_requested: false, timer: None, last_frame: t, overlay: None }));
    shell.borrow_mut().resize();
    SHELL.with(|s| *s.borrow_mut() = Some(shell.clone()));

    FRAME.with(|f| {
        *f.borrow_mut() = Some(Closure::new(move |_t: f64| {
            let Some(shell) = self::shell() else { return };
            {
                let mut s = shell.borrow_mut();
                s.frame_requested = false;
                // performance.now() rather than the callback time, so a replaced clock (promo capture) is honoured.
                s.frame(now());
            }
            schedule(&shell);
        }));
    });
    TIMER.with(|cb| {
        *cb.borrow_mut() = Some(Closure::new(move || {
            with_shell(|s| s.timer = None);
        }));
    });

    let target: &web_sys::EventTarget = element.as_ref();
    listen(target, "pointerdown", move |e: PointerEvent| {
        with_shell(|s| {
            e.prevent_default();
            // Only the main mouse button drags; right and middle clicks do nothing.
            if e.pointer_type() == "mouse" && e.button() != 0 {
                return;
            }
            // Synthetic or already-released pointers can't be captured; dragging still works.
            let _ = s.element.set_pointer_capture(e.pointer_id());
            let (x, y) = s.point(&e);
            let kind = if e.pointer_type() == "mouse" { PointerKind::Mouse } else { PointerKind::Touch };
            s.app.pointer_down(e.pointer_id() as u64, kind, x, y);
        });
    });
    listen(target, "pointermove", move |e: PointerEvent| {
        with_shell(|s| {
            let (x, y) = s.point(&e);
            s.app.pointer_move(e.pointer_id() as u64, x, y);
        });
    });
    listen(target, "pointerup", move |e: PointerEvent| {
        with_shell(|s| {
            let (x, y) = s.point(&e);
            s.app.pointer_up(e.pointer_id() as u64, x, y);
        });
    });
    listen(target, "pointercancel", move |e: PointerEvent| {
        with_shell(|s| s.app.pointer_cancel(e.pointer_id() as u64));
    });
    // Losing the capture without a pointerup (the browser took over the gesture) ends the drag;
    // after a normal release this finds nothing to cancel.
    listen(target, "lostpointercapture", move |e: PointerEvent| {
        with_shell(|s| s.app.pointer_cancel(e.pointer_id() as u64));
    });
    listen(target, "contextmenu", move |e: web_sys::Event| e.prevent_default());
    listen(target, "wheel", move |e: WheelEvent| {
        e.prevent_default();
        // Firefox reports mouse wheels in lines, some devices in pages.
        let unit = match e.delta_mode() {
            WheelEvent::DOM_DELTA_LINE => 40.0,
            WheelEvent::DOM_DELTA_PAGE => window().inner_height().ok().and_then(|h| h.as_f64()).unwrap_or(800.0) * 0.8,
            _ => 1.0,
        };
        with_shell(|s| s.app.wheel((e.delta_y() * unit) as f32));
    });
    let win: web_sys::EventTarget = window().into();
    listen(&win, "keydown", move |e: KeyboardEvent| {
        if e.key() == "Escape" {
            with_shell(|s| s.app.key_escape());
        }
    });
    listen(&win, "resize", move |_: web_sys::Event| {
        with_shell(|s| s.resize());
    });
    // Switching away mid-drag may swallow the pointerup: drop what was held.
    listen(&win, "blur", move |_: web_sys::Event| {
        with_shell(|s| s.app.cancel_input());
    });
    listen(&win, "pagehide", move |_: web_sys::Event| {
        with_shell(|s| s.app.save());
    });
    let doc_target: web_sys::EventTarget = doc.clone().into();
    listen(&doc_target, "visibilitychange", move |_: web_sys::Event| {
        let hidden = window().document().is_some_and(|d| d.hidden());
        with_shell(|s| s.app.set_visible(!hidden));
    });
    // The canvas also changes size without a window resize (mobile toolbars, rotation).
    let observer_cb = Closure::<dyn FnMut()>::new(move || {
        with_shell(|s| s.resize());
    });
    if let Ok(observer) = web_sys::ResizeObserver::new(observer_cb.as_ref().unchecked_ref()) {
        observer.observe(&element);
        std::mem::forget(observer);
    }
    observer_cb.forget();

    schedule(&shell);
    if let Some(body) = doc.body() {
        body.dataset().set("ready", "true")?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Test and tool API, wrapped as `window.blockBaster` by index.html.
// ---------------------------------------------------------------------------

fn rect_json(app: &App, id: UiId) -> Value {
    app.ui.rect_of(id).map_or(Value::Null, |h| json!({ "x": h.x, "y": h.y, "w": h.w, "h": h.h, "cx": h.x + h.w / 2.0, "cy": h.y + h.h / 2.0 }))
}

fn state(app: &mut App) -> Value {
    let l = app.layout;
    let g = &app.game;
    let music = app.audio.music_state();
    let style = app.scene.style;
    json!({
        "score": g.score,
        "shownScore": app.shown_score,
        "best": app.best,
        "bestAtStart": app.best_at_start,
        "moves": g.moves,
        "combo": g.combo,
        "over": g.over,
        "lucky": g.lucky,
        "boardClears": g.board_clears,
        "board": g.board.to_vec(),
        "tray": g.tray.iter().map(|p| p.map(|p| json!({
            "id": p.shape.id, "rows": p.shape.rows, "cols": p.shape.cols, "color": p.color,
            "cells": p.shape.cells.iter().map(|&(r, c)| [r, c]).collect::<Vec<_>>(),
        }))).collect::<Vec<_>>(),
        "layout": {
            "width": l.width, "height": l.height, "cell": l.cell, "boardX": l.board_x, "boardY": l.board_y,
            "boardSize": l.board_size, "trayCell": l.tray_cell,
            "slots": l.slots.iter().map(|s| json!({ "cx": s.cx, "cy": s.cy, "x": s.x, "y": s.y, "w": s.w, "h": s.h })).collect::<Vec<_>>(),
        },
        "settings": { "sound": app.settings.sound, "music": app.settings.music, "haptics": app.settings.haptics },
        "style": { "key": style.key(), "tier": style.tier.level, "name": format!("{} · {}", style.tier.name, style.palette.name) },
        "overlay": match app.overlay() { Overlay::None => "none", Overlay::Menu => "menu", Overlay::GameOver => "gameover" },
        "record": app.ui.game_over.as_ref().is_some_and(|g| g.record),
        "helpOpen": app.ui.help_open,
        "dragging": app.dragging(),
        "scroll": app.ui.scroll,
        "canScroll": app.ui.can_scroll(),
        "popping": app.ui.popping(app.now()),
        "music": { "playing": app.audio.music_playing(), "bpm": music.bpm, "e": music.e, "volume": app.audio.music_volume() },
        "soundVolume": app.audio.sound_volume(),
        "stats": app.debug_stats(),
        "ui": {
            "menuButton": rect_json(app, UiId::MenuButton),
            "resume": rect_json(app, UiId::Resume),
            "restart": rect_json(app, UiId::Restart),
            "again": rect_json(app, UiId::Again),
            "exit": rect_json(app, UiId::Exit),
            "help": rect_json(app, UiId::Help),
            "music": rect_json(app, UiId::Slider(SettingKey::Music)),
            "sound": rect_json(app, UiId::Slider(SettingKey::Sound)),
            "haptics": rect_json(app, UiId::Slider(SettingKey::Haptics)),
        },
    })
}

fn setting_key(name: &str) -> Option<SettingKey> {
    match name {
        "sound" => Some(SettingKey::Sound),
        "music" => Some(SettingKey::Music),
        "haptics" => Some(SettingKey::Haptics),
        _ => None,
    }
}

fn command(app: &mut App, cmd: &Value) -> Result<Value, String> {
    let name = cmd["cmd"].as_str().ok_or("missing cmd")?;
    let num = |k: &str| cmd[k].as_f64().ok_or_else(|| format!("missing {k}"));
    match name {
        "state" => Ok(state(app)),
        "canPlace" => Ok(json!(app.game.can_place(num("slot")? as usize, num("r")? as i32, num("c")? as i32))),
        "commit" => Ok(app.commit(num("slot")? as usize, num("r")? as i32, num("c")? as i32).map_or(Value::Null, |r| {
            json!({ "points": r.points, "lines": r.lines.count(), "combo": r.combo, "boardCleared": r.board_cleared, "gameOver": r.game_over, "refilled": r.refilled })
        })),
        "setTray" => {
            let ids: Vec<Option<String>> = serde_json::from_value(cmd["ids"].clone()).map_err(|e| e.to_string())?;
            for id in ids.iter().flatten() {
                shape_by_id(id).ok_or_else(|| format!("unknown shape {id}"))?;
            }
            let refs: Vec<Option<&str>> = ids.iter().map(|s| s.as_deref()).collect();
            app.game.set_tray(&refs);
            if let Some(colors) = cmd["colors"].as_array() {
                for (slot, c) in colors.iter().enumerate() {
                    if let (Some(p), Some(c)) = (app.game.tray.get_mut(slot).and_then(|p| p.as_mut()), c.as_u64()) {
                        p.color = c as u8;
                    }
                }
            }
            app.game.over = false;
            app.sync();
            Ok(Value::Null)
        }
        "setBoard" => {
            let cells: Vec<u8> = serde_json::from_value(cmd["board"].clone()).map_err(|e| e.to_string())?;
            if cells.len() != SIZE * SIZE {
                return Err("board needs 64 cells".into());
            }
            app.game.board.copy_from_slice(&cells);
            app.sync();
            Ok(Value::Null)
        }
        "set" => {
            // Plain numbers of the game and the app.
            if let Some(v) = cmd["score"].as_u64() {
                app.game.score = v as u32;
            }
            if let Some(v) = cmd["combo"].as_u64() {
                app.game.combo = v as u32;
            }
            if let Some(v) = cmd["moves"].as_u64() {
                app.game.moves = v as u32;
            }
            if let Some(v) = cmd["best"].as_u64() {
                app.best = v as u32;
            }
            if let Some(v) = cmd["bestAtStart"].as_u64() {
                app.best_at_start = v as u32;
            }
            app.sync();
            Ok(Value::Null)
        }
        "settle" => {
            app.settle_hud();
            Ok(Value::Null)
        }
        "updateStyle" => {
            app.update_style(cmd["delay"].as_f64().unwrap_or(0.0));
            Ok(Value::Null)
        }
        "applyStyle" => {
            app.apply_style(style_for(app.game.score, app.game.board_clears));
            Ok(Value::Null)
        }
        "newGame" => {
            app.new_game(cmd["seed"].as_u64().map(|s| s as u32));
            Ok(Value::Null)
        }
        "serialize" => serde_json::to_value(app.game.serialize()).map_err(|e| e.to_string()),
        "setSetting" => {
            let key = setting_key(cmd["key"].as_str().unwrap_or_default()).ok_or("unknown setting")?;
            app.set_setting(key, num("value")? as f32);
            app.commit_level(key);
            Ok(Value::Null)
        }
        "openMenu" => {
            app.open_menu();
            Ok(Value::Null)
        }
        "closeMenu" => {
            app.close_menu();
            Ok(Value::Null)
        }
        "save" => {
            app.save();
            Ok(Value::Null)
        }
        "invalidate" => {
            app.invalidate();
            Ok(Value::Null)
        }
        other => Err(format!("unknown command {other}")),
    }
}

/// Runs a JSON command (see `command`) and returns `{"ok": result}` or `{"error": message}` as JSON.
#[wasm_bindgen]
pub fn bb_command(json: &str) -> String {
    let parsed: Result<Value, String> = serde_json::from_str(json).map_err(|e| e.to_string());
    let result = with_shell(|s| parsed.and_then(|cmd| command(&mut s.app, &cmd))).unwrap_or_else(|| Err("not started".into()));
    match result {
        Ok(v) => json!({ "ok": v }).to_string(),
        Err(e) => json!({ "error": e }).to_string(),
    }
}
