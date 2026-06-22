//! `caixa` segment — surfaces the current repo's caixa kind.
//!
//! Pleme-io-native. Walks up from `RenderContext::cwd` looking for a
//! `caixa.lisp` file (stops at filesystem root or the parent of a
//! `.git` directory), parses the leading `(defcaixa …)` form's
//! `:kind` slot via a minimal token scan, and emits the typed caixa
//! kind into the prompt.
//!
//! ## Theme
//!
//! Nord-frost blue `#88C0D0` for a clean parse (matches the
//! snowflake glyph + `nix_shell` segment); Nord-aurora red
//! `#BF616A` for present-but-unparseable (`:kind` missing /
//! unrecognised).
//!
//! ## Probe budget
//!
//! Filesystem walk only — no subprocess, no network. The scan is
//! bounded by depth-from-cwd (stops at root or `.git` parent).
//! Caching across renders is a follow-up (M3.2) — at typical
//! pleme-io repo depths (`~/code/github/<org>/<repo>/…`) the walk
//! is a handful of `stat` calls and well under any reasonable
//! scan_timeout_ms.

use seki_core::{
    Module, RenderContext, Segment, SekiResult,
    config::caixa::CaixaConfig,
    segment::StyledFragment,
};
use std::path::{Path, PathBuf};

pub struct CaixaModule {
    cfg: CaixaConfig,
}

impl CaixaModule {
    pub fn new(cfg: CaixaConfig) -> Self {
        Self { cfg }
    }
}

impl Module for CaixaModule {
    fn name(&self) -> &'static str {
        "caixa"
    }

    fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    fn render(&self, ctx: &RenderContext) -> SekiResult<Option<Segment>> {
        let Some(caixa_path) = find_caixa_lisp(&ctx.cwd) else {
            return Ok(None);
        };
        let parsed = parse_caixa_kind_from_path(&caixa_path);
        let rel = relative_path(&ctx.cwd, &caixa_path);
        let (kind_text, style) = match parsed {
            Some(kind) => (kind, self.cfg.style.resolve()),
            None => ("?".to_owned(), self.cfg.error_style.resolve()),
        };
        let text = seki_core::format::render(&self.cfg.format, |__n| match __n {
            "kind" => Some(kind_text.to_owned()),
            "path" => Some(rel.to_owned()),
            _ => None,
        });
        Ok(Some(
            Segment::new("caixa").push(StyledFragment::new(text, style)),
        ))
    }
}

/// The five typed caixa kinds — keep in sync with caixa-author docs.
const CAIXA_KINDS: &[&str] = &["Servico", "Biblioteca", "Aplicacao", "Supervisor", "Binario"];

/// Walk up from `start` looking for a `caixa.lisp`. Stops at the
/// filesystem root or at the parent of a `.git` directory (the
/// repo root marker), whichever comes first.
pub fn find_caixa_lisp(start: &Path) -> Option<PathBuf> {
    let mut current: &Path = start;
    loop {
        let candidate = current.join("caixa.lisp");
        if candidate.is_file() {
            return Some(candidate);
        }
        // Stop at repo root: if this directory contains `.git`, the
        // caixa.lisp must live here or not at all (we don't walk
        // out of the repo).
        if current.join(".git").exists() && !candidate.exists() {
            return None;
        }
        match current.parent() {
            Some(p) if p != current => current = p,
            _ => return None,
        }
    }
}

/// Read + parse the `:kind` slot from a `caixa.lisp` file. Returns
/// `None` for missing files, unreadable files, or files with no
/// recognised `:kind` token.
pub fn parse_caixa_kind_from_path(path: &Path) -> Option<String> {
    let body = std::fs::read_to_string(path).ok()?;
    parse_caixa_kind(&body)
}

/// Pure-string parser — exposed for testability. Scans the leading
/// `(defcaixa …)` form for the `:kind` slot. Tolerates any whitespace
/// between `:kind` and its value (caixa.lisp authors align slots
/// vertically). Returns the first matched kind or `None`.
pub fn parse_caixa_kind(body: &str) -> Option<String> {
    // Find the start of the (defcaixa form — bound the scan window.
    let start = body.find("(defcaixa")?;
    let tail = &body[start..];
    // Find :kind anywhere in the form. Bound by the next top-level
    // `)` would require paren-matching; for the typed caixa.lisp
    // shape (slots are flat key/value pairs), scanning the whole
    // tail is cheap and safe — :kind is unique in a defcaixa.
    let kind_idx = tail.find(":kind")?;
    let after = &tail[kind_idx + ":kind".len()..];
    // Skip whitespace, then match one of the five typed kinds.
    let trimmed = after.trim_start();
    for kind in CAIXA_KINDS {
        if trimmed.starts_with(kind) {
            // Reject substring matches that bleed into longer ids
            // (e.g. ServicoFoo). The next char must be a separator.
            let next = trimmed.as_bytes().get(kind.len()).copied();
            let is_word_boundary = match next {
                None => true,
                Some(b) => !is_lisp_symbol_char(b),
            };
            if is_word_boundary {
                return Some((*kind).to_owned());
            }
        }
    }
    None
}

/// Lisp symbol chars: anything that could legitimately appear in a
/// caixa kind identifier. Whitespace, parens, semicolons (comments)
/// all act as boundaries.
fn is_lisp_symbol_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_'
}

/// Render `caixa.lisp` path relative to cwd. Falls back to a `.`
/// representation when the file is exactly at cwd, and to the
/// absolute path when the relative resolution fails.
pub fn relative_path(cwd: &Path, caixa_path: &Path) -> String {
    if let Ok(rel) = caixa_path.strip_prefix(cwd) {
        if rel.as_os_str().is_empty() {
            return ".".to_owned();
        }
        return rel.display().to_string();
    }
    caixa_path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmp_dir(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("seki-caixa-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn parses_aligned_kind_slot() {
        let body = "(defcaixa\n  :nome \"foo\"\n  :kind        Binario\n  :versao \"0.1.0\")";
        assert_eq!(parse_caixa_kind(body), Some("Binario".to_owned()));
    }

    #[test]
    fn parses_single_space_kind_slot() {
        let body = "(defcaixa :kind Servico :nome \"svc\")";
        assert_eq!(parse_caixa_kind(body), Some("Servico".to_owned()));
    }

    #[test]
    fn rejects_unknown_kind() {
        let body = "(defcaixa :kind Bogus)";
        assert_eq!(parse_caixa_kind(body), None);
    }

    #[test]
    fn rejects_substring_kind() {
        // `ServicoFoo` is not a typed caixa kind — must not match
        // the `Servico` prefix.
        let body = "(defcaixa :kind ServicoFoo)";
        assert_eq!(parse_caixa_kind(body), None);
    }

    #[test]
    fn parses_all_five_typed_kinds() {
        for k in CAIXA_KINDS {
            let body = format!("(defcaixa :kind {k})");
            assert_eq!(parse_caixa_kind(&body), Some((*k).to_owned()));
        }
    }

    #[test]
    fn ignores_caixa_keyword_outside_defcaixa() {
        let body = ";; mentions defcaixa but no form\n";
        assert_eq!(parse_caixa_kind(body), None);
    }

    #[test]
    fn render_format_kind_substitution() {
        let out = seki_core::format::render_one("[$kind]($style)", "kind", "Biblioteca");
        assert_eq!(out, "Biblioteca");
    }

    #[test]
    fn render_format_kind_and_path_substitution() {
        let out = seki_core::format::render("$kind@$path", |__n| match __n {
            "kind" => Some("Servico".to_owned()),
            "path" => Some("svc/caixa.lisp".to_owned()),
            _ => None,
        });
        assert_eq!(out, "Servico@svc/caixa.lisp");
    }

    #[test]
    fn find_caixa_lisp_at_cwd() {
        let dir = tmp_dir("at-cwd");
        fs::write(dir.join("caixa.lisp"), "(defcaixa :kind Binario)").unwrap();
        let found = find_caixa_lisp(&dir).expect("should find");
        assert_eq!(found, dir.join("caixa.lisp"));
    }

    #[test]
    fn find_caixa_lisp_walks_up() {
        let root = tmp_dir("walk-up");
        let sub = root.join("a").join("b");
        fs::create_dir_all(&sub).unwrap();
        fs::write(root.join("caixa.lisp"), "(defcaixa :kind Aplicacao)").unwrap();
        let found = find_caixa_lisp(&sub).expect("should find via walk-up");
        assert_eq!(found, root.join("caixa.lisp"));
    }

    #[test]
    fn find_caixa_lisp_absent_returns_none() {
        let dir = tmp_dir("absent");
        // .git marker so we stop the walk at this dir.
        fs::create_dir(dir.join(".git")).unwrap();
        assert!(find_caixa_lisp(&dir).is_none());
    }

    #[test]
    fn renders_segment_for_caixa_repo() {
        let dir = tmp_dir("render");
        fs::write(
            dir.join("caixa.lisp"),
            "(defcaixa\n  :nome \"x\"\n  :kind        Servico)",
        )
        .unwrap();
        let module = CaixaModule::new(CaixaConfig::default());
        let ctx = RenderContext::from_env().with_cwd(&dir).with_colors(false);
        let seg = module.render(&ctx).unwrap().expect("segment");
        assert_eq!(seg.module, "caixa");
        assert_eq!(seg.fragments[0].text, "Servico");
    }

    #[test]
    fn renders_nothing_outside_caixa_repo() {
        let dir = tmp_dir("no-caixa");
        fs::create_dir(dir.join(".git")).unwrap();
        let module = CaixaModule::new(CaixaConfig::default());
        let ctx = RenderContext::from_env().with_cwd(&dir).with_colors(false);
        assert!(module.render(&ctx).unwrap().is_none());
    }

    #[test]
    fn bare_config_is_disabled() {
        let cfg = CaixaConfig::bare();
        assert!(!cfg.enabled);
        assert_eq!(cfg.format, "");
    }

    #[test]
    fn default_uses_nord_frost_palette() {
        let cfg = CaixaConfig::default();
        assert_eq!(cfg.style.as_str(), "bold #88C0D0");
        assert_eq!(cfg.error_style.as_str(), "bold #BF616A");
    }
}
