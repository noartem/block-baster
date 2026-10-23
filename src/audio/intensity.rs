//! Turns the state of the game into the two numbers the soundtrack follows:
//! score progress and how fast the player is moving. Kept free of any audio
//! code so it can be unit-tested.

use std::collections::VecDeque;

/// Tempo of the calm lofi start and of the full EDM peak.
pub const BPM_MIN: f32 = 76.0;
pub const BPM_MAX: f32 = 150.0;
/// Above 1 the tempo stays low for longer and speeds up near the end; 1 is linear.
pub const BPM_CURVE: f32 = 1.6;

/// Weights of score progress and action speed in the overall intensity.
pub const W_SCORE: f32 = 0.65;
pub const W_SPEED: f32 = 0.35;

/// Score at which the music reaches full EDM: the best score, within these bounds.
pub const TARGET_MIN: u32 = 4000;
pub const TARGET_MAX: u32 = 15000;
/// Above 1 the music lingers in lofi longer and builds up faster near the target.
pub const PROGRESS_CURVE: f32 = 1.4;

/// Moves within this window count towards the action speed...
pub const SPEED_WINDOW_MS: f64 = 6000.0;
/// ...and this many moves in it is full speed.
pub const SPEED_MAX_MOVES: usize = 5;
/// Each step of a combo streak adds this much speed.
pub const SPEED_PER_COMBO: f32 = 0.1;

/// Smoothing time constants, in seconds. Speed rises fast and falls slowly so a short pause doesn't drop the music back to lofi.
pub const SMOOTH_SCORE: f32 = 5.0;
pub const SMOOTH_SPEED_UP: f32 = 0.35;
pub const SMOOTH_SPEED_DOWN: f32 = 3.3;

/// Clamps to 0..1; NaN counts as 0.
pub fn clamp01(x: f32) -> f32 {
    if x.is_nan() { 0.0 } else { x.clamp(0.0, 1.0) }
}

/// 0 below `a`, 1 above `b`, linear in between.
pub fn ramp(x: f32, a: f32, b: f32) -> f32 {
    clamp01((x - a) / (b - a))
}

/// How far the score has come towards the full-energy target, 0..1.
pub fn score_progress(score: u32, best: u32) -> f32 {
    let target = best.clamp(TARGET_MIN, TARGET_MAX);
    clamp01((score as f64 / target as f64) as f32).powf(PROGRESS_CURVE)
}

/// Overall intensity 0..1: progress sets the stage, speed adds energy on top.
pub fn intensity(score: f32, speed: f32) -> f32 {
    clamp01(W_SCORE * clamp01(score) + W_SPEED * clamp01(speed))
}

pub fn bpm_for(e: f32) -> f32 {
    BPM_MIN + (BPM_MAX - BPM_MIN) * clamp01(e).powf(BPM_CURVE)
}

/// Counts recent moves to measure how fast the player is acting.
#[derive(Clone, Debug, Default)]
pub struct ActionMeter {
    moves: VecDeque<f64>,
    combo: u32,
}

impl ActionMeter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a move at `now_ms` with the current combo streak.
    pub fn record(&mut self, now_ms: f64, combo: u32) {
        self.moves.push_back(now_ms);
        self.combo = combo;
        self.trim(now_ms);
    }

    /// Same as [`ActionMeter::record`] (named after `move` in the TypeScript, a Rust keyword).
    pub fn move_(&mut self, now_ms: f64, combo: u32) {
        self.record(now_ms, combo);
    }

    pub fn reset(&mut self) {
        self.moves.clear();
        self.combo = 0;
    }

    pub fn speed(&mut self, now_ms: f64) -> f32 {
        self.trim(now_ms);
        let combo = if self.moves.is_empty() {
            0
        } else {
            self.combo.min(5)
        };
        clamp01(self.moves.len() as f32 / SPEED_MAX_MOVES as f32 + SPEED_PER_COMBO * combo as f32)
    }

    fn trim(&mut self, now_ms: f64) {
        while self
            .moves
            .front()
            .is_some_and(|&t| now_ms - t > SPEED_WINDOW_MS)
        {
            self.moves.pop_front();
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MusicState {
    /// Overall intensity 0..1: picks the genre and the tempo.
    pub e: f32,
    /// Smoothed action speed 0..1: adds small details on top.
    pub speed: f32,
    pub bpm: f32,
}

impl Default for MusicState {
    fn default() -> Self {
        MusicState {
            e: 0.0,
            speed: 0.0,
            bpm: BPM_MIN,
        }
    }
}

/// Smooths the raw inputs over time and derives the music state from them.
#[derive(Clone, Debug, Default)]
pub struct IntensityTracker {
    score: f32,
    speed: f32,
    pub state: MusicState,
}

impl IntensityTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Advances by `dt` seconds towards the raw `score` and `speed` (both 0..1).
    pub fn update(&mut self, dt: f32, score: f32, speed: f32) -> MusicState {
        let follow =
            |from: f32, to: f32, tau: f32| from + (to - from) * (1.0 - (-dt.max(0.0) / tau).exp());
        self.score = follow(self.score, clamp01(score), SMOOTH_SCORE);
        let target = clamp01(speed);
        self.speed = follow(
            self.speed,
            target,
            if target > self.speed {
                SMOOTH_SPEED_UP
            } else {
                SMOOTH_SPEED_DOWN
            },
        );
        let e = intensity(self.score, self.speed);
        self.state = MusicState {
            e,
            speed: self.speed,
            bpm: bpm_for(e),
        };
        self.state
    }

    /// Jumps straight to a state (for previews and tests).
    pub fn set(&mut self, score: f32, speed: f32) -> MusicState {
        self.score = clamp01(score);
        self.speed = clamp01(speed);
        self.update(0.0, self.score, self.speed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        // Like vitest's toBeCloseTo with the default precision of 2 digits.
        (a - b).abs() < 0.005
    }

    #[test]
    fn ramps_linearly_between_two_thresholds() {
        assert_eq!(ramp(0.1, 0.2, 0.4), 0.0);
        assert!(close(ramp(0.3, 0.2, 0.4), 0.5));
        assert_eq!(ramp(0.9, 0.2, 0.4), 1.0);
    }

    #[test]
    fn measures_score_progress_against_the_best_score_within_bounds_easing_in() {
        assert_eq!(score_progress(0, 0), 0.0);
        assert!(close(
            score_progress(TARGET_MIN / 2, 0),
            0.5f32.powf(PROGRESS_CURVE)
        ));
        assert!(score_progress(TARGET_MIN / 2, 0) < 0.5);
        assert!(close(
            score_progress(5000, 10000),
            0.5f32.powf(PROGRESS_CURVE)
        ));
        assert_eq!(score_progress(TARGET_MAX, 1_000_000), 1.0);
        assert_eq!(score_progress(TARGET_MIN, 0), 1.0);
    }

    #[test]
    fn weighs_progress_over_speed_and_needs_both_for_full_intensity() {
        assert_eq!(intensity(0.0, 0.0), 0.0);
        assert!(intensity(1.0, 0.0) > intensity(0.0, 1.0));
        assert!(intensity(1.0, 0.0) < 1.0);
        assert_eq!(intensity(1.0, 1.0), 1.0);
    }

    #[test]
    fn keeps_the_tempo_low_for_a_while_and_reaches_the_maximum_at_full_intensity() {
        assert_eq!(bpm_for(0.0), BPM_MIN);
        assert_eq!(bpm_for(1.0), BPM_MAX);
        assert!(bpm_for(0.5) < (BPM_MIN + BPM_MAX) / 2.0);
    }

    #[test]
    fn counts_moves_within_the_window_and_adds_combos_on_top() {
        let mut m = ActionMeter::new();
        assert_eq!(m.speed(0.0), 0.0);
        for i in 0..SPEED_MAX_MOVES {
            m.record(i as f64 * 500.0, 0);
        }
        assert_eq!(m.speed(SPEED_MAX_MOVES as f64 * 500.0), 1.0);
        m.reset();
        m.record(0.0, 0);
        m.move_(100.0, 2);
        assert!(close(m.speed(200.0), 2.0 / SPEED_MAX_MOVES as f32 + 0.2));
        // Old moves (and their combo) drop out of the window.
        assert_eq!(m.speed(100.0 + SPEED_WINDOW_MS + 1.0), 0.0);
    }

    #[test]
    fn follows_the_inputs_smoothly_speed_rises_fast_and_falls_slowly() {
        let mut t = IntensityTracker::new();
        t.update(0.5, 0.0, 1.0);
        let risen = t.state.speed;
        assert!(risen > 0.6);
        t.update(0.5, 0.0, 0.0);
        assert!(t.state.speed > risen * 0.8);
        for _ in 0..1000 {
            t.update(0.1, 1.0, 1.0);
        }
        assert!((t.state.e - 1.0).abs() < 0.0005);
        assert!((t.state.bpm - BPM_MAX).abs() < 0.05);
    }

    #[test]
    fn set_jumps_straight_to_a_state() {
        let mut t = IntensityTracker::new();
        let s = t.set(1.0, 1.0);
        assert_eq!(s.e, 1.0);
        assert_eq!(s.bpm, BPM_MAX);
        assert_eq!(t.state, s);
    }
}
