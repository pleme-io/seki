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
//! ## The supersession invariant (the second way to be stale)
//!
//! Generation validation makes the *data* never stale. It says nothing about
//! the **code**, and that turned out to be the wider hole. This daemon is
//! long-lived and autostarted on demand; [`maybe_autostart`] spawns one only
//! when nothing is listening. So a daemon started before a rebuild keeps
//! answering forever from the superseded binary, and a fix that ships to disk
//! never reaches a single prompt — no restart, no supervisor, no signal.
//!
//! That is not hypothetical: on 2026-07-31 the three commits fixing the
//! phantom-`?` event-drop landed, the system was rebuilt, and the prompt was
//! still wrong — because the process answering it had been running since
//! before the fix. Both binaries were `rust_seki-0.1.14`; only the store path
//! differed, so nothing version-shaped could have noticed.
//!
//! The seam where old-meets-new is the request, so the check lives there:
//! every client sends its own [`git_status::build_id`] (its exe path, i.e.
//! its store path), and a daemon whose own id differs **exits** rather than
//! answer. The client falls back to a live read for that one render and its
//! `maybe_autostart` then spawns the current binary. Self-healing, with no
//! supervisor: the first prompt after a rebuild retires the old daemon, the
//! second starts the new one, the third is hot again.
//!
//! ## What the generation actually proves (and what it assumes)
//!
//! Generation validation proves *no event said this repo changed*. It proves
//! the repo did not change only if the event stream never loses a change
//! silently — and that premise is unfalsifiable from inside: a change nobody
//! was told about looks exactly like no change. When it fails, the cache is
//! confidently and permanently wrong.
//!
//! It did fail. On 2026-07-31 a daemon carrying both the rescan fix and the
//! watcher-rebuild fix served a phantom untracked `?` on a clean tree for
//! hours. So the premise is not something to reason harder about; it is
//! something to stop depending on. [`REVALIDATE_AFTER`] caps how long any
//! generation-current value may be believed, which bounds staleness by
//! construction no matter how the OS behaves.
//!
//! Design guarantees:
//! - **Bounded stale (enforced, no premise).** Generation-validated reads,
//!   *plus* an age ceiling: a value is recomputed once it exceeds
//!   [`REVALIDATE_AFTER`] even if every event said nothing changed. A cache
//!   miss (no daemon) falls back to a live fork, equally fresh.
//! - **Never superseded (enforced).** A daemon running a build other than the
//!   caller's retires instead of answering, so a shipped fix cannot sit unused
//!   behind a live old process.
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

/// A cached status, the repo generation it was computed at, and when. The
/// generation proves no event says it changed; the [`Instant`] bounds how long
/// we are willing to believe that (see [`REVALIDATE_AFTER`]).
type Cache = Arc<Mutex<HashMap<PathBuf, (GitStatus, u64, Instant)>>>;

/// How long a generation-current value may be served before it is recomputed
/// anyway.
///
/// ★ Generation validation is only as good as its premise: that the event
/// stream never loses a change *silently* — that a coalesced or dropped event
/// always arrives as `need_rescan`. Every mechanism here rests on that, and
/// the mechanism cannot check it: a change nobody was told about is
/// indistinguishable from no change at all. So the cache stays confidently,
/// permanently wrong, which is the worst failure shape available.
///
/// On 2026-07-31 a daemon built *with* the rescan and watcher-rebuild fixes
/// still served a phantom untracked `?` on a clean tree for hours. Whatever
/// dropped that event did not announce itself, so no amount of event-path
/// reasoning would have caught it.
///
/// The fix is to stop needing the premise. An age bound makes staleness
/// **bounded by construction** rather than by the OS honoring a contract:
/// however the event stream misbehaves, a wrong value cannot outlive this
/// window. It costs one `git status` fork on the first prompt after a quiet
/// period — exactly what a daemonless prompt pays anyway, so the floor is the
/// status quo — while a burst of prompts (when the cache actually earns its
/// keep) still reads hot.
const REVALIDATE_AFTER: Duration = Duration::from_secs(10);

/// Whether a cached entry may be served: nothing has changed since it was
/// computed **and** we have not been believing that for too long.
///
/// Split out as a pure function because it is the whole correctness decision,
/// and the age half of it exists precisely because the generation half can be
/// silently wrong.
fn is_servable(cached_gen: u64, live_gen: u64, age: Duration) -> bool {
    cached_gen == live_gen && age < REVALIDATE_AFTER
}
type Watched = Arc<Mutex<WatchSet>>;

/// How many repos may be watched at once.
///
/// ★ This bound is the point, not the number. A recursive watch is not free
/// and its cost is invisible: it is paid in kernel objects, not in anything
/// the daemon can see about itself. Before this cap the watch set was an
/// unbounded `HashSet` — every repo the operator ever `cd`-ed into was
/// watched for the daemon's whole lifetime, and nothing ever released one.
///
/// That went unnoticed for a long time because the fleet raised its
/// per-process FD ceiling to 2^24 on 2026-06-30 to stop an unrelated
/// exhaustion. A ceiling that high means a runaway watcher never fails — on
/// 2026-07-31 this daemon was found holding 26,517 open file descriptors and
/// no limit had complained. Raising a ceiling removes the symptom AND the
/// signal; a budget is what puts the signal back.
///
/// 24 is chosen to sit far above real use (an operator hops between a handful
/// of repos) and far below anything that costs real resources, so eviction is
/// effectively unreachable in normal work and strictly bounded in pathological
/// ones. Eviction is not a failure mode: an unwatched repo simply recomputes
/// live on query, which `handle_client` already does correctly.
const MAX_WATCHED_ROOTS: usize = 24;

/// The set of watched repo roots, bounded and least-recently-used ordered.
///
/// `order` is most-recently-used first. Membership is O(n) over a set capped
/// at [`MAX_WATCHED_ROOTS`], so a linear scan is cheaper than the HashMap it
/// would take to avoid it.
struct WatchSet {
    order: Vec<PathBuf>,
    cap: usize,
}

impl WatchSet {
    fn new(cap: usize) -> Self {
        Self {
            order: Vec::new(),
            cap,
        }
    }

    fn contains(&self, root: &Path) -> bool {
        self.order.iter().any(|p| p == root)
    }

    /// Mark `root` as most-recently-used. No-op if not present.
    fn touch(&mut self, root: &Path) {
        if let Some(i) = self.order.iter().position(|p| p == root) {
            let p = self.order.remove(i);
            self.order.insert(0, p);
        }
    }

    /// Insert `root` as most-recently-used, returning the root evicted to
    /// stay within `cap` (if any). The caller MUST unwatch whatever is
    /// returned — this type owns the bound, not the FS registration.
    fn insert(&mut self, root: PathBuf) -> Option<PathBuf> {
        self.touch(&root);
        if self.contains(&root) {
            return None;
        }
        self.order.insert(0, root);
        if self.order.len() > self.cap {
            return self.order.pop();
        }
        None
    }

    fn roots(&self) -> Vec<PathBuf> {
        self.order.clone()
    }

    fn len(&self) -> usize {
        self.order.len()
    }
}
/// Per-repo monotonic change generation. Bumped on every FS event for the
/// repo, and on every watched repo on a rescan/overflow/error. The single
/// source of truth for "has this repo changed since we last computed it".
type Generations = Arc<Mutex<HashMap<PathBuf, u64>>>;
type WatcherHandle = Arc<Mutex<notify::RecommendedWatcher>>;

/// Read a repo's current generation (0 if never seen).
fn generation_of(generations: &Generations, root: &Path) -> u64 {
    *generations.lock().unwrap().get(root).unwrap_or(&0)
}

/// Bump every watched root's generation, invalidating every cached status.
///
/// The same move `classify_events` makes for a rescan/overflow flag, applied
/// to the one other situation that can drop events without announcing it: a
/// watcher rebuild. See the call site in [`ensure_watched`].
fn invalidate_all(watched: &Watched, generations: &Generations) {
    let roots = watched.lock().unwrap().roots();
    let mut g = generations.lock().unwrap();
    for root in roots {
        *g.entry(root).or_insert(0) += 1;
    }
}

/// Paths whose changes can never alter `git status --porcelain` output, and
/// so can be dropped before they cost a recompute.
///
/// ONLY `.git/objects` and `.git/lfs` qualify, and the bar for adding to this
/// list is that the exclusion be *output-preserving by construction*, not
/// merely usually-harmless. Object writes cannot change status on their own —
/// what changes it is the index or ref write that accompanies them, and both
/// land outside `objects/`. A `git gc` or a fetch's packfile is pure noise.
///
/// Deliberately NOT here: `target/`, `node_modules/`, and friends. They are
/// the actual event-storm sources, but excluding them is only safe when git
/// genuinely ignores them in that repo — a repo that tracks a directory named
/// `target` would silently stop updating. Getting that right needs a per-root
/// `git check-ignore` verification; until that exists, a missed event is a
/// silently stale prompt, which is worse than the wasted work it saves.
fn is_output_irrelevant(path: &Path, root: &Path) -> bool {
    let Ok(rel) = path.strip_prefix(root) else {
        return false;
    };
    let mut comps = rel.components();
    if comps.next().map(|c| c.as_os_str()) != Some(std::ffi::OsStr::new(".git")) {
        return false;
    }
    // Strictly BELOW .git/objects — never the directory node itself.
    matches!(
        comps.next().map(|c| c.as_os_str().to_string_lossy().into_owned()).as_deref(),
        Some("objects" | "lfs")
    ) && comps.next().is_some()
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
    let watched: Watched = Arc::new(Mutex::new(WatchSet::new(MAX_WATCHED_ROOTS)));
    let generations: Generations = Arc::new(Mutex::new(HashMap::new()));
    // Read once: this is what every client's build-id is compared against.
    let own_build = git_status::build_id();

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
        let (cache, watched, watcher, generations, own_build) = (
            cache.clone(),
            watched.clone(),
            watcher.clone(),
            generations.clone(),
            own_build.clone(),
        );
        std::thread::spawn(move || {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
            handle_client(
                stream,
                &cache,
                &watched,
                &watcher,
                &generations,
                own_build.as_deref(),
            );
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
                if path.starts_with(root) && !is_output_irrelevant(path, root) {
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
            let roots: Vec<PathBuf> = watched.lock().unwrap().roots();
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
                    cache
                        .lock()
                        .unwrap()
                        .insert(root, (st, generation, Instant::now()));
                }
            }
        }
    });
}

/// Answer one request: read the client's cwd, ensure its repo is watched,
/// and write back a **generation-validated** status — the cached value if it
/// was computed at the repo's current generation, else a fresh live compute.
///
/// Before any of that, honor the build handshake: a client from a *different*
/// build than ours means a rebuild has superseded this process, so we retire
/// (see [`run`]'s doc on the supersession hole).
fn handle_client(
    stream: UnixStream,
    cache: &Cache,
    watched: &Watched,
    watcher: &WatcherHandle,
    generations: &Generations,
    own_build: Option<&str>,
) {
    let Ok(reader_stream) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(reader_stream);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let (client_build, cwd_field) = git_status::parse_request(line.trim());
    if git_status::is_superseded(client_build, own_build) {
        // A newer seki is on disk and this process is running the old code.
        // Answer nothing (the client recomputes live, so this render is still
        // correct) and exit, releasing the socket — the client's
        // `maybe_autostart` then finds no listener and spawns the new binary.
        // Exiting is the whole point: staying alive is what let a fixed build
        // sit unused on disk while the old one kept answering.
        let _ = stream.shutdown(std::net::Shutdown::Both);
        std::process::exit(0);
    }
    let cwd = PathBuf::from(cwd_field);
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
                    Some((st, g, at)) if is_servable(*g, cur, at.elapsed()) => Some(*st),
                    _ => None,
                }
            };
            let fresh = hot.or_else(|| {
                git_status::compute_status(&root).map(|st| {
                    cache
                        .lock()
                        .unwrap()
                        .insert(root.clone(), (st, cur, Instant::now()));
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
        // Already watched — just refresh its recency so an active repo is
        // never the one evicted.
        let mut w = watched.lock().unwrap();
        if w.contains(root) {
            w.touch(root);
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
    // Admit under the bound. Anything displaced is released completely —
    // FS registration, cached status, and generation — so an evicted repo
    // leaves nothing behind that could later be served stale.
    let evicted = watched.lock().unwrap().insert(root.to_path_buf());
    if let Some(old) = evicted {
        let _ = watcher.lock().unwrap().unwatch(&old);
        cache.lock().unwrap().remove(&old);
        generations.lock().unwrap().remove(&old);
    }
    // ★ Every watch()/unwatch() BLANKS the event stream for every root.
    //
    // notify's FSEvents backend implements both as stop() → mutate the path
    // array → run(), and run() recreates the stream with `since_when`, which
    // is fixed at `kFSEventStreamEventIdSinceNow` when the watcher is built
    // and never advanced (notify 8.2.0 `src/fsevent.rs:299,440`). So the
    // rebuild resumes from "now" and anything that changed during the gap is
    // gone — delivered to nobody, with NO `need_rescan` flag to announce it.
    // Nothing in the event path can detect this; the cache would simply keep
    // serving a value whose invalidating event was never delivered.
    //
    // The window is small but the failure is silent and sticky: a repo whose
    // only change landed in that gap stays wrong until something else touches
    // it. So treat a stream rebuild exactly like the rescan/overflow case —
    // bump EVERY watched root, forcing the next query on each to recompute
    // live. One recompute per watched repo, only when the watch set actually
    // changes, in exchange for removing the silent-miss class entirely.
    invalidate_all(watched, generations);
    generations
        .lock()
        .unwrap()
        .entry(root.to_path_buf())
        .or_insert(0);
    if let Some(st) = git_status::compute_status(root) {
        let generation = generation_of(generations, root);
        cache
            .lock()
            .unwrap()
            .insert(root.to_path_buf(), (st, generation, Instant::now()));
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
        // A change the event stream DID report bumps the generation.
        let fresh = Duration::from_millis(0);
        assert!(is_servable(7, 7, fresh), "unchanged repo → serve hot cache");
        assert!(!is_servable(7, 8, fresh), "reported change → recompute");
    }

    /// ★ The invariant for the failure the generation cannot see.
    ///
    /// A silently-dropped event leaves the generation matching forever, so on
    /// the generation alone this entry is servable for the rest of the
    /// process's life — which is exactly how a phantom `?` survived hours on
    /// a clean tree. The age ceiling is what makes that impossible without
    /// having to trust the event stream at all.
    #[test]
    fn a_generation_current_value_still_expires() {
        let agreed = (9u64, 9u64);
        assert!(
            is_servable(agreed.0, agreed.1, REVALIDATE_AFTER - Duration::from_millis(1)),
            "inside the window a matching generation is served — the cache still earns its keep"
        );
        assert!(
            !is_servable(agreed.0, agreed.1, REVALIDATE_AFTER),
            "at the ceiling it MUST recompute even though no event ever reported a change"
        );
        assert!(
            !is_servable(agreed.0, agreed.1, Duration::from_secs(60 * 60)),
            "an hour-old 'unchanged' value is the phantom-? bug; it must never be served"
        );
    }

    /// ★ The invariant that would have prevented 2026-07-31 entirely.
    ///
    /// notify selects its macOS backend as
    /// `#[cfg(all(target_os = "macos", not(feature = "macos_kqueue")))] →
    /// FsEventWatcher`. `macos_kqueue` is a POISON feature: Cargo features are
    /// additive and unify across the whole graph, so ANY crate anywhere in our
    /// dependency tree enabling it silently flips this daemon to the kqueue
    /// backend — and we cannot un-enable it from here. kqueue holds one open
    /// file descriptor per WATCHED PATH; FSEvents holds none.
    ///
    /// That is exactly what happened: shikumi carried the feature (copied from
    /// gen's own fix-hint, which recommended it), seki inherited it
    /// transitively, and this daemon was found holding 26,517 open FDs across
    /// 3,014 paths while every layer looked correct — seki's own Cargo.toml
    /// says plain `notify = "8"`, whose default is the RIGHT backend.
    ///
    /// So the assertion is on the resolved type, not on any manifest: a
    /// manifest audit cannot see a transitive feature union, and the compiled
    /// type can never lie about which backend was selected. If a dependency
    /// re-introduces the feature, this test goes red instead of the fleet
    /// silently paying FDs for it again.
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_resolves_notify_to_the_fsevent_backend_never_kqueue() {
        let backend = std::any::type_name::<notify::RecommendedWatcher>();
        assert!(
            backend.contains("FsEvent"),
            "notify resolved to `{backend}` — expected the FSEvents backend. \
             Something in the dependency graph enabled notify's `macos_kqueue` \
             feature, which holds one open file descriptor per watched path \
             (measured: 26,517 FDs in this daemon on 2026-07-31). Features are \
             additive and cannot be un-enabled by a consumer: find the crate \
             that declares it and change it to `macos_fsevent`."
        );
    }

    #[test]
    fn watch_set_never_exceeds_its_cap_and_evicts_least_recently_used() {
        let mut w = WatchSet::new(3);
        assert_eq!(w.insert(root("/a")), None);
        assert_eq!(w.insert(root("/b")), None);
        assert_eq!(w.insert(root("/c")), None);
        assert_eq!(w.len(), 3);

        // /a is the least recently used → it is the one displaced.
        assert_eq!(w.insert(root("/d")), Some(root("/a")));
        assert_eq!(w.len(), 3, "the cap is a bound, not a suggestion");
        assert!(!w.contains(&root("/a")));
        assert!(w.contains(&root("/d")));
    }

    #[test]
    fn an_actively_used_root_is_never_the_one_evicted() {
        // The failure this guards: an operator working in one repo all day,
        // while background traffic touches others, must not have their own
        // repo evicted out from under them.
        let mut w = WatchSet::new(2);
        w.insert(root("/work"));
        w.insert(root("/other"));
        w.touch(&root("/work"));
        assert_eq!(
            w.insert(root("/new")),
            Some(root("/other")),
            "the untouched root is displaced, not the active one"
        );
        assert!(w.contains(&root("/work")));
    }

    #[test]
    fn a_watcher_rebuild_invalidates_every_cached_root() {
        // notify's FSEvents backend rebuilds its stream on every
        // watch()/unwatch() and resumes from "now", so changes during the gap
        // are delivered to NOBODY and no rescan flag is raised. The only
        // defence is to treat a rebuild as a rescan. If this ever regresses,
        // the symptom is a prompt that is silently wrong until something else
        // touches the repo — so assert the generations actually move.
        let watched: Watched = Arc::new(Mutex::new(WatchSet::new(8)));
        let generations: Generations = Arc::new(Mutex::new(HashMap::new()));
        for r in ["/a", "/b", "/c"] {
            watched.lock().unwrap().insert(root(r));
            generations.lock().unwrap().insert(root(r), 5);
        }
        invalidate_all(&watched, &generations);
        for r in ["/a", "/b", "/c"] {
            assert_eq!(
                generation_of(&generations, &root(r)),
                6,
                "{r} must be invalidated by a stream rebuild, not just the evicted root"
            );
        }
    }

    #[test]
    fn git_object_churn_is_dropped_but_index_and_refs_are_not() {
        let roots = [root("/repo")];
        // A gc/fetch writing packfiles cannot change status output.
        assert!(
            classify_events(&[plain(&["/repo/.git/objects/pack/pack-abc.pack"])], &roots).is_empty(),
            ".git/objects churn must not cost a recompute"
        );
        // Everything that CAN change the output must still get through.
        for p in [
            "/repo/.git/index",
            "/repo/.git/HEAD",
            "/repo/.git/refs/heads/main",
            "/repo/.git/logs/refs/stash",
            "/repo/src/main.rs",
            "/repo/untracked-new-file",
        ] {
            assert!(
                !classify_events(&[plain(&[p])], &roots).is_empty(),
                "{p} changes git status output and MUST dirty the root"
            );
        }
    }

    #[test]
    fn the_objects_directory_node_itself_is_not_excluded() {
        // The exclusion is for paths strictly BELOW .git/objects. Excluding
        // the node itself would be a different, unverified claim.
        let roots = [root("/repo")];
        assert!(!classify_events(&[plain(&["/repo/.git/objects"])], &roots).is_empty());
    }

    #[test]
    fn a_target_dir_is_still_watched_because_excluding_it_is_not_yet_safe() {
        // Pins the deliberate scope limit: target/ is the real storm source
        // but excluding it without a per-root `git check-ignore` check would
        // silently break any repo that tracks a directory of that name.
        let roots = [root("/repo")];
        assert!(!classify_events(&[plain(&["/repo/target/debug/x.o"])], &roots).is_empty());
    }

    /// ★ The invariant that would have prevented 2026-07-31 reaching the
    /// prompt at all: a daemon from a superseded build must retire, not answer.
    #[test]
    fn a_daemon_retires_when_the_caller_is_from_a_different_build() {
        use seki_modules::git_status::is_superseded;
        // The real pair, verbatim — both `rust_seki-0.1.14`. This is why the
        // discriminator is the store path and not the crate version.
        let old = Some("/nix/store/h0bw2av0j1v250j83ll3wakssz8c0wbc-rust_seki-0.1.14/bin/seki");
        let new = Some("/nix/store/9hrl6jbflz63yk72d15gcsmnv5yn0rrh-rust_seki-0.1.14/bin/seki");
        assert!(
            is_superseded(new, old),
            "a daemon running the old build MUST retire when the new one calls"
        );
        assert!(!is_superseded(old, old), "same build → keep serving");
    }

    #[test]
    fn an_unidentifiable_peer_never_retires_a_working_daemon() {
        use seki_modules::git_status::is_superseded;
        // An old client predating the handshake, and a failed `current_exe()`
        // on either side, must all be inert — otherwise a daemon that cannot
        // read its own exe path would exit on every single request.
        let id = Some("/nix/store/aaa-rust_seki-0.1.14/bin/seki");
        assert!(!is_superseded(None, id), "old client → serve, don't retire");
        assert!(!is_superseded(id, None), "unknown own id → serve");
        assert!(!is_superseded(None, None));
    }

    #[test]
    fn re_inserting_a_watched_root_evicts_nothing() {
        // ensure_watched returns early on a known root, but the bound must
        // hold even if that guard is ever removed — re-admitting an existing
        // member is a recency update, not growth.
        let mut w = WatchSet::new(2);
        w.insert(root("/a"));
        w.insert(root("/b"));
        assert_eq!(w.insert(root("/a")), None);
        assert_eq!(w.len(), 2);
    }
}
