import type { SoundProfile } from '../render/styles';
import { audioContext } from './context';

/**
 * All sound effects are synthesised at runtime with the Web Audio API, so the
 * game ships without any audio files (and without licensing questions). Each
 * style tier has its own sound profile: instrument, scale, echo and extras.
 */
type Wave = OscillatorType;

const MASTER = 0.5;

const DEFAULT_PROFILE: SoundProfile = { lead: 'square', root: 392, scale: [0, 4, 7, 12, 16, 19, 24], echo: 0, bell: false, pad: false, thud: 180 };

export class Sound {
  profile: SoundProfile = DEFAULT_PROFILE;
  private ctx: AudioContext | null = null;
  private master: GainNode | null = null;
  /** Input of the echo effect; tones send part of their signal here. */
  private echoIn: GainNode | null = null;
  private noise: AudioBuffer | null = null;
  private level = 1;

  /** Volume 0..1 (0 = silent), on a perceptual curve. */
  get volume(): number {
    return this.level;
  }

  set volume(v: number) {
    this.level = Math.min(1, Math.max(0, v));
    if (this.ctx && this.master) this.master.gain.setTargetAtTime(MASTER * this.level ** 2, this.ctx.currentTime, 0.02);
  }

  /** Must be called from a user gesture before the first sound can play. */
  unlock(): void {
    if (!this.ctx) {
      this.ctx = audioContext();
      if (!this.ctx) return;
      this.master = this.ctx.createGain();
      this.master.gain.value = MASTER * this.level ** 2;
      this.master.connect(this.ctx.destination);
      // Feedback delay for the echo of higher style tiers.
      const delay = this.ctx.createDelay(1);
      delay.delayTime.value = 0.16;
      const feedback = this.ctx.createGain();
      feedback.gain.value = 0.38;
      const damp = this.ctx.createBiquadFilter();
      damp.type = 'lowpass';
      damp.frequency.value = 3200;
      this.echoIn = this.ctx.createGain();
      this.echoIn.connect(delay);
      delay.connect(damp).connect(feedback).connect(delay);
      damp.connect(this.master);
    }
    if (this.ctx.state === 'suspended') void this.ctx.resume();
  }

  /** Resumes a context that was suspended in the background; never creates one. */
  resume(): void {
    if (this.ctx?.state === 'suspended') void this.ctx.resume();
  }

  suspend(): void {
    if (this.ctx?.state === 'running') void this.ctx.suspend();
  }

  private ready(): AudioContext | null {
    return this.level > 0 && this.ctx && this.ctx.state === 'running' ? this.ctx : null;
  }

  private tone(freq: number, start: number, dur: number, opts: { type?: Wave; vol?: number; to?: number; attack?: number; echo?: number; detune?: number } = {}) {
    const ctx = this.ctx!;
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.type = opts.type ?? 'sine';
    osc.frequency.setValueAtTime(freq, start);
    if (opts.detune) osc.detune.value = opts.detune;
    if (opts.to) osc.frequency.exponentialRampToValueAtTime(opts.to, start + dur);
    const vol = opts.vol ?? 0.3;
    gain.gain.setValueAtTime(0.0001, start);
    gain.gain.exponentialRampToValueAtTime(vol, start + (opts.attack ?? 0.008));
    gain.gain.exponentialRampToValueAtTime(0.0001, start + dur);
    let out: AudioNode = osc;
    if (osc.type === 'sawtooth') {
      // Raw saws are harsh; soften them like an analogue synth.
      const filter = ctx.createBiquadFilter();
      filter.type = 'lowpass';
      filter.frequency.value = 2400;
      out = osc.connect(filter);
    }
    out.connect(gain).connect(this.master!);
    const echo = opts.echo ?? this.profile.echo;
    if (echo > 0 && this.echoIn) {
      const send = ctx.createGain();
      send.gain.value = echo;
      gain.connect(send).connect(this.echoIn);
    }
    osc.start(start);
    osc.stop(start + dur + 0.02);
  }

  private burst(start: number, dur: number, vol: number, cutoff: number) {
    const ctx = this.ctx!;
    if (!this.noise) {
      this.noise = ctx.createBuffer(1, ctx.sampleRate * 0.5, ctx.sampleRate);
      const data = this.noise.getChannelData(0);
      for (let i = 0; i < data.length; i++) data[i] = Math.random() * 2 - 1;
    }
    const src = ctx.createBufferSource();
    src.buffer = this.noise;
    const filter = ctx.createBiquadFilter();
    filter.type = 'lowpass';
    filter.frequency.value = cutoff;
    const gain = ctx.createGain();
    gain.gain.setValueAtTime(vol, start);
    gain.gain.exponentialRampToValueAtTime(0.0001, start + dur);
    src.connect(filter).connect(gain).connect(this.master!);
    src.start(start);
    src.stop(start + dur);
  }

  pickup(): void {
    const ctx = this.ready();
    if (!ctx) return;
    this.tone(520, ctx.currentTime, 0.07, { type: 'triangle', vol: 0.12, to: 700 });
  }

  place(): void {
    const ctx = this.ready();
    if (!ctx) return;
    const t = ctx.currentTime;
    const f = this.profile.thud;
    this.tone(f, t, 0.12, { type: 'triangle', vol: 0.4, to: f / 2, echo: 0 });
    this.burst(t, 0.06, 0.25, 1800);
  }

  invalid(): void {
    const ctx = this.ready();
    if (!ctx) return;
    this.tone(220, ctx.currentTime, 0.12, { type: 'sine', vol: 0.15, to: 160 });
  }

  /** Volume that keeps the lead waveform roughly equally loud across profiles. */
  private leadVol(): number {
    return { square: 0.07, sawtooth: 0.08, triangle: 0.14, sine: 0.16, custom: 0.1 }[this.profile.lead];
  }

  /** Rising arpeggio; higher and longer with more lines and bigger combos. */
  clear(lines: number, combo: number): void {
    const ctx = this.ready();
    if (!ctx) return;
    const t = ctx.currentTime;
    const { scale, lead, bell, pad } = this.profile;
    const root = this.profile.root * Math.pow(2, Math.min(combo - 1, 7) / 12);
    const notes = Math.min(scale.length, 2 + lines);
    for (let i = 0; i < notes; i++) {
      const f = root * Math.pow(2, scale[i] / 12);
      this.tone(f, t + i * 0.055, 0.25, { type: lead, vol: this.leadVol() });
      this.tone(f * 2, t + i * 0.055, 0.2, { type: 'sine', vol: 0.1 });
      if (bell) this.tone(f * 4, t + i * 0.055 + 0.02, 0.35, { type: 'sine', vol: 0.035 });
    }
    if (pad) {
      // A soft chord under the arpeggio.
      for (const step of [0, 7, 12]) {
        this.tone((root / 2) * Math.pow(2, step / 12), t, 0.9, { type: 'triangle', vol: 0.06, attack: 0.08 });
      }
    }
    this.burst(t, 0.25, 0.12, 6000);
  }

  boardClear(): void {
    const ctx = this.ready();
    if (!ctx) return;
    const t = ctx.currentTime + 0.25;
    for (const [i, f] of [523.25, 659.25, 783.99, 1046.5].entries()) {
      this.tone(f, t + i * 0.08, 0.6, { type: 'triangle', vol: 0.18 });
    }
  }

  /** A sparkly upward sweep when the board clear switches the style. */
  styleShift(): void {
    const ctx = this.ready();
    if (!ctx) return;
    const t = ctx.currentTime + 0.7;
    const root = this.profile.root * 2;
    this.profile.scale.forEach((step, i) => {
      this.tone(root * Math.pow(2, step / 12), t + i * 0.035, 0.3, { type: 'sine', vol: 0.06, echo: 0.3 });
    });
  }

  /** A fanfare when a new style tier is unlocked; grander for higher tiers. */
  tierUp(level: number): void {
    const ctx = this.ready();
    if (!ctx) return;
    const t = ctx.currentTime + 0.2;
    const root = 261.63;
    // I - IV - V - I, adding voices and length as the tiers rise (Neon is level 7, Aurora 8, Legendary 9).
    const chords = [
      [0, 4, 7],
      [5, 9, 12],
      [7, 11, 14],
      [12, 16, 19, 24],
    ].slice(level >= 7 ? 0 : 2);
    const beat = 0.16;
    chords.forEach((chord, i) => {
      const last = i === chords.length - 1;
      for (const step of chord) {
        const f = root * Math.pow(2, step / 12);
        this.tone(f, t + i * beat, last ? 1.2 : 0.3, { type: this.profile.lead, vol: this.leadVol() * 0.9, echo: 0.3 });
        this.tone(f * 2, t + i * beat, last ? 0.9 : 0.25, { type: 'sine', vol: 0.05, echo: 0.3 });
        if (level >= 8) this.tone(f / 2, t + i * beat, last ? 1.4 : 0.35, { type: 'triangle', vol: 0.08, attack: 0.04 });
      }
    });
    if (level >= 9) {
      // Shimmering cascade on top for the final tier.
      for (let i = 0; i < 8; i++) {
        this.tone(1046.5 * Math.pow(2, [0, 4, 7, 12][i % 4] / 12 + Math.floor(i / 4)), t + chords.length * beat + i * 0.05, 0.4, { type: 'sine', vol: 0.03, echo: 0.5 });
      }
    }
  }

  gameOver(): void {
    const ctx = this.ready();
    if (!ctx) return;
    const t = ctx.currentTime;
    for (const [i, f] of [392, 330, 262, 196].entries()) {
      this.tone(f, t + i * 0.16, 0.3, { type: 'triangle', vol: 0.2 });
    }
  }
}
