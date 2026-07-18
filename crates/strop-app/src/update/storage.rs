use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use fs4::fs_std::FileExt;
use semver::Version;
use serde::{Deserialize, Serialize};

use super::manifest::Target;

static NONCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PersistentState {
    pub highest_seen: Option<String>,
    pub failed_reason: Option<String>,
    pub failed_attempted: Option<String>,
    pub applied_from: Option<String>,
    pub applied_to: Option<String>,
    pub applied_notes_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ready {
    pub version: String,
    pub sha256: String,
    pub artifact: String,
}

#[derive(Serialize, Deserialize)]
struct Metadata { notes_url: String }

pub struct UpdateLock { _file: File }

pub fn root() -> PathBuf { crate::paths::data_dir().join("updates") }
fn state_path() -> PathBuf { root().join("state.json") }

pub fn load_state() -> PersistentState {
    match fs::read(state_path()) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            eprintln!("strop updater: ignoring corrupt state.json: {e}");
            PersistentState::default()
        }),
        Err(_) => PersistentState::default(),
    }
}

pub fn save_state(state: &PersistentState) -> Result<(), String> {
    fs::create_dir_all(root()).map_err(|e| e.to_string())?;
    durable_json(&state_path(), state)
}

// Callers are the cfg-gated Windows/macOS apply bodies; on a Linux build
// only the recovery reader survives, so the writer is target-dead here.
#[cfg_attr(any(target_os = "linux", target_os = "freebsd"), allow(dead_code))]
pub fn save_journal(value: &impl Serialize) -> Result<(), String> {
    fs::create_dir_all(root()).map_err(|e| e.to_string())?;
    durable_json(&root().join("journal.json"), value)
}

pub fn load_journal<T: for<'de> Deserialize<'de>>() -> Result<Option<T>, String> {
    match fs::read(root().join("journal.json")) {
        Ok(bytes) => serde_json::from_slice(&bytes).map(Some).map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

pub fn clear_journal() -> Result<(), String> {
    match fs::remove_file(root().join("journal.json")) {
        Ok(()) => sync_dir(&root()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

pub fn remove_stage_for(artifact: &Path) -> Result<(), String> {
    if let Some(stage) = artifact.parent() {
        fs::remove_dir_all(stage).map_err(|e| e.to_string())?;
        sync_dir(&root())?;
    }
    Ok(())
}

pub fn try_lock() -> Result<Option<UpdateLock>, String> {
    try_lock_at(&root())
}

fn try_lock_at(root: &Path) -> Result<Option<UpdateLock>, String> {
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let file = OpenOptions::new().read(true).write(true).create(true).truncate(false)
        .open(root.join("lock")).map_err(|e| e.to_string())?;
    match file.try_lock_exclusive() {
        Ok(true) => Ok(Some(UpdateLock { _file: file })),
        Ok(false) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

pub fn stage(version: &Version, notes_url: &str, target: &Target, bytes: &[u8]) -> Result<(), String> {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos()
        as u64 ^ NONCE.fetch_add(1, Ordering::Relaxed);
    let tmp = root().join(format!("tmp-{}-{nonce:016x}", std::process::id()));
    fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let name = target.url.rsplit('/').next().filter(|n| !n.is_empty() && !n.contains(['/', '\\']))
        .ok_or_else(|| "artifact URL has no safe filename".to_owned())?;
    let artifact = tmp.join(name);
    let result = (|| {
        let mut file = File::create(&artifact).map_err(|e| e.to_string())?;
        file.write_all(bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        let (hash, size) = super::sha256::reader(File::open(&artifact).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        if size != target.size || !hash.eq_ignore_ascii_case(&target.sha256) {
            return Err("downloaded artifact failed size or SHA-256 verification".into());
        }
        #[cfg(target_os = "macos")]
        unpack_app_tar(&artifact, &tmp)?;
        let Some(_lock) = try_lock()? else { return Err("another updater is publishing".into()); };
        reconcile(&root())?;
        let stage = root().join(format!("stage-{version}"));
        if stage.exists() { fs::remove_dir_all(&stage).map_err(|e| e.to_string())?; }
        fs::rename(&tmp, &stage).map_err(|e| e.to_string())?;
        durable_json(&stage.join("ready"), &Ready {
            version: version.to_string(), sha256: hash, artifact: name.to_owned(),
        })?;
        durable_json(&stage.join("metadata.json"), &Metadata { notes_url: notes_url.to_owned() })?;
        sync_dir(&stage)?;
        sync_dir(&root())?;
        gc_except(&stage)?;
        Ok(())
    })();
    if result.is_err() { let _ = fs::remove_dir_all(&tmp); }
    result
}

#[cfg(target_os = "macos")]
#[derive(Serialize, Deserialize)]
struct ContentManifest { files: std::collections::BTreeMap<PathBuf, String> }

#[cfg(target_os = "macos")]
fn unpack_app_tar(artifact: &Path, tmp: &Path) -> Result<(), String> {
    let unpacked = tmp.join("unpacked");
    fs::create_dir(&unpacked).map_err(|e| e.to_string())?;
    let status = std::process::Command::new("/usr/bin/tar").args(["-xzf"])
        .arg(artifact).arg("-C").arg(&unpacked).arg("--no-same-owner")
        .status().map_err(|e| format!("couldn't run system tar: {e}"))?;
    if !status.success() { return Err("update app-tar could not be unpacked".into()); }
    let bundle = unpacked.join("Strop.app");
    if !bundle.is_dir() { return Err("update app-tar does not contain Strop.app".into()); }
    let mut files = std::collections::BTreeMap::new();
    hash_tree(&bundle, &bundle, &mut files)?;
    durable_json(&tmp.join("content.json"), &ContentManifest { files })
}

#[cfg(target_os = "macos")]
fn hash_tree(root: &Path, dir: &Path,
    files: &mut std::collections::BTreeMap<PathBuf, String>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_symlink() { return Err("update bundle contains a symbolic link".into()); }
        if kind.is_dir() { hash_tree(root, &entry.path(), files)?; }
        else if kind.is_file() {
            let relative = entry.path().strip_prefix(root).map_err(|e| e.to_string())?.to_owned();
            let (hash, _) = super::sha256::reader(File::open(entry.path()).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            files.insert(relative, hash);
        } else { return Err("update bundle contains a special file".into()); }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn unpacked_bundle(artifact: &Path) -> Result<PathBuf, String> {
    let stage = artifact.parent().ok_or_else(|| "staged artifact has no directory".to_owned())?;
    let bundle = stage.join("unpacked/Strop.app");
    if bundle.is_dir() { Ok(bundle) } else { Err("staged application bundle is missing".into()) }
}

#[cfg(target_os = "macos")]
pub fn verify_content_manifest(artifact: &Path, bundle: &Path) -> Result<(), String> {
    let stage = artifact.parent().ok_or_else(|| "staged artifact has no directory".to_owned())?;
    let expected: ContentManifest = serde_json::from_slice(&fs::read(stage.join("content.json"))
        .map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let mut actual = std::collections::BTreeMap::new();
    hash_tree(bundle, bundle, &mut actual)?;
    if actual == expected.files { Ok(()) }
    else { Err("copied application bundle failed content verification".into()) }
}

pub fn find_ready() -> Result<Option<(PathBuf, Ready, String)>, String> {
    let Ok(entries) = fs::read_dir(root()) else { return Ok(None); };
    for entry in entries.flatten() {
        if !entry.file_name().to_string_lossy().starts_with("stage-") { continue; }
        let stage = entry.path();
        let ready: Ready = match fs::read(stage.join("ready")).ok()
            .and_then(|b| serde_json::from_slice(&b).ok()) { Some(v) => v, None => continue };
        let artifact = stage.join(&ready.artifact);
        if !artifact.is_file() { continue; }
        let metadata: Metadata = match fs::read(stage.join("metadata.json")).ok()
            .and_then(|b| serde_json::from_slice(&b).ok()) { Some(v) => v, None => continue };
        return Ok(Some((artifact, ready, metadata.notes_url)));
    }
    Ok(None)
}

fn reconcile(root: &Path) -> Result<(), String> {
    let Ok(entries) = fs::read_dir(root) else { return Ok(()); };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("stage-") && !entry.path().join("ready").is_file() {
            fs::remove_dir_all(entry.path()).map_err(|e| e.to_string())?;
        } else if name.starts_with("tmp-") && owner_dead(&name) {
            fs::remove_dir_all(entry.path()).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn gc_except(keep: &Path) -> Result<(), String> {
    for entry in fs::read_dir(root()).map_err(|e| e.to_string())?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.path() != keep && (name.starts_with("stage-") || name.starts_with("tmp-") && owner_dead(&name)) {
            fs::remove_dir_all(entry.path()).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn owner_dead(name: &str) -> bool {
    let pid = name.strip_prefix("tmp-").and_then(|s| s.split('-').next())
        .and_then(|s| s.parse::<u32>().ok());
    #[cfg(target_os = "linux")]
    { pid.is_none_or(|p| !Path::new("/proc").join(p.to_string()).exists()) }
    #[cfg(not(target_os = "linux"))]
    { let _ = pid; false }
}

fn durable_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let tmp = path.with_extension("new");
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    let mut file = File::create(&tmp).map_err(|e| e.to_string())?;
    file.write_all(&bytes).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    fs::rename(tmp, path).map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() { sync_dir(parent)?; }
    Ok(())
}

fn sync_dir(path: &Path) -> Result<(), String> {
    File::open(path).and_then(|f| f.sync_all()).map_err(|e| e.to_string())
}

pub fn verify_ready(path: &Path, ready: &Ready) -> Result<(), String> {
    let (hash, _) = super::sha256::reader(File::open(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if hash.eq_ignore_ascii_case(&ready.sha256) { Ok(()) }
    else { Err("staged artifact no longer matches its ready marker".into()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    fn temp(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("strop-update-{}-{}-{tag}",
            std::process::id(), NONCE.fetch_add(1, Ordering::Relaxed)))
    }

    #[test]
    fn installation_lock_has_one_winner_in_a_two_thread_race() {
        let root = temp("race");
        let barrier = Arc::new(Barrier::new(3));
        let winners = Arc::new(AtomicU64::new(0));
        let attempted = Arc::new(AtomicU64::new(0));
        let mut threads = Vec::new();
        for _ in 0..2 {
            let root = root.clone(); let barrier = barrier.clone();
            let winners = winners.clone(); let attempted = attempted.clone();
            threads.push(std::thread::spawn(move || {
                barrier.wait();
                let lock = try_lock_at(&root).unwrap();
                attempted.fetch_add(1, Ordering::Release);
                if let Some(_lock) = lock {
                    winners.fetch_add(1, Ordering::Relaxed);
                    while attempted.load(Ordering::Acquire) != 2 { std::thread::yield_now(); }
                }
            }));
        }
        barrier.wait();
        for thread in threads { thread.join().unwrap(); }
        assert_eq!(winners.load(Ordering::Relaxed), 1);
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn reconcile_removes_dead_pid_tmp_and_markerless_stage_only() {
        let root = temp("gc");
        fs::create_dir_all(root.join("tmp-4294967295-dead")).unwrap();
        fs::create_dir_all(root.join(format!("tmp-{}-live", std::process::id()))).unwrap();
        fs::create_dir_all(root.join("stage-0.3.0")).unwrap();
        fs::create_dir_all(root.join("stage-0.3.1")).unwrap();
        fs::write(root.join("stage-0.3.1/ready"), b"{}").unwrap();
        reconcile(&root).unwrap();
        assert!(!root.join("tmp-4294967295-dead").exists());
        assert!(root.join(format!("tmp-{}-live", std::process::id())).exists());
        assert!(!root.join("stage-0.3.0").exists());
        assert!(root.join("stage-0.3.1").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ready_marker_reverification_detects_fault_injection() {
        let root = temp("ready"); fs::create_dir_all(&root).unwrap();
        let artifact = root.join("strop.exe"); fs::write(&artifact, b"abc").unwrap();
        let ready = Ready { version: "0.3.1".into(),
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into(),
            artifact: "strop.exe".into() };
        assert!(verify_ready(&artifact, &ready).is_ok());
        fs::write(&artifact, b"corrupt").unwrap();
        assert!(verify_ready(&artifact, &ready).is_err());
        let _ = fs::remove_dir_all(root);
    }
}
