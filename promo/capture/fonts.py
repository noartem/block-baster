"""Converts the game's Baloo 2 web fonts (from @fontsource/baloo-2) to TTF files fframes can load.

Usage (from the repository root, after `npm ci`): pip install fonttools && python3 promo/capture/fonts.py
"""

from pathlib import Path

from fontTools.ttLib import TTFont

root = Path(__file__).resolve().parents[2]
source = root / "node_modules/@fontsource/baloo-2/files"
media = root / "promo/media"

for weight in (500, 600, 700, 800):
    font = TTFont(source / f"baloo-2-latin-{weight}-normal.woff")
    font.flavor = None
    font.save(media / f"Baloo2-{weight}.ttf")
    print(f"Baloo2-{weight}.ttf")
