"""Builds the promo soundtrack into promo/media/music.wav.

The music is the game's own adaptive soundtrack (src/audio/music.rs), rendered offline by the
game's `soundtrack` binary (src/bin/soundtrack.rs). It follows the video like it follows a game: calm
lofi over the intro, livelier with the gameplay, house through the styles and features, and full
EDM from the best score on. On top, the game's sound effects (pick up, place, line clear, board
clear) are re-created and placed on the frames where they happen in the video.

Usage (from the repository root): python3 promo/audio/music.py   (needs numpy and cargo)
"""

from pathlib import Path
import json
import subprocess
import tempfile
import wave

import numpy as np

SR = 44100
rng = np.random.default_rng(7)

# Scene lengths in seconds, as in promo/src/scenes (DURATION of each scene). Every scene after the
# intro overlaps the previous one by CROSSFADE.
CROSSFADE = 0.4
CLIP_FRAMES = 268  # length of media/gameplay.mp4
SCENES = [
    ("intro", 4.4),
    ("gameplay", 0.45 + CLIP_FRAMES / 30),
    ("styles", 5.8),
    ("features", 6.6),
    ("best", 4.8),
    ("anywhere", 5.0),
    ("outro", 5.6),
]
STARTS = {}
_t = 0.0
for _name, _dur in SCENES:
    STARTS[_name] = _t
    _t += _dur - CROSSFADE
LENGTH = round(_t + CROSSFADE + 0.1, 1)
INTRO, GAMEPLAY, STYLES, FEATURES, BEST, ANYWHERE, OUTRO = (STARTS[n] for n, _ in SCENES)
CLIP = GAMEPLAY + 0.45  # the gameplay recording starts playing here

# What the music "sees" over the video: [time, score progress, action speed], linear in between.
# The music smooths these itself, so it eases from one stage into the next.
INTENSITY = [
    [INTRO, 0.0, 0.1],
    [GAMEPLAY, 0.25, 0.5],
    [STYLES, 0.55, 0.6],
    [FEATURES, 0.75, 0.7],
    [BEST - 1.0, 0.85, 0.8],
    [BEST, 1.0, 1.0],
    [LENGTH, 1.0, 1.0],
]
# Loudness of the music under the effects (RMS).
MUSIC_RMS = 0.085

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


# --- Game sound effects (same recipes as src/audio/sound.rs) -----------------------------------


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

def soundtrack():
    root = Path(__file__).resolve().parents[2]
    with tempfile.TemporaryDirectory() as tmp:
        wav = Path(tmp) / "soundtrack.wav"
        subprocess.run(
            ["cargo", "run", "--release", "--quiet", "--bin", "soundtrack", "--", str(wav), str(LENGTH), json.dumps(INTENSITY)],
            cwd=root,
            check=True,
        )
        with wave.open(str(wav)) as w:
            x = np.frombuffer(w.readframes(w.getnframes()), dtype="<i2").reshape(-1, 2) / 32768
    n = min(len(x), len(out))
    out[:n] += x[:n] * MUSIC_RMS / np.sqrt(np.mean(x**2))


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
    # The board clear switches the style half a second later (src/app.rs), with a sparkly sweep.
    for i, step in enumerate([0, 4, 7, 12, 16, 19, 24]):
        tone(f(199) + 0.7 + i * 0.035, 784 * 2 ** (step / 12), 0.3, "sine", 0.06)
    # Styles: each phone lands with a thud and a bell, climbing up the scale; then a fanfare for the top level.
    whoosh(STYLES + 0.2)
    for i, m in enumerate([72, 74, 76, 79, 81, 84]):
        land = STYLES + 0.5 + i * 0.32 + 0.12
        place(land, 0.45)
        tone(land, hz(m), 0.5, "sine", 0.1)
        tone(land, hz(m + 24), 0.3, "sine", 0.03)
    crown = STYLES + 0.5 + 5 * 0.32 + 0.45
    clear(crown, 4, 3)
    board_clear(crown - 0.25)
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


soundtrack()
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
