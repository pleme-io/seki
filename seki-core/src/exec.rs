//! Running a subprocess under a wall-clock bound.
//!
//! ── ★ WHY A PROMPT CANNOT AFFORD AN UNBOUNDED CHILD ─────────────────────
//! A prompt renderer runs before EVERY shell prompt. A segment that blocks
//! blocks the terminal — not a slow render, an unusable shell. So every
//! subprocess a segment spawns needs a deadline, and roughly eight
//! segments already hand-roll one with `thread::spawn` + `recv_timeout`.
//!
//! `custom.rs` did not, and it is the one that most needed it: it runs
//! OPERATOR-CONFIGURED commands (`sh -c "<cmd>"`, both for the value and
//! for the `when` predicate) via plain `.output()` / `.status()`. Its own
//! module docs asserted the bound existed somewhere else —
//!
//! > Output is bounded by the top-level `command_timeout_ms`; we
//! > don't enforce that inside the module (the M2 renderer hasn't
//! > taken on subprocess supervision yet).
//!
//! — which names a budget and then says nothing applies it. Nothing did.
//! A configured command touching a stale NFS mount, a sleeping VPN, or a
//! down HTTP endpoint would hang the prompt indefinitely. The config even
//! carries an `ignore_timeout` field, so the surface promised a bound with
//! no implementation behind it.
//!
//! ── ★ IT KILLS THE CHILD; recv_timeout ONLY STOPS WAITING ───────────────
//! The `thread::spawn` + `mpsc::recv_timeout` idiom the other segments use
//! returns control on time but ABANDONS the child — it keeps running,
//! holding its pipes, and the worker thread leaks with it. That is
//! tolerable once; at prompt frequency it accumulates a process per render
//! against a wedged endpoint.
//!
//! Here the child is spawned directly, polled with `try_wait`, and KILLED
//! on deadline. The cost is a poll loop instead of a blocking wait, which
//! for sub-second budgets is irrelevant.

use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// How often the deadline is checked. Small enough to be invisible next to
/// any realistic budget, large enough not to spin a core.
const POLL: Duration = Duration::from_millis(5);

/// Why a bounded run produced no output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecError {
    /// The process could not be started at all (missing binary, no fork).
    Spawn(String),
    /// The budget elapsed. The child was killed.
    ///
    /// Distinct from `Spawn` on purpose: "your command is too slow" and
    /// "your command does not exist" are different operator problems, and
    /// a caller that collapses them tells the operator to check the wrong
    /// thing.
    TimedOut { after: Duration },
    /// Waiting on the child itself failed.
    Wait(String),
}

/// Run `cmd` and return its output, or kill it once `budget` elapses.
///
/// stdin is `/dev/null`: a command that reads stdin would otherwise block
/// on the terminal the prompt is about to draw, which looks like a hang
/// with no CPU use and is very hard to attribute.
///
/// # Errors
/// [`ExecError::Spawn`] if the child cannot start, [`ExecError::TimedOut`]
/// if the budget elapses (the child is killed and reaped),
/// [`ExecError::Wait`] if waiting fails.
pub fn bounded_output(cmd: &mut Command, budget: Duration) -> Result<Output, ExecError> {
    let started = Instant::now();
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| ExecError::Spawn(e.to_string()))?;

    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .map_err(|e| ExecError::Wait(e.to_string()));
            }
            Ok(None) => {
                if started.elapsed() >= budget {
                    // Kill, then REAP. Without the wait the child becomes a
                    // zombie held by this process, and a prompt accumulates
                    // one per render against a wedged command — the leak
                    // this helper exists to stop.
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(ExecError::TimedOut { after: budget });
                }
                std::thread::sleep(POLL);
            }
            Err(e) => return Err(ExecError::Wait(e.to_string())),
        }
    }
}

/// `bounded_output`, reduced to trimmed stdout on a zero exit.
///
/// Returns `None` for a timeout, a spawn failure, OR a non-zero exit —
/// callers that render a segment want "no reading" for all three. A caller
/// that must distinguish them uses [`bounded_output`] directly.
#[must_use]
pub fn bounded_stdout(cmd: &mut Command, budget: Duration) -> Option<String> {
    let out = bounded_output(cmd, budget).ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// `bounded_output`, reduced to "did it exit zero".
///
/// A timeout is `false`: a predicate that could not be answered inside its
/// budget has not been answered, and treating an unfinished check as true
/// would render a segment the operator's `when` clause meant to suppress.
#[must_use]
pub fn bounded_success(cmd: &mut Command, budget: Duration) -> bool {
    bounded_output(cmd, budget).is_ok_and(|o| o.status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sh(script: &str) -> Command {
        let mut c = Command::new("sh");
        c.arg("-c").arg(script);
        c
    }

    #[test]
    fn a_fast_command_returns_its_stdout() {
        let got = bounded_stdout(&mut sh("printf hello"), Duration::from_secs(5));
        assert_eq!(got.as_deref(), Some("hello"));
    }

    /// The defect this module exists for: without a bound, this call never
    /// returns and the shell prompt never draws.
    #[test]
    fn a_hanging_command_is_killed_at_the_deadline() {
        let started = Instant::now();
        let got = bounded_output(&mut sh("sleep 30"), Duration::from_millis(150));
        let elapsed = started.elapsed();
        assert!(
            matches!(got, Err(ExecError::TimedOut { .. })),
            "expected a timeout, got {got:?}"
        );
        assert!(
            elapsed < Duration::from_secs(5),
            "returned after {elapsed:?} — the budget was not enforced"
        );
    }

    /// The child must be GONE, not merely un-awaited. `wait_with_output`
    /// on a killed-but-unreaped child is how a prompt leaks a process per
    /// render.
    #[test]
    fn a_killed_child_is_reaped_not_leaked() {
        // If the child were left unreaped this would accumulate zombies;
        // running the timeout path repeatedly and still completing quickly
        // is the observable proxy available without inspecting the process
        // table.
        for _ in 0..5 {
            let _ = bounded_output(&mut sh("sleep 30"), Duration::from_millis(50));
        }
        let got = bounded_stdout(&mut sh("printf ok"), Duration::from_secs(5));
        assert_eq!(
            got.as_deref(),
            Some("ok"),
            "spawning still works after repeated timeouts"
        );
    }

    #[test]
    fn a_missing_binary_is_a_spawn_error_not_a_timeout() {
        let got = bounded_output(
            &mut Command::new("seki-definitely-not-a-real-binary"),
            Duration::from_secs(5),
        );
        assert!(
            matches!(got, Err(ExecError::Spawn(_))),
            "a missing binary must not be reported as slow: {got:?}"
        );
    }

    #[test]
    fn a_nonzero_exit_yields_no_reading() {
        assert_eq!(
            bounded_stdout(&mut sh("exit 3"), Duration::from_secs(5)),
            None
        );
        assert!(!bounded_success(&mut sh("exit 3"), Duration::from_secs(5)));
    }

    /// A `when` predicate that times out must not enable the segment.
    #[test]
    fn a_predicate_that_times_out_is_false_not_true() {
        assert!(
            !bounded_success(&mut sh("sleep 30"), Duration::from_millis(100)),
            "an unfinished predicate is not a satisfied one"
        );
    }

    /// A command reading stdin must not block on the terminal the prompt
    /// is about to draw.
    #[test]
    fn stdin_is_closed_so_a_reader_does_not_block() {
        let got = bounded_stdout(&mut sh("cat; printf done"), Duration::from_secs(3));
        assert_eq!(got.as_deref(), Some("done"));
    }
}

/// Was there anything in this body worth counting?
///
/// ── ★ "ZERO" AND "NOTHING TO COUNT" ARE DIFFERENT ANSWERS ───────────────
/// A probe that tallies matches over a document returns 0 for both "the
/// system is idle" and "this is not the document I think it is" — an error
/// page, a changed schema, a binary that printed a warning and exited 0.
/// The first renders green and is true; the second renders green and is a
/// fabrication, and they are byte-identical downstream.
///
/// `shigoto` hit this first and fixed it with a `has_phases` guard plus a
/// recorded red run: "A body with no explicit counts AND no phase entries
/// used to tally to exactly that, so an error page or a moved schema read
/// as 'nothing to do'." `tend` and `tatara_workload` had the same shape and
/// no guard. Three copies is where the rule stops living in one segment.
///
/// Returns `Some(count)` only when `body` carries at least one `marker`;
/// `None` means "no evidence", which a caller must render as absence, never
/// as zero.
#[must_use]
pub fn tally_with_evidence(
    body: &str,
    markers: &[&str],
    count: impl FnOnce(&str) -> u32,
) -> Option<u32> {
    if !markers.iter().any(|m| body.contains(m)) {
        return None;
    }
    Some(count(body))
}

#[cfg(test)]
mod tally_tests {
    use super::*;

    #[test]
    fn a_body_with_no_marker_is_no_reading_not_zero() {
        assert_eq!(
            tally_with_evidence("<html>502 Bad Gateway</html>", &["\"phase\""], |_| 0),
            None,
            "an error page must not tally to a green zero"
        );
    }

    #[test]
    fn an_empty_body_is_no_reading() {
        assert_eq!(tally_with_evidence("", &["["], |_| 0), None);
    }

    /// The negative control: a body that DOES carry the marker and
    /// genuinely counts zero is a real reading and must survive. Without
    /// this, returning `None` unconditionally would pass the two above and
    /// silence every healthy segment.
    fn _control() {}
    #[test]
    fn a_real_zero_is_still_a_reading() {
        assert_eq!(
            tally_with_evidence("[ok] foo clean", &["["], |_| 0),
            Some(0),
            "a document that carries entries and counts zero IS measured"
        );
    }
}
