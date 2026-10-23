/// Touch drags move the piece faster than the finger, so a short swipe from the
/// tray reaches the top of the board. Vertical travel is longer, so it gets more.
pub const TOUCH_GAIN: (f32, f32) = (1.25, 1.5);

/// How far (in cells) a dropped piece may be pulled to the nearest spot where it fits.
pub const SNAP_RADIUS: f32 = 0.9;

/// The placement closest to the fractional cell position (fr, fc) for which
/// `fits` holds, within SNAP_RADIUS; None when nothing nearby fits.
pub fn nearest_fit(fr: f32, fc: f32, fits: impl Fn(i32, i32) -> bool) -> Option<(i32, i32)> {
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
        assert_eq!(nearest_fit(2.3, 4.6, |_, _| true), Some((2, 5)));
    }

    #[test]
    fn pulls_a_near_miss_onto_the_closest_spot_that_fits() {
        assert_eq!(nearest_fit(2.3, 4.6, only(&[(3, 4), (2, 4)])), Some((2, 4)));
        assert_eq!(nearest_fit(0.4, 0.4, only(&[(1, 1)])), Some((1, 1)));
    }

    #[test]
    fn ignores_spots_outside_the_snap_radius() {
        assert_eq!(nearest_fit(2.0, 2.0, only(&[(2, 4)])), None);
        assert_eq!(nearest_fit(2.0, 2.0, |_, _| false), None);
    }
}
