/**
 * Touch drags move the piece faster than the finger, so a short swipe from the
 * tray reaches the top of the board. Vertical travel is longer, so it gets more.
 */
export const TOUCH_GAIN = { x: 1.25, y: 1.5 };

/** How far (in cells) a dropped piece may be pulled to the nearest spot where it fits. */
export const SNAP_RADIUS = 0.9;

/**
 * The placement closest to the fractional cell position (fr, fc) for which
 * `fits` holds, within SNAP_RADIUS; null when nothing nearby fits.
 */
export function nearestFit(fr: number, fc: number, fits: (r: number, c: number) => boolean): { r: number; c: number } | null {
  let best: { r: number; c: number } | null = null;
  let bestDist = SNAP_RADIUS;
  for (let r = Math.floor(fr - SNAP_RADIUS); r <= Math.ceil(fr + SNAP_RADIUS); r++) {
    for (let c = Math.floor(fc - SNAP_RADIUS); c <= Math.ceil(fc + SNAP_RADIUS); c++) {
      const dist = Math.hypot(r - fr, c - fc);
      if (dist <= bestDist && fits(r, c)) {
        best = { r, c };
        bestDist = dist;
      }
    }
  }
  return best;
}
