/// Touch drags move the piece faster than the finger, so a short swipe from the
/// tray reaches the top of the board. The same gain on both axes keeps the piece
/// moving in the direction of the finger and not sliding away from it.
pub const TOUCH_GAIN: (f32, f32) = (1.3, 1.3);

/// How far (in cells) a dropped piece may be pulled to the nearest spot where it fits.
pub const SNAP_RADIUS: f32 = 0.9;

/// How far (in cells) past a cell edge the piece has to go before the target moves
/// off the current spot, so the preview doesn't flicker when the piece sits on the edge.
pub const SNAP_HYSTERESIS: f32 = 0.08;

/// The placement closest to the fractional cell position (fr, fc) for which
/// `fits` holds, within SNAP_RADIUS; None when nothing nearby fits. The current
/// target `keep` stays while the piece is still over it, give or take SNAP_HYSTERESIS.
pub fn nearest_fit(fr: f32, fc: f32, keep: Option<(i32, i32)>, fits: impl Fn(i32, i32) -> bool) -> Option<(i32, i32)> {
    if let Some((r, c)) = keep
        && (r as f32 - fr).abs() <= 0.5 + SNAP_HYSTERESIS
        && (c as f32 - fc).abs() <= 0.5 + SNAP_HYSTERESIS
        && fits(r, c)
    {
        return keep;
    }
    let mut best = None;
    let mut best_dist = SNAP_RADIUS;
    for r in (fr - SNAP_RADIUS).floor() as i32..=(fr + SNAP_RADIUS).ceil() as i32 {
        for c in (fc - SNAP_RADIUS).floor() as i32..=(fc + SNAP_RADIUS).ceil() as i32 {
            let dist = (r as f32 - fr).hypot(c as f32 - fc);
            if dist <= best_dist && fits(r, c) {
                best = Some((r, c));
                best_dist = dist;
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn only(spots: &'static [(i32, i32)]) -> impl Fn(i32, i32) -> bool {
        move |r, c| spots.contains(&(r, c))
    }

    #[test]
    fn picks_the_rounded_cell_when_it_fits() {
        assert_eq!(nearest_fit(2.3, 4.6, None, |_, _| true), Some((2, 5)));
    }

    #[test]
    fn pulls_a_near_miss_onto_the_closest_spot_that_fits() {
        assert_eq!(nearest_fit(2.3, 4.6, None, only(&[(3, 4), (2, 4)])), Some((2, 4)));
        assert_eq!(nearest_fit(0.4, 0.4, None, only(&[(1, 1)])), Some((1, 1)));
    }

    #[test]
    fn ignores_spots_outside_the_snap_radius() {
        assert_eq!(nearest_fit(2.0, 2.0, None, only(&[(2, 4)])), None);
        assert_eq!(nearest_fit(2.0, 2.0, None, |_, _| false), None);
    }

    #[test]
    fn keeps_the_current_target_across_a_cell_edge() {
        // Just past the edge between columns 4 and 5: stay on 4 ...
        assert_eq!(nearest_fit(2.0, 4.55, Some((2, 4)), |_, _| true), Some((2, 4)));
        assert_eq!(nearest_fit(2.4, 4.55, Some((2, 4)), |_, _| true), Some((2, 4)));
        // ... until the piece is clearly over 5.
        assert_eq!(nearest_fit(2.0, 4.6, Some((2, 4)), |_, _| true), Some((2, 5)));
        assert_eq!(nearest_fit(1.4, 4.6, Some((2, 4)), |_, _| true), Some((1, 5)));
        // A kept spot that no longer fits is dropped.
        assert_eq!(nearest_fit(2.0, 4.55, Some((2, 4)), only(&[(2, 5)])), Some((2, 5)));
    }
}
