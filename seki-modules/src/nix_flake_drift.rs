//! `nix_flake_drift` segment — count of pleme-io inputs behind HEAD.
//!
//! Pleme-io-native (Tier 3). Runs `nix flake metadata --json`, then
//! for each pleme-io input compares the locked rev to upstream
//! `origin/main` HEAD via `git ls-remote`.
//!
//! ## Theme
//!
//! - Nord-aurora red `#BF616A` — drift ≥ 1
//! - Snowstorm dim white `#D8DEE9` — drift == 0
//!
//! ## Probe budget
//!
//! Hard-bounded by `command_timeout_ms`, split across subprocess
//! legs. Gracefully absent outside a flake dir.

use seki_core::{
    Module, RenderContext, Segment, SekiResult,
    config::nix_flake_drift::NixFlakeDriftConfig,
    segment::StyledFragment,
    style::{Style, StyleSpec},
};
use std::process::{Command, Stdio};
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
struct CachedCount {
    count: u32,
    captured_at: Instant,
}

pub struct NixFlakeDriftModule {
    cfg: NixFlakeDriftConfig,
    cache: Mutex<Option<CachedCount>>,
}

impl NixFlakeDriftModule {
    pub fn new(cfg: NixFlakeDriftConfig) -> Self {
        Self {
            cfg,
            cache: Mutex::new(None),
        }
    }
}

impl Module for NixFlakeDriftModule {
    fn name(&self) -> &'static str {
        "nix_flake_drift"
    }

    fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    fn render(&self, ctx: &RenderContext) -> SekiResult<Option<Segment>> {
        let now = Instant::now();
        let ttl = Duration::from_secs(self.cfg.cache_ttl_secs);
        let cached = self.cache.lock().ok().and_then(|g| *g);
        if let Some(entry) = cached {
            if now.duration_since(entry.captured_at) < ttl {
                return Ok(Some(build_segment(&self.cfg, entry.count, false)));
            }
        }
        // ── ★ THE ROOT, NOT THE CWD ────────────────────────────────────
        // This was `!ctx.cwd.join("flake.nix").is_file()`, which is the
        // same blind spot that made the `rust` segment print `stable`:
        // it only ever looked in the directory the shell happened to be
        // in. Every subdirectory of every flake repo in the fleet — which
        // is nearly every directory the operator stands in — rendered
        // nothing, and nothing is indistinguishable from "no drift".
        //
        // The walk returns the ROOT rather than a bool because the probe
        // below runs `nix flake metadata` in it. A bool would have fixed
        // detection and left the command running where there is no
        // flake.nix, turning a silent segment into a failing one.
        let Some(flake_root) = seki_core::project::find_root_containing(&ctx.cwd, "flake.nix")
        else {
            return Ok(None);
        };
        let probe = probe_drift(&self.cfg, &flake_root);
        match probe {
            Some(count) => {
                if let Ok(mut g) = self.cache.lock() {
                    *g = Some(CachedCount {
                        count,
                        captured_at: now,
                    });
                }
                Ok(Some(build_segment(&self.cfg, count, false)))
            }
            None => {
                if let Some(entry) = cached {
                    Ok(Some(build_segment(&self.cfg, entry.count, true)))
                } else {
                    Ok(None)
                }
            }
        }
    }
}

fn build_segment(cfg: &NixFlakeDriftConfig, count: u32, stale: bool) -> Segment {
    let status_label = format_status(count, stale);
    let text = seki_core::format::render(&cfg.format, |__n| match __n {
        "count" => Some(count.to_string()),
        "status" => Some(status_label.to_owned()),
        _ => None,
    });
    let style = pick_style(cfg, count);
    Segment::new("nix_flake_drift").push(StyledFragment::new(text, style))
}

pub fn pick_style(cfg: &NixFlakeDriftConfig, count: u32) -> Style {
    let spec: &StyleSpec = if count == 0 {
        &cfg.fresh_style
    } else {
        &cfg.drift_style
    };
    spec.resolve()
}

pub fn format_status(count: u32, stale: bool) -> String {
    let mut s = String::from("drift: ");
    s.push_str(&count.to_string());
    if stale {
        s.push_str(" (stale)");
    }
    s
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlakeInput {
    pub url: String,
    pub rev: String,
}

fn probe_drift(cfg: &NixFlakeDriftConfig, cwd: &std::path::Path) -> Option<u32> {
    let cfg_clone = cfg.clone();
    let cwd_clone = cwd.to_path_buf();
    let total_timeout = Duration::from_millis(cfg.command_timeout_ms);
    let (tx, rx) = mpsc::channel::<Option<u32>>();
    std::thread::spawn(move || {
        let result = run_drift_sweep(&cfg_clone, &cwd_clone, total_timeout);
        let _ = tx.send(result);
    });
    rx.recv_timeout(total_timeout).ok().and_then(|r| r)
}

fn run_drift_sweep(
    cfg: &NixFlakeDriftConfig,
    cwd: &std::path::Path,
    total_timeout: Duration,
) -> Option<u32> {
    let started = Instant::now();
    let metadata_budget = total_timeout / 2;
    let metadata = run_nix_metadata(&cfg.nix_command, cwd, metadata_budget)?;
    let inputs = parse_pleme_io_inputs(&metadata, &cfg.input_prefix);

    // ── ★ ZERO DRIFT AND ZERO KNOWLEDGE ARE NOT THE SAME ANSWER ──────────
    // Each of these paths used to `return Some(0)`, which the renderer
    // styles as "drift: 0" — the fleet is current. But no input was
    // compared on any of them:
    //
    //   * metadata parsed to zero inputs  → we do not know what to compare
    //   * the time budget is spent        → we did not get to compare
    //
    // The offline case is the one that bites: on a laptop with no network
    // every `ls-remote` fails, and the old loop counted each failure as
    // "not behind" — so a machine that could not reach a single upstream
    // reported the whole fleet up to date. That is precisely backwards from
    // what the operator needs, and it is silent, and the maintenance
    // directive calls a stale lock a cardinal sin. `None` renders nothing
    // at all, which is honest: we have no reading.
    if inputs.is_empty() {
        return None;
    }
    let remaining = total_timeout.saturating_sub(started.elapsed());
    if remaining.is_zero() {
        return None;
    }
    let per_input = (remaining / inputs.len() as u32).max(Duration::from_millis(50));

    let mut behind: u32 = 0;
    let mut compared: u32 = 0;
    for input in &inputs {
        if started.elapsed() >= total_timeout {
            break;
        }
        let upstream = run_git_ls_remote(&cfg.git_command, &input.url, "main", per_input)
            .or_else(|| run_git_ls_remote(&cfg.git_command, &input.url, "HEAD", per_input));
        if let Some(upstream_rev) = upstream {
            if input.rev.is_empty() {
                // A locked input with no rev is unjudgeable, not up to date.
                continue;
            }
            compared += 1;
            if upstream_rev != input.rev {
                behind += 1;
            }
        }
    }
    drift_from_counts(compared, behind)
}

/// The rule, separated from the IO: a drift count is reportable ONLY when
/// at least one input was actually compared.
///
/// Pure so the distinction between "measured zero" and "measured nothing"
/// is provable without nix, git, a network or a clock — a rule reachable
/// only through three subprocesses is a rule nobody tests.
#[must_use]
pub fn drift_from_counts(compared: u32, behind: u32) -> Option<u32> {
    if compared == 0 { None } else { Some(behind) }
}

/// Test-facing shim: `compared[i]` is whether input `i` was reachable.
#[cfg(test)]
fn drift_from(reachable: &[bool], behind: u32) -> Option<u32> {
    drift_from_counts(
        u32::try_from(reachable.iter().filter(|r| **r).count()).unwrap_or(0),
        behind,
    )
}

fn run_nix_metadata(nix_command: &str, cwd: &std::path::Path, timeout: Duration) -> Option<String> {
    let cmd = nix_command.to_owned();
    let cwd_clone = cwd.to_path_buf();
    let (tx, rx) = mpsc::channel::<Option<String>>();
    std::thread::spawn(move || {
        let output = Command::new(&cmd)
            .arg("flake")
            .arg("metadata")
            .arg("--json")
            .current_dir(&cwd_clone)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .output();
        let result = output.ok().and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok()
            } else {
                None
            }
        });
        let _ = tx.send(result);
    });
    rx.recv_timeout(timeout).ok().and_then(|r| r)
}

fn run_git_ls_remote(
    git_command: &str,
    url: &str,
    branch: &str,
    timeout: Duration,
) -> Option<String> {
    let remote = convert_input_url_to_git(url)?;
    let cmd = git_command.to_owned();
    let branch_owned = branch.to_owned();
    let (tx, rx) = mpsc::channel::<Option<String>>();
    std::thread::spawn(move || {
        let output = Command::new(&cmd)
            .arg("ls-remote")
            .arg(&remote)
            .arg(&branch_owned)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .output();
        let result = output.ok().and_then(|o| {
            if o.status.success() {
                let text = String::from_utf8(o.stdout).ok()?;
                text.lines()
                    .next()?
                    .split_whitespace()
                    .next()
                    .map(str::to_owned)
            } else {
                None
            }
        });
        let _ = tx.send(result);
    });
    rx.recv_timeout(timeout).ok().and_then(|r| r)
}

/// `github:owner/repo` → `https://github.com/owner/repo`.
pub fn convert_input_url_to_git(url: &str) -> Option<String> {
    if let Some(rest) = url.strip_prefix("github:") {
        let body = rest.split('?').next().unwrap_or(rest);
        let body = body.split('/').take(2).collect::<Vec<_>>().join("/");
        return Some(format!("https://github.com/{body}"));
    }
    None
}

/// Parse `nix flake metadata --json` for inputs whose URL starts
/// with `prefix`. For each `"url": "<match>"` it scans the next 1024
/// chars for a sibling `"rev"`.
pub fn parse_pleme_io_inputs(metadata_json: &str, prefix: &str) -> Vec<FlakeInput> {
    let mut out: Vec<FlakeInput> = Vec::new();
    let needle_url = "\"url\":";
    let mut search_from = 0;
    while let Some(idx_rel) = metadata_json[search_from..].find(needle_url) {
        let idx = search_from + idx_rel;
        let after = &metadata_json[idx + needle_url.len()..];
        let after = after.trim_start();
        let after = match after.strip_prefix('"') {
            Some(rest) => rest,
            None => {
                search_from = idx + needle_url.len();
                continue;
            }
        };
        let end = match after.find('"') {
            Some(e) => e,
            None => break,
        };
        let url = &after[..end];

        if url.starts_with(prefix) {
            // Compute the absolute index of the closing `"` so we can
            // window past it for the sibling rev search.
            let url_start_abs = idx
                + needle_url.len()
                + (metadata_json.len()
                    - after.len()
                    - metadata_json[..idx + needle_url.len()].len()
                    + (metadata_json[idx + needle_url.len()..].len() - after.len()));
            // Simpler: just find the URL string boundary forward of idx.
            let after_url_abs = idx
                + needle_url.len()
                + metadata_json[idx + needle_url.len()..]
                    .find(url)
                    .unwrap_or(0)
                + url.len()
                + 1; // closing `"`
            let _ = url_start_abs;
            let window_end = (after_url_abs + 1024).min(metadata_json.len());
            let window = &metadata_json[after_url_abs..window_end];
            if let Some(rev) = extract_string_field(window, "rev") {
                out.push(FlakeInput {
                    url: url.to_owned(),
                    rev,
                });
            }
        }
        search_from = idx + needle_url.len() + end + 1;
    }
    out
}

fn extract_string_field(body: &str, name: &str) -> Option<String> {
    let key = format!("\"{name}\"");
    let idx = body.find(&key)?;
    let after = &body[idx + key.len()..];
    let after = after.trim_start();
    let after = after.strip_prefix(':')?;
    let after = after.trim_start();
    let after = after.strip_prefix('"')?;
    let end = after.find('"')?;
    Some(after[..end].to_owned())
}

#[cfg(test)]
mod tests {

    // ── ★ "drift: 0" MUST MEAN "I COMPARED, AND NOTHING MOVED" ──────────
    // Three paths used to return Some(0), which renders as the fleet being
    // current. None of them compared an input. The offline case is the one
    // that bites: every ls-remote fails, so a laptop with no network
    // reported the whole fleet up to date.
    //
    // These pin the DISTINCTION, not the arithmetic: Some(0) is a reading,
    // None is the absence of one, and only the first may render.

    #[test]
    fn an_empty_input_set_is_no_reading_not_zero_drift() {
        // Nothing to compare means we do not know the drift, and saying
        // "0" would assert a measurement never taken.
        assert_eq!(super::drift_from(&[], 0), None);
    }

    #[test]
    fn comparing_nothing_because_every_upstream_was_unreachable_is_no_reading() {
        // 4 inputs known, 0 successfully compared — the offline laptop.
        assert_eq!(super::drift_from(&[false; 4], 0), None);
    }

    #[test]
    fn a_real_comparison_still_reports_its_count() {
        // The honest half: when inputs WERE compared, the count renders,
        // including a genuine zero. Without this the fix could pass by
        // never reporting anything.
        assert_eq!(super::drift_from(&[true, true, true], 0), Some(0));
        assert_eq!(super::drift_from(&[true, true, true], 2), Some(2));
    }

    use super::*;

    #[test]
    fn convert_input_url_github_basic() {
        assert_eq!(
            convert_input_url_to_git("github:pleme-io/substrate"),
            Some("https://github.com/pleme-io/substrate".to_owned())
        );
    }

    #[test]
    fn convert_input_url_strips_ref_suffix() {
        assert_eq!(
            convert_input_url_to_git("github:pleme-io/blackmatter?ref=main"),
            Some("https://github.com/pleme-io/blackmatter".to_owned())
        );
    }

    #[test]
    fn convert_input_url_rejects_non_github() {
        assert_eq!(
            convert_input_url_to_git("git+https://example.com/foo"),
            None
        );
        assert_eq!(convert_input_url_to_git("path:./foo"), None);
    }

    #[test]
    fn parse_pleme_io_inputs_finds_match() {
        let body = r#"{
            "locks": { "nodes": {
                "substrate": {
                    "original": {"url": "github:pleme-io/substrate"},
                    "locked": {"rev": "abc123def", "type": "github"}
                }
            }}
        }"#;
        let inputs = parse_pleme_io_inputs(body, "github:pleme-io/");
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].url, "github:pleme-io/substrate");
        assert_eq!(inputs[0].rev, "abc123def");
    }

    #[test]
    fn parse_pleme_io_inputs_skips_non_matching_prefix() {
        let body = r#"{
            "nodes": {
                "nixpkgs": {
                    "original": {"url": "github:NixOS/nixpkgs"},
                    "locked": {"rev": "deadbeef"}
                }
            }
        }"#;
        let inputs = parse_pleme_io_inputs(body, "github:pleme-io/");
        assert!(inputs.is_empty());
    }

    #[test]
    fn parse_pleme_io_inputs_empty_locks() {
        let body = r#"{"locks": {"nodes": {}}}"#;
        assert!(parse_pleme_io_inputs(body, "github:pleme-io/").is_empty());
    }

    #[test]
    fn format_status_zero() {
        assert_eq!(format_status(0, false), "drift: 0");
    }

    #[test]
    fn format_status_with_count() {
        assert_eq!(format_status(3, false), "drift: 3");
    }

    #[test]
    fn format_status_stale_suffix() {
        assert_eq!(format_status(2, true), "drift: 2 (stale)");
    }

    #[test]
    fn render_format_count_substitution() {
        let out = seki_core::format::render("[$status]($style)", |__n| match __n {
            "count" => Some(7u32.to_string()),
            "status" => Some("drift: 7".to_owned()),
            _ => None,
        });
        assert_eq!(out, "drift: 7");
    }

    #[test]
    fn pick_style_zero_uses_fresh() {
        let cfg = NixFlakeDriftConfig::default();
        assert_eq!(pick_style(&cfg, 0), cfg.fresh_style.resolve());
    }

    #[test]
    fn pick_style_nonzero_uses_drift() {
        let cfg = NixFlakeDriftConfig::default();
        assert_eq!(pick_style(&cfg, 1), cfg.drift_style.resolve());
        assert_eq!(pick_style(&cfg, 99), cfg.drift_style.resolve());
    }

    #[test]
    fn bare_config_is_disabled() {
        let cfg = NixFlakeDriftConfig::bare();
        assert!(!cfg.enabled);
        assert_eq!(cfg.nix_command, "");
    }

    #[test]
    fn default_uses_nord_palette() {
        let cfg = NixFlakeDriftConfig::default();
        assert_eq!(cfg.drift_style.as_str(), "bold #BF616A");
        assert_eq!(cfg.fresh_style.as_str(), "dim #D8DEE9");
    }

    #[test]
    fn default_targets_pleme_io_prefix() {
        let cfg = NixFlakeDriftConfig::default();
        assert_eq!(cfg.input_prefix, "github:pleme-io/");
    }

    #[test]
    fn renders_nothing_outside_flake_dir() {
        let cfg = NixFlakeDriftConfig {
            enabled: true,
            ..NixFlakeDriftConfig::default()
        };
        let module = NixFlakeDriftModule::new(cfg);
        let tmp = std::env::temp_dir().join(format!("seki-flake-drift-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let ctx = RenderContext::from_env().with_cwd(&tmp).with_colors(false);
        assert!(module.render(&ctx).unwrap().is_none());
    }
}
