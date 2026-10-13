"""Synthesises the promo soundtrack into promo/media/music.wav.

Like the game itself (src/audio/sound.ts), nothing is sampled: a 120 BPM beat, bass, arpeggio and
pad are generated from oscillators and noise, and the game's own sound effects (pick up, place,
line clear, board clear) are re-created and placed on the frames where they happen in the video.

Usage: python3 promo/audio/music.py   (needs numpy)
"""

from pathlib import Path
import wave

import numpy as np

SR = 44100
LENGTH = 33.0
BEAT = 0.5  # 120 BPM
rng = np.random.default_rng(7)

# Scene starts in the video (seconds), see promo/src/scenes: every scene after the intro
# overlaps the previous one by 0.4 s.
INTRO, GAMEPLAY, FEATURES, BEST, ANYWHERE, OUTRO = 0.0, 4.0, 12.1133, 18.3133, 22.7133, 27.3133
CLIP = GAMEPLAY + 0.45  # the gameplay recording starts playing here
DROP = GAMEPLAY  # the beat kicks in with the gameplay

out = np.zeros((int(SR * LENGTH), 2))


def t_axis(dur):
    return np.arange(int(SR * dur)) / SR


def add(start, signal, gain=1.0, pan=0.0):
    i = int(start * SR)
    if i >= len(out):
        return
    signal = signal[: len(out) - i]
    left, right = np.sqrt(0.5 * (1 - pan)), np.sqrt(0.5 * (1 + pan))
    out[i : i + len(signal), 0] += signal * gain * left
    out[i : i + len(signal), 1] += signal * gain * right


def env(n, attack=0.005, release=None, dur=None):
    """Linear attack, exponential decay envelope over n samples."""
    t = np.arange(n) / SR
    a = np.clip(t / attack, 0, 1)
    if release is None:
        return a
    return a * np.exp(-t / release)


def osc(kind, freq, dur, harmonics=10):
    t = t_axis(dur)
    if np.isscalar(freq):
        phase = 2 * np.pi * freq * t
    else:
        phase = 2 * np.pi * np.cumsum(freq) / SR
    if kind == "sine":
        return np.sin(phase)
    if kind == "triangle":
        return sum(((-1) ** k) * np.sin((2 * k + 1) * phase) / (2 * k + 1) ** 2 for k in range(5)) * 8 / np.pi**2
    if kind == "square":
        return sum(np.sin((2 * k + 1) * phase) / (2 * k + 1) for k in range(harmonics // 2)) * 4 / np.pi
    if kind == "saw":
        return sum(np.sin(k * phase) / k for k in range(1, harmonics)) * 2 / np.pi
    raise ValueError(kind)


def lowpass(x, cutoff):
    """One-pole low-pass filter (vectorised in blocks is overkill here; signals are short)."""
    a = np.exp(-2 * np.pi * cutoff / SR)
    y = np.empty_like(x)
    acc = 0.0
    for i, v in enumerate(x):
        acc = (1 - a) * v + a * acc
        y[i] = acc
    return y


def noise(dur):
    return rng.uniform(-1, 1, int(SR * dur))


def hz(midi):
    return 440.0 * 2 ** ((midi - 69) / 12)


# --- Game sound effects (same recipes as src/audio/sound.ts) -----------------------------------


def tone(start, freq, dur, kind="sine", vol=0.3, to=None, pan=0.0):
    n = int(SR * dur)
    f = freq if to is None else freq * (to / freq) ** (np.arange(n) / n)
    add(start, osc(kind, f, dur) * env(n, 0.008, dur / 5), vol, pan)


def burst(start, dur, vol, cutoff):
    x = noise(dur)
    if cutoff < 4000:
        x = lowpass(x, cutoff)
    add(start, x * env(len(x), 0.002, dur / 5), vol)


def pickup(start):
    tone(start, 520, 0.07, "triangle", 0.12, to=700)


def place(start, vol=1.0):
    tone(start, 180, 0.12, "triangle", 0.4 * vol, to=90)
    burst(start, 0.06, 0.25 * vol, 1800)


def clear(start, lines, combo):
    scale = [0, 4, 7, 12, 16, 19, 24]
    root = 392 * 2 ** (min(combo - 1, 7) / 12)
    for i in range(min(len(scale), 2 + lines)):
        f = root * 2 ** (scale[i] / 12)
        tone(start + i * 0.055, f, 0.25, "square", 0.07)
        tone(start + i * 0.055, f * 2, 0.2, "sine", 0.1)
    burst(start, 0.25, 0.12, 6000)


def board_clear(start):
    for i, f in enumerate([523.25, 659.25, 783.99, 1046.5]):
        tone(start + 0.25 + i * 0.08, f, 0.6, "triangle", 0.18)


def pluck(start, midi, vol=0.12, pan=0.0):
    tone(start, hz(midi), 0.35, "triangle", vol, pan=pan)
    tone(start, hz(midi + 12), 0.15, "sine", vol * 0.5, pan=pan)


def whoosh(start, dur=0.55, vol=0.16):
    n = int(SR * dur)
    x = noise(dur)
    # Rising filtered noise: blend low-passed and raw noise over time.
    lp = lowpass(x, 900)
    mix = np.linspace(0, 1, n) ** 2
    shape = np.sin(np.linspace(0, np.pi, n)) ** 2
    add(start - dur * 0.7, (lp * (1 - mix) + x * mix * 0.5) * shape, vol)


# --- Music -------------------------------------------------------------------------------------

# C - G - Am - F, one bar (4 beats) each.
CHORDS = [[48, 52, 55, 60], [43, 47, 50, 55], [45, 48, 52, 57], [41, 45, 48, 53]]


def chord_at(time):
    return CHORDS[int(time / (4 * BEAT)) % 4]


def kick(start, vol=0.42):
    dur = 0.32
    n = int(SR * dur)
    f = 45 + 110 * np.exp(-np.arange(n) / SR / 0.04)
    add(start, osc("sine", f, dur) * env(n, 0.002, 0.11), vol)


def hat(start, vol=0.06, open_=False):
    dur = 0.18 if open_ else 0.05
    x = np.diff(noise(dur + 1 / SR))
    add(start, x * env(len(x), 0.001, dur / 4), vol, pan=0.25)


def snare(start, vol=0.22):
    dur = 0.22
    x = noise(dur) - lowpass(noise(dur), 1500) * 0.5
    n = len(x)
    add(start, x * env(n, 0.001, 0.05) + osc("sine", 190, dur) * env(n, 0.001, 0.04) * 0.6, vol)


def bass(start, midi, dur, vol=0.22):
    n = int(SR * dur)
    x = lowpass(osc("saw", hz(midi), dur, 8), 700)
    add(start, x * env(n, 0.004, dur * 0.8), vol)


def pad(start, notes, dur, vol=0.05):
    n = int(SR * dur)
    shape = np.minimum(np.clip(np.arange(n) / (SR * 0.4), 0, 1), np.clip((n - np.arange(n)) / (SR * 0.5), 0, 1))
    for i, m in enumerate(notes):
        for detune in (-0.08, 0.08):
            x = osc("saw", hz(m + 12 + detune), dur, 6)
            add(start, x * shape, vol / len(notes), pan=detune * 6 * (1 if i % 2 else -1))


def music():
    bar = 4 * BEAT
    # Pad all the way through, the arpeggio from the start, the beat from the drop.
    t = 0.0
    while t < LENGTH - 1:
        pad(t, chord_at(t + 0.01), bar + 0.4, 0.07 if t >= DROP else 0.1)
        t += bar
    step = BEAT / 4
    for i in range(int((LENGTH - 1.5) / step)):
        time = i * step
        notes = chord_at(time)
        pattern = [0, 2, 3, 1, 2, 3, 0, 3]
        midi = notes[pattern[i % 8]] + 24
        gain = 0.07 if time < DROP else 0.05
        pluck(time, midi, gain, pan=0.35 if i % 2 else -0.35)
    beat = DROP
    while beat < LENGTH - 2.0:
        k = round((beat - DROP) / BEAT)
        kick(beat)
        hat(beat + BEAT / 2, open_=k % 4 == 3)
        hat(beat + BEAT / 4, 0.03)
        hat(beat + 3 * BEAT / 4, 0.03)
        if k % 2 == 1:
            snare(beat)
        root = chord_at(beat + 0.01)[0] - 12
        bass(beat, root, BEAT / 2 - 0.02)
        bass(beat + BEAT / 2, root + (12 if k % 4 == 3 else 0), BEAT / 2 - 0.02)
        beat += BEAT
    # Final chord rings out over the end card.
    end = DROP + round((LENGTH - 2.0 - DROP) / BEAT) * BEAT
    kick(end, 0.6)
    pad(end, [48, 52, 55, 60, 64], 2.2, 0.09)
    for i, m in enumerate([72, 76, 79, 84]):
        pluck(end + i * 0.06, m, 0.09)


def effects():
    # Intro: the logo blocks land one by one, then sparkle.
    for i in range(7):
        place(INTRO + 0.24 + i * 0.09, 0.45)
    clear(INTRO + 0.95, 3, 1)
    whoosh(GAMEPLAY + 0.2)
    # Gameplay clip, frame numbers from promo/capture/capture.mjs.
    f = lambda frame: CLIP + frame / 30
    pickup(f(21)); place(f(48)); clear(f(48), 1, 1)
    pickup(f(84)); place(f(111)); clear(f(111), 2, 2)
    pickup(f(157)); place(f(184)); clear(f(184), 2, 3); board_clear(f(184))
    # Features: one pluck per card.
    whoosh(FEATURES + 0.2)
    for i, m in enumerate([72, 74, 76, 79, 81, 84]):
        pluck(FEATURES + 0.75 + i * 0.2, m, 0.1, pan=-0.3 if i % 2 == 0 else 0.3)
    # Best score: the counter ticks up, then the stamp lands.
    whoosh(BEST + 0.2)
    for i in range(18):
        p = i / 17
        time = BEST + 0.8 + 1.5 * (1 - (1 - p) ** (1 / 3))
        tone(time, 900 + 600 * p, 0.04, "square", 0.025)
    place(BEST + 2.35, 0.8)
    board_clear(BEST + 2.35 - 0.25)
    whoosh(ANYWHERE + 0.2)
    for i in range(4):
        pluck(ANYWHERE + 1.2 + i * 0.18, [67, 71, 74, 79][i], 0.1)
    # Outro: the logo again, then the call to action.
    whoosh(OUTRO + 0.2)
    for i in range(7):
        place(OUTRO + 0.24 + i * 0.09, 0.45)
    clear(OUTRO + 0.95, 4, 2)
    board_clear(OUTRO + 1.55 - 0.25)


music()
effects()

# Master: gentle soft clip and normalisation to -1 dBFS.
out = np.tanh(out * 1.4) / np.tanh(1.4)
out *= 10 ** (-1 / 20) / np.max(np.abs(out))
fade = int(SR * 0.02)
out[:fade] *= np.linspace(0, 1, fade)[:, None]

path = Path(__file__).resolve().parents[1] / "media" / "music.wav"
with wave.open(str(path), "wb") as w:
    w.setnchannels(2)
    w.setsampwidth(2)
    w.setframerate(SR)
    w.writeframes((out * 32767).astype("<i2").tobytes())
print(f"{path.name}: {LENGTH:.1f}s")
