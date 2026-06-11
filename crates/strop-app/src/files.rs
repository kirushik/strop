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
    if !ok {
        if let Some(dir) = path.parent() {
            let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
        }
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

    #[test]
    fn stem_sanitizes_separators_and_dotfiles() {
        assert_eq!(stem_from_title("  Моё эссе  "), Some("Моё эссе".into()));
        assert_eq!(stem_from_title("a/b\\c"), Some("a b c".into()));
        assert_eq!(stem_from_title("..hidden"), Some("hidden".into()));
        assert_eq!(stem_from_title("   "), None);
        assert_eq!(stem_from_title("///"), None);
    }
}
