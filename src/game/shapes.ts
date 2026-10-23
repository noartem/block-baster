/** A cell offset inside a shape: [row, col]. */
export type Cell = readonly [number, number];

export interface Shape {
  /** Unique id of this exact orientation, e.g. "L4-2". */
  id: string;
  /** Id of the shape family, shared by all rotations. */
  family: string;
  cells: readonly Cell[];
  rows: number;
  cols: number;
  /** One bitmask per shape row (bit c set = cell at column c). */
  rowMasks: readonly number[];
  /** Relative probability of this orientation being dealt. */
  weight: number;
  /** 0 = small/easy, 1 = medium, 2 = large/awkward. */
  tier: 0 | 1 | 2;
}

interface Family {
  family: string;
  pattern: string[];
  /** Total weight of the family, split evenly over its distinct rotations. */
  weight: number;
  tier: 0 | 1 | 2;
  /** Whether to generate all distinct rotations (true) or keep the pattern as-is. */
  rotate: boolean;
}

const FAMILIES: Family[] = [
  { family: 'dot', pattern: ['#'], weight: 3, tier: 0, rotate: false },
  { family: 'I2', pattern: ['##'], weight: 6, tier: 0, rotate: true },
  { family: 'I3', pattern: ['###'], weight: 8, tier: 0, rotate: true },
  { family: 'I4', pattern: ['####'], weight: 8, tier: 1, rotate: true },
  { family: 'I5', pattern: ['#####'], weight: 4, tier: 2, rotate: true },
  { family: 'O2', pattern: ['##', '##'], weight: 8, tier: 0, rotate: false },
  { family: 'O3', pattern: ['###', '###', '###'], weight: 3, tier: 2, rotate: false },
  { family: 'R23', pattern: ['###', '###'], weight: 5, tier: 1, rotate: true },
  { family: 'V3', pattern: ['#.', '##'], weight: 8, tier: 0, rotate: true },
  { family: 'L4', pattern: ['#.', '#.', '##'], weight: 7, tier: 1, rotate: true },
  { family: 'J4', pattern: ['.#', '.#', '##'], weight: 7, tier: 1, rotate: true },
  { family: 'T4', pattern: ['###', '.#.'], weight: 7, tier: 1, rotate: true },
  { family: 'S4', pattern: ['.##', '##.'], weight: 5, tier: 1, rotate: true },
  { family: 'Z4', pattern: ['##.', '.##'], weight: 5, tier: 1, rotate: true },
  { family: 'V5', pattern: ['#..', '#..', '###'], weight: 4, tier: 2, rotate: true },
];

function parse(pattern: string[]): Cell[] {
  const cells: Cell[] = [];
  pattern.forEach((line, r) => {
    for (let c = 0; c < line.length; c++) if (line[c] === '#') cells.push([r, c]);
  });
  return cells;
}

function normalize(cells: Cell[]): Cell[] {
  const minR = Math.min(...cells.map((c) => c[0]));
  const minC = Math.min(...cells.map((c) => c[1]));
  return cells
    .map(([r, c]) => [r - minR, c - minC] as Cell)
    .sort((a, b) => a[0] - b[0] || a[1] - b[1]);
}

function rotate(cells: Cell[]): Cell[] {
  // 90° clockwise: (r, c) -> (c, -r)
  return normalize(cells.map(([r, c]) => [c, -r] as Cell));
}

const key = (cells: Cell[]) => cells.map((c) => c.join(',')).join(';');

function makeShape(f: Family, cells: Cell[], index: number, weight: number): Shape {
  const rows = Math.max(...cells.map((c) => c[0])) + 1;
  const cols = Math.max(...cells.map((c) => c[1])) + 1;
  const rowMasks = new Array<number>(rows).fill(0);
  for (const [r, c] of cells) rowMasks[r] |= 1 << c;
  return { id: `${f.family}-${index}`, family: f.family, cells, rows, cols, rowMasks, weight, tier: f.tier };
}

function buildShapes(): Shape[] {
  const shapes: Shape[] = [];
  for (const f of FAMILIES) {
    const variants: Cell[][] = [];
    let cells = normalize(parse(f.pattern));
    const seen = new Set<string>();
    for (let i = 0; i < (f.rotate ? 4 : 1); i++) {
      if (!seen.has(key(cells))) {
        seen.add(key(cells));
        variants.push(cells);
      }
      cells = rotate(cells);
    }
    variants.forEach((v, i) => shapes.push(makeShape(f, v, i, f.weight / variants.length)));
  }
  return shapes;
}

export const SHAPES: readonly Shape[] = buildShapes();

const BY_ID = new Map(SHAPES.map((s) => [s.id, s]));

export function shapeById(id: string): Shape {
  const shape = BY_ID.get(id);
  if (!shape) throw new Error(`Unknown shape: ${id}`);
  return shape;
}
