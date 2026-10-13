/** Block colours, indexed by colour id - 1. */
export const BLOCK_COLORS = ['#ef4b5a', '#f7962f', '#f6cd30', '#4cc85d', '#2fbde8', '#4f6ef2', '#a75ae6'];
export const GRAY = '#7a8296';

export const THEME = {
  board: '#1b2556',
  boardEdge: '#121a40',
  cell: '#232f68',
  cellInner: '#1f2a5f',
  text: '#ffffff',
  font: "'Baloo 2', system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif",
};

export function colorFor(id: number): string {
  return id <= 0 ? GRAY : BLOCK_COLORS[(id - 1) % BLOCK_COLORS.length];
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

export function rgba(hex: string, alpha: number): string {
  const [r, g, b] = parse(hex);
  return `rgba(${r},${g},${b},${alpha})`;
}
