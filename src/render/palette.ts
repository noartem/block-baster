import type { Palette } from './styles';

export const GRAY = '#7a8296';

/** Block colour for a 1-based colour id; 0 is the grey used for unplayable pieces and game over. */
export function colorFor(id: number, palette: Palette): string {
  return id <= 0 ? GRAY : palette.blocks[(id - 1) % palette.blocks.length];
}

function parse(hex: string): [number, number, number] {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

/** Mixes a colour towards white (amount > 0) or black (amount < 0). */
export function shade(hex: string, amount: number): string {
  const target = amount > 0 ? 255 : 0;
  const t = Math.abs(amount);
  const [r, g, b] = parse(hex).map((v) => Math.round(v + (target - v) * t));
  return `rgb(${r},${g},${b})`;
}

/** Like `shade`, but returns a hex colour so the result can be shaded again. */
export function shadeHex(hex: string, amount: number): string {
  const target = amount > 0 ? 255 : 0;
  const t = Math.abs(amount);
  const n = parse(hex).reduce((acc, v) => (acc << 8) | Math.round(v + (target - v) * t), 0);
  return `#${n.toString(16).padStart(6, '0')}`;
}

export function rgba(hex: string, alpha: number): string {
  const [r, g, b] = parse(hex);
  return `rgba(${r},${g},${b},${alpha})`;
}
