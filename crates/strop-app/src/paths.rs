//! Cross-platform user-directory resolution. The one place that knows where
//! Strop keeps its files on each OS, so the rest of the app never reads
//! `$HOME` / `$XDG_*` directly — those are Unix conventions, and the old
//! `HOME.expect("HOME not set")` calls panicked outright on Windows.
//!
//! Backed by the `directories` crate: XDG base dirs on Linux,
//! `~/Library/Application Support` on macOS, the Known Folders on Windows.
//! The reverse-DNS id is `cc.pimenov.strop` (the pimenov.cc domain), which
//! gives macOS a tidy bundle-style folder. Linux normalizes the *application*
//! component to `strop`, so `~/.config/strop`, `~/.local/state/strop`,
//! `~/.local/share/strop` are byte-for-byte what Strop has always used —
//! existing Linux installs need no migration.
//!
//! Every lookup is recomputed (never memoized) so the env-repointing tests in
//! `files.rs` / `config.rs` keep isolating: `directories` reads the XDG/HOME
//! variables at construction, and these are not hot paths.

use std::path::{Path, PathBuf};

use directories::{BaseDirs, ProjectDirs, UserDirs};

fn project() -> Option<ProjectDirs> {
    ProjectDirs::from("cc", "pimenov", "strop")
}

/// Last-resort base when the platform reports no home directory at all (should
/// never happen on a real desktop). Keeps every lookup below total instead of
/// panicking the way the old `HOME.expect()` did.
fn fallback_root() -> PathBuf {
    std::env::temp_dir().join("strop")
}

/// Per-user configuration directory.
/// Lin `~/.config/strop` · mac `~/Library/Application Support/cc.pimenov.strop`
/// · win `%APPDATA%\pimenov\strop\config`.
pub fn config_dir() -> PathBuf {
    project()
        .map(|p| p.config_dir().to_path_buf())
        .unwrap_or_else(|| fallback_root().join("config"))
}

/// Per-user state directory for small, machine-local, non-essential state
/// (recents, window bounds, palette frequencies, intents). Lin
/// `~/.local/state/strop` (XDG_STATE_HOME). macOS and Windows have no distinct
/// state location, so it falls back to the data directory there.
pub fn state_dir() -> PathBuf {
    match project() {
        Some(p) => p.state_dir().unwrap_or_else(|| p.data_dir()).to_path_buf(),
        None => fallback_root().join("state"),
    }
}

/// Per-user data directory. The legacy hidden scratch lived here.
/// Lin `~/.local/share/strop`.
pub fn data_dir() -> PathBuf {
    project()
        .map(|p| p.data_dir().to_path_buf())
        .unwrap_or_else(|| fallback_root().join("data"))
}

/// The Strop subfolder inside the user's Documents folder — where new
/// documents are born visible (see `files.rs`). Lin honors `XDG_DOCUMENTS_DIR`
/// via xdg-user-dirs (falling back to `~/Documents`); macOS and Windows use
/// the OS Documents folder.
pub fn documents_dir() -> PathBuf {
    let base = UserDirs::new()
        .and_then(|u| u.document_dir().map(Path::to_path_buf))
        .unwrap_or_else(|| home_dir().join("Documents"));
    base.join("Strop")
}

/// Ephemeral rendezvous directory for the single-instance socket. Lin
/// `$XDG_RUNTIME_DIR/strop` (tmpfs, cleared on logout); macOS and Windows have
/// no runtime dir, so the OS temp dir stands in. On Windows the path is only a
/// source for the named-pipe name — nothing is written there.
pub fn runtime_dir() -> PathBuf {
    project()
        .and_then(|p| p.runtime_dir().map(Path::to_path_buf))
        .unwrap_or_else(std::env::temp_dir)
}

/// The user's home directory, non-panicking. Used only to expand a leading
/// `~/` in user-supplied corpus globs and to tilde-compress displayed paths
/// (`editor.rs`).
pub fn home_dir() -> PathBuf {
    BaseDirs::new()
        .map(|b| b.home_dir().to_path_buf())
        .unwrap_or_else(std::env::temp_dir)
}
