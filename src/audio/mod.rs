//! STUB: replaced by the synthesiser port.
pub use crate::styles::{SoundProfile, Wave};

pub enum Sfx { Pickup, Place, Invalid, Clear { lines: u32, combo: u32 }, BoardClear, StyleShift, TierUp(usize), GameOver }

#[derive(Clone, Copy, Debug)]
pub struct MusicState { pub e: f32, pub speed: f32, pub bpm: f32 }

pub struct Audio { sound: f32, music: f32, playing: bool, state: MusicState }

impl Default for Audio { fn default() -> Self { Self::new() } }

impl Audio {
    pub fn new() -> Self { Audio { sound: 1.0, music: 1.0, playing: false, state: MusicState { e: 0.0, speed: 0.0, bpm: 76.0 } } }
    pub fn unlock(&mut self) {}
    pub fn suspend(&mut self) {}
    pub fn resume(&mut self) {}
    pub fn set_sound_volume(&mut self, v: f32) { self.sound = v }
    pub fn sound_volume(&self) -> f32 { self.sound }
    pub fn set_music_volume(&mut self, v: f32) { self.music = v }
    pub fn music_volume(&self) -> f32 { self.music }
    pub fn set_profile(&mut self, _p: SoundProfile) {}
    pub fn play(&mut self, _sfx: Sfx) {}
    pub fn start_music(&mut self) { self.playing = true }
    pub fn stop_music(&mut self) { self.playing = false }
    pub fn music_playing(&self) -> bool { self.playing }
    pub fn set_music_paused(&mut self, _paused: bool) {}
    pub fn update(&mut self, _now_ms: f64, score: f32, speed: f32) { self.state.e = 0.65 * score + 0.35 * speed; self.state.bpm = 76.0 + 74.0 * self.state.e.powf(1.6); }
    pub fn music_state(&self) -> MusicState { self.state }
}

pub mod intensity {
    pub fn score_progress(score: u32, best: u32) -> f32 {
        let target = best.clamp(4000, 15000) as f32;
        (score as f32 / target).clamp(0.0, 1.0).powf(1.4)
    }
    #[derive(Default)]
    pub struct ActionMeter { moves: Vec<f64>, combo: u32 }
    impl ActionMeter {
        pub fn new() -> Self { Self::default() }
        pub fn record(&mut self, now: f64, combo: u32) { self.moves.push(now); self.combo = combo; }
        pub fn reset(&mut self) { self.moves.clear(); self.combo = 0; }
        pub fn speed(&mut self, now: f64) -> f32 { self.moves.retain(|m| now - m <= 6000.0); let combo = if self.moves.is_empty() { 0 } else { self.combo.min(5) }; (self.moves.len() as f32 / 5.0 + 0.1 * combo as f32).clamp(0.0, 1.0) }
    }
}
