<div align="center">

<img src="https://github.com/noartem/block-baster/releases/latest/download/showcase.png" alt="Block Baster: three phones showing a colourful 8×8 block puzzle, a line clear with a combo, and the game over screen" width="100%">

# Block Baster

**The relaxing block puzzle: free, open source, and no ads. Ever.**

[![CI/CD](https://github.com/noartem/block-baster/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/noartem/block-baster/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/noartem/block-baster?sort=date&label=release)](https://github.com/noartem/block-baster/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-yellow.svg)](LICENSE)

### [⬇️ Download for Android](https://github.com/noartem/block-baster/releases/latest/download/block-baster.apk) &nbsp;·&nbsp; [▶️ Play in your browser](https://noartem.github.io/block-baster/)

## [▶️ Watch the promo video](https://github.com/noartem/block-baster/releases/latest/download/block-baster-promo.mp4)

</div>

<table>
<tr>
<td width="45%"><img src="https://github.com/noartem/block-baster/releases/latest/download/gameplay.gif" alt="Gameplay recording: pieces are dragged onto the board and full lines burst away" width="100%"></td>
<td>

### Drag. Drop. Blast.

Place the pieces on the 8×8 board. Fill a **row or column** and it clears.
Clear several lines at once, or keep clearing turn after turn, to build **combos**.
The game ends when nothing fits.

- 🎯 **Fair**: every new set of pieces can always be placed somehow
- 🍀 **Lucky sets**: some sets glow in the tray; they are picked to clear lines and free up room. At first every set is lucky, then every 2nd from 3k points and every 3rd from 40k (never rarer). Once the board is 45% full the next set is always lucky and clears a line whenever any piece can
- 🧩 **Gentle difficulty**: small, friendly pieces come most often; big squares, long bars and big corners stay rare and only slowly get more common as your score grows
- 💥 Juicy effects, synthesized sounds and haptics
- 🎵 Music that plays along: calm lofi at the start, building up to EDM as your score grows and you play faster, on instruments that suit each style (felt piano for Cardboard, music box for Gingerbread, synths for Neon, harpsichord and brass for Legendary…)
- ✨ Ten style levels (Classic, Cardboard, Candy, Gingerbread, Wooden, Gems, Iron, Neon, Aurora, Legendary) unlocked at 5k, 12k, 25k, 40k, 60k, 90k, 130k, 200k and 350k points, each with new blocks (in several variations, so no two boxes or planks look alike), effects, fonts and sounds; clearing the whole board switches the colours
- 💾 Your game and best score are saved automatically
- 📴 Works offline: native apps for Android and the desktop, and an installable web version
- 🚫 No ads, no tracking, no network needed

| | Points |
|---|---|
| Each block placed | 1 |
| *n* lines at once | 10·n·(n+1) × combo |
| Whole board cleared | +300 |

</td>
</tr>
</table>

## Install

- **Android:** download [`block-baster.apk`](https://github.com/noartem/block-baster/releases/latest/download/block-baster.apk), open it and allow installing from this source. New releases install over old ones. Needs Android 8.0 or newer.
- **Desktop:** the [latest release](https://github.com/noartem/block-baster/releases/latest) has builds for Linux, Windows and macOS: unpack and run `block-baster`.
- **Web / iOS:** open the [web version](https://noartem.github.io/block-baster/) and use *Add to Home Screen* or *Install app*.

## How it's built

Block Baster is written in Rust. One codebase runs everywhere:

| Platform | How |
| --- | --- |
| Desktop | a native window and OpenGL context ([winit](https://github.com/rust-windowing/winit) + [glutin](https://github.com/rust-windowing/glutin)) |
| Android | a `NativeActivity` loading the same Rust library; no Java, no WebView |
| Web | WebAssembly drawing with WebGL 2 on one canvas, installable as an offline PWA |

Each frame is drawn with [femtovg](https://github.com/femtovg/femtovg), an antialiased 2D vector renderer on OpenGL ES / WebGL.
The block looks and board frames are rasterised once per style with [tiny-skia](https://github.com/RazrFalcon/tiny-skia).
The sounds and the adaptive music come from a small synthesiser, played through [cpal](https://github.com/RustAudio/cpal).
Everything is drawn and synthesised by code. The only assets are the fonts.

```
src/
  game/       the rules: board, pieces, the fair dealer, scoring, saves
  view/       the per-frame drawing: board, effects, HUD, menus, tier backgrounds
  render/     sprites and board frames rasterised with tiny-skia
  audio/      the synthesiser, sound effects and adaptive soundtrack
  platform/   the desktop/Android shell, the web shell, storage, haptics
  app.rs      input, moves, effects, styles and menus, independent of the platform
  bin/        tools: icon generator, soundtrack renderer, sprite contact sheet
web/          the page, PWA manifest and service worker of the web build
android/      the Gradle project that packages the library into an APK
tests/        browser tests and README media capture (Playwright)
```

## Develop

You need [Rust](https://rustup.rs). On Linux you also need the ALSA headers (`libasound2-dev`).

```bash
cargo run --release                 # play the desktop version
cargo test                          # unit tests
cargo run --release -- --help       # options: --seed, --reset, --size, --screenshot
```

**Web.** Add the target and `wasm-bindgen-cli` (same version as the `wasm-bindgen` crate in `Cargo.lock`), then build and serve `dist/`:

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
./scripts/build-web.sh              # builds dist/
npm run serve                       # http://localhost:4173 (needs Python 3)
```

`?seed=123` starts a reproducible game and `?reset` clears saved data. Saves of the earlier web version carry over.

**Browser tests and media** (Node 22):

```bash
npm ci
npx playwright install chromium
npm run test:e2e                    # builds dist/, then runs the browser tests
npm run screenshots                 # regenerate the README media into screenshots/
# promo video: see promo/README.md (Playwright capture + Rust/fframes render)
```

**Android.** Needs JDK 21, the Android SDK and NDK (`ANDROID_NDK_HOME`), the Rust Android targets and [cargo-ndk](https://github.com/bbqsrc/cargo-ndk):

```bash
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-ndk
cd android && ./gradlew assembleRelease   # builds the Rust library for each ABI, then the APK
```

**Icons.** `cargo run --release --bin assets` redraws every app and launcher icon (see [`src/bin/assets.rs`](src/bin/assets.rs)).

<details>
<summary><b>CI/CD, releases and signing</b></summary>

[`.github/workflows/ci.yml`](.github/workflows/ci.yml):

| Trigger | What happens |
| --- | --- |
| Pull request | Rust tests and clippy, the web build with browser tests and README media capture, the APK, desktop builds and the promo video render (all downloadable as workflow artifacts) |
| Push to `main` | the same, then a GitHub Release `v<version>-build.<run>` with the APK, desktop builds, screenshots, GIF and promo video, plus a GitHub Pages deploy |
| `v*` tag | the same checks, then a release under that tag |

The images in this README are captured from the real game in CI ([`tests/screenshots`](tests/screenshots/capture.spec.ts)) and served from the latest release, so they always match the current build.
The promo video gets its own job: it re-captures the game footage, renders the game's music as the soundtrack and renders the video with [fframes](promo/README.md). The finished MP4 is not stored in git; it lives in the releases (and as a workflow artifact on pull requests).

**Signing.** APKs are signed with the public `android/app/debug.keystore` by default. To sign with your own private key, add the repository secrets
`ANDROID_KEYSTORE_BASE64` (`base64 -w0 release.jks`), `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS` and `ANDROID_KEY_PASSWORD`.
Android won't update an app signed with a different key, so uninstall the old build once after switching.

**Web hosting.** Enable *Settings → Pages → Source: GitHub Actions* once. The build uses relative paths, so `dist/` also works on any static host.

</details>

## Credits

Code under the [MIT license](LICENSE). Fonts (bundled in [`assets/fonts`](assets/fonts)): [Baloo 2](https://github.com/EkType/Baloo2), [Lilita One](https://fonts.google.com/specimen/Lilita+One), [Orbitron](https://github.com/theleagueof/orbitron), [Righteous](https://fonts.google.com/specimen/Righteous) and [Cinzel Decorative](https://fonts.google.com/specimen/Cinzel+Decorative) (all SIL OFL).
Block Baster is an independent fan project and is not affiliated with the makers of *Block Blast*.
