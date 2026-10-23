/**
 * Visual styles. The game score unlocks style *tiers* (Classic → Legendary),
 * and each tier comes in a few colour *variants*; clearing the whole board
 * switches to the next variant of the current tier. A tier decides how the
 * blocks are drawn, the board frame, effects, font and sounds; a variant
 * decides the colours.
 */

export type BlockKind = 'jewel' | 'cardboard' | 'candy' | 'gingerbread' | 'wood' | 'gem' | 'iron' | 'neon' | 'glass' | 'royal';
export type FrameKind = 'plain' | 'cardboard' | 'soft' | 'gingerbread' | 'wood' | 'metal' | 'iron' | 'neon' | 'glass' | 'gold';
export type ParticleShape = 'square' | 'circle' | 'diamond' | 'spark' | 'star';

export interface Palette {
  name: string;
  /** Block colours, indexed by colour id - 1. */
  blocks: string[];
  board: string;
  boardEdge: string;
  cell: string;
  cellInner: string;
  /** Page background gradient: top, middle, bottom. */
  bg: [string, string, string];
  /** HUD best score, buttons and banners. */
  accent: string;
  /** Glows, combo text and ambient background effects. */
  glow: string;
  glow2: string;
  card: string;
  cardEdge: string;
}

export interface Effects {
  /** Particles per cleared cell. */
  particles: number;
  shape: ParticleShape;
  /** Additive blending for particles, rings and beams. */
  glow: boolean;
  gravity: number;
  /** Shockwave ring where lines are cleared. */
  ring: boolean;
  /** Light beam along each cleared row and column. */
  beam: boolean;
  /** Share of particles drawn in the palette's glow colours instead of the block colour. */
  sparkle: number;
  /** Colour of the flash on clearing blocks. */
  flash: string;
  /** Screen shake multiplier. */
  shake: number;
  /** Firework bursts when the whole board is cleared. */
  fireworks: number;
}

export interface SoundProfile {
  /** Waveform of the line-clear arpeggio. */
  lead: OscillatorType;
  /** Root note of the arpeggio in Hz. */
  root: number;
  /** Arpeggio steps in semitones. */
  scale: number[];
  /** Echo send, 0 = dry. */
  echo: number;
  /** Adds high bell overtones. */
  bell: boolean;
  /** Adds a soft chord under clears. */
  pad: boolean;
  /** Pitch of the placement thud in Hz. */
  thud: number;
}

export interface Tier {
  level: number;
  name: string;
  minScore: number;
  block: BlockKind;
  frame: FrameKind;
  font: string;
  fontWeight: number;
  /** Banner text glows in the palette's glow colour. */
  textGlow: boolean;
  /** Banner text is filled with a metallic gradient of the accent colour. */
  textGradient: boolean;
  fx: Effects;
  sound: SoundProfile;
  variants: Palette[];
}

export interface Style {
  /** Stable id, e.g. "2.1" for the second variant of tier 2. */
  key: string;
  tier: Tier;
  palette: Palette;
  variant: number;
}

const FALLBACK_FONTS = "system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif";
const font = (family: string) => `'${family}', 'Baloo 2', ${FALLBACK_FONTS}`;

const CLASSIC_BLOCKS = ['#ff6f61', '#ff9f1c', '#ffd23f', '#8bd346', '#41a6f6', '#8b5cf6', '#e85d9f'];
const CARDBOARD_BLOCKS = ['#e0524c', '#ee8a35', '#f0c53c', '#6db24e', '#36a9b8', '#4a72c4', '#a060b8'];
const CANDY_BLOCKS = ['#ff5d8f', '#ff9f43', '#ffd93d', '#5fd97f', '#4fd3ff', '#7b8cff', '#c77dff'];
const GINGERBREAD_BLOCKS = ['#ff4d6d', '#ff9a3c', '#ffdb4d', '#5ad16a', '#4cc9f0', '#5b7cfa', '#c06cf0'];
const WOOD_BLOCKS = ['#d23c2e', '#f08a2c', '#f5c829', '#3fb05a', '#1fb0b8', '#3b78dc', '#9a4fc0'];
const GEM_BLOCKS = ['#e0115f', '#ff8c1a', '#ffd21f', '#14c46b', '#1fd1e0', '#2456e8', '#9b30ff'];
const IRON_BLOCKS = ['#e23b3b', '#f08c25', '#f2c62e', '#3dbb4c', '#22b5c8', '#3c6fe6', '#9c50dc'];
const NEON_BLOCKS = ['#ff2a6d', '#ff8a00', '#f9f871', '#39ff14', '#05d9e8', '#4d6bff', '#e100d4'];
const GLASS_BLOCKS = ['#ff6b8b', '#ffa36b', '#ffe66b', '#6bffb0', '#6be4ff', '#7b93ff', '#c56bff'];
const ROYAL_BLOCKS = ['#e8173c', '#ff7a1a', '#ffcc00', '#1fbf5a', '#14b8d4', '#2f5bff', '#a020f0'];

export const TIERS: Tier[] = [
  {
    level: 0,
    name: 'Classic',
    minScore: 0,
    block: 'jewel',
    frame: 'plain',
    font: font('Baloo 2'),
    fontWeight: 800,
    textGlow: false,
    textGradient: false,
    fx: { particles: 4, shape: 'square', glow: false, gravity: 900, ring: false, beam: false, sparkle: 0, flash: '#ffffff', shake: 1, fireworks: 0 },
    sound: { lead: 'square', root: 392, scale: [0, 4, 7, 12, 16, 19, 24], echo: 0, bell: false, pad: false, thud: 180 },
    variants: [
      {
        name: 'Lagoon',
        blocks: CLASSIC_BLOCKS,
        board: '#0a2f38',
        boardEdge: '#061f26',
        cell: '#0f3d48',
        cellInner: '#0d3640',
        bg: ['#1fa2a0', '#137a80', '#0b4f5c'],
        accent: '#ffb547',
        glow: '#9ff5e6',
        glow2: '#ffffff',
        card: '#0f5e66',
        cardEdge: '#073a40',
      },
      {
        name: 'Ember',
        blocks: CLASSIC_BLOCKS,
        board: '#2e130f',
        boardEdge: '#1e0b08',
        cell: '#3d1b15',
        cellInner: '#361712',
        bg: ['#e8794a', '#b84f33', '#6e2a22'],
        accent: '#ffd84a',
        glow: '#ffc59a',
        glow2: '#ffffff',
        card: '#7a3324',
        cardEdge: '#3e160f',
      },
      {
        name: 'Sunset',
        blocks: CLASSIC_BLOCKS,
        board: '#3a1d4a',
        boardEdge: '#2a1236',
        cell: '#4a2860',
        cellInner: '#422357',
        bg: ['#e0679a', '#a8467f', '#5e2a63'],
        accent: '#ffc94a',
        glow: '#ffb38a',
        glow2: '#ffffff',
        card: '#6a2f6f',
        cardEdge: '#3f1844',
      },
    ],
  },
  {
    level: 1,
    name: 'Cardboard',
    minScore: 5_000,
    block: 'cardboard',
    frame: 'cardboard',
    font: font('Baloo 2'),
    fontWeight: 800,
    textGlow: false,
    textGradient: false,
    fx: { particles: 5, shape: 'square', glow: false, gravity: 1100, ring: false, beam: false, sparkle: 0.1, flash: '#fff3dc', shake: 1, fireworks: 2 },
    sound: { lead: 'sine', root: 329.63, scale: [0, 3, 5, 7, 10, 12, 15], echo: 0, bell: false, pad: false, thud: 140 },
    variants: [
      {
        name: 'Moving Day',
        blocks: CARDBOARD_BLOCKS,
        board: '#3b2a1a',
        boardEdge: '#24180d',
        cell: '#4a3622',
        cellInner: '#43301e',
        bg: ['#c9a06a', '#a37a48', '#6b4c2a'],
        accent: '#ffd166',
        glow: '#ffe2b0',
        glow2: '#ffffff',
        card: '#7a5632',
        cardEdge: '#3b2a1a',
      },
      {
        name: 'Post Office',
        blocks: CARDBOARD_BLOCKS,
        board: '#2b313c',
        boardEdge: '#1a1e26',
        cell: '#384050',
        cellInner: '#333a49',
        bg: ['#8fa3b8', '#61758c', '#3a4859'],
        accent: '#ffcf5c',
        glow: '#d8e6f5',
        glow2: '#ffffff',
        card: '#4c5b70',
        cardEdge: '#2b313c',
      },
      {
        name: 'Recycled',
        blocks: CARDBOARD_BLOCKS,
        board: '#2c3624',
        boardEdge: '#1b2316',
        cell: '#3a4730',
        cellInner: '#34402b',
        bg: ['#a3b17a', '#76874f', '#475530'],
        accent: '#ffe07a',
        glow: '#e6f0c8',
        glow2: '#ffffff',
        card: '#56653c',
        cardEdge: '#2c3624',
      },
    ],
  },
  {
    level: 2,
    name: 'Candy',
    minScore: 12_000,
    block: 'candy',
    frame: 'soft',
    font: font('Lilita One'),
    fontWeight: 400,
    textGlow: false,
    textGradient: false,
    fx: { particles: 6, shape: 'circle', glow: false, gravity: 700, ring: true, beam: false, sparkle: 0.25, flash: '#ffffff', shake: 1, fireworks: 3 },
    sound: { lead: 'triangle', root: 440, scale: [0, 2, 4, 7, 9, 12, 14, 16], echo: 0.15, bell: true, pad: false, thud: 220 },
    variants: [
      {
        name: 'Bubblegum',
        blocks: CANDY_BLOCKS,
        board: '#6b2350',
        boardEdge: '#4d1539',
        cell: '#7d2d60',
        cellInner: '#742858',
        bg: ['#ff9ac9', '#e86aa8', '#b44886'],
        accent: '#fff06a',
        glow: '#ffffff',
        glow2: '#ffd1ea',
        card: '#a23a78',
        cardEdge: '#6b2350',
      },
      {
        name: 'Mint Soda',
        blocks: CANDY_BLOCKS,
        board: '#0f5450',
        boardEdge: '#0a3b38',
        cell: '#146460',
        cellInner: '#125c58',
        bg: ['#6ee6c9', '#33bfa6', '#1d8a7e'],
        accent: '#fff27a',
        glow: '#ffffff',
        glow2: '#c8fff1',
        card: '#1f8a7e',
        cardEdge: '#0f5450',
      },
      {
        name: 'Lemon Sky',
        blocks: CANDY_BLOCKS,
        board: '#1d3f8a',
        boardEdge: '#142d66',
        cell: '#24499a',
        cellInner: '#214492',
        bg: ['#7cc8ff', '#4d9cf0', '#3a6fd1'],
        accent: '#ffe45c',
        glow: '#ffffff',
        glow2: '#fff7b0',
        card: '#2f5bb8',
        cardEdge: '#1d3f8a',
      },
    ],
  },
  {
    level: 3,
    name: 'Gingerbread',
    minScore: 25_000,
    block: 'gingerbread',
    frame: 'gingerbread',
    font: font('Lilita One'),
    fontWeight: 400,
    textGlow: false,
    textGradient: false,
    fx: { particles: 6, shape: 'circle', glow: false, gravity: 650, ring: true, beam: false, sparkle: 0.4, flash: '#fff8e8', shake: 1, fireworks: 4 },
    sound: { lead: 'triangle', root: 523.25, scale: [0, 2, 4, 7, 9, 12, 16, 19], echo: 0.2, bell: true, pad: false, thud: 200 },
    variants: [
      {
        name: 'Christmas Eve',
        blocks: GINGERBREAD_BLOCKS,
        board: '#3a1f12',
        boardEdge: '#241208',
        cell: '#4a2a18',
        cellInner: '#432515',
        bg: ['#c8402f', '#8e2219', '#4a0f0a'],
        accent: '#ffe08a',
        glow: '#fff4e0',
        glow2: '#ff9aa8',
        card: '#7a1f17',
        cardEdge: '#3a0d08',
      },
      {
        name: 'Snowy Cottage',
        blocks: GINGERBREAD_BLOCKS,
        board: '#3a2214',
        boardEdge: '#22130a',
        cell: '#4a2d1b',
        cellInner: '#432818',
        bg: ['#86b9e3', '#4f82ba', '#2a4d7a'],
        accent: '#ffd27a',
        glow: '#ffffff',
        glow2: '#cfe8ff',
        card: '#2f5a8a',
        cardEdge: '#1a3354',
      },
      {
        name: 'Pine Forest',
        blocks: GINGERBREAD_BLOCKS,
        board: '#352013',
        boardEdge: '#1f1209',
        cell: '#45291a',
        cellInner: '#3e2516',
        bg: ['#3f9a5a', '#246b3c', '#0f3d22'],
        accent: '#ffcf5a',
        glow: '#fff1d0',
        glow2: '#ff8080',
        card: '#1f5a34',
        cardEdge: '#0c2c18',
      },
    ],
  },
  {
    level: 4,
    name: 'Wooden',
    minScore: 40_000,
    block: 'wood',
    frame: 'wood',
    font: font('Righteous'),
    fontWeight: 400,
    textGlow: false,
    textGradient: false,
    fx: { particles: 6, shape: 'diamond', glow: false, gravity: 850, ring: true, beam: false, sparkle: 0.15, flash: '#fff4dc', shake: 1.2, fireworks: 4 },
    sound: { lead: 'triangle', root: 392, scale: [0, 2, 4, 7, 9, 12, 14, 16], echo: 0.1, bell: false, pad: false, thud: 165 },
    variants: [
      {
        name: 'Oak Workshop',
        blocks: WOOD_BLOCKS,
        board: '#2b1a0e',
        boardEdge: '#1a0f07',
        cell: '#3a2414',
        cellInner: '#331f11',
        bg: ['#c58b4f', '#8f5a2c', '#4f2f14'],
        accent: '#ffd27a',
        glow: '#ffe0b0',
        glow2: '#ffffff',
        card: '#6b4122',
        cardEdge: '#2b1a0e',
      },
      {
        name: 'Walnut Study',
        blocks: WOOD_BLOCKS,
        board: '#1f140d',
        boardEdge: '#120b06',
        cell: '#2c1d13',
        cellInner: '#271a11',
        bg: ['#7a5238', '#4e3322', '#24160d'],
        accent: '#f5c46a',
        glow: '#ffcf9a',
        glow2: '#ffffff',
        card: '#4e3322',
        cardEdge: '#1f140d',
      },
      {
        name: 'Birch Grove',
        blocks: WOOD_BLOCKS,
        board: '#2a2a20',
        boardEdge: '#191912',
        cell: '#38382b',
        cellInner: '#323226',
        bg: ['#c9c3a0', '#9a9472', '#5c5942'],
        accent: '#ffe27a',
        glow: '#fff6d8',
        glow2: '#ffffff',
        card: '#6a664c',
        cardEdge: '#2a2a20',
      },
    ],
  },
  {
    level: 5,
    name: 'Gems',
    minScore: 60_000,
    block: 'gem',
    frame: 'metal',
    font: font('Lilita One'),
    fontWeight: 400,
    textGlow: true,
    textGradient: false,
    fx: { particles: 6, shape: 'diamond', glow: true, gravity: 600, ring: true, beam: true, sparkle: 0.3, flash: '#ffffff', shake: 1.1, fireworks: 4 },
    sound: { lead: 'sine', root: 523.25, scale: [0, 4, 7, 11, 12, 16, 19, 23], echo: 0.25, bell: true, pad: false, thud: 200 },
    variants: [
      {
        name: 'Ruby Cave',
        blocks: GEM_BLOCKS,
        board: '#2a0a18',
        boardEdge: '#1a0510',
        cell: '#3a1022',
        cellInner: '#330d1e',
        bg: ['#9a1a42', '#5c0d2a', '#2e0618'],
        accent: '#ffd86b',
        glow: '#ff6b9a',
        glow2: '#ffffff',
        card: '#5c0d2a',
        cardEdge: '#2e0618',
      },
      {
        name: 'Emerald Vault',
        blocks: GEM_BLOCKS,
        board: '#06241c',
        boardEdge: '#031510',
        cell: '#0b3428',
        cellInner: '#092d23',
        bg: ['#14855f', '#0a4f3c', '#052a20'],
        accent: '#ffe27a',
        glow: '#6bffc8',
        glow2: '#ffffff',
        card: '#0a4f3c',
        cardEdge: '#052a20',
      },
      {
        name: 'Amethyst Grotto',
        blocks: GEM_BLOCKS,
        board: '#1e0a3a',
        boardEdge: '#12052a',
        cell: '#2a1150',
        cellInner: '#250e47',
        bg: ['#7330c4', '#44187e', '#220a44'],
        accent: '#ffd86b',
        glow: '#d59bff',
        glow2: '#ffffff',
        card: '#44187e',
        cardEdge: '#220a44',
      },
    ],
  },
  {
    level: 6,
    name: 'Iron',
    minScore: 90_000,
    block: 'iron',
    frame: 'iron',
    font: font('Orbitron'),
    fontWeight: 900,
    textGlow: true,
    textGradient: false,
    fx: { particles: 7, shape: 'spark', glow: true, gravity: 900, ring: true, beam: false, sparkle: 0.55, flash: '#ffe6c2', shake: 1.5, fireworks: 5 },
    sound: { lead: 'sine', root: 440, scale: [0, 5, 7, 12, 17, 19, 24, 29], echo: 0.3, bell: true, pad: false, thud: 95 },
    variants: [
      {
        name: 'Foundry',
        blocks: IRON_BLOCKS,
        board: '#141618',
        boardEdge: '#0a0b0c',
        cell: '#1f2225',
        cellInner: '#1b1d20',
        bg: ['#5a5f66', '#33373d', '#16181b'],
        accent: '#ff9a3c',
        glow: '#ffa54c',
        glow2: '#fff0c0',
        card: '#2a2e33',
        cardEdge: '#0a0b0c',
      },
      {
        name: 'Rust Belt',
        blocks: IRON_BLOCKS,
        board: '#1c120d',
        boardEdge: '#0e0805',
        cell: '#291b13',
        cellInner: '#241710',
        bg: ['#8a4b2a', '#5a2e18', '#2a140a'],
        accent: '#ffc46b',
        glow: '#ff8a4c',
        glow2: '#ffd9a0',
        card: '#4a2615',
        cardEdge: '#0e0805',
      },
      {
        name: 'Blue Steel',
        blocks: IRON_BLOCKS,
        board: '#0f1720',
        boardEdge: '#070b10',
        cell: '#18222e',
        cellInner: '#151e29',
        bg: ['#4a6a8a', '#2a4058', '#121e2b'],
        accent: '#9fd8ff',
        glow: '#8fcfff',
        glow2: '#ffffff',
        card: '#22344a',
        cardEdge: '#070b10',
      },
    ],
  },
  {
    level: 7,
    name: 'Neon',
    minScore: 130_000,
    block: 'neon',
    frame: 'neon',
    font: font('Orbitron'),
    fontWeight: 900,
    textGlow: true,
    textGradient: false,
    fx: { particles: 7, shape: 'spark', glow: true, gravity: 300, ring: true, beam: true, sparkle: 0.3, flash: '#ffffff', shake: 1.4, fireworks: 5 },
    sound: { lead: 'sawtooth', root: 220, scale: [0, 3, 5, 7, 10, 12, 15, 17], echo: 0.35, bell: false, pad: true, thud: 120 },
    variants: [
      {
        name: 'Synthwave',
        blocks: NEON_BLOCKS,
        board: '#0b0420',
        boardEdge: '#05010f',
        cell: '#120830',
        cellInner: '#0f0628',
        bg: ['#3a0d5e', '#160633', '#05010f'],
        accent: '#ff2a6d',
        glow: '#05d9e8',
        glow2: '#ff2a6d',
        card: '#1a0a3a',
        cardEdge: '#ff2a6d',
      },
      {
        name: 'Toxic',
        blocks: NEON_BLOCKS,
        board: '#021006',
        boardEdge: '#010803',
        cell: '#06190c',
        cellInner: '#051509',
        bg: ['#0e4a20', '#05210f', '#010a04'],
        accent: '#39ff14',
        glow: '#39ff14',
        glow2: '#f9f871',
        card: '#06250f',
        cardEdge: '#39ff14',
      },
      {
        name: 'Ultraviolet',
        blocks: NEON_BLOCKS,
        board: '#070320',
        boardEdge: '#030112',
        cell: '#0d0838',
        cellInner: '#0b0630',
        bg: ['#24107a', '#0c0533', '#030112'],
        accent: '#00f0ff',
        glow: '#b026ff',
        glow2: '#00f0ff',
        card: '#120a4a',
        cardEdge: '#b026ff',
      },
    ],
  },
  {
    level: 8,
    name: 'Aurora',
    minScore: 200_000,
    block: 'glass',
    frame: 'glass',
    font: font('Righteous'),
    fontWeight: 400,
    textGlow: true,
    textGradient: false,
    fx: { particles: 8, shape: 'star', glow: true, gravity: 180, ring: true, beam: true, sparkle: 0.45, flash: '#ffffff', shake: 1.4, fireworks: 6 },
    sound: { lead: 'sine', root: 349.23, scale: [0, 2, 4, 6, 7, 9, 11, 12, 14], echo: 0.45, bell: true, pad: true, thud: 160 },
    variants: [
      {
        name: 'Borealis',
        blocks: GLASS_BLOCKS,
        board: '#06223a',
        boardEdge: '#031526',
        cell: '#0a2f4d',
        cellInner: '#082944',
        bg: ['#0e4a6e', '#072a45', '#031526'],
        accent: '#7dffcf',
        glow: '#4dffb5',
        glow2: '#b35dff',
        card: '#0a2f4d',
        cardEdge: '#4dffb5',
      },
      {
        name: 'Ice Crystal',
        blocks: GLASS_BLOCKS,
        board: '#102a52',
        boardEdge: '#0a1c3a',
        cell: '#163566',
        cellInner: '#13305d',
        bg: ['#4a8fd4', '#285a99', '#173a6b'],
        accent: '#e6f6ff',
        glow: '#9fe8ff',
        glow2: '#ffffff',
        card: '#1d4580',
        cardEdge: '#9fe8ff',
      },
      {
        name: 'Nebula',
        blocks: GLASS_BLOCKS,
        board: '#1c0a2e',
        boardEdge: '#0f0519',
        cell: '#26103d',
        cellInner: '#210d36',
        bg: ['#5a2080', '#2c0f45', '#140620'],
        accent: '#ff9ff3',
        glow: '#ff6bd6',
        glow2: '#6bd6ff',
        card: '#2c0f45',
        cardEdge: '#ff6bd6',
      },
    ],
  },
  {
    level: 9,
    name: 'Legendary',
    minScore: 350_000,
    block: 'royal',
    frame: 'gold',
    font: font('Cinzel Decorative'),
    fontWeight: 900,
    textGlow: true,
    textGradient: true,
    fx: { particles: 10, shape: 'star', glow: true, gravity: 500, ring: true, beam: true, sparkle: 0.5, flash: '#fff1a8', shake: 1.6, fireworks: 9 },
    sound: { lead: 'triangle', root: 261.63, scale: [0, 4, 7, 12, 16, 19, 24, 28], echo: 0.4, bell: true, pad: true, thud: 140 },
    variants: [
      {
        name: 'Golden Throne',
        blocks: ROYAL_BLOCKS,
        board: '#1a0f02',
        boardEdge: '#0c0700',
        cell: '#261705',
        cellInner: '#211304',
        bg: ['#5a360c', '#2a1804', '#100800'],
        accent: '#ffd700',
        glow: '#ffb300',
        glow2: '#fff1a8',
        card: '#2a1804',
        cardEdge: '#ffd700',
      },
      {
        name: 'Celestial',
        blocks: ROYAL_BLOCKS,
        board: '#0a0a24',
        boardEdge: '#04040f',
        cell: '#12123a',
        cellInner: '#0f0f32',
        bg: ['#22225c', '#0d0d2e', '#04040f'],
        accent: '#ffe27a',
        glow: '#8fb8ff',
        glow2: '#ffd700',
        card: '#14143f',
        cardEdge: '#ffd700',
      },
      {
        name: 'Phoenix',
        blocks: ROYAL_BLOCKS,
        board: '#240502',
        boardEdge: '#120201',
        cell: '#330903',
        cellInner: '#2c0702',
        bg: ['#8a1a06', '#420903', '#160200'],
        accent: '#ffb000',
        glow: '#ff5a00',
        glow2: '#ffd700',
        card: '#420903',
        cardEdge: '#ffb000',
      },
    ],
  },
];

/** The highest tier unlocked by the score. */
export function tierFor(score: number): Tier {
  let tier = TIERS[0];
  for (const t of TIERS) if (score >= t.minScore) tier = t;
  return tier;
}

/** The style for a game: its tier comes from the score, its variant from the number of board clears. */
export function styleFor(score: number, boardClears: number): Style {
  const tier = tierFor(score);
  const variant = Math.max(0, boardClears) % tier.variants.length;
  return { key: `${tier.level}.${variant}`, tier, palette: tier.variants[variant], variant };
}

/** The next tier to unlock, or null at the top. */
export function nextTier(tier: Tier): Tier | null {
  return TIERS[tier.level + 1] ?? null;
}

/** "50k", "1M"… for compact HUD labels. */
export function formatScore(n: number): string {
  if (n >= 1_000_000) return `${+(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1000) return `${+(n / 1000).toFixed(1)}k`;
  return String(n);
}
