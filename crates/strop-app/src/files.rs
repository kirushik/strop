//! Document lifecycle (PLAN.md E2): visible-from-birth files.
//!
//! The research verdict: never create a document in a hidden location —
//! GNOME Text Editor's drafts folder is the documented anti-pattern, and
//! Strop's old silent scratch.strop was exactly it. Documents are born as
//! real files in the user's documents folder, findable from second one.

use std::path::{Path, PathBuf};

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").expect("HOME not set"))
}

/// `$XDG_DOCUMENTS_DIR/Strop` — the localized documents folder per
/// xdg-user-dirs (~/.config/user-dirs.dirs), falling back to ~/Documents.
pub fn documents_dir() -> PathBuf {
    let base = user_dirs_documents().unwrap_or_else(|| home().join("Documents"));
    base.join("Strop")
}

fn user_dirs_documents() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"));
    let text = std::fs::read_to_string(config.join("user-dirs.dirs")).ok()?;
    for line in text.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("XDG_DOCUMENTS_DIR=") {
            let value = value.trim_matches('"');
            let expanded = value.replace("$HOME", &home().to_string_lossy());
            if !expanded.is_empty() {
                return Some(PathBuf::from(expanded));
            }
        }
    }
    None
}

/// First free "Untitled.strop" / "Untitled 2.strop" / … in the Strop folder.
pub fn untitled_path() -> PathBuf {
    let dir = documents_dir();
    // Documents are visible from birth — the folder must exist before the
    // first autosave tries to land there.
    let _ = std::fs::create_dir_all(&dir);
    let first = dir.join("Untitled.strop");
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|n| dir.join(format!("Untitled {n}.strop")))
        .find(|p| !p.exists())
        .expect("unbounded range")
}

/// One-time migration of the old hidden scratch into the visible folder.
pub fn migrate_scratch() -> Option<PathBuf> {
    let old = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local/share"))
        .join("strop/scratch.strop");
    if !old.exists() {
        return None;
    }
    let dir = documents_dir();
    std::fs::create_dir_all(&dir).ok()?;
    let mut target = dir.join("Scratch.strop");
    let mut n = 2;
    while target.exists() {
        target = dir.join(format!("Scratch {n}.strop"));
        n += 1;
    }
    match std::fs::rename(&old, &target) {
        Ok(()) => {
            eprintln!(
                "strop: moved the old hidden scratch document to {}",
                target.display()
            );
            Some(target)
        }
        Err(e) => {
            eprintln!("strop: could not migrate scratch: {e}");
            None
        }
    }
}

fn recents_file() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local/state"))
        .join("strop/recents.json")
}

/// Most-recent-first, existing files only.
pub fn recents() -> Vec<PathBuf> {
    let Ok(json) = std::fs::read_to_string(recents_file()) else {
        return Vec::new();
    };
    let list: Vec<PathBuf> = serde_json::from_str(&json).unwrap_or_default();
    list.into_iter().filter(|p| p.exists()).collect()
}

pub fn push_recent(path: &Path) {
    let Ok(path) = path.canonicalize().or_else(|_| {
        // Brand-new file may not exist yet; canonicalize the parent.
        path.parent()
            .unwrap_or(Path::new("."))
            .canonicalize()
            .map(|d| d.join(path.file_name().unwrap_or_default()))
    }) else {
        return;
    };
    let mut list = recents();
    list.retain(|p| p != &path);
    list.insert(0, path);
    list.truncate(20);
    let file = recents_file();
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string_pretty(&list) {
        let _ = std::fs::write(file, json);
    }
}

pub fn replace_recent(old: &Path, new: &Path) {
    let mut list = recents();
    list.retain(|p| p != old);
    let _ = std::fs::write(
        recents_file(),
        serde_json::to_string_pretty(&list).unwrap_or_default(),
    );
    push_recent(new);
}

/// ~/.local/state/strop/palette_freq.json (DESIGN §3.3, hit-frequency
/// ordering): label → execution count, written through on every palette
/// execution so the palette slowly becomes *your* instrument.
fn palette_freq_file() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local/state"))
        .join("strop/palette_freq.json")
}

pub fn load_palette_freq() -> std::collections::HashMap<String, u32> {
    load_palette_freq_at(&palette_freq_file())
}

fn load_palette_freq_at(file: &Path) -> std::collections::HashMap<String, u32> {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

/// Count one execution; returns the new count (the caller keeps its
/// in-memory copy in step without re-reading the file).
pub fn bump_palette_freq(label: &str) -> u32 {
    bump_palette_freq_at(&palette_freq_file(), label)
}

fn bump_palette_freq_at(file: &Path, label: &str) -> u32 {
    let mut map = load_palette_freq_at(file);
    let count = map.entry(label.to_owned()).or_insert(0);
    *count += 1;
    let count = *count;
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string_pretty(&map) {
        let _ = std::fs::write(file, json);
    }
    count
}

/// One entry of ~/.local/state/strop/intents.json (DESIGN §4.1, the
/// close-time if-then ritual): the "Next session I will ___" sentence
/// recorded by End Session, plus the caret offset at quit so the writer
/// resumes mid-sentence. Keyed by the document's canonical path.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct IntentEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    #[serde(default)]
    pub set_unix: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caret: Option<usize>,
}

fn intents_file() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local/state"))
        .join("strop/intents.json")
}

fn intent_key(doc: &Path) -> String {
    doc.canonicalize()
        .unwrap_or_else(|_| doc.to_owned())
        .display()
        .to_string()
}

fn load_intents_at(file: &Path) -> std::collections::HashMap<String, IntentEntry> {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

fn save_intents_at(file: &Path, map: &std::collections::HashMap<String, IntentEntry>) {
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string_pretty(map) {
        let _ = std::fs::write(file, json);
    }
}

/// The intent (and caret) recorded for this document, if any.
pub fn load_intent(doc: &Path) -> Option<IntentEntry> {
    load_intent_at(&intents_file(), doc)
}

fn load_intent_at(file: &Path, doc: &Path) -> Option<IntentEntry> {
    load_intents_at(file).remove(&intent_key(doc))
}

/// End Session writes the full entry: the sentence, when, and where.
pub fn set_intent(doc: &Path, intent: &str, caret: usize) {
    set_intent_at(&intents_file(), doc, intent, caret);
}

fn set_intent_at(file: &Path, doc: &Path, intent: &str, caret: usize) {
    let mut map = load_intents_at(file);
    map.insert(
        intent_key(doc),
        IntentEntry {
            intent: Some(intent.to_owned()),
            set_unix: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
            caret: Some(caret),
        },
    );
    save_intents_at(file, &map);
}

/// First edit or an explicit dismiss honors the intent: the whole entry
/// goes — the caret is re-recorded at the next quit anyway.
pub fn clear_intent(doc: &Path) {
    clear_intent_at(&intents_file(), doc);
}

fn clear_intent_at(file: &Path, doc: &Path) {
    let mut map = load_intents_at(file);
    if map.remove(&intent_key(doc)).is_some() {
        save_intents_at(file, &map);
    }
}

/// Quit-time: remember where the caret was (the resume-mid-sentence half
/// of the ritual). An intent already in the entry survives untouched.
pub fn record_caret(doc: &Path, caret: usize) {
    record_caret_at(&intents_file(), doc, caret);
}

fn record_caret_at(file: &Path, doc: &Path, caret: usize) {
    let mut map = load_intents_at(file);
    map.entry(intent_key(doc)).or_default().caret = Some(caret);
    save_intents_at(file, &map);
}

/// Select the file in the system file manager (freedesktop FileManager1),
/// falling back to opening the containing folder.
pub fn reveal(path: &Path) {
    let uri = format!("file://{}", path.display());
    let ok = std::process::Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.freedesktop.FileManager1",
            "--object-path",
            "/org/freedesktop/FileManager1",
            "--method",
            "org.freedesktop.FileManager1.ShowItems",
            &format!("['{uri}']"),
            "",
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok
        && let Some(dir) = path.parent()
    {
        let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
    }
}

/// Open another document as its own window — one window per document,
/// one process per window (simple, and two windows can never fight over
/// the same CRDT file unless the user opens the same path twice).
pub fn open_in_new_window(path: &Path) {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).arg(path).spawn();
    }
}

/// A fresh Untitled in its own window.
pub fn new_window_blank() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).arg("--new").spawn();
    }
}

/// First free "Welcome to Strop[ N].strop" — the tutorial document's home.
pub fn welcome_path() -> PathBuf {
    let dir = documents_dir();
    let _ = std::fs::create_dir_all(&dir);
    let first = dir.join("Welcome to Strop.strop");
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|n| dir.join(format!("Welcome to Strop {n}.strop")))
        .find(|p| !p.exists())
        .expect("unbounded range")
}

/// A fresh tutorial in its own window (reopenable from the palette).
pub fn open_welcome_window() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).arg("--welcome").spawn();
    }
}

/// Sanitize a typed title into a file stem (keeps Unicode letters, trims
/// path separators and leading dots).
pub fn stem_from_title(title: &str) -> Option<String> {
    let cleaned: String = title
        .trim()
        .chars()
        .map(|c| if c == '/' || c == '\\' || c == '\0' { ' ' } else { c })
        .collect::<String>()
        .trim()
        .trim_start_matches('.')
        .to_string();
    (!cleaned.is_empty()).then_some(cleaned)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One sequential test for everything env-dependent (env is process
    /// global; parallel tests must not each repoint HOME).
    #[test]
    fn lifecycle_in_isolated_home() {
        let tmp = std::env::temp_dir().join(format!("strop-files-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        unsafe {
            std::env::set_var("HOME", &tmp);
            std::env::set_var("XDG_STATE_HOME", tmp.join("state"));
            std::env::set_var("XDG_DATA_HOME", tmp.join("data"));
            std::env::set_var("XDG_CONFIG_HOME", tmp.join("config"));
        }

        // Recents: round-trip, dedupe, most-recent-first.
        let a = tmp.join("a.strop");
        let b = tmp.join("b.strop");
        std::fs::write(&a, b"x").unwrap();
        std::fs::write(&b, b"x").unwrap();
        push_recent(&a);
        push_recent(&b);
        push_recent(&a);
        let r = recents();
        assert_eq!(r.len(), 2);
        assert!(r[0].ends_with("a.strop"));

        // Untitled: visible folder created, numbering advances.
        let u1 = untitled_path();
        assert!(u1.ends_with("Documents/Strop/Untitled.strop"), "{u1:?}");
        assert!(u1.parent().unwrap().is_dir(), "folder exists from birth");
        std::fs::write(&u1, b"x").unwrap();
        assert!(untitled_path().to_string_lossy().contains("Untitled 2"));

        // Legacy hidden scratch migrates into the visible folder.
        let scratch = tmp.join("data/strop/scratch.strop");
        std::fs::create_dir_all(scratch.parent().unwrap()).unwrap();
        std::fs::write(&scratch, b"old").unwrap();
        let migrated = migrate_scratch().expect("migration happens");
        assert!(migrated.ends_with("Scratch.strop"));
        assert!(!scratch.exists());
        assert_eq!(std::fs::read(&migrated).unwrap(), b"old");
        assert!(migrate_scratch().is_none(), "one-time only");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// intents.json round-trip against an injected path (env vars are
    /// process-global; `lifecycle_in_isolated_home` owns them — see
    /// config.rs `save_ai_to` for the same pattern).
    #[test]
    fn intents_round_trip_at_injected_path() {
        let tmp = std::env::temp_dir().join(format!("strop-intents-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let file = tmp.join("state/strop/intents.json");
        let doc_a = tmp.join("Essay.strop");
        let doc_b = tmp.join("Other.strop");
        std::fs::write(&doc_a, b"x").unwrap();
        std::fs::write(&doc_b, b"x").unwrap();

        assert!(load_intent_at(&file, &doc_a).is_none(), "empty start");

        // End Session writes intent + caret; load round-trips both.
        set_intent_at(&file, &doc_a, "finish the bridge scene", 42);
        let entry = load_intent_at(&file, &doc_a).expect("entry saved");
        assert_eq!(entry.intent.as_deref(), Some("finish the bridge scene"));
        assert_eq!(entry.caret, Some(42));
        assert!(entry.set_unix > 0);

        // Keyed by canonical path: a second doc doesn't collide.
        assert!(load_intent_at(&file, &doc_b).is_none());
        record_caret_at(&file, &doc_b, 7);
        let b = load_intent_at(&file, &doc_b).expect("caret-only entry");
        assert!(b.intent.is_none());
        assert_eq!(b.caret, Some(7));

        // Quit-time caret update preserves a recorded intent.
        record_caret_at(&file, &doc_a, 99);
        let entry = load_intent_at(&file, &doc_a).unwrap();
        assert_eq!(entry.intent.as_deref(), Some("finish the bridge scene"));
        assert_eq!(entry.caret, Some(99));

        // First edit / dismiss removes the whole entry; idempotent.
        clear_intent_at(&file, &doc_a);
        assert!(load_intent_at(&file, &doc_a).is_none());
        clear_intent_at(&file, &doc_a);
        assert!(load_intent_at(&file, &doc_b).is_some(), "other doc kept");

        // A garbage file degrades to empty, never panics.
        std::fs::write(&file, b"not json").unwrap();
        assert!(load_intent_at(&file, &doc_a).is_none());
        set_intent_at(&file, &doc_a, "re-seeded", 1);
        assert!(load_intent_at(&file, &doc_a).is_some());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// palette_freq.json round-trip against an injected path (same env
    /// discipline as `intents_round_trip_at_injected_path`).
    #[test]
    fn palette_freq_round_trip_at_injected_path() {
        let tmp = std::env::temp_dir().join(format!("strop-freq-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let file = tmp.join("state/strop/palette_freq.json");

        assert!(load_palette_freq_at(&file).is_empty(), "empty start");

        // Each bump writes through and returns the running count.
        assert_eq!(bump_palette_freq_at(&file, "Toggle Bold"), 1);
        assert_eq!(bump_palette_freq_at(&file, "Toggle Bold"), 2);
        assert_eq!(bump_palette_freq_at(&file, "Find in Document"), 1);
        let map = load_palette_freq_at(&file);
        assert_eq!(map.get("Toggle Bold"), Some(&2));
        assert_eq!(map.get("Find in Document"), Some(&1));

        // A garbage file degrades to empty, never panics.
        std::fs::write(&file, b"not json").unwrap();
        assert!(load_palette_freq_at(&file).is_empty());
        assert_eq!(bump_palette_freq_at(&file, "Undo"), 1, "re-seeded");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn stem_sanitizes_separators_and_dotfiles() {
        assert_eq!(stem_from_title("  Моё эссе  "), Some("Моё эссе".into()));
        assert_eq!(stem_from_title("a/b\\c"), Some("a b c".into()));
        assert_eq!(stem_from_title("..hidden"), Some("hidden".into()));
        assert_eq!(stem_from_title("   "), None);
        assert_eq!(stem_from_title("///"), None);
    }
}
