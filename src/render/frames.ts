import { shade } from './palette';
import { rivet, rng, roundRectLength, roundRectPoint, wavyRoundRect } from './sprites';
import type { FrameKind, Palette } from './styles';

/**
 * Board frames made of a material (cookie, cardboard, iron, wood). They are drawn
 * as a band around the board surface, reaching slightly into the margin.
 */
export interface FrameBox {
  /** Outer size of the framed board in CSS pixels; the frame starts at 0. */
  size: number;
  /** Where the board surface starts. */
  inset: number;
  cell: number;
  outerR: number;
  innerR: number;
  palette: Palette;
}

type Ctx = CanvasRenderingContext2D;
type Box = FrameBox & { ext: number; band: number; mid: number };

/** Adds a rounded rectangle to the current path without starting a new one. */
function addRoundRect(g: Ctx, x: number, y: number, w: number, h: number, r: number) {
  g.moveTo(x + r, y);
  g.arcTo(x + w, y, x + w, y + h, r);
  g.arcTo(x + w, y + h, x, y + h, r);
  g.arcTo(x, y + h, x, y, r);
  g.arcTo(x, y, x + w, y, r);
  g.closePath();
}

/** The path of the frame band, between the outer edge and the board surface. */
function bandPath(g: Ctx, b: Box) {
  g.beginPath();
  addRoundRect(g, -b.ext, -b.ext, b.size + b.ext * 2, b.size + b.ext * 2, b.outerR + b.ext);
  addRoundRect(g, b.inset, b.inset, b.size - b.inset * 2, b.size - b.inset * 2, b.innerR);
}

function surfacePath(g: Ctx, b: Box) {
  g.beginPath();
  addRoundRect(g, b.inset, b.inset, b.size - b.inset * 2, b.size - b.inset * 2, b.innerR);
}

/** Points spread along the middle of the band, with their outward normals. */
function alongBand(b: Box, count: number, offset = 0): [number, number, number, number][] {
  const m = b.mid;
  const w = b.size - m * 2;
  const r = Math.max(1, b.outerR - (m + b.ext));
  const total = roundRectLength(w, w, r);
  return Array.from({ length: count }, (_, i) => roundRectPoint(m, m, w, w, r, ((i + offset) / count) * total));
}

/** Dark line where the board surface sinks into the frame. */
function innerShadow(g: Ctx, b: Box, alpha: number) {
  g.save();
  g.lineWidth = Math.max(1, b.cell * 0.035);
  g.strokeStyle = `rgba(0,0,0,${alpha})`;
  surfacePath(g, b);
  g.stroke();
  g.restore();
}

/** A gingerbread cookie rim with wavy white icing and candy buttons. */
function gingerbread(g: Ctx, b: Box) {
  const { size, cell, palette: p } = b;
  g.save();
  bandPath(g, b);
  const dough = g.createLinearGradient(0, -b.ext, 0, size + b.ext);
  dough.addColorStop(0, '#d48c4a');
  dough.addColorStop(0.5, '#b06630');
  dough.addColorStop(1, '#84441b');
  g.fillStyle = dough;
  g.fill('evenodd');
  g.clip('evenodd');
  const rand = rng(0x91a9);
  for (let i = 0; i < 120; i++) {
    g.fillStyle = rand() < 0.75 ? 'rgba(92,40,10,0.4)' : 'rgba(255,220,170,0.35)';
    const [x, y] = alongBand(b, 1, rand())[0];
    g.beginPath();
    g.arc(x + (rand() - 0.5) * b.band, y + (rand() - 0.5) * b.band, cell * (0.008 + rand() * 0.01), 0, Math.PI * 2);
    g.fill();
  }
  g.restore();

  g.lineWidth = Math.max(1, cell * 0.03);
  g.strokeStyle = '#5a2a0c';
  g.beginPath();
  addRoundRect(g, -b.ext, -b.ext, size + b.ext * 2, size + b.ext * 2, b.outerR + b.ext);
  g.stroke();

  // Piped icing along the middle of the rim.
  const m = b.mid;
  const r = Math.max(1, b.outerR - (m + b.ext));
  g.lineCap = 'round';
  g.lineJoin = 'round';
  g.lineWidth = Math.max(1.5, b.band * 0.26);
  for (const [color, dy] of [['rgba(70,25,0,0.35)', g.lineWidth * 0.45], ['#fffaf2', 0]] as const) {
    g.save();
    g.translate(0, dy);
    g.strokeStyle = color;
    wavyRoundRect(g, m, m, size - m * 2, size - m * 2, r, b.band * 0.2, cell * 0.32);
    g.stroke();
    g.restore();
  }

  // Candy buttons at the corners and the middle of each side, in the block colours.
  const s = size - m * 2;
  const side = s - 2 * r;
  const arc = (Math.PI * r) / 2;
  const buttons = Array.from({ length: 8 }, (_, i) => {
    const k = Math.floor(i / 2);
    // Even: middle of a side; odd: middle of the following corner.
    const d = k * (side + arc) + (i % 2 ? side + arc / 2 : side / 2);
    return roundRectPoint(m, m, s, s, r, d);
  });
  buttons.forEach(([x, y], i) => {
    const color = p.blocks[(i * 3) % p.blocks.length];
    const cr = b.band * 0.42;
    g.fillStyle = 'rgba(60,20,0,0.4)';
    g.beginPath();
    g.arc(x + cr * 0.15, y + cr * 0.3, cr, 0, Math.PI * 2);
    g.fill();
    const candy = g.createRadialGradient(x - cr * 0.35, y - cr * 0.4, 0, x, y, cr);
    candy.addColorStop(0, '#ffffff');
    candy.addColorStop(0.35, color);
    candy.addColorStop(1, shade(color, -0.35));
    g.fillStyle = candy;
    g.beginPath();
    g.arc(x, y, cr, 0, Math.PI * 2);
    g.fill();
  });
  innerShadow(g, b, 0.45);
}

/** The cut edge of a cardboard box, sealed with packing tape, around a corrugated surface. */
function cardboard(g: Ctx, b: Box) {
  const { size, cell, inset } = b;
  // Corrugated flutes showing on the board surface between the cells.
  g.save();
  surfacePath(g, b);
  g.clip();
  for (let x = inset; x < size; x += cell * 0.12) {
    g.fillStyle = 'rgba(0,0,0,0.12)';
    g.fillRect(x, 0, cell * 0.05, size);
    g.fillStyle = 'rgba(255,230,190,0.04)';
    g.fillRect(x + cell * 0.06, 0, cell * 0.025, size);
  }
  g.restore();

  g.save();
  bandPath(g, b);
  const kraft = g.createLinearGradient(0, -b.ext, size, size + b.ext);
  kraft.addColorStop(0, '#d9ae74');
  kraft.addColorStop(0.5, '#c39158');
  kraft.addColorStop(1, '#a87841');
  g.fillStyle = kraft;
  g.fill('evenodd');
  g.clip('evenodd');
  // Fluting visible in the cut edge: short ribs across the band.
  g.lineWidth = Math.max(1, cell * 0.022);
  const total = Math.round((4 * size) / (cell * 0.11));
  for (const [x, y, nx, ny] of alongBand(b, total)) {
    g.strokeStyle = 'rgba(110,70,25,0.35)';
    g.beginPath();
    g.moveTo(x - nx * b.band * 0.22, y - ny * b.band * 0.22);
    g.lineTo(x + nx * b.band * 0.22, y + ny * b.band * 0.22);
    g.stroke();
  }
  g.restore();

  // Fold lines on both sides of the edge.
  g.lineWidth = Math.max(1, cell * 0.025);
  g.strokeStyle = 'rgba(255,236,200,0.6)';
  g.beginPath();
  addRoundRect(g, -b.ext + g.lineWidth, -b.ext + g.lineWidth, size + b.ext * 2 - g.lineWidth * 2, size + b.ext * 2 - g.lineWidth * 2, b.outerR + b.ext);
  g.stroke();
  g.strokeStyle = '#6b4520';
  g.beginPath();
  addRoundRect(g, -b.ext, -b.ext, size + b.ext * 2, size + b.ext * 2, b.outerR + b.ext);
  g.stroke();
  innerShadow(g, b, 0.5);

  // Packing tape over the middle of the top and bottom edges and two corners.
  const tape = (x: number, y: number, angle: number, len: number, th: number) => {
    g.save();
    g.translate(x, y);
    g.rotate(angle);
    g.beginPath();
    // Torn, zigzag ends.
    const teeth = 5;
    g.moveTo(-len / 2, -th / 2);
    g.lineTo(len / 2, -th / 2);
    for (let i = 1; i <= teeth; i++) g.lineTo(len / 2 + (i % 2 ? cell * 0.04 : 0), -th / 2 + (th * i) / teeth);
    g.lineTo(-len / 2, th / 2);
    for (let i = teeth - 1; i >= 0; i--) g.lineTo(-len / 2 - (i % 2 ? cell * 0.04 : 0), -th / 2 + (th * i) / teeth);
    g.closePath();
    g.fillStyle = 'rgba(236,214,160,0.78)';
    g.fill();
    g.fillStyle = 'rgba(255,255,255,0.3)';
    g.fillRect(-len / 2, -th / 2, len, th * 0.25);
    g.restore();
  };
  const len = b.band + cell * 0.18;
  const c = (inset - b.ext) / 2;
  // The tape folds over the edges of the box, so only the part on top of it shows.
  g.save();
  g.beginPath();
  addRoundRect(g, -b.ext, -b.ext, size + b.ext * 2, size + b.ext * 2, b.outerR + b.ext);
  g.clip();
  tape(size / 2, c, Math.PI / 2, len, cell * 0.5);
  tape(size / 2, size - c, Math.PI / 2, len, cell * 0.5);
  // Corner pieces run across the corner, from one side of the box to the other.
  const k = cell * 0.13;
  tape(k, k, -Math.PI / 4, cell, cell * 0.26);
  tape(size - k, size - k, -Math.PI / 4, cell, cell * 0.26);
  g.restore();
}

/** A riveted, brushed iron frame. */
function iron(g: Ctx, b: Box) {
  const { size, cell } = b;
  g.save();
  bandPath(g, b);
  const steel = g.createLinearGradient(0, -b.ext, size, size + b.ext);
  ['#c3c9d1', '#737a84', '#3d424a', '#9aa1aa', '#2c3036'].forEach((col, i, all) => steel.addColorStop(i / (all.length - 1), col));
  g.fillStyle = steel;
  g.fill('evenodd');
  g.clip('evenodd');
  const rand = rng(0x1205);
  for (let i = 0; i < 160; i++) {
    const y = -b.ext + rand() * (size + b.ext * 2);
    const x = -b.ext + rand() * size;
    g.fillStyle = rand() < 0.5 ? `rgba(255,255,255,${0.05 + rand() * 0.1})` : `rgba(0,0,0,${0.06 + rand() * 0.12})`;
    g.fillRect(x, y, cell * (0.6 + rand() * 1.4), Math.max(0.5, cell * 0.01));
  }
  g.restore();

  // Bevelled edges.
  g.lineWidth = Math.max(1, cell * 0.025);
  g.strokeStyle = 'rgba(255,255,255,0.45)';
  g.beginPath();
  addRoundRect(g, -b.ext + g.lineWidth, -b.ext + g.lineWidth, size + b.ext * 2 - g.lineWidth * 2, size + b.ext * 2 - g.lineWidth * 2, b.outerR + b.ext);
  g.stroke();
  g.strokeStyle = '#16181c';
  g.beginPath();
  addRoundRect(g, -b.ext, -b.ext, size + b.ext * 2, size + b.ext * 2, b.outerR + b.ext);
  g.stroke();
  innerShadow(g, b, 0.75);

  // A rivet roughly every cell along the frame.
  const count = Math.max(8, Math.round((4 * size) / cell));
  for (const [x, y] of alongBand(b, count - (count % 4), 0.5)) rivet(g, x, y, b.band * 0.24);
}

/** Four mitred planks of wood, nailed at the corners. */
function wood(g: Ctx, b: Box) {
  const { size, cell, inset, ext } = b;
  const o0 = -ext;
  const o1 = size + ext;
  const i0 = inset;
  const i1 = size - inset;
  const planks: { pts: number[]; along: 'x' | 'y'; tone: number }[] = [
    { pts: [o0, o0, o1, o0, i1, i0, i0, i0], along: 'x', tone: 0.12 }, // top
    { pts: [o1, o0, o1, o1, i1, i1, i1, i0], along: 'y', tone: -0.05 }, // right
    { pts: [o0, o1, i0, i1, i1, i1, o1, o1], along: 'x', tone: -0.15 }, // bottom
    { pts: [o0, o0, i0, i0, i0, i1, o0, o1], along: 'y', tone: 0.04 }, // left
  ];
  const rand = rng(0x00d);
  g.save();
  bandPath(g, b);
  g.clip('evenodd');
  for (const plank of planks) {
    g.save();
    g.beginPath();
    g.moveTo(plank.pts[0], plank.pts[1]);
    for (let i = 2; i < plank.pts.length; i += 2) g.lineTo(plank.pts[i], plank.pts[i + 1]);
    g.closePath();
    const across = plank.along === 'x' ? g.createLinearGradient(0, o0, 0, i0) : g.createLinearGradient(o0, 0, i0, 0);
    const light = plank.tone > 0 ? `rgba(255,235,200,${plank.tone})` : `rgba(40,15,0,${-plank.tone})`;
    across.addColorStop(0, '#b97a42');
    across.addColorStop(1, '#94592b');
    g.fillStyle = across;
    g.fill();
    g.fillStyle = light;
    g.fill();
    g.clip();
    // Grain running along the plank.
    for (let i = 0; i < 6; i++) {
      const off = o0 + ((i + 0.5) / 6) * b.band + (rand() - 0.5) * b.band * 0.1;
      const amp = b.band * (0.04 + rand() * 0.08);
      const freq = (Math.PI * 2) / (cell * (1.5 + rand() * 2));
      const phase = rand() * 6;
      g.strokeStyle = `rgba(80,38,10,${0.25 + rand() * 0.25})`;
      g.lineWidth = Math.max(0.6, cell * (0.008 + rand() * 0.012));
      for (const side of [off, size - off]) {
        g.beginPath();
        for (let t = o0; t <= o1; t += cell * 0.2) {
          const wob = Math.sin(t * freq + phase) * amp;
          if (plank.along === 'x') g.lineTo(t, side + wob);
          else g.lineTo(side + wob, t);
        }
        g.stroke();
      }
    }
    g.restore();
  }
  // Mitre joints.
  g.lineWidth = Math.max(1, cell * 0.022);
  g.strokeStyle = 'rgba(50,22,4,0.7)';
  g.beginPath();
  for (const [x0, y0, x1, y1] of [[o0, o0, i0, i0], [o1, o0, i1, i0], [o1, o1, i1, i1], [o0, o1, i0, i1]]) {
    g.moveTo(x0, y0);
    g.lineTo(x1, y1);
  }
  g.stroke();
  g.restore();

  g.lineWidth = Math.max(1, cell * 0.022);
  g.strokeStyle = '#3d1d06';
  g.beginPath();
  addRoundRect(g, o0, o0, o1 - o0, o1 - o0, b.outerR + ext);
  g.stroke();
  g.strokeStyle = 'rgba(255,230,190,0.35)';
  g.beginPath();
  addRoundRect(g, o0 + g.lineWidth, o0 + g.lineWidth, o1 - o0 - g.lineWidth * 2, o1 - o0 - g.lineWidth * 2, b.outerR + ext);
  g.stroke();
  innerShadow(g, b, 0.6);

  // Nail heads either side of each joint.
  const n = b.mid;
  const k = cell * 0.32;
  for (const [x, y] of [
    [n + k, n], [n, n + k], [size - n - k, n], [size - n, n + k],
    [n + k, size - n], [n, size - n - k], [size - n - k, size - n], [size - n, size - n - k],
  ]) {
    const r = Math.max(1, b.band * 0.13);
    g.fillStyle = 'rgba(255,230,190,0.35)';
    g.beginPath();
    g.arc(x + r * 0.3, y + r * 0.4, r, 0, Math.PI * 2);
    g.fill();
    const head = g.createRadialGradient(x - r * 0.3, y - r * 0.3, 0, x, y, r);
    head.addColorStop(0, '#c9c4bc');
    head.addColorStop(1, '#3a332c');
    g.fillStyle = head;
    g.beginPath();
    g.arc(x, y, r, 0, Math.PI * 2);
    g.fill();
  }
}

const MATERIALS: Partial<Record<FrameKind, (g: Ctx, b: Box) => void>> = { gingerbread, cardboard, iron, wood };

/** True for frames drawn by `drawMaterialFrame`; they need a wider band around the board. */
export function isMaterialFrame(kind: FrameKind): boolean {
  return kind in MATERIALS;
}

/** Draws a material frame over the plain one, after the board surface has been filled. */
export function drawMaterialFrame(g: Ctx, kind: FrameKind, box: FrameBox): void {
  const draw = MATERIALS[kind];
  if (!draw) return;
  const ext = box.cell * 0.06;
  const band = box.inset + ext;
  draw(g, { ...box, ext, band, mid: (box.inset - ext) / 2 });
}
