//! `gitops` — is this node still reconciling itself to the branch?
//!
//! ── ★ WHY A PROMPT SEGMENT AND NOT A LOG LINE ───────────────────────────
//! The reconciler already recorded everything. On ryn it failed 4136
//! consecutive ticks across 27.9 days, and on cid its LaunchDaemon was dead
//! for 17 hours while the node fell 14 commits behind — both times the
//! evidence existed the whole while, in a root-owned log nobody tails.
//!
//! Silence is the failure mode: a reconciler with nothing to do and one
//! that died look identical from every surface an operator passes. So the
//! verdict goes where the operator's eyes already are, and it renders
//! NOTHING when the loop is healthy — the segment appears precisely when
//! something is wrong, which is the only way a passive readout stays worth
//! reading.
//!
//! ── ★ IT READS FILES, NEVER RUNS THE DAEMON ─────────────────────────────
//! A prompt renders on every keystroke-to-enter; it may not spawn
//! `sentinela status`, and it may not reach the network. Both facts the
//! segment needs are already on disk, published by the daemon itself:
//!
//!   heartbeat.json   { at_unix_ms, outcome, head_rev }  — rewritten every
//!                    tick, so its AGE is the liveness signal.
//!   receipts.json    the BLAKE3 chain — read only for its tail.
//!
//! An ACTIVATION time cannot serve as liveness: a converged loop activates
//! nothing for weeks, so an old receipt is equally consistent with health
//! and with death. That ambiguity is the whole reason the heartbeat exists.
//!
//! ── ★ NEVER FABRICATE THE VERDICT ───────────────────────────────────────
//! Every unmeasurable case renders nothing. A missing state dir means this
//! host is not a gitops node; an unparseable heartbeat means we cannot tell.
//! Neither is "converged", and neither is an alarm. This is the same rule
//! the `rust` segment broke by printing a hardcoded `stable` for a channel
//! it never read.

use seki_core::{
    Module, RenderContext, Segment, SekiResult, config::gitops::GitopsConfig,
    segment::StyledFragment,
};
use std::path::Path;

pub struct GitopsModule {
    cfg: GitopsConfig,
}

impl GitopsModule {
    pub fn new(cfg: GitopsConfig) -> Self {
        Self { cfg }
    }
}

/// What the on-disk state says about the loop. `Unknown` is a real answer,
/// not a failure to compute one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Health {
    /// No state dir: this host does not run a reconciler.
    NotAGitopsHost,
    /// State exists but cannot be read into a verdict.
    Unknown,
    /// Last tick within the staleness budget.
    Ticking,
    /// No tick for `silent_secs`, against a `poll` interval.
    Stopped { silent_secs: u64, poll: u64 },
    /// Ticking, but the chain records a run of failures.
    Failing { streak: u64 },
}

/// How many poll intervals of silence before a loop is presumed stopped.
/// Three tolerates one slow build plus one missed cycle — the same budget
/// `fleet rebuild` and `sentinela status --gate` use, deliberately, so the
/// three surfaces cannot disagree about what "stale" means.
const STALE_AFTER_POLLS: u64 = 3;

/// Decide the verdict from already-read values. Pure, so every arm is
/// provable without a daemon, a clock, or a filesystem.
#[must_use]
pub fn classify(
    heartbeat_at_ms: Option<u64>,
    now_ms: u64,
    poll_seconds: Option<u64>,
    consecutive_failures: Option<u64>,
) -> Health {
    let (Some(beat), Some(poll)) = (heartbeat_at_ms, poll_seconds) else {
        // Staleness is unjudgeable without both the pulse and the interval.
        // Guessing a default interval here would manufacture a verdict out
        // of a number nobody published.
        return Health::Unknown;
    };
    let silent_ms = now_ms.saturating_sub(beat);
    if silent_ms > STALE_AFTER_POLLS * poll * 1000 {
        return Health::Stopped {
            silent_secs: silent_ms / 1000,
            poll,
        };
    }
    match consecutive_failures {
        Some(n) if n > 0 => Health::Failing { streak: n },
        _ => Health::Ticking,
    }
}

/// Render a duration the way an operator reads one at a glance.
#[must_use]
pub fn humanize(secs: u64) -> String {
    if secs < 90 {
        let mut s = secs.to_string();
        s.push('s');
        s
    } else if secs < 5400 {
        let mut s = (secs / 60).to_string();
        s.push('m');
        s
    } else if secs < 172_800 {
        let mut s = (secs / 3600).to_string();
        s.push('h');
        s
    } else {
        let mut s = (secs / 86400).to_string();
        s.push('d');
        s
    }
}

impl Module for GitopsModule {
    fn name(&self) -> &'static str {
        "gitops"
    }

    fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    fn render(&self, ctx: &RenderContext) -> SekiResult<Option<Segment>> {
        let dir = Path::new(&self.cfg.state_dir);
        if !dir.is_dir() {
            return Ok(None); // not a gitops host
        }
        let beat = read_heartbeat(&dir.join("heartbeat.json"));
        let now_ms = ctx_now_ms(ctx);
        let health = classify(
            beat.0,
            now_ms,
            beat.1.or(Some(self.cfg.assumed_poll_seconds)),
            read_failure_streak(&dir.join("receipts.json")),
        );

        // Healthy and unknowable both render nothing: a segment that is
        // always present is one the eye learns to skip, and this one has to
        // survive being ignored for weeks and still be seen on the day it
        // fires.
        let (text, style) = match health {
            Health::NotAGitopsHost | Health::Unknown | Health::Ticking => return Ok(None),
            Health::Stopped { silent_secs, poll } => (
                {
                    let mut t = String::from("gitops STOPPED ");
                    t.push_str(&humanize(silent_secs));
                    t.push_str(" (poll ");
                    t.push_str(&humanize(poll));
                    t.push(')');
                    t
                },
                self.cfg.stopped_style.resolve(),
            ),
            Health::Failing { streak } => (
                {
                    let mut t = String::from("gitops FAILING ×");
                    t.push_str(&streak.to_string());
                    t
                },
                self.cfg.failing_style.resolve(),
            ),
        };

        let mut out = String::new();
        out.push_str(&self.cfg.prefix);
        out.push_str(&text);
        out.push_str(&self.cfg.suffix);
        Ok(Some(
            Segment::new("gitops").push(StyledFragment::new(out, style)),
        ))
    }
}

/// `(at_unix_ms, poll_seconds)` from the heartbeat file. Both `None` when
/// absent or unparseable — never a substituted default.
fn read_heartbeat(path: &Path) -> (Option<u64>, Option<u64>) {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return (None, None);
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return (None, None);
    };
    (
        v["at_unix_ms"].as_u64(),
        // The daemon may publish its interval alongside the pulse; when it
        // does, that beats any locally-configured assumption.
        v["poll_seconds"].as_u64(),
    )
}

/// Count the trailing run of failed outcomes in the receipt chain.
///
/// Reads the file's TAIL rather than parsing it whole: the chain reached
/// 31 MB on ryn, and a prompt segment that parses 31 MB per keystroke is a
/// worse defect than the one it reports.
fn read_failure_streak(path: &Path) -> Option<u64> {
    const TAIL_BYTES: u64 = 64 * 1024;
    let meta = std::fs::metadata(path).ok()?;
    let raw = if meta.len() > TAIL_BYTES {
        use std::io::{Read as _, Seek as _, SeekFrom};
        let mut f = std::fs::File::open(path).ok()?;
        f.seek(SeekFrom::End(-(TAIL_BYTES as i64))).ok()?;
        let mut s = String::new();
        // A mid-UTF8 cut is possible; lossy-read rather than fail, since a
        // truncated tail still carries the trailing outcomes we count.
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).ok()?;
        s.push_str(&String::from_utf8_lossy(&buf));
        s
    } else {
        std::fs::read_to_string(path).ok()?
    };
    // The chain is YAML-ish `kind: <outcome>` lines; count the trailing run
    // that are not `activated`. Line-oriented on purpose — it survives a
    // truncated head, which a document parser would not.
    let mut streak = 0u64;
    for line in raw.lines().rev() {
        let t = line.trim();
        if let Some(kind) = t.strip_prefix("kind:") {
            match kind.trim() {
                "activated" => return Some(streak),
                "" => {}
                _ => streak += 1,
            }
        }
    }
    Some(streak)
}

fn ctx_now_ms(_ctx: &RenderContext) -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    const POLL: u64 = 60;
    const NOW: u64 = 1_785_724_816_000;

    /// cid's real numbers, 2026-08-02: last tick 1785664639s, judged at
    /// 1785724816s — silent 16.7h against a 60s poll, while every other
    /// surface reported the node healthy.
    #[test]
    fn cids_dead_daemon_is_reported_as_stopped() {
        let h = classify(Some(1_785_664_639_000), NOW, Some(POLL), Some(0));
        assert_eq!(
            h,
            Health::Stopped {
                silent_secs: 60177,
                poll: 60
            }
        );
    }

    /// A healthy loop renders nothing — the segment exists to be absent.
    #[test]
    fn a_fresh_tick_is_ticking_and_therefore_invisible() {
        assert_eq!(
            classify(Some(NOW - 30_000), NOW, Some(POLL), Some(0)),
            Health::Ticking
        );
    }

    /// The ryn outage shape: alive, but failing every tick.
    #[test]
    fn a_live_loop_with_a_failure_streak_is_failing() {
        assert_eq!(
            classify(Some(NOW - 1000), NOW, Some(POLL), Some(4136)),
            Health::Failing { streak: 4136 }
        );
    }

    /// ★ The rule the `rust` segment broke: never invent what you did not
    /// measure. No pulse, or no interval, means we cannot tell — and
    /// "cannot tell" renders nothing rather than a reassuring nothing.
    #[test]
    fn missing_evidence_is_unknown_never_healthy() {
        assert_eq!(classify(None, NOW, Some(POLL), Some(0)), Health::Unknown);
        assert_eq!(classify(Some(NOW), NOW, None, Some(0)), Health::Unknown);
        assert_eq!(classify(None, NOW, None, None), Health::Unknown);
    }

    /// Exactly at the budget is alive; one millisecond past is not. Pinned
    /// because an off-by-one here either cries wolf every few minutes or
    /// never fires at all.
    #[test]
    fn the_staleness_boundary_is_three_poll_intervals() {
        let budget = STALE_AFTER_POLLS * POLL * 1000;
        assert_eq!(
            classify(Some(NOW - budget), NOW, Some(POLL), Some(0)),
            Health::Ticking
        );
        assert!(matches!(
            classify(Some(NOW - budget - 1), NOW, Some(POLL), Some(0)),
            Health::Stopped { .. }
        ));
    }

    #[test]
    fn durations_read_the_way_an_operator_reads_them() {
        assert_eq!(humanize(45), "45s");
        assert_eq!(humanize(600), "10m");
        assert_eq!(humanize(60177), "16h");
        assert_eq!(humanize(600_000), "6d");
    }
}
