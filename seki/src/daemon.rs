//! seki refresh daemon — the FS-event-driven hot-status cache.
//!
//! Staleness is the enemy: a prompt that shows yesterday's git state is
//! worse than no segment at all. The cheapest way to be *never stale* is
//! to recompute live on every render (which `seki prompt` already does),
//! but that forks `git` each time. The daemon makes the same freshness
//! **efficient, the way mado stays cheap by being event-driven rather
//! than polling**: it watches each repo with the OS filesystem-event
//! primitive (`notify` — kernel FSEvents/inotify, the same primitive
//! `shikumi` hot-reload rides) and keeps that repo's status hot. A prompt
//! then reads the hot value over a unix socket in microseconds, and the
//! value is current because *every* relevant filesystem write — a commit,
//! a `git add`, a bare worktree edit, a `git fetch` moving the upstream —
//! fired an event that already recomputed it.
//!
//! Design guarantees:
//! - **Never stale.** Any change under a watched repo fires an event that
//!   recomputes before the next read. A cache miss (no daemon) falls back
//!   to a live fork, which is equally fresh.
//! - **Singleton.** A second daemon that finds the socket already
//!   answering exits immediately.
//! - **On-demand.** A repo is watched the first time it's queried, never
//!   speculatively.
//! - **Coalesced.** A single git operation's event storm collapses into
//!   one recompute per repo (a 60 ms window), so the daemon is quiet.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use notify::{RecursiveMode, Watcher};
use seki_modules::git_status::{self, GitStatus};

type Cache = Arc<Mutex<HashMap<PathBuf, GitStatus>>>;
type Watched = Arc<Mutex<HashSet<PathBuf>>>;
type WatcherHandle = Arc<Mutex<notify::RecommendedWatcher>>;

/// Run the daemon. Binds the socket, watches repos on demand, keeps their
/// status hot via FS events, serves hot reads. Blocks until killed.
/// Returns immediately (Ok) if another daemon already owns the socket.
pub fn run() -> anyhow::Result<()> {
    let sock = git_status::socket_path();
    if let Some(parent) = sock.parent() {
        std::fs::create_dir_all(parent)?;
        // Lock the socket dir to the owner — the predictable path (esp. the
        // /tmp fallback when XDG_RUNTIME_DIR is unset, as on macOS) must not
        // be squattable by another local user.
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
    }
    // Common singleton case: a live daemon already answers → we're redundant.
    if UnixStream::connect(&sock).is_ok() {
        return Ok(());
    }
    let listener = bind_singleton(&sock)?;
    // Past this point we own the socket.

    let cache: Cache = Arc::new(Mutex::new(HashMap::new()));
    let watched: Watched = Arc::new(Mutex::new(HashSet::new()));

    let (tx, rx) = mpsc::channel();
    let watcher = notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    })?;
    let watcher: WatcherHandle = Arc::new(Mutex::new(watcher));

    spawn_recompute_thread(rx, cache.clone(), watched.clone());

    // One short-lived thread per connection: a stalled/half-open client can
    // only block its own thread (which times out), never the accept loop —
    // so one stuck peer can't wedge the daemon for the whole session.
    for conn in listener.incoming() {
        let Ok(stream) = conn else { continue };
        let (cache, watched, watcher) = (cache.clone(), watched.clone(), watcher.clone());
        std::thread::spawn(move || {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
            handle_client(stream, &cache, &watched, &watcher);
        });
    }
    Ok(())
}

/// Acquire the socket as a singleton without ever deleting a *live*
/// daemon's socket. Bind first; only on `AddrInUse` do we probe — if a
/// daemon answers we are redundant (Ok, exit); if nobody answers the file
/// is a stale leftover from a crash, so remove it and rebind.
fn bind_singleton(sock: &Path) -> anyhow::Result<UnixListener> {
    match UnixListener::bind(sock) {
        Ok(l) => Ok(l),
        Err(e) if e.kind() == ErrorKind::AddrInUse => {
            if UnixStream::connect(sock).is_ok() {
                // A live daemon already owns it — surface as "already running".
                return Err(anyhow::anyhow!("seki daemon already running"));
            }
            let _ = std::fs::remove_file(sock);
            Ok(UnixListener::bind(sock)?)
        }
        Err(e) => Err(e.into()),
    }
}

/// The watcher → channel → recompute loop. Coalesces a burst of events
/// (one git operation = many inotify/FSEvents) into a single recompute
/// per affected repo root, so the daemon never busy-loops on churn.
fn spawn_recompute_thread(
    rx: mpsc::Receiver<notify::Result<notify::Event>>,
    cache: Cache,
    watched: Watched,
) {
    std::thread::spawn(move || {
        loop {
            let Ok(first) = rx.recv() else { break };
            let mut events = vec![first];
            // Coalesce a burst, but with a HARD cap on the window: a 60ms
            // quiet gap closes it, and so does a 250ms / 256-event ceiling.
            // Without the ceiling, sustained churn (a long `cargo build`
            // hammering target/) would keep the window open forever and the
            // recompute would never fire — i.e. the status would go stale
            // exactly when it's changing. The cap guarantees a recompute at
            // least ~4×/s under any load.
            let deadline = Instant::now() + Duration::from_millis(250);
            while events.len() < 256 {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    break;
                }
                let wait = remaining.min(Duration::from_millis(60));
                match rx.recv_timeout(wait) {
                    Ok(more) => events.push(more),
                    Err(_) => break,
                }
            }
            let roots: Vec<PathBuf> = watched.lock().unwrap().iter().cloned().collect();
            let mut affected: HashSet<PathBuf> = HashSet::new();
            for ev in events.into_iter().flatten() {
                for path in ev.paths {
                    for root in &roots {
                        if path.starts_with(root) {
                            affected.insert(root.clone());
                        }
                    }
                }
            }
            for root in affected {
                if let Some(st) = git_status::compute_status(&root) {
                    cache.lock().unwrap().insert(root, st);
                }
            }
        }
    });
}

/// Answer one request: read the client's cwd, ensure its repo is watched
/// and hot, write back the wire-encoded status.
fn handle_client(stream: UnixStream, cache: &Cache, watched: &Watched, watcher: &WatcherHandle) {
    let Ok(reader_stream) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(reader_stream);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let cwd = PathBuf::from(line.trim());
    let response = match git_status::repo_root(&cwd) {
        Some(root) => {
            ensure_watched(&root, cache, watched, watcher);
            // A cache MISS means we never successfully computed this repo
            // (the first compute timed out / errored). Reply empty — NOT a
            // synthesized all-zero "clean" — so the client's `from_wire`
            // rejects it and falls back to a live fork. Turning a miss into
            // a false "clean" is exactly the staleness this daemon forbids.
            match cache.lock().unwrap().get(&root).copied() {
                Some(st) => {
                    let mut wire = st.to_wire();
                    wire.push('\n');
                    wire
                }
                None => String::new(),
            }
        }
        None => String::new(),
    };
    let mut sink = &stream;
    let _ = sink.write_all(response.as_bytes());
}

/// Watch `root` the first time it's seen: compute its status now (so the
/// first answer is correct) and register a recursive FS watch so future
/// answers stay hot.
fn ensure_watched(root: &Path, cache: &Cache, watched: &Watched, watcher: &WatcherHandle) {
    {
        if watched.lock().unwrap().contains(root) {
            return;
        }
    }
    if let Some(st) = git_status::compute_status(root) {
        cache.lock().unwrap().insert(root.to_path_buf(), st);
    }
    if watcher
        .lock()
        .unwrap()
        .watch(root, RecursiveMode::Recursive)
        .is_ok()
    {
        watched.lock().unwrap().insert(root.to_path_buf());
    }
}

/// If `SEKI_DAEMON=auto` and no daemon is listening, spawn one detached.
/// Called after a prompt renders, so the *first* prompt in a session pays
/// the live-fork cost and every subsequent one reads the hot cache. Safe
/// to call repeatedly — the daemon's singleton guard collapses races.
pub fn maybe_autostart() {
    if std::env::var("SEKI_DAEMON").ok().as_deref() != Some("auto") {
        return;
    }
    if UnixStream::connect(git_status::socket_path()).is_ok() {
        return; // already running
    }
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe)
            .arg("daemon")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
}
