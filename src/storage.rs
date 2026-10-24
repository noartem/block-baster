//! Saved data: best score, the game in progress and settings. Stored as JSON
//! strings under the same keys the web version used, so browser saves carry over.

use serde_json::Value;

use crate::game::game::SavedGame;

const KEY_BEST: &str = "bb.best";
const KEY_BEST_AT_START: &str = "bb.bestAtStart";
const KEY_GAME: &str = "bb.game";
const KEY_SETTINGS: &str = "bb.settings";

/// A string key-value store (localStorage on the web, a JSON file natively).
pub trait KeyValue {
    fn get(&self, key: &str) -> Option<String>;
    /// `None` removes the key.
    fn set(&mut self, key: &str, value: Option<&str>);
    /// Removes everything (`?reset` / `--reset`).
    fn clear(&mut self);
}

/// Levels 0..1; 0 turns the channel off.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub sound: f32,
    pub music: f32,
    /// Vibration strength: 0.5 is normal.
    pub haptics: f32,
}

pub const DEFAULT_SETTINGS: Settings = Settings { sound: 1.0, music: 1.0, haptics: 0.5 };

impl Settings {
    pub fn get(&self, key: SettingKey) -> f32 {
        match key {
            SettingKey::Sound => self.sound,
            SettingKey::Music => self.music,
            SettingKey::Haptics => self.haptics,
        }
    }

    pub fn set(&mut self, key: SettingKey, v: f32) {
        let v = v.clamp(0.0, 1.0);
        match key {
            SettingKey::Sound => self.sound = v,
            SettingKey::Music => self.music = v,
            SettingKey::Haptics => self.haptics = v,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingKey {
    Sound,
    Music,
    Haptics,
}

/// Reads a level, upgrading the on/off switches of older versions.
fn level(value: Option<&Value>, fallback: f32) -> f32 {
    match value {
        Some(Value::Bool(on)) => {
            if *on {
                fallback
            } else {
                0.0
            }
        }
        Some(Value::Number(n)) => n.as_f64().filter(|v| v.is_finite()).map_or(fallback, |v| v.clamp(0.0, 1.0) as f32),
        _ => fallback,
    }
}

fn read(kv: &dyn KeyValue, key: &str) -> Option<Value> {
    kv.get(key).and_then(|raw| serde_json::from_str(&raw).ok())
}

fn write(kv: &mut dyn KeyValue, key: &str, value: Option<Value>) {
    match value {
        Some(v) if !v.is_null() => kv.set(key, Some(&v.to_string())),
        _ => kv.set(key, None),
    }
}

pub fn load_best(kv: &dyn KeyValue) -> u32 {
    read(kv, KEY_BEST).and_then(|v| v.as_f64()).filter(|v| v.is_finite() && *v > 0.0).map_or(0, |v| v as u32)
}

pub fn save_best(kv: &mut dyn KeyValue, best: u32) {
    write(kv, KEY_BEST, Some(best.into()));
}

/// Best score when the current game started, to tell a new record from a tie.
pub fn load_best_at_start(kv: &dyn KeyValue) -> Option<u32> {
    read(kv, KEY_BEST_AT_START).and_then(|v| v.as_f64()).map(|v| v.max(0.0) as u32)
}

pub fn save_best_at_start(kv: &mut dyn KeyValue, best: u32) {
    write(kv, KEY_BEST_AT_START, Some(best.into()));
}

pub fn load_game(kv: &dyn KeyValue) -> Option<SavedGame> {
    read(kv, KEY_GAME).and_then(|v| serde_json::from_value(v).ok())
}

pub fn save_game(kv: &mut dyn KeyValue, game: Option<&SavedGame>) {
    write(kv, KEY_GAME, game.and_then(|g| serde_json::to_value(g).ok()));
}

pub fn load_settings(kv: &dyn KeyValue) -> Settings {
    let saved = read(kv, KEY_SETTINGS);
    let field = |name: &str| saved.as_ref().and_then(|s| s.get(name));
    Settings {
        sound: level(field("sound"), DEFAULT_SETTINGS.sound),
        music: level(field("music"), DEFAULT_SETTINGS.music),
        haptics: level(field("haptics"), DEFAULT_SETTINGS.haptics),
    }
}

pub fn save_settings(kv: &mut dyn KeyValue, s: &Settings) {
    let round = |v: f32| (v as f64 * 100.0).round() / 100.0;
    write(kv, KEY_SETTINGS, Some(serde_json::json!({ "sound": round(s.sound), "music": round(s.music), "haptics": round(s.haptics) })));
}

/// An in-memory store, for tests and tools.
#[derive(Default)]
pub struct MemoryStore(pub std::collections::BTreeMap<String, String>);

impl KeyValue for MemoryStore {
    fn get(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }

    fn set(&mut self, key: &str, value: Option<&str>) {
        match value {
            Some(v) => {
                self.0.insert(key.to_owned(), v.to_owned());
            }
            None => {
                self.0.remove(key);
            }
        }
    }

    fn clear(&mut self) {
        self.0.clear();
    }
}

/// A store kept in one JSON file (desktop and Android). Every write is flushed
/// at once, through a temporary file so a crash never leaves half a save.
#[cfg(not(target_arch = "wasm32"))]
pub struct FileStore {
    path: std::path::PathBuf,
    data: std::collections::BTreeMap<String, String>,
}

#[cfg(not(target_arch = "wasm32"))]
impl FileStore {
    pub fn open(path: std::path::PathBuf) -> Self {
        let data = std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        FileStore { path, data }
    }

    fn flush(&self) {
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = self.path.with_extension("tmp");
        if let Ok(json) = serde_json::to_string_pretty(&self.data)
            && std::fs::write(&tmp, json).is_ok()
        {
            let _ = std::fs::rename(&tmp, &self.path);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl KeyValue for FileStore {
    fn get(&self, key: &str) -> Option<String> {
        self.data.get(key).cloned()
    }

    fn set(&mut self, key: &str, value: Option<&str>) {
        let changed = match value {
            Some(v) => self.data.insert(key.to_owned(), v.to_owned()).as_deref() != Some(v),
            None => self.data.remove(key).is_some(),
        };
        if changed {
            self.flush();
        }
    }

    fn clear(&mut self) {
        self.data.clear();
        self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_settings_and_best() {
        let mut kv = MemoryStore::default();
        assert_eq!(load_settings(&kv), DEFAULT_SETTINGS);
        assert_eq!(load_best(&kv), 0);
        save_best(&mut kv, 1234);
        assert_eq!(load_best(&kv), 1234);
        let s = Settings { sound: 0.4, music: 0.0, haptics: 1.0 };
        save_settings(&mut kv, &s);
        assert_eq!(load_settings(&kv), s);
        assert_eq!(kv.get(KEY_SETTINGS).unwrap(), r#"{"haptics":1.0,"music":0.0,"sound":0.4}"#);
    }

    #[test]
    fn turns_the_on_off_switches_of_older_versions_into_levels() {
        let mut kv = MemoryStore::default();
        kv.set(KEY_SETTINGS, Some(r#"{"sound":false,"music":true,"haptics":true}"#));
        assert_eq!(load_settings(&kv), Settings { sound: 0.0, music: 1.0, haptics: 0.5 });
    }

    #[test]
    fn ignores_corrupt_data() {
        let mut kv = MemoryStore::default();
        kv.set(KEY_GAME, Some("{not json"));
        kv.set(KEY_BEST, Some("\"abc\""));
        assert!(load_game(&kv).is_none());
        assert_eq!(load_best(&kv), 0);
        save_game(&mut kv, None);
        assert!(kv.get(KEY_GAME).is_none());
    }
}
