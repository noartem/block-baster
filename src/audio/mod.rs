//! Sound effects and the adaptive soundtrack, synthesised at runtime by a small
//! software synth (a port of the original Web Audio code). [`Audio`] is the
//! app's handle: it owns the output device and shares the [`Engine`] with the
//! audio callback.
//!
//! Both the effects and the music take on the visual style: the tier's material
//! picks the instruments, the palette colours them (see [`timbre`]).
//!
//! Browsers only allow audio after a user gesture, so nothing touches the
//! device until [`Audio::unlock`]. Without a device (CI, headless machines)
//! everything keeps working silently, and the intensity tracker still follows
//! the game.

pub mod dsp;
pub mod engine;
pub mod intensity;
pub mod music;
pub mod output;
pub mod sound;
pub mod timbre;

use std::sync::{Arc, Mutex, MutexGuard};

pub use crate::styles::{SoundProfile, Wave};
pub use engine::Engine;
pub use intensity::MusicState;
pub use timbre::SoundStyle;

use intensity::IntensityTracker;

/// The sound effects of the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sfx {
    Pickup,
    Place,
    Invalid,
    Clear {
        lines: u32,
        combo: u32,
    },
    BoardClear,
    StyleShift,
    /// A new style tier was unlocked; the fanfare grows with the level.
    TierUp(usize),
    GameOver,
}

pub struct Audio {
    engine: Option<Arc<Mutex<Engine>>>,
    output: Option<output::Output>,
    /// Opening the device failed; don't retry on every tap.
    failed: bool,
    suspended: bool,
    sound_level: f32,
    music_level: f32,
    /// Style tier level and palette variant.
    style: (usize, usize),
    music_on: bool,
    paused: bool,
    tracker: IntensityTracker,
    last_update_ms: Option<f64>,
}

impl Default for Audio {
    fn default() -> Self {
        Self::new()
    }
}

impl Audio {
    /// Doesn't open the audio device; that waits for [`Audio::unlock`].
    pub fn new() -> Self {
        Audio {
            engine: None,
            output: None,
            failed: false,
            suspended: false,
            sound_level: 1.0,
            music_level: 1.0,
            style: (0, 0),
            music_on: false,
            paused: false,
            tracker: IntensityTracker::new(),
            last_update_ms: None,
        }
    }

    fn engine(&self) -> Option<MutexGuard<'_, Engine>> {
        self.engine
            .as_ref()
            .map(|e| e.lock().unwrap_or_else(|p| p.into_inner()))
    }

    /// Opens (or resumes) the output; call it from a user gesture. Idempotent.
    pub fn unlock(&mut self) {
        if self.output.is_none() && !self.failed {
            let (sound, music, state) = (self.sound_level, self.music_level, self.tracker.state);
            let (music_on, paused, style) = (self.music_on, self.paused, self.style);
            let opened = output::Output::open(|sample_rate| {
                let mut e = Engine::new(sample_rate, crate::platform::time_seed());
                e.init_sound_volume(sound);
                e.set_music_state(state);
                e.set_style(style.0, style.1);
                if music_on {
                    e.start_music(music, paused);
                }
                Arc::new(Mutex::new(e))
            });
            match opened {
                Ok((out, engine)) => {
                    self.output = Some(out);
                    self.engine = Some(engine);
                    self.suspended = false;
                }
                Err(e) => {
                    output::report(&e);
                    self.failed = true;
                }
            }
            return;
        }
        self.resume();
    }

    /// The app went to the background.
    pub fn suspend(&mut self) {
        if let Some(out) = &self.output
            && !self.suspended
        {
            out.pause();
            self.suspended = true;
        }
    }

    /// Back in the foreground; never opens a device that was never unlocked.
    pub fn resume(&mut self) {
        if let Some(out) = &self.output
            && self.suspended
        {
            out.play();
            self.suspended = false;
        }
    }

    /// Volume of the effects 0..1 (0 = silent), on a perceptual curve.
    pub fn set_sound_volume(&mut self, v: f32) {
        self.sound_level = intensity::clamp01(v);
        let level = self.sound_level;
        if let Some(mut e) = self.engine() {
            e.set_sound_volume(level);
        }
    }

    pub fn sound_volume(&self) -> f32 {
        self.sound_level
    }

    /// Volume of the music 0..1, on a perceptual curve.
    pub fn set_music_volume(&mut self, v: f32) {
        self.music_level = intensity::clamp01(v);
        let level = self.music_level;
        if let Some(mut e) = self.engine() {
            e.set_music_volume(level);
        }
    }

    pub fn music_volume(&self) -> f32 {
        self.music_level
    }

    /// Switches the sound to a style: its tier level picks the instruments, its
    /// palette variant colours them. The effects follow right away, the music
    /// at its next bar (without restarting).
    pub fn set_style(&mut self, tier: usize, variant: usize) {
        if self.style == (tier, variant) {
            return;
        }
        self.style = (tier, variant);
        if let Some(mut e) = self.engine() {
            e.set_style(tier, variant);
        }
    }

    /// Style tier level and palette variant of the sound.
    pub fn style(&self) -> (usize, usize) {
        self.style
    }

    /// Plays an effect; silently ignored while locked, suspended or at volume 0.
    pub fn play(&mut self, sfx: Sfx) {
        if self.sound_level <= 0.0 || self.suspended {
            return;
        }
        if let Some(mut e) = self.engine() {
            e.play(sfx);
        }
    }

    /// Starts the music, fading in over 1.5 s. Safe to call repeatedly.
    pub fn start_music(&mut self) {
        if self.music_on {
            return;
        }
        self.music_on = true;
        self.last_update_ms = None;
        let (level, paused) = (self.music_level, self.paused);
        if let Some(mut e) = self.engine() {
            e.start_music(level, paused);
        }
    }

    /// Fades the music out, then releases it.
    pub fn stop_music(&mut self) {
        if !self.music_on {
            return;
        }
        self.music_on = false;
        if let Some(mut e) = self.engine() {
            e.stop_music();
        }
    }

    /// True between `start_music` and `stop_music`, even before the device is unlocked.
    pub fn music_playing(&self) -> bool {
        self.music_on
    }

    /// Muffles the music while a menu is open.
    pub fn set_music_paused(&mut self, paused: bool) {
        if self.paused == paused {
            return;
        }
        self.paused = paused;
        if let Some(mut e) = self.engine() {
            e.set_music_paused(paused);
        }
    }

    /// Called every frame (and at least every ~50 ms) while the music plays:
    /// advances the intensity tracker by real time towards the raw score
    /// progress and action speed (both 0..1).
    pub fn update(&mut self, now_ms: f64, score_progress: f32, speed: f32) {
        if !self.music_on {
            return;
        }
        let dt = self
            .last_update_ms
            .map_or(0.0, |last| ((now_ms - last) / 1000.0) as f32);
        self.last_update_ms = Some(now_ms);
        let state = self.tracker.update(dt, score_progress, speed);
        if let Some(mut e) = self.engine() {
            e.set_music_state(state);
        }
    }

    pub fn music_state(&self) -> MusicState {
        self.tracker.state
    }
}

#[cfg(test)]
mod tests;
