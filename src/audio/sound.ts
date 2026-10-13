/**
 * All sound effects are synthesised at runtime with the Web Audio API, so the
 * game ships without any audio files (and without licensing questions).
 */
type Wave = OscillatorType;

export class Sound {
  enabled = true;
  private ctx: AudioContext | null = null;
  private master: GainNode | null = null;
  private noise: AudioBuffer | null = null;

  /** Must be called from a user gesture before the first sound can play. */
  unlock(): void {
    if (!this.ctx) {
      const Ctor = window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
      if (!Ctor) return;
      this.ctx = new Ctor();
      this.master = this.ctx.createGain();
      this.master.gain.value = 0.5;
      this.master.connect(this.ctx.destination);
    }
    if (this.ctx.state === 'suspended') void this.ctx.resume();
  }

  suspend(): void {
    if (this.ctx?.state === 'running') void this.ctx.suspend();
  }

  private ready(): AudioContext | null {
    return this.enabled && this.ctx && this.ctx.state === 'running' ? this.ctx : null;
  }

  private tone(freq: number, start: number, dur: number, opts: { type?: Wave; vol?: number; to?: number } = {}) {
    const ctx = this.ctx!;
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.type = opts.type ?? 'sine';
    osc.frequency.setValueAtTime(freq, start);
    if (opts.to) osc.frequency.exponentialRampToValueAtTime(opts.to, start + dur);
    const vol = opts.vol ?? 0.3;
    gain.gain.setValueAtTime(0.0001, start);
    gain.gain.exponentialRampToValueAtTime(vol, start + 0.008);
    gain.gain.exponentialRampToValueAtTime(0.0001, start + dur);
    osc.connect(gain).connect(this.master!);
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
    this.tone(180, t, 0.12, { type: 'triangle', vol: 0.4, to: 90 });
    this.burst(t, 0.06, 0.25, 1800);
  }

  invalid(): void {
    const ctx = this.ready();
    if (!ctx) return;
    this.tone(220, ctx.currentTime, 0.12, { type: 'sine', vol: 0.15, to: 160 });
  }

  /** Rising arpeggio; higher and longer with more lines and bigger combos. */
  clear(lines: number, combo: number): void {
    const ctx = this.ready();
    if (!ctx) return;
    const t = ctx.currentTime;
    const scale = [0, 4, 7, 12, 16, 19, 24];
    const root = 392 * Math.pow(2, Math.min(combo - 1, 7) / 12);
    const notes = Math.min(scale.length, 2 + lines);
    for (let i = 0; i < notes; i++) {
      const f = root * Math.pow(2, scale[i] / 12);
      this.tone(f, t + i * 0.055, 0.25, { type: 'square', vol: 0.07 });
      this.tone(f * 2, t + i * 0.055, 0.2, { type: 'sine', vol: 0.1 });
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

  gameOver(): void {
    const ctx = this.ready();
    if (!ctx) return;
    const t = ctx.currentTime;
    for (const [i, f] of [392, 330, 262, 196].entries()) {
      this.tone(f, t + i * 0.16, 0.3, { type: 'triangle', vol: 0.2 });
    }
  }
}
