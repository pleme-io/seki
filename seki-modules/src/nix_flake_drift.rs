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
        if !ctx.cwd.join("flake.nix").is_file() {
            return Ok(None);
        }
        let probe = probe_drift(&self.cfg, &ctx.cwd);
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

    if inputs.is_empty() {
        return Some(0);
    }
    let remaining = total_timeout.saturating_sub(started.elapsed());
    if remaining.is_zero() {
        return Some(0);
    }
    let per_input = (remaining / inputs.len() as u32).max(Duration::from_millis(50));

    let mut behind: u32 = 0;
    for input in &inputs {
        if started.elapsed() >= total_timeout {
            break;
        }
        let upstream =
            run_git_ls_remote(&cfg.git_command, &input.url, "main", per_input).or_else(|| {
                run_git_ls_remote(&cfg.git_command, &input.url, "HEAD", per_input)
            });
        if let Some(upstream_rev) = upstream {
            if !input.rev.is_empty() && upstream_rev != input.rev {
                behind += 1;
            }
        }
    }
    Some(behind)
}

fn run_nix_metadata(
    nix_command: &str,
    cwd: &std::path::Path,
    timeout: Duration,
) -> Option<String> {
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
            let url_start_abs =
                idx + needle_url.len() + (metadata_json.len() - after.len() - metadata_json[..idx + needle_url.len()].len() + (metadata_json[idx + needle_url.len()..].len() - after.len()));
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
        assert_eq!(convert_input_url_to_git("git+https://example.com/foo"), None);
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
