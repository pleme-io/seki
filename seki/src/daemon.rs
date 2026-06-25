//! seki refresh daemon — the FS-event-driven hot-status cache.
//!
//! Staleness is the enemy: a prompt that shows yesterday's git state is
//! worse than no segment at all. The cheapest way to be *never stale* is
//! to recompute live on every render (which `seki prompt` already does),
//! but that forks `git` each time. The daemon makes the same freshness
//! **efficient, the way mado stays cheap by being event-driven rather
//! than polling**: it watches each repo with the OS filesystem-event
//! primitive (`notify` — kernel FSEvents/inotify, the same primitive
//! `shikumi` hot-reload rides) and keeps that repo's status hot.
//!
//! ## The freshness invariant (enforced, not hoped)
//!
//! The event stream is treated as an **optimization hint, never the source
//! of truth** — because it isn't reliable: FSEvents coalesces and drops
//! events under churn (a `git clean` mid-`cargo build`), signalling only a
//! "you must rescan" flag, and a watcher can error. An earlier design
//! *assumed* "every change fired an event that recomputed before the next
//! read" and so trusted the cache blindly; a dropped event then stranded a
//! stale untracked count forever (the phantom `?` on a clean tree).
//!
//! Instead each watched repo carries a monotonic **generation**:
//! - any FS event for a repo bumps its generation;
//! - a rescan/overflow flag (`Event::need_rescan`) or a watcher error —
//!   which can't be attributed to a path — bumps **every** watched repo's
//!   generation (FSEvents' contract is "coalesce/drop ⇒ rescan", never
//!   "silently lose", so honoring rescan is what closes the hole);
//! - the cache stores, with each status, the generation it was computed at.
//!
//! A query serves the cached status **only if its stored generation still
//! equals the repo's current generation** — i.e. nothing has changed since.
//! Otherwise it recomputes live *before answering*. So the wire value is, by
//! construction, exactly what a live `git status` would return at query
//! time: a value that disagrees with ground truth has no path to the prompt.
//! The hot path (unchanged repo) is still a microsecond cache read; only a
//! genuinely-changed repo pays a fork, exactly when it must.
//!
//! Design guarantees:
//! - **Never stale (enforced).** Generation-validated reads; a changed repo
//!   recomputes on the query that observes it. A cache miss (no daemon)
//!   falls back to a live fork, equally fresh.
//! - **Singleton.** A second daemon that finds the socket already
//!   answering exits immediately.
//! - **On-demand.** A repo is watched the first time it's queried, never
//!   speculatively.
//! - **Coalesced.** A single git operation's event storm collapses into
//!   one generation bump + pre-warm per repo (a bounded window), so the
//!   daemon is quiet.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use notify::{RecursiveMode, Watcher};
use seki_modules::git_status::{self, GitStatus};

/// A cached status plus the repo generation it was computed at. The pair is
/// the freshness proof: the value is current iff `gen` still equals the
/// repo's live generation.
type Cache = Arc<Mutex<HashMap<PathBuf, (GitStatus, u64)>>>;
type Watched = Arc<Mutex<HashSet<PathBuf>>>;
/// Per-repo monotonic change generation. Bumped on every FS event for the
/// repo, and on every watched repo on a rescan/overflow/error. The single
/// source of truth for "has this repo changed since we last computed it".
type Generations = Arc<Mutex<HashMap<PathBuf, u64>>>;
type WatcherHandle = Arc<Mutex<notify::RecommendedWatcher>>;

/// Read a repo's current generation (0 if never seen).
fn generation_of(generations: &Generations, root: &Path) -> u64 {
    *generations.lock().unwrap().get(root).unwrap_or(&0)
}

/// Run the daemon. Binds the socket, watches repos on demand, keeps their
/// status hot via FS events, serves generation-validated reads. Blocks until
/// killed. Returns immediately (Ok) if another daemon already owns the socket.
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
    let generations: Generations = Arc::new(Mutex::new(HashMap::new()));

    let (tx, rx) = mpsc::channel();
    let watcher = notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    })?;
    let watcher: WatcherHandle = Arc::new(Mutex::new(watcher));

    spawn_recompute_thread(rx, cache.clone(), watched.clone(), generations.clone());

    // One short-lived thread per connection: a stalled/half-open client can
    // only block its own thread (which times out), never the accept loop —
    // so one stuck peer can't wedge the daemon for the whole session.
    for conn in listener.incoming() {
        let Ok(stream) = conn else { continue };
        let (cache, watched, watcher, generations) = (
            cache.clone(),
            watched.clone(),
            watcher.clone(),
            generations.clone(),
        );
        std::thread::spawn(move || {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
            handle_client(stream, &cache, &watched, &watcher, &generations);
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

/// One coalesced FS-event burst, reduced to the typed facts the classifier
/// needs: which paths changed, whether the event demanded a rescan, and
/// whether it was a watcher error. Decouples the `notify` types from the
/// pure classification so the never-stale logic is unit-testable.
struct EventFacts {
    paths: Vec<PathBuf>,
    need_rescan: bool,
    is_error: bool,
}

/// Pure: given a burst of event facts and the watched roots, decide which
/// roots changed. A rescan/overflow flag or a watcher error cannot be tied
/// to a specific path, so it dirties **every** watched root — the move that
/// turns FSEvents' "coalesce/drop ⇒ rescan" guarantee into a never-stale
/// cache. A plain event dirties only the roots its paths fall under.
fn classify_events(events: &[EventFacts], roots: &[PathBuf]) -> HashSet<PathBuf> {
    let mut affected: HashSet<PathBuf> = HashSet::new();
    let mut rescan_all = false;
    for ev in events {
        if ev.need_rescan || ev.is_error {
            rescan_all = true;
            continue;
        }
        for path in &ev.paths {
            for root in roots {
                if path.starts_with(root) {
                    affected.insert(root.clone());
                }
            }
        }
    }
    if rescan_all {
        affected.extend(roots.iter().cloned());
    }
    affected
}

/// The watcher → channel → recompute loop. Coalesces a burst of events
/// (one git operation = many inotify/FSEvents), bumps the generation of
/// every affected repo, and pre-warms its cache so the next prompt reads a
/// hot, generation-matched value instead of forking. Correctness does not
/// depend on this pre-warm winning the race with a query: the query
/// re-validates the generation and recomputes itself on any mismatch.
fn spawn_recompute_thread(
    rx: mpsc::Receiver<notify::Result<notify::Event>>,
    cache: Cache,
    watched: Watched,
    generations: Generations,
) {
    std::thread::spawn(move || {
        loop {
            let Ok(first) = rx.recv() else { break };
            let mut events = vec![first];
            // Coalesce a burst, but with a HARD cap on the window: a 60ms
            // quiet gap closes it, and so does a 250ms / 256-event ceiling.
            // Without the ceiling, sustained churn (a long `cargo build`
            // hammering target/) would keep the window open forever and the
            // generation bump would never fire — i.e. the status would go
            // stale exactly when it's changing. The cap guarantees a bump at
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
            let facts: Vec<EventFacts> = events
                .into_iter()
                .map(|res| match res {
                    Ok(ev) => EventFacts {
                        need_rescan: ev.need_rescan(),
                        paths: ev.paths,
                        is_error: false,
                    },
                    // A watcher error can't be attributed to a path; treat it
                    // as "rescan everything" rather than silently drop it (the
                    // original `.flatten()` bug that stranded stale state).
                    Err(_) => EventFacts {
                        paths: Vec::new(),
                        need_rescan: false,
                        is_error: true,
                    },
                })
                .collect();
            for root in classify_events(&facts, &roots) {
                // Bump first so any query that races the recompute observes
                // the new generation and recomputes itself; then pre-warm.
                let generation = {
                    let mut g = generations.lock().unwrap();
                    let e = g.entry(root.clone()).or_insert(0);
                    *e += 1;
                    *e
                };
                if let Some(st) = git_status::compute_status(&root) {
                    cache.lock().unwrap().insert(root, (st, generation));
                }
            }
        }
    });
}

/// Answer one request: read the client's cwd, ensure its repo is watched,
/// and write back a **generation-validated** status — the cached value if it
/// was computed at the repo's current generation, else a fresh live compute.
fn handle_client(
    stream: UnixStream,
    cache: &Cache,
    watched: &Watched,
    watcher: &WatcherHandle,
    generations: &Generations,
) {
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
            ensure_watched(&root, cache, watched, watcher, generations);
            let cur = generation_of(generations, &root);
            // Serve the cache ONLY when its stored generation still matches —
            // i.e. nothing has changed since it was computed. A mismatch (a
            // change the pre-warm hasn't caught yet) or a miss recomputes live
            // before answering, so staleness has no path to the wire. A failed
            // compute replies empty — NOT a synthesized all-zero "clean" — so
            // the client's `from_wire` rejects it and forks live itself.
            let hot = {
                let c = cache.lock().unwrap();
                match c.get(&root) {
                    Some((st, g)) if *g == cur => Some(*st),
                    _ => None,
                }
            };
            let fresh = hot.or_else(|| {
                git_status::compute_status(&root).map(|st| {
                    cache.lock().unwrap().insert(root.clone(), (st, cur));
                    st
                })
            });
            match fresh {
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

/// Watch `root` the first time it's seen. Register the FS watch **before**
/// the initial compute so no change in the gap is missed, seed its
/// generation, then compute its status now (so the first answer is correct).
/// If the watch can't be registered we cache nothing — every query then
/// recomputes live, which is correct, just unaccelerated.
fn ensure_watched(
    root: &Path,
    cache: &Cache,
    watched: &Watched,
    watcher: &WatcherHandle,
    generations: &Generations,
) {
    {
        if watched.lock().unwrap().contains(root) {
            return;
        }
    }
    if watcher
        .lock()
        .unwrap()
        .watch(root, RecursiveMode::Recursive)
        .is_err()
    {
        // No watch ⇒ we can't keep it hot ⇒ don't cache (handle_client forks
        // live each query). Better an honest live read than an unwatched,
        // never-invalidated cache entry.
        return;
    }
    watched.lock().unwrap().insert(root.to_path_buf());
    generations
        .lock()
        .unwrap()
        .entry(root.to_path_buf())
        .or_insert(0);
    if let Some(st) = git_status::compute_status(root) {
        let generation = generation_of(generations, root);
        cache.lock().unwrap().insert(root.to_path_buf(), (st, generation));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn root(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    fn plain(paths: &[&str]) -> EventFacts {
        EventFacts {
            paths: paths.iter().map(PathBuf::from).collect(),
            need_rescan: false,
            is_error: false,
        }
    }

    #[test]
    fn plain_event_dirties_only_its_root() {
        let roots = [root("/a"), root("/b")];
        let got = classify_events(&[plain(&["/a/src/lib.rs"])], &roots);
        assert!(got.contains(&root("/a")));
        assert!(!got.contains(&root("/b")));
    }

    #[test]
    fn untracked_delete_under_root_is_caught() {
        // A `git clean` removing /a/scratch.rs is a plain event under /a.
        let roots = [root("/a")];
        let got = classify_events(&[plain(&["/a/scratch.rs"])], &roots);
        assert_eq!(got, HashSet::from([root("/a")]));
    }

    #[test]
    fn rescan_flag_dirties_every_watched_root() {
        // The phantom-`?` bug: FSEvents coalesced/dropped the real events and
        // only raised a rescan flag. That MUST invalidate every repo, not be
        // dropped — else a stale count is stranded forever.
        let roots = [root("/a"), root("/b"), root("/c")];
        let rescan = EventFacts {
            paths: vec![],
            need_rescan: true,
            is_error: false,
        };
        let got = classify_events(&[rescan], &roots);
        assert_eq!(got, roots.iter().cloned().collect());
    }

    #[test]
    fn watcher_error_dirties_every_watched_root() {
        let roots = [root("/a"), root("/b")];
        let err = EventFacts {
            paths: vec![],
            need_rescan: false,
            is_error: true,
        };
        assert_eq!(
            classify_events(&[err], &roots),
            roots.iter().cloned().collect()
        );
    }

    #[test]
    fn event_outside_every_root_dirties_nothing() {
        let roots = [root("/a")];
        assert!(classify_events(&[plain(&["/elsewhere/x"])], &roots).is_empty());
    }

    #[test]
    fn generation_mismatch_is_the_staleness_signal() {
        // Models the wire decision in handle_client: a cached (status, gen)
        // is servable iff gen still equals the live generation.
        let cached_gen = 7u64;
        let servable = |live: u64| cached_gen == live;
        assert!(servable(7), "unchanged repo → serve hot cache");
        assert!(!servable(8), "a change bumped the generation → recompute");
    }
}
