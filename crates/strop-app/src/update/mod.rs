//! Self-update — the frozen contract (docs/releasing.md §4).
//!
//! This module is the surface the rest of the app sees: `main()` hooks
//! `startup_apply_if_staged()` before any rendezvous socket is claimed, and
//! About renders `status()`. The machinery — manifest fetch + minisign
//! verification, staging, the installation update lock, the journaled
//! swap — lives in this directory's submodules behind these types.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

mod fetch;
mod manifest;
// The recovery table's consumers are the cfg-gated Windows/macOS apply
// paths; a Linux build carries the module (and its tests) target-dead.
#[cfg_attr(any(target_os = "linux", target_os = "freebsd"), allow(dead_code))]
mod recovery;
mod sha256;
mod storage;

const MAX_ARTIFACT_SIZE: u64 = 256 * 1024 * 1024;
const MANIFEST_LIMIT: u64 = 1024 * 1024;

/// Updater wire-protocol version. Bound into the signed manifest and
/// compared for exact equality before anything is downloaded — a manifest
/// for a future protocol is not "newer", it is *not ours*.
pub const UPDATER_PROTOCOL: u32 = 1;

/// How this binary reached the user's disk. Baked at package time via
/// `STROP_DIST_CHANNEL`; a local `cargo build` carries none and stays
/// `Dev`, where the updater is fully inert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    GithubWin,
    GithubMac,
    GithubWinPortable,
    GithubLinux,
    Flathub,
    Deb,
    Rpm,
    Dev,
}

pub fn channel() -> Channel {
    match option_env!("STROP_DIST_CHANNEL") {
        Some("github-win") => Channel::GithubWin,
        Some("github-mac") => Channel::GithubMac,
        Some("github-win-portable") => Channel::GithubWinPortable,
        Some("github-linux") => Channel::GithubLinux,
        Some("flathub") => Channel::Flathub,
        Some("deb") => Channel::Deb,
        Some("rpm") => Channel::Rpm,
        _ => Channel::Dev,
    }
}

impl Channel {
    /// The two channels that swap their own binary (§4). Everything else
    /// belongs to a package manager, or to nobody.
    pub fn self_updates(self) -> bool {
        matches!(self, Channel::GithubWin | Channel::GithubMac)
    }

    /// Channels that only ever *mention* a new version in About — the
    /// passive "0.3.2 is out" line, nothing more.
    pub fn passive_notify(self) -> bool {
        matches!(self, Channel::GithubWinPortable | Channel::GithubLinux)
    }
}

/// Every state About can show — the updater's one surface (§5: all states,
/// one calm place; the control is the indicator).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateState {
    /// Dev build or store-managed channel: there is nothing to say.
    Inert,
    /// `[update] check = false`: the writer turned even the check off.
    Disabled,
    Idle { last_check: Option<SystemTime> },
    Checking,
    /// A newer version exists. Passive channels stop here.
    Available { version: String },
    /// Verified and staged: "0.3.1 downloaded — next launch gets it."
    Staged { version: String },
    /// This launch swapped binaries: "updated 0.3.0 → 0.3.1 · what changed."
    AppliedThisLaunch { from: String, to: String, notes_url: String },
    /// "couldn't apply 0.3.1 — kept 0.3.0." Never a dialog.
    Failed { attempted: Option<String>, kept: String, reason: String },
}

static STATE: Mutex<UpdateState> = Mutex::new(UpdateState::Inert);
static CHECKS_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn status() -> UpdateState {
    STATE.lock().expect("update state poisoned").clone()
}

#[allow(dead_code)] // the machinery lands behind this
pub(crate) fn set_status(s: UpdateState) {
    *STATE.lock().expect("update state poisoned") = s;
}

/// Called at the very top of `main()`, before document resolution and
/// before any rendezvous socket exists (§4 "Apply — strictly before
/// rendezvous"): if a verified update is staged, take the installation
/// update lock, swap, re-exec. No-op until the machinery lands, and forever
/// a no-op on channels that don't self-update.
pub fn startup_apply_if_staged() {
    if !channel().self_updates() || manifest::baked_keys().is_empty() { return; }
    if std::env::args_os().any(|arg| arg == "--rollback") {
        rollback();
        return;
    }
    let Some(_lock) = storage::try_lock().ok().flatten() else { return; };
    if let Err(reason) = recover_if_needed() {
        fail(reason, None, None);
        return;
    }
    let mut state = storage::load_state();
    if let (Some(from), Some(to), Some(notes_url)) = (state.applied_from.take(),
        state.applied_to.take(), state.applied_notes_url.take())
    {
        let _ = storage::save_state(&state);
        set_status(UpdateState::AppliedThisLaunch { from, to, notes_url });
        return;
    }
    let staged = storage::find_ready().ok().flatten();
    let Some((artifact, ready, notes_url)) = staged else {
        if let Some(reason) = state.failed_reason {
            set_status(UpdateState::Failed { attempted: state.failed_attempted,
                kept: env!("CARGO_PKG_VERSION").to_owned(), reason });
        }
        return;
    };
    if let Err(reason) = storage::verify_ready(&artifact, &ready) {
        fail(reason, Some(ready.version.clone()), Some(&artifact));
        return;
    }
    if let Err(reason) = apply(&artifact, &ready, &notes_url) {
        fail(reason, Some(ready.version.clone()), Some(&artifact));
    }
}

/// Background check/download loop (launch + every 8 h). Spawned once the
/// first window is up; respects `[update] check` and the channel gate.
pub fn spawn_checks(config: &crate::config::Config) {
    if !active_channel() { set_status(UpdateState::Inert); return; }
    if !config.update.check {
        CHECKS_ENABLED.store(false, Ordering::Release);
        set_status(UpdateState::Disabled);
        return;
    }
    CHECKS_ENABLED.store(true, Ordering::Release);
    let _ = std::thread::Builder::new().name("strop-update-check".into()).spawn(|| loop {
        check_cycle(&fetch::NetworkFetcher);
        std::thread::sleep(Duration::from_secs(8 * 60 * 60));
    });
}

/// About's "check now" button (§5): one immediate check + download,
/// driving `status()` through the same states as the background loop.
/// Honors the same gates; a no-op wherever the loop would be.
pub fn check_now() {
    if !CHECKS_ENABLED.load(Ordering::Acquire) || !active_channel() { return; }
    let _ = std::thread::Builder::new().name("strop-update-now".into())
        .spawn(|| check_cycle(&fetch::NetworkFetcher));
}

fn active_channel() -> bool {
    !manifest::baked_keys().is_empty() && (channel().self_updates() || channel().passive_notify())
}

fn check_cycle(fetcher: &dyn fetch::Fetcher) {
    set_status(UpdateState::Checking);
    let result = (|| {
        let bytes = fetch::fetch_following(fetcher, fetch::LATEST_URL, MANIFEST_LIMIT)?;
        let mut signatures = Vec::new();
        signatures.push(fetch::fetch_following(fetcher, &format!("{}.minisig", fetch::LATEST_URL), MANIFEST_LIMIT)?);
        if let Ok(second) = fetch::fetch_following(fetcher, &format!("{}.minisig2", fetch::LATEST_URL), MANIFEST_LIMIT) {
            signatures.push(second);
        }
        let keys = manifest::baked_keys();
        manifest::verify_signed(&bytes, &signatures, &keys)?;
        let mut state = storage::load_state();
        let highest = state.highest_seen.as_deref().and_then(|s| semver::Version::parse(s).ok());
        let Some((manifest, target, version)) =
            manifest::parse_and_validate(&bytes, channel(), highest.as_ref())?
        else {
            // The healthy everyday outcome: verified, and nothing newer.
            state.failed_reason = None;
            storage::save_state(&state)?;
            set_status(UpdateState::Idle { last_check: Some(SystemTime::now()) });
            return Ok(());
        };
        state.highest_seen = Some(version.to_string());
        state.failed_reason = None;
        storage::save_state(&state)?;
        if channel().passive_notify() {
            set_status(UpdateState::Available { version: version.to_string() });
            return Ok(());
        }
        set_status(UpdateState::Available { version: version.to_string() });
        let artifact = fetch::fetch_following(fetcher, &target.url, MAX_ARTIFACT_SIZE)?;
        storage::stage(&version, &manifest.notes_url, &target, &artifact)?;
        set_status(UpdateState::Staged { version: version.to_string() });
        Ok::<_, String>(())
    })();
    if let Err(reason) = result { fail(reason, None, None); }
}

fn fail(reason: String, attempted: Option<String>, artifact: Option<&std::path::Path>) {
    if let Some(path) = artifact { let _ = storage::remove_stage_for(path); }
    let mut state = storage::load_state();
    state.failed_reason = Some(reason.clone());
    state.failed_attempted = attempted.clone();
    let _ = storage::save_state(&state);
    set_status(UpdateState::Failed {
        attempted, kept: env!("CARGO_PKG_VERSION").to_owned(), reason,
    });
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Journal { phase: recovery::Phase, from: String, to: String,
    notes_url: String }

#[cfg(windows)]
fn apply(artifact: &std::path::Path, ready: &storage::Ready, notes_url: &str) -> Result<(), String> {
    let current = std::env::current_exe().map_err(|e| e.to_string())?;
    let previous = current.with_file_name("strop-prev.exe");
    let from = env!("CARGO_PKG_VERSION");
    storage::save_journal(&Journal { phase: recovery::Phase::Prepared,
        from: from.into(), to: ready.version.clone(), notes_url: notes_url.into() })?;
    std::fs::copy(&current, &previous).map_err(|e| format!("couldn't preserve previous binary: {e}"))?;
    self_replace::self_replace(artifact).map_err(|e| format!("couldn't replace executable: {e}"))?;
    storage::save_journal(&Journal { phase: recovery::Phase::PreviousSaved,
        from: from.into(), to: ready.version.clone(), notes_url: notes_url.into() })?;
    rewrite_display_version(&ready.version);
    record_applied_and_respawn(from, &ready.version, notes_url, &current, artifact)
}

#[cfg(target_os = "macos")]
fn apply(artifact: &std::path::Path, ready: &storage::Ready, notes_url: &str) -> Result<(), String> {
    use std::os::unix::ffi::OsStrExt;
    let current = std::env::current_exe().map_err(|e| e.to_string())?;
    let macos = current.parent().filter(|p| p.file_name().is_some_and(|n| n == "MacOS"))
        .ok_or_else(|| "current executable is not inside Contents/MacOS".to_owned())?;
    let contents = macos.parent().filter(|p| p.file_name().is_some_and(|n| n == "Contents"))
        .ok_or_else(|| "current executable is not inside Contents/MacOS".to_owned())?;
    let bundle = contents.parent().ok_or_else(|| "application bundle has no root".to_owned())?;
    let parent = bundle.parent().ok_or_else(|| "application bundle has no writable parent".to_owned())?;
    let staged = parent.join("Strop.app.staged");
    let previous = parent.join("Strop.app.previous");
    let unpacked = storage::unpacked_bundle(artifact)?;
    if staged.exists() { std::fs::remove_dir_all(&staged).map_err(|e| e.to_string())?; }
    copy_tree(&unpacked, &staged)?;
    storage::verify_content_manifest(artifact, &staged)?;
    let journal = Journal { phase: recovery::Phase::Copied,
        from: env!("CARGO_PKG_VERSION").into(), to: ready.version.clone(),
        notes_url: notes_url.into() };
    storage::save_journal(&journal)?;
    let old = std::ffi::CString::new(bundle.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
    let new = std::ffi::CString::new(staged.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
    const RENAME_SWAP: u32 = 0x00000002;
    unsafe extern "C" { fn renamex_np(old: *const i8, new: *const i8, flags: u32) -> i32; }
    if unsafe { renamex_np(old.as_ptr(), new.as_ptr(), RENAME_SWAP) } != 0 {
        return Err(format!("atomic bundle exchange failed: {}", std::io::Error::last_os_error()));
    }
    storage::save_journal(&Journal { phase: recovery::Phase::Swapped, ..journal })?;
    if previous.exists() { std::fs::remove_dir_all(&previous).map_err(|e| e.to_string())?; }
    std::fs::rename(&staged, &previous).map_err(|e| e.to_string())?;
    storage::save_journal(&Journal { phase: recovery::Phase::PreviousSaved,
        from: env!("CARGO_PKG_VERSION").into(), to: ready.version.clone(),
        notes_url: notes_url.into() })?;
    let new_exe = bundle.join("Contents/MacOS").join(current.file_name().unwrap());
    record_applied_and_respawn(env!("CARGO_PKG_VERSION"), &ready.version,
        notes_url, &new_exe, artifact)
}

#[cfg(not(any(windows, target_os = "macos")))]
fn apply(_artifact: &std::path::Path, _ready: &storage::Ready, _notes_url: &str) -> Result<(), String> {
    Err("self-update target does not match this operating system".into())
}

#[cfg(any(windows, target_os = "macos"))]
fn record_applied_and_respawn(from: &str, to: &str, notes_url: &str,
    current: &std::path::Path, artifact: &std::path::Path) -> Result<(), String> {
    let mut state = storage::load_state();
    state.applied_from = Some(from.to_owned()); state.applied_to = Some(to.to_owned());
    state.applied_notes_url = Some(notes_url.to_owned());
    storage::save_state(&state)?;
    storage::remove_stage_for(artifact)?;
    storage::clear_journal()?;
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    std::process::Command::new(current).args(args).spawn().map_err(|e| e.to_string())?;
    std::process::exit(0)
}

#[cfg(target_os = "macos")]
fn copy_tree(source: &std::path::Path, destination: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir(destination).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let to = destination.join(entry.file_name());
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            copy_tree(&entry.path(), &to)?;
        } else {
            std::fs::copy(entry.path(), to).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(windows)]
fn rewrite_display_version(version: &str) {
    use windows_sys::Win32::System::Registry::*;
    let path: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Strop\0"
        .encode_utf16().collect();
    let name: Vec<u16> = "DisplayVersion\0".encode_utf16().collect();
    let value: Vec<u16> = format!("{version}\0").encode_utf16().collect();
    unsafe {
        let mut key = std::ptr::null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, path.as_ptr(), 0, KEY_SET_VALUE, &mut key) == 0 {
            let _ = RegSetValueExW(key, name.as_ptr(), 0, REG_SZ,
                value.as_ptr().cast(), (value.len() * 2) as u32);
            let _ = RegCloseKey(key);
        }
    }
}

fn recover_if_needed() -> Result<(), String> {
    let Some(journal) = storage::load_journal::<Journal>()? else { return Ok(()); };
    #[cfg(windows)]
    {
        let current = std::env::current_exe().map_err(|e| e.to_string())?;
        let previous = current.with_file_name("strop-prev.exe");
        let action = recovery::classify(journal.phase, recovery::Observed {
            current: current.exists(), staged: storage::find_ready()?.is_some(),
            previous: previous.exists(),
        });
        match action {
            recovery::Action::RestorePrevious => self_replace::self_replace(previous).map_err(|e| e.to_string())?,
            recovery::Action::Finish | recovery::Action::DiscardStaged => storage::clear_journal()?,
            recovery::Action::ResumeSwap => {
                // Prepared spans three crash positions; whether the swap
                // already happened is decided by content, not guesswork —
                // the staged artifact IS the executable we would become.
                let Some((artifact, ready, _)) = storage::find_ready()? else {
                    return Err("recovery expected a staged update".into());
                };
                let swapped = storage::verify_ready(&current, &ready).is_ok();
                if swapped {
                    // The swap completed but its journal record didn't:
                    // finish the interrupted bookkeeping. The binary on disk
                    // is already the new version — consume the stage, or the
                    // next launch re-applies it and copies the NEW executable
                    // over strop-prev.exe, destroying the rollback.
                    rewrite_display_version(&journal.to);
                    let mut state = storage::load_state();
                    state.applied_from = Some(journal.from); state.applied_to = Some(journal.to);
                    state.applied_notes_url = Some(journal.notes_url); storage::save_state(&state)?;
                    storage::remove_stage_for(&artifact)?;
                }
                // Otherwise the swap never happened: current is still the old
                // binary, so clearing the journal and letting the normal
                // startup path re-verify and re-apply from scratch is correct
                // (re-copying old-current to strop-prev.exe loses nothing).
                storage::clear_journal()?;
            }
            recovery::Action::RespawnCurrent => {
                // The journal proves the previous binary was saved, but the
                // stage must still be consumed here — same rollback-erasing
                // re-apply as ResumeSwap otherwise — and the display version
                // rewritten (the crash may have preceded it).
                rewrite_display_version(&journal.to);
                let mut state = storage::load_state();
                state.applied_from = Some(journal.from); state.applied_to = Some(journal.to);
                state.applied_notes_url = Some(journal.notes_url); storage::save_state(&state)?;
                if let Some((artifact, _, _)) = storage::find_ready()? {
                    storage::remove_stage_for(&artifact)?;
                }
                storage::clear_journal()?;
            }
            recovery::Action::Fail => return Err("update recovery found no valid executable".into()),
            // Mac-shaped action; Windows never journals the Swapped phase.
            recovery::Action::SavePrevious => storage::clear_journal()?,
        }
    }
    #[cfg(target_os = "macos")]
    {
        let current = std::env::current_exe().map_err(|e| e.to_string())?;
        let bundle = current.parent().and_then(|p| p.parent()).and_then(|p| p.parent())
            .ok_or_else(|| "couldn't derive bundle during recovery".to_owned())?;
        let parent = bundle.parent().ok_or_else(|| "bundle has no parent".to_owned())?;
        let staged = parent.join("Strop.app.staged");
        let previous = parent.join("Strop.app.previous");
        let action = recovery::classify(journal.phase, recovery::Observed {
            current: bundle.exists(), staged: staged.exists(), previous: previous.exists(),
        });
        match action {
            recovery::Action::DiscardStaged => { if staged.exists() { std::fs::remove_dir_all(staged).map_err(|e| e.to_string())?; } }
            recovery::Action::SavePrevious => { if previous.exists() { std::fs::remove_dir_all(&previous).map_err(|e| e.to_string())?; } std::fs::rename(staged, previous).map_err(|e| e.to_string())?; }
            recovery::Action::RestorePrevious => { return Err("current application bundle is missing".into()); }
            recovery::Action::Fail => return Err("update recovery found no valid application bundle".into()),
            _ => {}
        }
        if matches!(action, recovery::Action::SavePrevious |
            recovery::Action::RespawnCurrent | recovery::Action::Finish)
            && !matches!(journal.phase, recovery::Phase::RollbackPrepared |
                recovery::Phase::RolledBack)
        {
            let mut state = storage::load_state();
            state.applied_from = Some(journal.from.clone());
            state.applied_to = Some(journal.to.clone());
            state.applied_notes_url = Some(journal.notes_url.clone());
            storage::save_state(&state)?;
            if let Some((artifact, _, _)) = storage::find_ready()? {
                storage::remove_stage_for(&artifact)?;
            }
        }
        storage::clear_journal()?;
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    { let _ = journal; storage::clear_journal()?; }
    Ok(())
}

fn rollback() {
    #[cfg(windows)]
    let result = (|| {
        let current = std::env::current_exe().map_err(|e| e.to_string())?;
        let previous = current.with_file_name("strop-prev.exe");
        if !previous.is_file() { return Err("no previous Strop binary is available".to_owned()); }
        self_replace::self_replace(&previous).map_err(|e| e.to_string())?;
        let args: Vec<_> = std::env::args_os().skip(1).filter(|a| a != "--rollback").collect();
        std::process::Command::new(current).args(args).spawn().map_err(|e| e.to_string())?;
        Ok::<_, String>(())
    })();
    #[cfg(target_os = "macos")]
    let result = (|| {
        use std::os::unix::ffi::OsStrExt;
        let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let bundle = current_exe.parent().and_then(|p| p.parent()).and_then(|p| p.parent())
            .filter(|p| p.file_name().is_some_and(|n| n == "Strop.app"))
            .ok_or_else(|| "current executable is not inside Strop.app/Contents/MacOS".to_owned())?;
        let previous = bundle.parent().unwrap().join("Strop.app.previous");
        if !previous.is_dir() { return Err("no previous Strop application is available".to_owned()); }
        storage::save_journal(&Journal { phase: recovery::Phase::RollbackPrepared,
            from: env!("CARGO_PKG_VERSION").into(), to: "previous".into(), notes_url: String::new() })?;
        let old = std::ffi::CString::new(bundle.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
        let new = std::ffi::CString::new(previous.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
        const RENAME_SWAP: u32 = 0x00000002;
        unsafe extern "C" { fn renamex_np(old: *const i8, new: *const i8, flags: u32) -> i32; }
        if unsafe { renamex_np(old.as_ptr(), new.as_ptr(), RENAME_SWAP) } != 0 {
            return Err(format!("atomic rollback exchange failed: {}", std::io::Error::last_os_error()));
        }
        storage::clear_journal()?;
        let exe = bundle.join("Contents/MacOS").join(current_exe.file_name().unwrap());
        let args: Vec<_> = std::env::args_os().skip(1).filter(|a| a != "--rollback").collect();
        std::process::Command::new(exe).args(args).spawn().map_err(|e| e.to_string())?;
        Ok::<_, String>(())
    })();
    #[cfg(not(any(windows, target_os = "macos")))]
    let result: Result<(), String> = Err("no previous Strop application is available".into());
    if let Err(reason) = result { eprintln!("strop: rollback failed: {reason}"); std::process::exit(1); }
    std::process::exit(0);
}
