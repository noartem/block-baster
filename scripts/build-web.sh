#!/usr/bin/env bash
# Builds the web version into dist/: the game compiled to WebAssembly, its JS
# glue (wasm-bindgen), the page, the PWA manifest, the service worker and icons.
# Needs: rustup target add wasm32-unknown-unknown, and wasm-bindgen-cli of the
# same version as the wasm-bindgen crate in Cargo.lock:
#   cargo install wasm-bindgen-cli --version <version> --locked
set -euo pipefail
cd "$(dirname "$0")/.."

profile="${PROFILE:-release}"
dir="$profile"
[ "$profile" = dev ] && dir=debug

cargo build --lib --profile "$profile" --target wasm32-unknown-unknown
rm -rf dist
mkdir -p dist
wasm-bindgen --target web --no-typescript --out-dir dist "target/wasm32-unknown-unknown/$dir/block_baster.wasm"
cp web/* public/* dist/
version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
hash="$(cat dist/block_baster_bg.wasm dist/block_baster.js dist/index.html | sha256sum | cut -c1-10)"
sed -i.bak "s/__BUILD__/$version-$hash/" dist/sw.js && rm dist/sw.js.bak
echo "dist/ ready ($(du -sh dist | cut -f1), wasm $(du -h dist/block_baster_bg.wasm | cut -f1))"
