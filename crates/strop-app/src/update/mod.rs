//! Self-update — the frozen contract (docs/releasing.md §4).
//!
//! This module is the surface the rest of the app sees: `main()` hooks
//! `startup_apply_if_staged()` before any rendezvous socket is claimed, and
//! About renders `status()`. The machinery (manifest fetch + minisign
//! verification, staging, the installation update lock, the journaled swap)
//! lands inside this directory behind these exact types; until it does,
//! everything here is inert by construction.

// Skeleton allowance: every item below gains a consumer as the updater
// machinery (W1) and About (W2) land. Remove with the last stub.
#![allow(dead_code)]

use std::sync::Mutex;
use std::time::SystemTime;

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
    AppliedThisLaunch { from: String, to: String },
    /// "couldn't apply 0.3.1 — kept 0.3.0." Never a dialog.
    Failed { reason: String },
}

static STATE: Mutex<UpdateState> = Mutex::new(UpdateState::Inert);

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
pub fn startup_apply_if_staged() {}

/// Background check/download loop (launch + every 8 h). Spawned once the
/// first window is up; respects `[update] check` and the channel gate.
pub fn spawn_checks(_config: &crate::config::Config) {}

/// About's "check now" button (§5): one immediate check + download,
/// driving `status()` through the same states as the background loop.
/// Honors the same gates; a no-op wherever the loop would be.
pub fn check_now() {}
