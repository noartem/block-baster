# Block Baster promo video

A 39-second marketing video for Block Baster, written in Rust with [fframes](https://fframes.studio).
Every screen in it is real. The screenshots and the gameplay recording come from the actual game,
driven by Playwright.

**Result:** [`block-baster-promo.mp4`](https://github.com/noartem/block-baster/releases/latest/download/block-baster-promo.mp4) (1920×1080, 30 fps, H.264 + AAC).
CI renders it from every commit (the *Promo video* job in [`.github/workflows/ci.yml`](../.github/workflows/ci.yml)) and attaches it to each
GitHub release. The rendered file is not kept in git.

| Scene | What happens |
| --- | --- |
| Intro | The app icon builds itself from falling blocks, then the name and the promise appear: *the block puzzle without the ads* |
| Gameplay | A real gameplay recording plays on a phone, with a touch indicator. The headline follows the moves: *Drag. Drop. Blast!*, *Chain combos.*, *Clear the whole board.* The board clear switches the game to a new colour style |
| Styles | The six style levels (Classic, Candy, Gems, Neon, Aurora, Legendary) pop up side by side with the score that unlocks each one |
| Features | The game's pause menu next to six cards: no ads, no tracking, offline, fair dealing, auto-save, open source |
| Best score | The game over screen with a new record, and a counter racing up to it |
| Anywhere | The game in a desktop browser and on a phone, plus where to get it |
| Outro | Logo, name and the call to action: `noartem.github.io/block-baster` |

The soundtrack is the game's own adaptive music ([`src/audio/music.rs`](../src/audio/music.rs)),
rendered offline by the game's `soundtrack` binary ([`src/bin/soundtrack.rs`](../src/bin/soundtrack.rs)). It follows the video the way it follows a game: lofi over the
intro, livelier with the gameplay, house through the styles and features, and full EDM from the best
score on. The game's sound effects play on top, exactly on the frames where the moves happen.

## Layout

```
promo/
  capture/capture.mjs   Playwright: stages the game, captures the screenshots and the gameplay clip
  capture/fonts.py      converts the game's Baloo 2 web font to TTF for fframes
  audio/music.py        builds media/music.wav: the game's adaptive music, rendered along an intensity
                        curve by `cargo run --release --bin soundtrack`, + the game's sound effects
  media/                everything the video uses (screenshots, gameplay.mp4, fonts, music)
  src/main.rs           the fframes Video: background, shared defs, scene list, encoder settings
  src/draw.rs           easing, game-style bevelled blocks, logo, phone and browser mockups
  src/scenes/           one file per scene
```

## Re-capturing the media

The capture script uses a virtual clock: `performance.now` and `requestAnimationFrame` are replaced,
so the game renders each animation frame exactly when the script asks for it. The clip is smooth and
reproducible however slow screenshots are. The touch position of each frame is saved to
`media/gameplay-touch.json`, and the video draws the finger from it.

```bash
# from the repository root
npm ci
npm run build
node promo/capture/capture.mjs      # needs ffmpeg on PATH (encodes media/gameplay.mp4)
pip install fonttools numpy
python3 promo/capture/fonts.py
python3 promo/audio/music.py        # renders the music with `cargo run --release --bin soundtrack`
```

If the gameplay clip changes length, update `CLIP_LENGTH` in `src/scenes/gameplay.rs` and `CLIP_FRAMES`
in `audio/music.py`. If the moves happen on different frames, update the sound-effect frames in
`audio/music.py`. Scene lengths in `audio/music.py` must match the `DURATION` of each scene. How the
music builds up over the scenes is set by `INTENSITY` there.

## Rendering

Requirements: Rust (edition 2024), clang, nasm and the x264 development package
(`apt install libx264-dev`). fframes links a static FFmpeg. On the first build it downloads a prebuilt
FFmpeg, and if that build doesn't match your x264 version it compiles FFmpeg from source:

```bash
cd promo
cargo run --release -- render                   # writes block-baster-promo.mp4
# If linking fails with "undefined symbol: x264_encoder_open_…", build FFmpeg from source once
# (clean the crate first if a prebuilt FFmpeg was already unpacked; CI always builds from source):
cargo clean --release -p ffmpeg-sys-fframes
FFMPEG_FORCE_BUILD=1 cargo run --release -- render
```

The fframes CLI also helps while editing:

```bash
cargo run --release -- timeline                 # scenes and their timing
cargo run --release -- frame 6s 14.5s           # render single frames to PNG (frames/)
cargo run --release -- strip Gameplay           # contact sheet of one scene
cargo run --release -- render --draft Features  # fast, half-resolution render of one scene
```
