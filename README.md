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
- 🍀 **Lucky sets**: some sets glow in the tray; they are picked to clear lines and free up room. At first every set is lucky, then every 2nd from 2k points, every 3rd from 10k and every 4th from 50k, and the next set is always lucky once the board is half full
- 💥 Juicy effects, synthesized sounds and haptics
- 🎵 Music that plays along: calm lofi at the start, building up to EDM as your score grows and you play faster
- ✨ Ten style levels (Classic, Cardboard, Candy, Gingerbread, Wooden, Gems, Iron, Neon, Aurora, Legendary) unlocked at 5k, 12k, 25k, 40k, 60k, 90k, 130k, 200k and 350k points, each with new blocks, effects, fonts and sounds; clearing the whole board switches the colours
- 💾 Your game and best score are saved automatically
- 📴 Works offline, installs as an app on any device
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

- **Android:** download [`block-baster.apk`](https://github.com/noartem/block-baster/releases/latest/download/block-baster.apk), open it and allow installing from this source. New releases install over old ones.
- **Web / iOS / desktop:** open the [web version](https://noartem.github.io/block-baster/) and use *Add to Home Screen* or *Install app*.

## Develop

```bash
npm install
npm run dev           # play locally with hot reload
npm test              # unit tests (Vitest)
npm run test:e2e      # browser tests (Playwright)
npm run screenshots   # regenerate the README media into screenshots/
# promo video: see promo/README.md (Playwright capture + Rust/fframes render)
npm run android:apk   # build the APK (needs JDK 21 + Android SDK)
```

Everything is drawn and synthesized by code: blocks, icons and splash screens come from [`scripts/generate-assets.mjs`](scripts/generate-assets.mjs), the sounds are generated in [`src/audio/sound.ts`](src/audio/sound.ts) and the music in [`src/audio/music.ts`](src/audio/music.ts).
`?seed=123` starts a reproducible game and `?reset` clears saved data.

<details>
<summary><b>CI/CD, releases and signing</b></summary>

[`.github/workflows/ci.yml`](.github/workflows/ci.yml):

| Trigger | What happens |
| --- | --- |
| Pull request | typecheck, unit and browser tests, README media capture, APK build, promo video render (all downloadable as workflow artifacts) |
| Push to `main` | the same, then a GitHub Release `v<version>-build.<run>` with the APK, screenshots, GIF and promo video, plus a GitHub Pages deploy |
| `v*` tag | the same checks, then a release under that tag |

The images in this README are captured from the real game in CI ([`tests/screenshots`](tests/screenshots/capture.spec.ts)) and served from the latest release, so they always match the current build.
The promo video gets its own job: it re-captures the game footage, renders the game's music as the soundtrack and renders the video with [fframes](promo/README.md). The finished MP4 is not stored in git; it lives in the releases (and as a workflow artifact on pull requests).

**Signing.** APKs are signed with the public `android/app/debug.keystore` by default. To sign with your own private key, add the repository secrets
`ANDROID_KEYSTORE_BASE64` (`base64 -w0 release.jks`), `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS` and `ANDROID_KEY_PASSWORD`.
Android won't update an app signed with a different key, so uninstall the old build once after switching.

**Web hosting.** Enable *Settings → Pages → Source: GitHub Actions* once. The build uses relative paths, so `dist/` also works on any static host.

</details>

## Credits

Code under the [MIT license](LICENSE). Fonts: [Baloo 2](https://github.com/EkType/Baloo2), [Lilita One](https://fonts.google.com/specimen/Lilita+One), [Orbitron](https://github.com/theleagueof/orbitron), [Righteous](https://fonts.google.com/specimen/Righteous) and [Cinzel Decorative](https://fonts.google.com/specimen/Cinzel+Decorative) (all SIL OFL).
Block Baster is an independent fan project and is not affiliated with the makers of *Block Blast*.
