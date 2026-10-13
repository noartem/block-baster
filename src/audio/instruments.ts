/**
 * Synthesised instruments for the soundtrack. Every hit creates a few
 * short-lived Web Audio nodes that are released once the note ends.
 */

export const midiToHz = (m: number): number => 440 * 2 ** ((m - 69) / 12);

export interface ToneOpts {
  type: OscillatorType;
  /** MIDI note number. */
  note: number;
  vol: number;
  attack?: number;
  decay?: number;
  /** Sustain level relative to `vol`, 0..1. */
  sustain?: number;
  release?: number;
  /** How long the note is held before the release starts, in seconds. */
  hold?: number;
  detune?: number;
  /** Optional per-note low-pass with an envelope: from `cutoff` down to `cutoffEnd` over the decay. */
  cutoff?: number;
  cutoffEnd?: number;
  q?: number;
}

export class Instruments {
  private noise: AudioBuffer;
  private pink: AudioBuffer;

  constructor(private ctx: BaseAudioContext) {
    this.noise = makeNoise(ctx, 1, false);
    this.pink = makeNoise(ctx, 2, true);
  }

  /** Attack, then decay to sustain, hold, release. Returns when the sound is silent. */
  private envelope(param: AudioParam, t: number, vol: number, o: { attack?: number; decay?: number; sustain?: number; hold?: number; release?: number }): number {
    const a = o.attack ?? 0.003;
    const d = o.decay ?? 0.2;
    const s = o.sustain ?? 0;
    const r = o.release ?? 0.05;
    const end = t + Math.max(a, o.hold ?? a + d);
    param.setValueAtTime(0, t);
    param.linearRampToValueAtTime(vol, t + a);
    param.setTargetAtTime(vol * s, t + a, d / 4);
    param.setTargetAtTime(0, end, r / 4);
    return end + r * 1.5;
  }

  tone(dest: AudioNode, t: number, o: ToneOpts, extra?: (osc: OscillatorNode) => void): void {
    const ctx = this.ctx;
    const osc = ctx.createOscillator();
    osc.type = o.type;
    osc.frequency.value = midiToHz(o.note);
    if (o.detune) osc.detune.value = o.detune;
    const gain = ctx.createGain();
    let out: AudioNode = osc;
    if (o.cutoff) {
      const f = ctx.createBiquadFilter();
      f.type = 'lowpass';
      f.Q.value = o.q ?? 0.7;
      f.frequency.setValueAtTime(o.cutoff, t);
      if (o.cutoffEnd) f.frequency.setTargetAtTime(o.cutoffEnd, t, (o.decay ?? 0.2) / 3);
      osc.connect(f);
      out = f;
    }
    out.connect(gain).connect(dest);
    const stop = this.envelope(gain.gain, t, o.vol, o);
    extra?.(osc);
    osc.start(t);
    osc.stop(stop);
  }

  /** A chord on a two-operator FM electric piano, with a gentle shared vibrato. */
  epiano(dest: AudioNode, t: number, notes: number[], vol: number, hold: number): void {
    const ctx = this.ctx;
    const stop = t + hold + 2;
    const lfo = ctx.createOscillator();
    lfo.frequency.value = 5;
    const vibrato = ctx.createGain();
    vibrato.gain.value = 8; // cents
    lfo.connect(vibrato);
    lfo.start(t);
    lfo.stop(stop);
    for (const note of notes) {
      const f = midiToHz(note);
      const mod = ctx.createOscillator();
      mod.frequency.value = f;
      const depth = ctx.createGain();
      // Bright tine at the attack that settles into a soft, round tone.
      depth.gain.setValueAtTime(f * 1.8, t);
      depth.gain.setTargetAtTime(f * 1.8 * 0.15, t, 0.25);
      mod.connect(depth);
      // Players never hit all keys at once.
      const at = t + Math.random() * 0.012;
      this.tone(dest, at, { type: 'sine', note, vol, attack: 0.01, decay: 1.6, sustain: 0.35, hold, release: 1.2 }, (osc) => {
        depth.connect(osc.frequency);
        vibrato.connect(osc.detune);
      });
      mod.start(at);
      mod.stop(stop);
    }
  }

  private noiseSource(t: number, dur: number, pink = false): AudioBufferSourceNode {
    const src = this.ctx.createBufferSource();
    const buf = pink ? this.pink : this.noise;
    src.buffer = buf;
    src.loop = true;
    src.start(t, Math.random() * (buf.duration - 0.1));
    src.stop(t + dur);
    return src;
  }

  private filter(type: BiquadFilterType, freq: number, q = 0.7): BiquadFilterNode {
    const f = this.ctx.createBiquadFilter();
    f.type = type;
    f.frequency.value = freq;
    f.Q.value = q;
    return f;
  }

  /** Noise through a filter with a percussive envelope. */
  noiseHit(dest: AudioNode, t: number, vol: number, o: { type: BiquadFilterType; freq: number; q?: number; decay: number; attack?: number }): void {
    const gain = this.ctx.createGain();
    const stop = this.envelope(gain.gain, t, vol, { attack: o.attack ?? 0.001, decay: o.decay });
    this.noiseSource(t, stop - t).connect(this.filter(o.type, o.freq, o.q)).connect(gain).connect(dest);
  }

  /** Sine kick with a pitch drop; `punch` 0 = soft and round (lofi), 1 = tight and clicky (house). */
  kick(dest: AudioNode, t: number, vol: number, punch: number): void {
    const ctx = this.ctx;
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    const from = 120 + 110 * punch;
    const to = 46 + 6 * punch;
    const decay = 0.42 - 0.12 * punch;
    osc.frequency.setValueAtTime(from, t);
    osc.frequency.exponentialRampToValueAtTime(to, t + 0.07 + 0.04 * (1 - punch));
    gain.gain.setValueAtTime(0, t);
    gain.gain.linearRampToValueAtTime(vol, t + 0.002);
    gain.gain.setTargetAtTime(0, t + 0.02, decay / 4);
    osc.connect(gain).connect(dest);
    osc.start(t);
    osc.stop(t + decay * 1.6);
    // The beater click: what's left of the kick on small phone speakers.
    this.noiseHit(dest, t, vol * (0.1 + 0.25 * punch), { type: 'bandpass', freq: 2200 + 1800 * punch, q: 0.8, decay: 0.012 });
  }

  snare(dest: AudioNode, t: number, vol: number, decay = 0.16): void {
    this.noiseHit(dest, t, vol, { type: 'bandpass', freq: 1900, q: 0.6, decay });
    this.tone(dest, t, { type: 'triangle', note: 54, vol: vol * 0.6, decay: decay * 0.5 });
  }

  clap(dest: AudioNode, t: number, vol: number): void {
    const gain = this.ctx.createGain();
    const g = gain.gain;
    g.setValueAtTime(0, t);
    // Three quick slaps and a short tail, like several hands at once.
    for (const dt of [0, 0.011, 0.022]) {
      g.setValueAtTime(vol, t + dt);
      g.setTargetAtTime(vol * 0.15, t + dt, 0.003);
    }
    g.setValueAtTime(vol * 0.8, t + 0.033);
    g.setTargetAtTime(0, t + 0.033, 0.045);
    this.noiseSource(t, 0.3).connect(this.filter('bandpass', 1150, 1.1)).connect(gain).connect(dest);
  }

  hat(dest: AudioNode, t: number, vol: number, decay = 0.035, freq = 7500): void {
    this.noiseHit(dest, t, vol, { type: 'highpass', freq, decay });
  }

  crash(dest: AudioNode, t: number, vol: number): void {
    this.noiseHit(dest, t, vol, { type: 'highpass', freq: 4500, decay: 1.6 });
  }

  /** Noise that rises in volume and brightness over `dur` seconds, leading into a drop. */
  riser(dest: AudioNode, t: number, dur: number, vol: number): void {
    const f = this.filter('bandpass', 400, 1.2);
    f.frequency.setValueAtTime(400, t);
    f.frequency.exponentialRampToValueAtTime(9000, t + dur);
    const gain = this.ctx.createGain();
    gain.gain.setValueAtTime(0, t);
    gain.gain.linearRampToValueAtTime(vol, t + dur);
    gain.gain.linearRampToValueAtTime(0, t + dur + 0.03);
    this.noiseSource(t, dur + 0.05).connect(f).connect(gain).connect(dest);
  }

  /** Vinyl crackle: a tiny bright click. */
  crackle(dest: AudioNode, t: number, vol: number): void {
    this.noiseHit(dest, t, vol, { type: 'highpass', freq: 2500, decay: 0.004 + Math.random() * 0.012 });
  }

  /** Endless soft hiss, started and stopped by the caller. */
  hiss(dest: AudioNode, t: number): AudioBufferSourceNode {
    const src = this.ctx.createBufferSource();
    src.buffer = this.pink;
    src.loop = true;
    src.connect(dest);
    src.start(t);
    return src;
  }
}

function makeNoise(ctx: BaseAudioContext, seconds: number, pink: boolean): AudioBuffer {
  const buf = ctx.createBuffer(1, Math.floor(ctx.sampleRate * seconds), ctx.sampleRate);
  const d = buf.getChannelData(0);
  // Paul Kellet's economy pink filter.
  let b0 = 0;
  let b1 = 0;
  let b2 = 0;
  for (let i = 0; i < d.length; i++) {
    const w = Math.random() * 2 - 1;
    if (!pink) {
      d[i] = w;
      continue;
    }
    b0 = 0.99765 * b0 + w * 0.099046;
    b1 = 0.963 * b1 + w * 0.2965164;
    b2 = 0.57 * b2 + w * 1.0526913;
    d[i] = (b0 + b1 + b2 + w * 0.1848) * 0.2;
  }
  return buf;
}

/** Stereo impulse response of a soft room: decaying noise, a little darker over time. */
export function makeReverb(ctx: BaseAudioContext, seconds: number): AudioBuffer {
  const len = Math.floor(ctx.sampleRate * seconds);
  const buf = ctx.createBuffer(2, len, ctx.sampleRate);
  for (let ch = 0; ch < 2; ch++) {
    const d = buf.getChannelData(ch);
    let lp = 0;
    for (let i = 0; i < len; i++) {
      const p = i / len;
      const k = 0.5 + 0.45 * p;
      lp = lp * k + (Math.random() * 2 - 1) * (1 - k);
      d[i] = lp * (1 - p) ** 2.5 * 2.2;
    }
  }
  return buf;
}
