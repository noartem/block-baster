//! Saves of the earlier Android app, which ran the web version in a WebView
//! and kept its data in that WebView's localStorage (a Chromium LevelDB). On
//! the first start of the native app they are copied over once, so players
//! keep their best score, their game in progress and their settings.

use std::path::Path;

use crate::storage::KeyValue;

/// The localStorage database of an app's WebView, relative to its data directory.
pub const WEBVIEW_STORAGE: &str = "app_webview/Default/Local Storage/leveldb";

/// Reads every `bb.*` localStorage item from a Chromium LevelDB directory.
/// The database is copied first, so the original is never touched.
pub fn read_local_storage(dir: &Path, scratch: &Path) -> Vec<(String, String)> {
    let _ = std::fs::remove_dir_all(scratch);
    if std::fs::create_dir_all(scratch).is_err() {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if name != "LOCK" && entry.path().is_file() {
            let _ = std::fs::copy(entry.path(), scratch.join(&name));
        }
    }
    let mut items = Vec::new();
    let options = rusty_leveldb::Options { create_if_missing: false, ..rusty_leveldb::Options::default() };
    if let Ok(mut db) = rusty_leveldb::DB::open(scratch, options)
        && let Ok(mut iter) = db.new_iter()
    {
        use rusty_leveldb::LdbIterator;
        while let Some((key, value)) = iter.next() {
            if let Some(item) = decode(&key, &value) {
                items.push(item);
            }
        }
    }
    let _ = std::fs::remove_dir_all(scratch);
    items
}

/// Chromium stores an item as key `_<origin>\0\x01<name>`, value `<encoding byte><text>`
/// (0 = UTF-16LE, 1 = Latin-1).
fn decode(key: &[u8], value: &[u8]) -> Option<(String, String)> {
    let rest = key.strip_prefix(b"_")?;
    let at = rest.windows(2).position(|w| w == b"\0\x01")?;
    let name = String::from_utf8(rest[at + 2..].to_vec()).ok()?;
    if !name.starts_with("bb.") {
        return None;
    }
    let (&kind, text) = value.split_first()?;
    let text = match kind {
        0 => String::from_utf16(&text.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect::<Vec<_>>()).ok()?,
        1 => text.iter().map(|&b| b as char).collect(),
        _ => return None,
    };
    Some((name, text))
}

/// Copies the old saves into `store` when it is still empty.
pub fn migrate(store: &mut dyn KeyValue, is_empty: bool, webview_dir: &Path, scratch: &Path) -> usize {
    if !is_empty || !webview_dir.is_dir() {
        return 0;
    }
    let items = read_local_storage(webview_dir, scratch);
    for (key, value) in &items {
        store.set(key, Some(value));
    }
    items.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{self, MemoryStore};

    #[test]
    fn reads_a_chromium_local_storage_database() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/webview-leveldb");
        let scratch = std::env::temp_dir().join(format!("bb-legacy-test-{}", std::process::id()));
        let mut store = MemoryStore::default();
        assert_eq!(migrate(&mut store, true, &fixture, &scratch), 3);
        assert_eq!(storage::load_best(&store), 4321);
        let settings = storage::load_settings(&store);
        assert_eq!((settings.sound, settings.music, settings.haptics), (0.4, 1.0, 0.5));
        let game = storage::load_game(&store).expect("game");
        assert_eq!((game.score, game.moves, game.board[5]), (120, 9, 2));
        assert!(store.get("bb.game").unwrap().contains("ünïcødé"));
        // The original database is left alone.
        assert!(fixture.join("000003.log").exists());
        assert!(!scratch.exists());
    }

    #[test]
    fn decodes_both_string_encodings() {
        let utf16: Vec<u8> = std::iter::once(0).chain("hé✓".encode_utf16().flat_map(|u| u.to_le_bytes())).collect();
        assert_eq!(decode(b"_https://localhost\0\x01bb.best", &utf16), Some(("bb.best".into(), "hé✓".into())));
        assert_eq!(decode(b"_https://localhost\0\x01bb.best", b"\x0112"), Some(("bb.best".into(), "12".into())));
        assert_eq!(decode(b"_https://localhost\0\x01other", b"\x0112"), None);
        assert_eq!(decode(b"META:https://localhost", b"x"), None);
    }
}
