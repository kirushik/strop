//! Document lifecycle (PLAN.md E2): visible-from-birth files.
//!
//! The research verdict: never create a document in a hidden location —
//! GNOME Text Editor's drafts folder is the documented anti-pattern, and
//! Strop's old silent scratch.strop was exactly it. Documents are born as
//! real files in the user's documents folder, findable from second one.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// The document ID and path below it for an XDG document-portal path.
fn portal_path_parts(path: &Path, runtime_dir: &Path) -> Option<(String, PathBuf)> {
    let relative = path.strip_prefix(runtime_dir.join("doc")).ok()?;
    let mut components = relative.components();
    let doc_id = components.next()?.as_os_str().to_str()?.to_owned();
    if doc_id.is_empty() {
        return None;
    }
    Some((doc_id, components.collect()))
}

async fn resolve_portal_path_async_at(path: PathBuf, runtime_dir: &Path) -> PathBuf {
    let Some((doc_id, tail)) = portal_path_parts(&path, runtime_dir) else {
        return path;
    };
    let result: Result<PathBuf, String> = async {
        let documents = ashpd::documents::Documents::new().await
            .map_err(|error| error.to_string())?;
        let id = ashpd::documents::DocumentID::from(doc_id.as_str());
        let paths = documents.host_paths(std::slice::from_ref(&id)).await
            .map_err(|error| error.to_string())?;
        let host_path = paths.get(&id)
            .ok_or_else(|| "portal returned no document host path".to_owned())?;
        let host_dir = host_path.as_ref().parent().unwrap_or(Path::new(""));
        Ok(host_dir.join(tail))
    }
    .await;
    match result {
        Ok(host_path) => host_path,
        Err(error) => {
            eprintln!("strop: could not resolve portal path {}: {error}", path.display());
            path
        }
    }
}

/// Resolve an XDG document-portal path, failing open when the portal is absent.
pub async fn resolve_portal_path_async(path: PathBuf) -> PathBuf {
    let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR") else {
        return path;
    };
    resolve_portal_path_async_at(path, Path::new(&runtime_dir)).await
}

/// Synchronous convenience for path boundaries outside GPUI tasks.
pub fn resolve_portal_path(path: impl Into<PathBuf>) -> PathBuf {
    gpui::block_on(resolve_portal_path_async(path.into()))
}

/// Still under the portal mount — i.e. `resolve_portal_path` couldn't
/// escape it (dead doc id, portal absent). Such a path names plumbing,
/// not a place a writer knows.
fn is_portal_path(path: &Path) -> bool {
    let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR") else {
        return false;
    };
    portal_path_parts(path, Path::new(&runtime_dir)).is_some()
}

/// First free "Untitled.strop" / "Untitled 2.strop" / … in the Strop folder.
pub fn untitled_path() -> PathBuf {
    let dir = crate::paths::documents_dir();
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
    let old = crate::paths::data_dir().join("scratch.strop");
    if !old.exists() {
        return None;
    }
    let dir = crate::paths::documents_dir();
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
    crate::paths::state_dir().join("recents.json")
}

/// Most-recent-first. Missing files stay visible as stale evidence.
/// Portal paths persisted before the resolver existed heal ONCE: the
/// resolved list is written back, so a stale entry cannot re-trigger a
/// D-Bus round-trip (and its failure line) on every palette render —
/// recents() sits on that render path via omni_rows.
pub fn recents() -> Vec<PathBuf> {
    let Ok(json) = std::fs::read_to_string(recents_file()) else {
        return Vec::new();
    };
    let list: Vec<PathBuf> = serde_json::from_str(&json).unwrap_or_default();
    let resolved: Vec<PathBuf> = list
        .iter()
        .cloned()
        .map(resolve_portal_path)
        .filter(|p| !is_portal_path(p))
        .collect();
    if resolved != list {
        let _ = std::fs::write(
            recents_file(),
            serde_json::to_string_pretty(&resolved).unwrap_or_default(),
        );
    }
    resolved
}

fn cap_recents(list: &mut Vec<PathBuf>) {
    let mut live = 0;
    list.retain(|path| {
        if path.exists() {
            live += 1;
            live <= 20
        } else {
            true
        }
    });
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
    cap_recents(&mut list);
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
    crate::paths::state_dir().join("palette_freq.json")
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

/// One entry of ~/.local/state/strop/intents.json: the caret offset at quit
/// so the writer resumes mid-sentence next open (DESIGN §4 re-entry invariant).
/// Keyed by the document's canonical path. The re-entry INTENT question was
/// retired (impl 04 §1); this is now a pure caret-resume sidecar. Old files
/// still carry an `intent`/`set_unix` key — serde ignores unknown fields, so
/// they load fine and drop those keys on the next quit write (the caret
/// survives). The on-disk filename stays `intents.json` so existing carets
/// aren't orphaned.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SessionEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caret: Option<usize>,
}

fn intents_file() -> PathBuf {
    crate::paths::state_dir().join("intents.json")
}

fn intent_key(doc: &Path) -> String {
    doc.canonicalize()
        .unwrap_or_else(|_| doc.to_owned())
        .display()
        .to_string()
}

fn load_sessions_at(file: &Path) -> std::collections::HashMap<String, SessionEntry> {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

fn save_sessions_at(file: &Path, map: &std::collections::HashMap<String, SessionEntry>) {
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string_pretty(map) {
        let _ = std::fs::write(file, json);
    }
}

/// The caret recorded for this document at last quit, if any.
pub fn load_session(doc: &Path) -> Option<SessionEntry> {
    load_session_at(&intents_file(), doc)
}

fn load_session_at(file: &Path, doc: &Path) -> Option<SessionEntry> {
    load_sessions_at(file).remove(&intent_key(doc))
}

/// Quit-time: remember where the caret was (the resume-mid-sentence half of
/// the re-entry ritual — now its whole job).
pub fn record_caret(doc: &Path, caret: usize) {
    record_caret_at(&intents_file(), doc, caret);
}

fn record_caret_at(file: &Path, doc: &Path, caret: usize) {
    let mut map = load_sessions_at(file);
    map.entry(intent_key(doc)).or_default().caret = Some(caret);
    save_sessions_at(file, &map);
}

/// Open a path or URL with the OS default handler. Best-effort and
/// fire-and-forget — a system with no handler registered does nothing, the
/// same failure the bare `xdg-open` had. Per-OS because there is no single
/// portable launcher: `ShellExecuteW` (Windows), `open` (macOS), `xdg-open`
/// (Linux).
pub fn open_external(target: impl AsRef<OsStr>) {
    let target = target.as_ref();
    #[cfg(target_os = "windows")]
    {
        // ShellExecuteW(.., "open", target, ..) — the documented "open with the
        // default handler" call. Deliberately NOT `cmd /C start`: cmd re-parses
        // its command line, so a `&` in a URL query string would split it into
        // separate commands — a broken link, and an injection vector if the URL
        // were ever untrusted. ShellExecuteW takes the target as one wide
        // string, so nothing is reinterpreted; it also spawns no console.
        use std::os::windows::ffi::OsStrExt;

        #[link(name = "shell32")]
        unsafe extern "system" {
            fn ShellExecuteW(
                hwnd: *mut std::ffi::c_void,
                operation: *const u16,
                file: *const u16,
                parameters: *const u16,
                directory: *const u16,
                show_cmd: i32,
            ) -> isize;
        }

        let file: Vec<u16> = target.encode_wide().chain(std::iter::once(0)).collect();
        let open: Vec<u16> = "open".encode_utf16().chain(std::iter::once(0)).collect();
        const SW_SHOWNORMAL: i32 = 1;
        // Best-effort: the returned pseudo-HINSTANCE (> 32 on success) is
        // ignored, matching the other branches' fire-and-forget contract.
        unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                open.as_ptr(),
                file.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            );
        }
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(target).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(target).spawn();
    }
}

/// Select the file in the system file manager, falling back to opening the
/// containing folder. Per-OS: Explorer's `/select,` (Windows), Finder's
/// `open -R` (macOS), the freedesktop FileManager1 D-Bus call with an
/// `xdg-open`-the-folder fallback (Linux).
pub fn reveal(path: &Path) {
    #[cfg(target_os = "windows")]
    {
        // explorer /select,<path> opens the folder with the file highlighted.
        // explorer returns a non-zero exit even on success, so don't check it.
        let mut arg = std::ffi::OsString::from("/select,");
        arg.push(path);
        let _ = std::process::Command::new("explorer").arg(arg).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
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
}

/// Open another document as its own window — one window per document,
/// one process per window (simple, and two windows can never fight over
/// the same CRDT file unless the user opens the same path twice).
pub fn open_in_new_window(path: &Path) {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe)
            .env("STROP_REQUIRE_EXISTING", "1")
            .arg(path)
            .spawn();
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
    let dir = crate::paths::documents_dir();
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

/// Sanitize a typed title into a portable file stem. Legal punctuation such
/// as hyphens and interior dots is preserved because titles rely on it.
pub fn stem_from_title(title: &str) -> Option<String> {
    let mut cleaned: String = title
        .trim()
        .chars()
        .map(|c| {
            if c.is_control()
                || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
            {
                ' '
            } else {
                c
            }
        })
        .collect::<String>()
        .trim()
        .trim_start_matches('.')
        .trim_end_matches(['.', ' '])
        .to_string();
    if cleaned.is_empty() {
        return None;
    }
    let device = cleaned
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let numbered_device = |prefix| {
        device
            .strip_prefix(prefix)
            .is_some_and(|n| {
                matches!(n, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            })
    };
    let reserved = matches!(device.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || numbered_device("COM")
        || numbered_device("LPT");
    if reserved {
        cleaned.push('_');
    }
    Some(cleaned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_portal_path_exposes_id_and_filename() {
        let runtime = Path::new("/tmp/strop-test-runtime");
        let path = runtime.join("doc/7ad41c2e/Draft.strop");
        assert_eq!(
            portal_path_parts(&path, runtime),
            Some(("7ad41c2e".to_owned(), PathBuf::from("Draft.strop")))
        );
    }

    #[test]
    fn ordinary_path_is_not_a_document_portal_path() {
        let runtime = Path::new("/tmp/strop-test-runtime");
        let path = Path::new("/home/writer/Documents/Draft.strop");
        assert_eq!(portal_path_parts(path, runtime), None);
        assert_eq!(
            gpui::block_on(resolve_portal_path_async_at(path.to_owned(), runtime)),
            path
        );
    }

    #[test]
    fn document_portal_path_keeps_nested_tail() {
        let runtime = Path::new("/tmp/strop-test-runtime");
        let path = runtime.join("doc/7ad41c2e/Project/images/cover.png");
        assert_eq!(
            portal_path_parts(&path, runtime),
            Some((
                "7ad41c2e".to_owned(),
                PathBuf::from("Project/images/cover.png")
            ))
        );
    }

    /// One sequential test for everything env-dependent (env is process
    /// global; parallel tests must not each repoint HOME). Linux/BSD only:
    /// it isolates by repointing HOME/XDG_*, but `directories` honours those
    /// only on the XDG platforms — on macOS/Windows it reads the OS Known
    /// Folders, so the redirection (and thus the isolation) has no effect.
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
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

    /// intents.json caret round-trip against an injected path (env vars are
    /// process-global; `lifecycle_in_isolated_home` owns them — see
    /// config.rs `save_ai_to` for the same pattern). The intent half was
    /// retired (impl 04 §1); the caret-resume half is what remains.
    #[test]
    fn session_caret_round_trips_at_injected_path() {
        let tmp = std::env::temp_dir().join(format!("strop-intents-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let file = tmp.join("state/strop/intents.json");
        let doc_a = tmp.join("Essay.strop");
        let doc_b = tmp.join("Other.strop");
        std::fs::write(&doc_a, b"x").unwrap();
        std::fs::write(&doc_b, b"x").unwrap();

        assert!(load_session_at(&file, &doc_a).is_none(), "empty start");

        // Quit records the caret; load round-trips it. Keyed by canonical
        // path, so a second document never collides.
        record_caret_at(&file, &doc_a, 42);
        assert_eq!(load_session_at(&file, &doc_a).unwrap().caret, Some(42));
        assert!(load_session_at(&file, &doc_b).is_none());
        record_caret_at(&file, &doc_b, 7);
        assert_eq!(load_session_at(&file, &doc_b).unwrap().caret, Some(7));

        // A later quit overwrites the caret in place, in one entry, without
        // disturbing the other document.
        record_caret_at(&file, &doc_a, 99);
        assert_eq!(load_session_at(&file, &doc_a).unwrap().caret, Some(99));
        assert_eq!(load_session_at(&file, &doc_b).unwrap().caret, Some(7));

        // An old file still carrying the retired intent keys loads fine
        // (serde ignores unknown fields) and keeps the caret.
        // Use serde_json::to_string to properly escape the key (backslashes
        // in Windows paths must be JSON-escaped; a raw format!() does not do
        // that, producing invalid JSON that fails to parse).
        let key_json = serde_json::to_string(&intent_key(&doc_a)).unwrap();
        std::fs::write(
            &file,
            format!(
                "{{{key_json}:{{\"intent\":\"finish the scene\",\"set_unix\":123,\"caret\":5}}}}",
            ),
        )
        .unwrap();
        assert_eq!(load_session_at(&file, &doc_a).unwrap().caret, Some(5));

        // A garbage file degrades to empty, never panics.
        std::fs::write(&file, b"not json").unwrap();
        assert!(load_session_at(&file, &doc_a).is_none());
        record_caret_at(&file, &doc_a, 1);
        assert!(load_session_at(&file, &doc_a).is_some());

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
        assert_eq!(stem_from_title("draft.old"), Some("draft.old".into()));
        assert_eq!(
            stem_from_title("my-notes v0.2"),
            Some("my-notes v0.2".into())
        );
        assert_eq!(stem_from_title("CON"), Some("CON_".into()));
        assert_eq!(stem_from_title("con.txt"), Some("con.txt_".into()));
        assert_eq!(stem_from_title("notes."), Some("notes".into()));
        assert_eq!(stem_from_title("a:b"), Some("a b".into()));
    }

    #[test]
    fn missing_recents_do_not_consume_the_live_cap() {
        let root = std::env::temp_dir().join(format!(
            "strop-recents-cap-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let missing = root.join("missing.strop");
        let mut list = vec![missing.clone()];
        for ix in 0..21 {
            let path = root.join(format!("live-{ix}.strop"));
            std::fs::write(&path, b"x").unwrap();
            list.push(path);
        }

        cap_recents(&mut list);
        assert_eq!(list.iter().filter(|path| path.exists()).count(), 20);
        assert!(list.contains(&missing));
        let _ = std::fs::remove_dir_all(root);
    }
}
