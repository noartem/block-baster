import { SIZE } from '../game/board';
import { TRAY_SIZE } from '../game/generator';

export interface Slot {
  /** Centre of the slot. */
  cx: number;
  cy: number;
  /** Hit area. */
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Layout {
  width: number;
  height: number;
  cell: number;
  boardX: number;
  boardY: number;
  boardSize: number;
  /** Size of one block when a piece rests in the tray. */
  trayCell: number;
  slots: Slot[];
}

/** Computes positions of the board and tray for a canvas of the given CSS size. */
export function computeLayout(width: number, height: number): Layout {
  const pad = Math.max(10, Math.min(width, height) * 0.035);
  const gap = Math.max(12, height * 0.03);
  // The tray needs roughly 3.3 board cells of height to show 5-long pieces at ~60% scale.
  const byWidth = width - pad * 2;
  const byHeight = (height - pad * 2 - gap) / (1 + 3.3 / SIZE);
  const boardSize = Math.max(SIZE * 8, Math.floor(Math.min(byWidth, byHeight, 640) / SIZE) * SIZE);
  const cell = boardSize / SIZE;
  const trayH = cell * 3.3;
  const total = boardSize + gap + trayH;
  const boardX = Math.round((width - boardSize) / 2);
  const boardY = Math.round(Math.max(pad, (height - total) / 2 - height * 0.02));
  const trayTop = boardY + boardSize + gap;
  const slotW = Math.min(width / TRAY_SIZE, (boardSize * 1.12) / TRAY_SIZE);
  const trayLeft = (width - slotW * TRAY_SIZE) / 2;
  const trayCell = Math.min(cell * 0.6, (slotW * 0.88) / 5);
  const slots: Slot[] = [];
  for (let i = 0; i < TRAY_SIZE; i++) {
    const x = trayLeft + slotW * i;
    slots.push({ cx: x + slotW / 2, cy: trayTop + trayH / 2, x, y: trayTop - gap / 2, w: slotW, h: trayH + gap });
  }
  return { width, height, cell, boardX, boardY, boardSize, trayCell, slots };
}

/** Index of the tray slot under the point, or -1. */
export function slotAt(layout: Layout, x: number, y: number): number {
  return layout.slots.findIndex((s) => x >= s.x && x < s.x + s.w && y >= s.y && y < s.y + s.h);
}
