/**
 * Adaptive soundtrack, synthesised live with Web Audio. It drifts from relaxed
 * lofi through chill house and house to fast EDM as the score grows and the
 * player speeds up:
 *
 *   e 0    – 0.25  lofi: jazz chords on an FM e-piano, triangle bass, swung boom-bap, a sparse melody, vinyl
 *   e 0.15 – 0.4   livelier lofi: more bass notes, 16th hats, a clap; the vinyl fades
 *   e 0.35 – 0.6   chill house: four-on-the-floor kick and offbeat hats replace the swung drums
 *   e 0.5  – 0.75  house: clap on 2 and 4, pumping saw chords, the e-piano leaves
 *   e 0.65 – 0.85  builds every 8 bars: snare roll, noise riser, the kick drops out before the downbeat
 *   e 0.75 – 1     EDM: arpeggiated lead, 16th hats, rolling bass
 *
 * Layers fade in and out between two thresholds of `e` instead of switching
 * abruptly. Lofi and EDM layers share one progression (jazz voicings vs. plain
 * triads over the same roots), so they never clash during a crossfade.
 */
import { Instruments, makeReverb } from './instruments';
import { IntensityTracker, type MusicState, ramp } from './intensity';

/** Raw inputs from the game, both 0..1. */
export type MusicInput = () => { score: number; speed: number };

const VOLUME = 0.4;
const LOOKAHEAD = 0.25;
const TICK_MS = 40;
const STEPS = 16;

// One progression, 8 bars; the last bar holds two chords (Gm9 → C9).
//                     Dm9  G13  C^9  Am9  F^9 E7b9  Am9  Gm9   C9
const ROOTS = /*    */ [38, 43, 36, 45, 41, 40, 45, 43, 36];
const JAZZ = [
  [53, 57, 60, 64],
  [53, 57, 59, 64],
  [52, 55, 59, 62],
  [52, 55, 59, 60],
  [52, 55, 57, 60],
  [50, 53, 56, 59],
  [52, 55, 59, 60],
  [53, 57, 58, 62],
  [52, 55, 58, 62],
];
//                Dm          G           C           Am          F           E           Am          Gm          C
const TRIADS = [[62, 65, 69], [62, 67, 71], [64, 67, 72], [64, 69, 72], [65, 69, 72], [64, 68, 71], [64, 69, 72], [62, 67, 70], [64, 67, 72]];
const E_CHORD = 5;

/** C major pentatonic from C4; over E7b9 the G is raised to G#. */
const PENTA = [60, 62, 64, 67, 69, 72, 74, 76, 79, 81];
const ARP = [0, 1, 2, 3, 1, 2, 3, 4, 2, 3, 4, 5, 3, 4, 5, 6];

// Rhythms as 16th steps within a bar.
const KEYS_HITS = [[0], [0, 10], [0, 14], [4]];
const LOFI_BASS = [[0, 12], [0, 6]];
const LOFI_BASS_EXTRA = [6, 15];
const LOFI_KICK = [[0, 10], [0, 6, 10]];
/** Swing for the lofi layers: how late each 16th of a beat lands, in steps. */
const SWING = [0, 0.33, 0.4, 0.4];

/** Which chord of the progression sounds at a step. */
export function chordAt(bar: number, step: number): number {
  const b = bar % 8;
  return b === 7 && step >= 8 ? 8 : b;
}

interface Bus {
  input: GainNode;
  filter?: BiquadFilterNode;
  pan?: StereoPannerNode;
}

/** All nodes of one playing instance; stopping fades it out and lets it go. */
class Graph {
  readonly ins: Instruments;
  readonly out: GainNode;
  readonly muffle: BiquadFilterNode;
  readonly delay: DelayNode;
  readonly pump: GainNode;
  readonly buses: Record<'keys' | 'melody' | 'arp' | 'pad' | 'bass' | 'kick' | 'drums' | 'vinyl' | 'fx', Bus>;
  private sources: AudioScheduledSourceNode[] = [];
  private nodes: AudioNode[] = [];

  constructor(
    readonly ctx: BaseAudioContext,
    destination: AudioNode,
  ) {
    const node = <T extends AudioNode>(n: T): T => (this.nodes.push(n), n);
    this.ins = new Instruments(ctx);

    this.out = node(ctx.createGain());
    this.out.gain.value = 0;
    const limiter = node(ctx.createDynamicsCompressor());
    limiter.threshold.value = -12;
    limiter.knee.value = 6;
    limiter.ratio.value = 8;
    limiter.attack.value = 0.003;
    limiter.release.value = 0.2;
    this.muffle = node(ctx.createBiquadFilter());
    this.muffle.type = 'lowpass';
    this.muffle.frequency.value = 20000;
    const mix = node(ctx.createGain());
    mix.connect(this.muffle).connect(limiter).connect(this.out).connect(destination);

    const reverb = node(ctx.createConvolver());
    reverb.buffer = makeReverb(ctx, 2.2);
    reverb.connect(mix);

    this.delay = node(ctx.createDelay(2));
    const feedback = node(ctx.createGain());
    feedback.gain.value = 0.38;
    const tape = node(ctx.createBiquadFilter());
    tape.type = 'lowpass';
    tape.frequency.value = 3200;
    this.delay.connect(tape).connect(feedback).connect(this.delay);
    tape.connect(mix);

    // Gentle saturation on kick and bass: warmer, and audible on phone speakers.
    const shaper = node(ctx.createWaveShaper());
    const curve = new Float32Array(1024);
    for (let i = 0; i < curve.length; i++) curve[i] = Math.tanh(2 * ((i / (curve.length - 1)) * 2 - 1)) / Math.tanh(2);
    shaper.curve = curve;
    shaper.connect(mix);

    this.pump = node(ctx.createGain());

    const bus = (o: { filter?: number; pan?: boolean; pump?: boolean; reverb?: number; delay?: number; to?: AudioNode; level?: number }): Bus => {
      const input = node(ctx.createGain());
      input.gain.value = o.level ?? 1;
      let last: AudioNode = input;
      const b: Bus = { input };
      if (o.filter) {
        b.filter = node(ctx.createBiquadFilter());
        b.filter.type = 'lowpass';
        b.filter.frequency.value = o.filter;
        last = last.connect(b.filter);
      }
      if (o.pan) last = last.connect((b.pan = node(ctx.createStereoPanner())));
      if (o.pump) last = last.connect(this.pump);
      last.connect(o.to ?? mix);
      for (const [amount, target] of [
        [o.reverb, reverb],
        [o.delay, this.delay],
      ] as const) {
        if (!amount) continue;
        const send = node(ctx.createGain());
        send.gain.value = amount;
        last.connect(send).connect(target);
      }
      return b;
    };

    this.buses = {
      keys: bus({ filter: 1500, reverb: 0.45, delay: 0.22, level: 0 }),
      melody: bus({ pan: true, reverb: 0.55, delay: 0.35 }),
      arp: bus({ filter: 1500, pan: true, reverb: 0.12, delay: 0.3 }),
      pad: bus({ filter: 700, pump: true, reverb: 0.3, delay: 0.1, level: 0 }),
      bass: bus({ to: shaper }),
      kick: bus({ to: shaper }),
      drums: bus({ filter: 4500, reverb: 0.1 }),
      vinyl: bus({ filter: 5000, level: 0 }),
      fx: bus({ reverb: 0.25 }),
    };

    // The vinyl hiss runs all the time; its bus fades it in and out.
    const hissLevel = node(ctx.createGain());
    hissLevel.gain.value = 0.025;
    hissLevel.connect(this.buses.vinyl.input);
    this.sources.push(this.ins.hiss(hissLevel, 0));
  }

  /** Fades out, then disconnects everything. */
  release(): void {
    const t = this.ctx.currentTime;
    this.out.gain.cancelScheduledValues(t);
    this.out.gain.setTargetAtTime(0, t, 0.08);
    setTimeout(() => {
      for (const s of this.sources) s.stop();
      for (const n of this.nodes) n.disconnect();
      this.sources = [];
      this.nodes = [];
    }, 600);
  }
}

export class Music {
  private graph: Graph | null = null;
  private timer = 0;
  private lastTick = 0;
  private step = 0;
  private nextTime = 0;
  private melody = 4;
  private paused = false;
  /** Last value sent to each automated parameter, to skip needless automation. */
  private sent = new Map<AudioParam, number>();
  readonly tracker = new IntensityTracker();

  constructor(private input: MusicInput = () => ({ score: 0, speed: 0 })) {}

  get playing(): boolean {
    return this.graph !== null;
  }

  get state(): MusicState {
    return this.tracker.state;
  }

  setInput(input: MusicInput): void {
    this.input = input;
  }

  /** Starts playing into `destination` (the context's output by default). Safe to call repeatedly. */
  start(ctx: AudioContext, destination: AudioNode = ctx.destination): void {
    if (this.graph) return;
    this.begin(ctx, destination);
    this.lastTick = performance.now();
    this.timer = window.setInterval(() => this.tick(), TICK_MS);
    this.tick();
  }

  stop(): void {
    clearInterval(this.timer);
    this.timer = 0;
    this.graph?.release();
    this.graph = null;
  }

  /** Muffles the music while a menu is open. */
  setPaused(paused: boolean): void {
    this.paused = paused;
    if (this.graph) this.applyPause(this.graph);
  }

  /**
   * Renders `seconds` of music offline, for previews and tests. `input` gives
   * the raw score progress and speed at each moment (in seconds). Notes are
   * scheduled a little ahead of rendering, just like when playing live.
   */
  async render(ctx: OfflineAudioContext, seconds: number, input: (time: number) => { score: number; speed: number }): Promise<AudioBuffer> {
    this.stop();
    const g = this.begin(ctx, ctx.destination);
    const dt = TICK_MS / 1000;
    let time = 0;
    const advance = (until: number) => {
      for (; time < until; time += dt) {
        const { score, speed } = input(time);
        if (time) this.tracker.update(dt, score, speed);
        else this.tracker.set(score, speed);
        this.applyLevels(g, time);
        this.schedule(g, time + LOOKAHEAD);
      }
    };
    const chunk = 0.5;
    for (let at = chunk; at < seconds; at += chunk)
      void ctx.suspend(at).then(() => {
        advance(at + chunk);
        void ctx.resume();
      });
    advance(chunk);
    try {
      return await ctx.startRendering();
    } finally {
      this.graph = null;
    }
  }

  private begin(ctx: BaseAudioContext, destination: AudioNode): Graph {
    const g = (this.graph = new Graph(ctx, destination));
    this.sent.clear();
    this.step = 0;
    this.nextTime = ctx.currentTime + 0.05;
    g.out.gain.setValueAtTime(0, ctx.currentTime);
    g.out.gain.linearRampToValueAtTime(VOLUME, ctx.currentTime + 1.5);
    this.applyPause(g);
    return g;
  }

  private tick(): void {
    const g = this.graph;
    if (!g) return;
    const now = performance.now();
    const { score, speed } = this.input();
    this.tracker.update((now - this.lastTick) / 1000, score, speed);
    this.lastTick = now;
    // Nothing advances while the context is suspended (app in background).
    if ((g.ctx as AudioContext).state !== 'running') return;
    this.applyLevels(g, g.ctx.currentTime);
    this.schedule(g, g.ctx.currentTime + LOOKAHEAD);
  }

  private applyPause(g: Graph): void {
    const t = g.ctx.currentTime;
    g.muffle.frequency.setTargetAtTime(this.paused ? 700 : 20000, t, 0.12);
  }

  /** Continuous parameters that follow the intensity. */
  private applyLevels(g: Graph, t: number): void {
    const { e, bpm } = this.state;
    const set = (p: AudioParam, v: number, tau = 0.15) => {
      const last = this.sent.get(p);
      if (last !== undefined && Math.abs(v - last) <= 0.004 * Math.max(1, Math.abs(last))) return;
      p.setTargetAtTime(v, t, tau);
      this.sent.set(p, v);
    };
    const b = g.buses;
    // The e-piano filter drifts slowly, like a hand on a knob.
    const drift = Math.sin(t / 7) * 0.6 + Math.sin(t / 3.1) * 0.4;
    set(b.keys.input.gain, 1 - ramp(e, 0.45, 0.65));
    set(b.keys.filter!.frequency, 1450 + 550 * drift + 2500 * e);
    set(b.pad.input.gain, ramp(e, 0.5, 0.65));
    set(b.pad.filter!.frequency, 700 + 3500 * ramp(e, 0.5, 1));
    set(b.arp.filter!.frequency, 1500 + 4000 * ramp(e, 0.75, 1));
    set(b.drums.filter!.frequency, 4500 + 14000 * ramp(e, 0.3, 0.6));
    set(b.vinyl.input.gain, 1 - ramp(e, 0.25, 0.5));
    // Echoes stay on the beat at any tempo.
    set(g.delay.delayTime, 30 / bpm, 0.3);
  }

  private schedule(g: Graph, until: number): void {
    // After a stall (e.g. the tab was throttled) skip ahead instead of playing catch-up.
    if (this.nextTime < g.ctx.currentTime - 0.05) this.nextTime = g.ctx.currentTime + 0.02;
    while (this.nextTime < until) {
      const dur = 15 / this.state.bpm;
      this.playStep(g, this.step, this.nextTime, dur);
      this.nextTime += dur;
      this.step++;
    }
  }

  private playStep(g: Graph, n: number, t: number, dur: number): void {
    const { e, speed } = this.state;
    const ins = g.ins;
    const b = g.buses;
    const bar = Math.floor(n / STEPS);
    const s = n % STEPS;
    const chord = chordAt(bar, s);
    const root = ROOTS[chord];
    const lastOf8 = bar % 8 === 7;

    // Stage envelopes.
    const lofi = 1 - ramp(e, 0.45, 0.6);
    const lofiDrums = (1 - ramp(e, 0.4, 0.55)) * (bar % 16 === 15 ? 0 : 1);
    const swung = t + SWING[s % 4] * dur * (1 - ramp(e, 0.35, 0.55));
    const chance = (p: number) => Math.random() < p;

    // ══════════ LOFI ══════════

    // E-piano chords, held until the next hit.
    const keyHits = [...KEYS_HITS[bar % 4]];
    if (lastOf8 && !keyHits.includes(8)) keyHits.push(8);
    keyHits.sort((x, y) => x - y);
    if (keyHits.includes(s) && ramp(e, 0.45, 0.65) < 0.999) {
      const next = keyHits.find((h) => h > s) ?? STEPS;
      const hold = (next - s) * dur * 0.95;
      ins.epiano(b.keys.input, swung, JAZZ[chord], 0.07, hold);
    }

    // Triangle bass on the root, with extra notes once lofi livens up.
    if (lofi > 0.001) {
      if (LOFI_BASS[bar % 2].includes(s)) ins.tone(b.bass.input, swung, { type: 'triangle', note: root, vol: 0.5 * lofi, attack: 0.005, decay: 0.4, sustain: 0.6, hold: dur * 1.8, release: 0.2 });
      const extra = 0.42 * lofi * ramp(e, 0.15, 0.3);
      if (extra > 0.001 && LOFI_BASS_EXTRA.includes(s)) ins.tone(b.bass.input, swung, { type: 'triangle', note: root, vol: extra, attack: 0.005, decay: 0.25, sustain: 0.4, hold: dur * 1.2, release: 0.15 });
    }

    // Melody: a random walk over the pentatonic scale, busier with speed and intensity.
    const melodyVol = 0.18 * (1 - ramp(e, 0.45, 0.65));
    if (melodyVol > 0.001 && chance(0.12 + 0.35 * speed + 0.15 * e)) {
      this.melody = Math.max(0, Math.min(PENTA.length - 1, this.melody + [-2, -1, -1, 1, 1, 2][Math.floor(Math.random() * 6)]));
      let note = PENTA[this.melody];
      if (chord === E_CHORD && note % 12 === 7) note++;
      if (chance(0.15)) note += 12;
      b.melody.pan!.pan.setValueAtTime(Math.random() * 0.8 - 0.4, swung);
      ins.tone(b.melody.input, swung, { type: 'sine', note, vol: melodyVol, attack: 0.005, decay: 0.25, release: 0.3 });
      ins.tone(b.melody.input, swung, { type: 'triangle', note: note + 12, vol: melodyVol * 0.12, decay: 0.08 });
    }

    // Swung boom-bap; every 16th bar the drums drop out for a bar.
    if (lofiDrums > 0.001) {
      if (LOFI_KICK[bar % 2].includes(s)) ins.kick(b.kick.input, swung, 0.75 * lofiDrums, 0);
      if (s === 4 || s === 12) {
        ins.snare(b.drums.input, swung, 0.4 * lofiDrums, 0.2);
        if (chance(0.1)) ins.snare(b.drums.input, swung + dur, 0.18 * lofiDrums, 0.12);
        const clap = 0.22 * ramp(e, 0.25, 0.4) * lofiDrums;
        if (clap > 0.001) ins.clap(b.drums.input, swung + 0.006, clap);
      }
      // 8th hats (16ths in the 8th bar), loose and humanised.
      const hats16 = lastOf8;
      if ((s % 2 === 0 || hats16) && chance(0.85)) ins.hat(b.drums.input, swung, (0.05 + 0.08 * Math.random()) * lofiDrums, 0.03);
      // In-between 16ths once it livens up.
      if (!hats16 && s % 2 === 1 && chance(ramp(e, 0.15, 0.3) * (0.3 + 0.6 * speed))) ins.hat(b.drums.input, swung, 0.06 * lofiDrums, 0.02, 9000);
      // Ghost snares follow the player's speed.
      if (s !== 4 && s !== 12 && chance(0.3 * ramp(speed, 0.35, 0.6))) ins.snare(b.drums.input, swung, 0.07 * lofiDrums, 0.06);
    }

    // Vinyl crackle.
    const vinyl = 1 - ramp(e, 0.25, 0.5);
    if (vinyl > 0.001 && chance(0.15)) ins.crackle(b.vinyl.input, t + Math.random() * dur, (0.02 + 0.06 * Math.random()) * vinyl);

    // ══════════ HOUSE → EDM ══════════

    // Four-on-the-floor; in the last of 8 bars the final kick drops out before the downbeat.
    const kick = 0.85 * ramp(e, 0.35, 0.5);
    if (kick > 0.001 && s % 4 === 0 && !(lastOf8 && s === 12)) ins.kick(b.kick.input, t, kick, ramp(e, 0.35, 0.8));

    // Sidechain pump on the chords from every beat, deeper as it gets more intense.
    if (s % 4 === 0) {
      const depth = 0.85 * ramp(e, 0.45, 0.7);
      g.pump.gain.setValueAtTime(1 - depth, t);
      g.pump.gain.setTargetAtTime(1, t + 0.01, dur * 1.1);
    }

    const clap = 0.5 * ramp(e, 0.5, 0.6);
    if (clap > 0.001 && (s === 4 || s === 12)) ins.clap(b.drums.input, t, clap);

    const offHat = 0.28 * ramp(e, 0.4, 0.55);
    if (offHat > 0.001 && s % 4 === 2) ins.hat(b.drums.input, t, offHat, 0.09, 6000);

    const hats = ramp(e, 0.7, 0.85);
    if (hats > 0.001 && s % 4 !== 2 && chance(0.85 * hats)) ins.hat(b.drums.input, t, 0.09 + 0.05 * (s % 2 === 0 ? 1 : 0), 0.025, 8500);

    // Offbeat saw bass, replaced by a rolling bass at the top.
    const cutoff = 500 + 1500 * e;
    const offBass = 0.3 * ramp(e, 0.45, 0.6) * (1 - ramp(e, 0.82, 0.92));
    if (offBass > 0.001 && s % 4 === 2) ins.tone(b.bass.input, t, { type: 'sawtooth', note: root, vol: offBass, decay: 0.16, sustain: 0.2, hold: dur * 1.5, release: 0.05, cutoff: cutoff * 2, cutoffEnd: cutoff, q: 3 });
    const roll = 0.26 * ramp(e, 0.82, 0.92);
    if (roll > 0.001 && s % 4 !== 0) ins.tone(b.bass.input, t, { type: 'sawtooth', note: root, vol: roll, decay: 0.09, sustain: 0.1, hold: dur * 0.8, release: 0.03, cutoff: (800 + 1800 * e) * 1.8, cutoffEnd: 600, q: 4 });

    // Two detuned saws per chord note, held for the chord; the pump makes them breathe.
    if ((s === 0 || (lastOf8 && s === 8)) && ramp(e, 0.5, 0.65) > 0.001) {
      const len = (lastOf8 ? 8 : STEPS) * dur;
      const notes = [ROOTS[chord] + 12, ...TRIADS[chord]];
      for (const note of notes)
        for (const detune of [-9, 9]) ins.tone(b.pad.input, t, { type: 'sawtooth', note, detune, vol: 0.034, attack: 0.02, decay: 0.3, sustain: 0.8, hold: len, release: 0.12 });
    }

    // Arpeggiated square lead over the chord tones, sweeping across the stereo field.
    const arp = 0.1 * ramp(e, 0.75, 0.9);
    if (arp > 0.001) {
      const i = ARP[s];
      const tri = TRIADS[chord];
      b.arp.pan!.pan.setValueAtTime(0.4 * Math.sin((s / STEPS) * Math.PI * 4), t);
      ins.tone(b.arp.input, t, { type: 'square', note: tri[i % 3] + 12 * Math.floor(i / 3), vol: arp, decay: 0.13, release: 0.04 });
    }

    // Build every 8 bars: a snare roll and a noise riser into the next downbeat, then a crash.
    const build = ramp(e, 0.65, 0.8);
    if (build > 0.001 && lastOf8) {
      const p = s / STEPS;
      ins.snare(b.fx.input, t, (0.04 + 0.26 * p * p) * build, 0.08);
      if (s >= 8) ins.snare(b.fx.input, t + dur / 2, (0.04 + 0.26 * p * p) * build, 0.06);
      if (s === 0) ins.riser(b.fx.input, t, STEPS * dur, 0.22 * build);
    }
    const crash = 0.12 * ramp(e, 0.6, 0.75);
    if (crash > 0.001 && bar > 0 && bar % 8 === 0 && s === 0) ins.crash(b.fx.input, t, crash);
  }
}
