import type { Sound } from './audio/sound';
import { idx, previewLines } from './game/board';
import { Game, type PlaceResult } from './game/game';
import { shapeById } from './game/shapes';
import { exitApp, isNative, onBackButton, vibrate } from './platform';
import { type Layout, computeLayout, slotAt } from './render/layout';
import { colorFor } from './render/palette';
import { CLEAR_DURATION, GAME_OVER_FILL, type DragView, Renderer, type Scene, trayPiecePosition } from './render/renderer';
import { clearSpriteCache } from './render/sprites';
import { type Settings, storage } from './storage';

interface DragState {
  slot: number;
  pointerId: number;
  /** Pointer position. */
  px: number;
  py: number;
  /** Offset from the pointer to the piece's top-left corner at full size. */
  offX: number;
  offY: number;
  /** Where the piece started (in the tray), for the pick-up animation. */
  from: { x: number; y: number; cell: number };
  start: number;
}

const PICKUP_MS = 110;
const PRAISE = ['', '', 'Good!', 'Great!', 'Excellent!', 'Amazing!', 'Unbelievable!'];

function $(id: string): HTMLElement {
  return document.getElementById(id)!;
}

export class BlockBasterApp {
  game: Game;
  layout: Layout;
  best: number;
  settings: Settings;

  private renderer: Renderer;
  private scene: Scene;
  private drag: DragState | null = null;
  private shownScore = 0;
  private locked = false;
  private frame = 0;
  private gameOverTimer = 0;
  private bestAtStart = 0;

  constructor(
    private canvas: HTMLCanvasElement,
    private sound: Sound,
    seed?: number,
  ) {
    this.settings = storage.loadSettings();
    this.sound.enabled = this.settings.sound;
    this.best = storage.loadBest();
    this.game = this.restoreOrCreate(seed);
    this.bestAtStart = this.game.moves > 0 ? (storage.loadBestAtStart() ?? this.best) : this.best;
    storage.saveBestAtStart(this.bestAtStart);
    this.shownScore = this.game.score;
    this.renderer = new Renderer(canvas);
    this.layout = computeLayout(canvas.clientWidth || 360, canvas.clientHeight || 600);
    this.scene = {
      layout: this.layout,
      dpr: window.devicePixelRatio || 1,
      now: performance.now(),
      board: this.game.board,
      tray: this.game.tray,
      playable: [],
      hiddenSlot: -1,
      drag: null,
      flying: null,
      trayDealtAt: performance.now(),
      placed: null,
      clearing: null,
      particles: [],
      texts: [],
      shake: null,
      gameOverAt: null,
    };
    this.syncScene();
    this.bindInput();
    this.bindUi();
    this.resize();
    new ResizeObserver(() => this.resize()).observe(canvas.parentElement!);
    this.updateHud(true);
  }

  private restoreOrCreate(seed?: number): Game {
    if (seed === undefined) {
      const saved = storage.loadGame();
      if (saved && !saved.over) {
        try {
          return Game.deserialize(saved);
        } catch {
          // Fall through to a fresh game when the save is corrupt or outdated.
        }
      }
    }
    return new Game(seed);
  }

  // ---------------------------------------------------------------------------
  // Rendering loop
  // ---------------------------------------------------------------------------

  /** Requests a frame; the loop keeps running only while something animates. */
  invalidate(): void {
    if (!this.frame) this.frame = requestAnimationFrame((t) => this.tick(t));
  }

  private tick(now: number): void {
    this.frame = 0;
    const s = this.scene;
    s.now = now;
    s.drag = this.dragView(now);
    this.renderer.render(s);
    this.updateHud();
    if (this.renderer.isAnimating(s) || this.drag || this.shownScore !== this.game.score) this.invalidate();
  }

  private resize(): void {
    const rect = this.canvas.getBoundingClientRect();
    this.layout = computeLayout(rect.width, rect.height);
    const dpr = Math.min(3, window.devicePixelRatio || 1);
    if (dpr !== this.scene.dpr || this.layout.cell !== this.scene.layout.cell) clearSpriteCache();
    this.scene.layout = this.layout;
    this.scene.dpr = dpr;
    this.invalidate();
  }

  private syncScene(): void {
    this.scene.board = this.game.board;
    this.scene.tray = this.game.tray;
    this.scene.playable = this.game.tray.map((_, i) => this.game.isPlayable(i));
  }

  private updateHud(force = false): void {
    const target = this.game.score;
    if (this.shownScore !== target || force) {
      const diff = target - this.shownScore;
      this.shownScore = Math.abs(diff) <= 1 ? target : this.shownScore + Math.ceil(Math.abs(diff) * 0.18) * Math.sign(diff);
      $('score').textContent = String(this.shownScore);
    }
    $('best').textContent = String(Math.max(this.best, this.game.score));
  }

  // ---------------------------------------------------------------------------
  // Dragging
  // ---------------------------------------------------------------------------

  private bindInput(): void {
    const c = this.canvas;
    const point = (e: PointerEvent) => {
      const r = c.getBoundingClientRect();
      return { x: e.clientX - r.left, y: e.clientY - r.top };
    };
    c.addEventListener('pointerdown', (e) => {
      this.sound.unlock();
      if (this.drag || this.locked || this.game.over) return;
      const { x, y } = point(e);
      const slot = slotAt(this.layout, x, y);
      const piece = slot >= 0 ? this.game.tray[slot] : null;
      if (!piece) return;
      e.preventDefault();
      try {
        c.setPointerCapture(e.pointerId);
      } catch {
        // Synthetic or already-released pointers can't be captured; dragging still works.
      }
      const cell = this.layout.cell;
      const w = piece.shape.cols * cell;
      const h = piece.shape.rows * cell;
      // Touch: lift the piece above the finger so it stays visible. Mouse: grab the centre.
      const lift = e.pointerType === 'mouse' ? -h / 2 : -h - cell * 1.1;
      this.drag = {
        slot,
        pointerId: e.pointerId,
        px: x,
        py: y,
        offX: -w / 2,
        offY: lift,
        from: trayPiecePosition(this.layout, slot, piece.shape),
        start: performance.now(),
      };
      this.scene.hiddenSlot = slot;
      this.scene.flying = null;
      this.sound.pickup();
      this.invalidate();
    });
    c.addEventListener('pointermove', (e) => {
      if (!this.drag || e.pointerId !== this.drag.pointerId) return;
      const { x, y } = point(e);
      this.drag.px = x;
      this.drag.py = y;
      this.invalidate();
    });
    const end = (e: PointerEvent, cancelled: boolean) => {
      if (!this.drag || e.pointerId !== this.drag.pointerId) return;
      const { x, y } = point(e);
      this.drag.px = x;
      this.drag.py = y;
      this.drop(cancelled);
    };
    c.addEventListener('pointerup', (e) => end(e, false));
    c.addEventListener('pointercancel', (e) => end(e, true));
    c.addEventListener('contextmenu', (e) => e.preventDefault());
  }

  /** Top-left of the dragged piece at full size (where it will snap from). */
  private dragTopLeft(d: DragState) {
    return { x: d.px + d.offX, y: d.py + d.offY };
  }

  private snapTarget(d: DragState) {
    const piece = this.game.tray[d.slot];
    if (!piece) return null;
    const { x, y } = this.dragTopLeft(d);
    const c = Math.round((x - this.layout.boardX) / this.layout.cell);
    const r = Math.round((y - this.layout.boardY) / this.layout.cell);
    if (!this.game.canPlace(d.slot, r, c)) return null;
    return { r, c };
  }

  private dragView(now: number): DragView | null {
    const d = this.drag;
    if (!d) return null;
    const piece = this.game.tray[d.slot];
    if (!piece) return null;
    const t = Math.min(1, (now - d.start) / PICKUP_MS);
    const to = this.dragTopLeft(d);
    const lerp = (a: number, b: number) => a + (b - a) * t;
    const target = this.snapTarget(d);
    return {
      piece,
      x: lerp(d.from.x, to.x),
      y: lerp(d.from.y, to.y),
      cell: lerp(d.from.cell, this.layout.cell),
      target: target ? { ...target, lines: previewLines(this.game.board, piece.shape, target.r, target.c) } : null,
    };
  }

  private drop(cancelled: boolean): void {
    const d = this.drag!;
    const view = this.dragView(performance.now());
    this.drag = null;
    this.scene.hiddenSlot = -1;
    const target = cancelled ? null : this.snapTarget(d);
    if (target) {
      this.commit(d.slot, target.r, target.c);
    } else if (view) {
      // Fly back to the tray.
      this.scene.flying = { piece: view.piece, from: { x: view.x, y: view.y, cell: view.cell }, slot: d.slot, start: performance.now(), duration: 200 };
      const { x, y } = this.dragTopLeft(d);
      const overBoard =
        x + view.piece.shape.cols * this.layout.cell > this.layout.boardX && x < this.layout.boardX + this.layout.boardSize && y < this.layout.boardY + this.layout.boardSize;
      if (overBoard) this.sound.invalid();
    }
    this.invalidate();
  }

  // ---------------------------------------------------------------------------
  // Moves & effects
  // ---------------------------------------------------------------------------

  /** Places the piece in `slot` at (r, c). Public so tests and tools can drive the game. */
  commit(slot: number, r: number, c: number): PlaceResult | null {
    const res = this.game.place(slot, r, c);
    if (!res) return null;
    const now = performance.now();
    const s = this.scene;
    const { cell, boardX, boardY } = this.layout;

    s.placed = { cells: new Set(res.piece.shape.cells.map(([dr, dc]) => idx(r + dr, c + dc))), start: now };
    const cx = boardX + (c + res.piece.shape.cols / 2) * cell;
    const cy = boardY + (r + res.piece.shape.rows / 2) * cell;
    const lines = res.lines.rows.length + res.lines.cols.length;

    this.sound.place();
    if (this.settings.haptics) vibrate(lines ? 'medium' : 'light');

    if (lines > 0) {
      const pr = r + res.piece.shape.rows / 2;
      const pc = c + res.piece.shape.cols / 2;
      s.clearing = {
        start: now,
        cells: res.cleared.map((cc) => ({ ...cc, delay: Math.hypot(cc.r + 0.5 - pr, cc.c + 0.5 - pc) * 28 })),
      };
      for (const cc of res.cleared) this.burst(boardX + (cc.c + 0.5) * cell, boardY + (cc.r + 0.5) * cell, colorFor(cc.color), now);
      if (lines >= 3) s.shake = { start: now, power: Math.min(10, lines * 2.5) };
      this.sound.clear(lines, res.combo);
      if (this.settings.haptics && lines >= 3) vibrate('heavy');

      const mid = boardY + this.layout.boardSize / 2;
      s.texts.push({ text: `+${res.points}`, x: cx, y: cy, size: cell * 0.7, color: '#ffffff', start: now, duration: 900 });
      if (res.boardCleared) {
        s.texts.push({ text: 'Board Clear!', x: this.layout.width / 2, y: mid, size: cell * 0.95, color: '#ffe066', start: now + 150, duration: 1400, banner: true });
        this.sound.boardClear();
      } else if (lines >= 2) {
        const word = PRAISE[Math.min(lines, PRAISE.length - 1)];
        s.texts.push({ text: word, x: this.layout.width / 2, y: mid, size: cell * 0.95, color: '#ffe066', start: now + 80, duration: 1100, banner: true });
      }
      if (res.combo >= 2) {
        s.texts.push({ text: `Combo ${res.combo}`, x: this.layout.width / 2, y: mid + cell * 1.05, size: cell * 0.62, color: '#7ee8ff', start: now + 80, duration: 1100, banner: true });
      }
    }

    if (res.refilled) s.trayDealtAt = now + (lines ? 120 : 0);
    this.syncScene();

    if (this.game.score > this.best) {
      this.best = this.game.score;
      storage.saveBest(this.best);
    }
    storage.saveGame(this.game.over ? null : this.game.serialize());

    if (res.gameOver) this.startGameOver(lines ? CLEAR_DURATION + 250 : 300);
    this.invalidate();
    return res;
  }

  private burst(x: number, y: number, color: string, now: number): void {
    for (let i = 0; i < 4; i++) {
      const a = Math.random() * Math.PI * 2;
      const v = 80 + Math.random() * 220;
      this.scene.particles.push({
        x,
        y,
        vx: Math.cos(a) * v,
        vy: Math.sin(a) * v - 220,
        size: this.layout.cell * (0.14 + Math.random() * 0.14),
        color,
        born: now + Math.random() * 120,
        life: 550 + Math.random() * 300,
      });
    }
  }

  private startGameOver(delay: number): void {
    this.locked = true;
    const record = this.game.score > this.bestAtStart;
    this.gameOverTimer = window.setTimeout(() => {
      this.scene.gameOverAt = performance.now();
      this.sound.gameOver();
      if (this.settings.haptics) vibrate('heavy');
      this.invalidate();
      this.gameOverTimer = window.setTimeout(() => this.showGameOver(record), GAME_OVER_FILL + 500);
    }, delay);
  }

  private showGameOver(record: boolean): void {
    $('final-score').textContent = String(this.game.score);
    $('final-best').textContent = String(this.best);
    $('record').hidden = !record;
    $('gameover').hidden = false;
    ($('again-btn') as HTMLButtonElement).focus({ preventScroll: true });
  }

  newGame(seed?: number): void {
    clearTimeout(this.gameOverTimer);
    this.game = new Game(seed);
    this.bestAtStart = this.best;
    storage.saveBestAtStart(this.best);
    this.locked = false;
    this.drag = null;
    const s = this.scene;
    Object.assign(s, { hiddenSlot: -1, flying: null, placed: null, clearing: null, particles: [], texts: [], shake: null, gameOverAt: null });
    s.trayDealtAt = performance.now();
    this.shownScore = 0;
    this.syncScene();
    storage.saveGame(this.game.serialize());
    $('gameover').hidden = true;
    $('menu').hidden = true;
    this.updateHud(true);
    this.invalidate();
  }

  /** Replaces the tray with the given shape ids (for tests and debugging). */
  debugSetTray(ids: (string | null)[]): void {
    this.game.tray = ids.map((id, i) => (id ? { shape: shapeById(id), color: i + 1 } : null));
    this.syncScene();
    this.invalidate();
  }

  save(): void {
    if (!this.game.over) storage.saveGame(this.game.serialize());
  }

  // ---------------------------------------------------------------------------
  // Menus
  // ---------------------------------------------------------------------------

  private bindUi(): void {
    const menu = $('menu');
    const sound = $('sound-toggle') as HTMLInputElement;
    const haptics = $('haptics-toggle') as HTMLInputElement;
    sound.checked = this.settings.sound;
    haptics.checked = this.settings.haptics;
    $('version').textContent = __APP_VERSION__;
    $('exit-btn').hidden = !isNative;

    const openMenu = () => {
      menu.hidden = false;
      ($('resume-btn') as HTMLButtonElement).focus({ preventScroll: true });
    };
    const closeMenu = () => (menu.hidden = true);

    $('menu-btn').addEventListener('click', openMenu);
    $('resume-btn').addEventListener('click', closeMenu);
    $('restart-btn').addEventListener('click', () => this.newGame());
    $('again-btn').addEventListener('click', () => this.newGame());
    $('exit-btn').addEventListener('click', () => exitApp());
    menu.addEventListener('click', (e) => e.target === menu && closeMenu());
    sound.addEventListener('change', () => {
      this.settings.sound = this.sound.enabled = sound.checked;
      storage.saveSettings(this.settings);
      if (sound.checked) {
        this.sound.unlock();
        this.sound.pickup();
      }
    });
    haptics.addEventListener('change', () => {
      this.settings.haptics = haptics.checked;
      storage.saveSettings(this.settings);
      if (haptics.checked) vibrate('medium');
    });
    window.addEventListener('keydown', (e) => {
      if (e.key !== 'Escape') return;
      if (menu.hidden) openMenu();
      else closeMenu();
    });

    onBackButton(() => {
      if (!menu.hidden) closeMenu();
      else if (!$('gameover').hidden) exitApp();
      else openMenu();
    });

    document.addEventListener('visibilitychange', () => {
      if (document.hidden) {
        this.save();
        this.sound.suspend();
      }
    });
    window.addEventListener('pagehide', () => this.save());
  }
}

