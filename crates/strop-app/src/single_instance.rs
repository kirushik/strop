//! One writer per document. Strop is "one document, one window, one process"
//! (editor.rs) and the durable `.strop` is a Loro store with no multi-process
//! merge — so opening a file that another live instance already holds must NOT
//! spawn a second writer. The first instance to open a file binds a per-file
//! local socket and becomes its PRIMARY; a later instance that finds a *live*
//! primary asks it to surface and exits before it ever touches the store.
//!
//! The rendezvous is a [`interprocess`] local socket: a Unix-domain socket on
//! Unix, a named pipe on Windows — one cross-platform API, one code path. The
//! socket name is derived from the document's canonical path, so every instance
//! of the same document agrees and distinct documents never collide.
//!
//! Liveness is the socket itself, never a lock file: connecting succeeds only
//! while a primary is alive to accept. On Unix a crashed primary can leave a
//! stale socket *file* behind, which the next launch unlinks before rebinding;
//! on Windows the named pipe simply ceases to exist when its server process
//! dies, so there is nothing to clean up. Either way a dead instance can never
//! lock you out of your own document — the one failure mode a naive lock would
//! introduce. The cost is a microscopic simultaneous-launch race (both fail to
//! connect, both try to bind): the loser of the bind reconnects and hands off;
//! only a truly concurrent bind could open two windows, which is exactly
//! today's behaviour, so never a regression.

use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use interprocess::local_socket::{Listener, ListenerOptions, Stream, prelude::*};

/// The hand-off message a secondary sends a live primary. Content is
/// irrelevant — receiving *anything* means "another instance wants this file
/// surfaced"; the byte is just so the primary's read returns.
const RAISE: &[u8] = b"raise\n";

/// Result of trying to claim a document for this process.
pub enum Claim {
    /// We are the sole owner. Hold the guard for the process lifetime; its
    /// accept loop records later hand-off requests for `pull_raise`.
    Primary(InstanceGuard),
    /// A live primary already owns this file and has been asked to surface.
    /// The caller should exit without opening a second window or store.
    AlreadyOpen,
}

/// The rendezvous socket name for a document. Keyed by the file's *canonical*
/// path so every instance of the same document agrees and distinct documents
/// never collide; hashed so the name stays well under both the ~108-byte
/// `sun_path` limit on Unix and the named-pipe name limit on Windows.
/// (`DefaultHasher` is deterministic for a given std build, and both instances
/// are the same binary, so they always agree.) On Windows this path is just a
/// source for the pipe name; no file is created there.
pub fn socket_path(file: &Path) -> PathBuf {
    let canon = std::fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    canon.hash(&mut hasher);
    let id = hasher.finish();
    crate::paths::runtime_dir().join(format!("strop-{id:016x}.sock"))
}

/// The interprocess name for that rendezvous socket. Windows named pipes live
/// in their own namespace (a bare name becomes `\\.\pipe\<name>`) and reject a
/// filesystem path outright ("not a named pipe path"), so there we use just
/// the unique file-name component as the pipe name; Unix uses the socket's
/// filesystem path directly.
#[cfg(windows)]
fn socket_name(socket: &Path) -> std::io::Result<interprocess::local_socket::Name<'_>> {
    use interprocess::local_socket::GenericNamespaced;
    let name = socket.file_name().and_then(|n| n.to_str()).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "bad rendezvous socket name")
    })?;
    name.to_ns_name::<GenericNamespaced>()
}

#[cfg(not(windows))]
fn socket_name(socket: &Path) -> std::io::Result<interprocess::local_socket::Name<'_>> {
    use interprocess::local_socket::GenericFilePath;
    socket.to_fs_name::<GenericFilePath>()
}

/// Try to claim `file` for this process. Connects first: a live primary
/// answers and is sent a raise (-> `AlreadyOpen`); otherwise any stale socket
/// is cleared and we bind it ourselves (-> `Primary`).
pub fn claim(file: &Path) -> std::io::Result<Claim> {
    let socket = socket_path(file);
    if connect_and_raise(&socket) {
        return Ok(Claim::AlreadyOpen);
    }
    // No live primary answered. On Unix a crashed holder may have left a stale
    // socket file and the runtime dir may not exist yet; clear and prepare it.
    // Windows named pipes leave nothing behind, so neither step applies.
    #[cfg(unix)]
    {
        let _ = std::fs::remove_file(&socket);
        if let Some(dir) = socket.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
    }
    let listener = match ListenerOptions::new()
        .name(socket_name(&socket)?)
        .create_sync()
    {
        Ok(listener) => listener,
        Err(e) => {
            // Lost a bind race to another just-launched instance: it is now
            // the live primary, so hand off to it instead of failing.
            if connect_and_raise(&socket) {
                return Ok(Claim::AlreadyOpen);
            }
            return Err(e);
        }
    };
    let raise = Arc::new(AtomicBool::new(false));
    let stop = Arc::new(AtomicBool::new(false));
    {
        let raise = raise.clone();
        let stop = stop.clone();
        // Detached: a blocked accept dies with the process; Drop wakes it via a
        // self-connect for the rare guard-outlives-process-exit case.
        let _ = std::thread::Builder::new()
            .name("strop-single-instance".into())
            .spawn(move || accept_loop(listener, raise, stop));
    }
    Ok(Claim::Primary(InstanceGuard { socket, raise, stop }))
}

/// Connect to a primary and ask it to surface. `true` iff something live
/// accepted the connection.
fn connect_and_raise(socket: &Path) -> bool {
    let Ok(name) = socket_name(socket) else {
        return false;
    };
    match Stream::connect(name) {
        Ok(mut stream) => {
            let _ = stream.write_all(RAISE);
            let _ = stream.flush();
            true
        }
        Err(_) => false,
    }
}

fn accept_loop(listener: Listener, raise: Arc<AtomicBool>, stop: Arc<AtomicBool>) {
    for stream in listener.incoming() {
        if stop.load(Ordering::Acquire) {
            break;
        }
        match stream {
            Ok(mut stream) => {
                let mut buf = [0u8; 8];
                let _ = stream.read(&mut buf);
                if stop.load(Ordering::Acquire) {
                    break;
                }
                raise.store(true, Ordering::Release);
            }
            Err(_) => break,
        }
    }
}

/// Owns the bound rendezvous socket for the process lifetime. Dropping it
/// stops the accept loop and (on Unix) unlinks the socket so the file is
/// immediately claimable again.
pub struct InstanceGuard {
    socket: PathBuf,
    raise: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
}

impl InstanceGuard {
    /// True — and resets — if a later instance asked us to surface since the
    /// last poll. The GPUI foreground drains this on a timer and activates the
    /// window.
    pub fn pull_raise(&self) -> bool {
        self.raise.swap(false, Ordering::AcqRel)
    }
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // Wake the accept loop so it observes `stop` and exits; on Unix also
        // unlink the socket file (the named pipe on Windows is released when
        // the listener thread drops it).
        let _ = connect_and_raise(&self.socket);
        #[cfg(unix)]
        let _ = std::fs::remove_file(&self.socket);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;
    use std::time::Duration;

    static N: AtomicU64 = AtomicU64::new(0);

    fn temp_doc(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "strop-si-test-{}-{}-{tag}.strop",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn wait_raise(guard: &InstanceGuard) -> bool {
        for _ in 0..200 {
            if guard.pull_raise() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        false
    }

    /// Claim with a bounded retry: after a guard drops, the accept loop releases
    /// the socket asynchronously (it must observe `stop` and drop the listener),
    /// so re-claiming may race the teardown for a few milliseconds. On Unix the
    /// Drop unlink makes the first attempt succeed; this keeps Windows honest.
    fn claim_within(doc: &Path) -> Option<InstanceGuard> {
        for _ in 0..200 {
            if let Ok(Claim::Primary(g)) = claim(doc) {
                return Some(g);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        None
    }

    #[test]
    fn socket_path_is_stable_per_file_and_unique_across_files() {
        let a = temp_doc("a");
        let b = temp_doc("b");
        assert_eq!(socket_path(&a), socket_path(&a), "same file -> same socket");
        assert_ne!(socket_path(&a), socket_path(&b), "distinct files -> distinct sockets");
    }

    #[test]
    fn second_claim_detects_primary_and_signals_raise() {
        let doc = temp_doc("dup");
        let guard = match claim(&doc).expect("first claim") {
            Claim::Primary(g) => g,
            Claim::AlreadyOpen => panic!("the first claim must be the primary"),
        };
        match claim(&doc).expect("second claim") {
            Claim::AlreadyOpen => {}
            Claim::Primary(_) => panic!("a second claim must detect the live primary"),
        }
        assert!(wait_raise(&guard), "the primary must receive the raise request");
        drop(guard);
    }

    /// Unix-specific: a crashed primary leaves a dead socket *file* behind. The
    /// next launch must take over, never refuse — the whole point of
    /// liveness-by-socket. (Windows named pipes leave no file, so there is no
    /// stale-file case to plant.)
    #[cfg(unix)]
    #[test]
    fn stale_socket_never_locks_the_file() {
        let doc = temp_doc("stale");
        let sock = socket_path(&doc);
        if let Some(dir) = sock.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        std::fs::write(&sock, b"dead leftover").expect("plant a stale socket file");
        match claim(&doc).expect("claim over stale socket") {
            Claim::Primary(_guard) => {}
            Claim::AlreadyOpen => panic!("a dead socket must never lock the file"),
        }
    }

    #[test]
    fn dropping_primary_frees_the_file() {
        let doc = temp_doc("free");
        #[cfg(unix)]
        let sock = socket_path(&doc);
        {
            let _guard = match claim(&doc).expect("claim") {
                Claim::Primary(g) => g,
                Claim::AlreadyOpen => panic!("first claim must be primary"),
            };
            #[cfg(unix)]
            assert!(sock.exists(), "primary binds the socket");
        }
        #[cfg(unix)]
        assert!(!sock.exists(), "guard drop unlinks the socket");
        assert!(
            claim_within(&doc).is_some(),
            "the file must be claimable again once the holder exits"
        );
    }
}
