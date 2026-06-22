//! `directory` segment — CWD truncated to `truncation_length`
//! components, with `~` substituted for `$HOME`.

use seki_core::{
    Module, RenderContext, Segment, SekiResult,
    config::directory::DirectoryConfig,
    segment::StyledFragment,
};
use std::path::Path;

pub struct DirectoryModule {
    cfg: DirectoryConfig,
}

impl DirectoryModule {
    pub fn new(cfg: DirectoryConfig) -> Self {
        Self { cfg }
    }
}

impl Module for DirectoryModule {
    fn name(&self) -> &'static str {
        "directory"
    }

    fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    fn render(&self, ctx: &RenderContext) -> SekiResult<Option<Segment>> {
        let path = display_path(
            &ctx.cwd,
            ctx.home.as_deref(),
            &self.cfg.home_symbol,
            self.cfg.truncation_length as usize,
            &self.cfg.truncation_symbol,
        );
        // The typed `format` is the single source of truth — rendered
        // through the shared engine so companion's `📁 [$path]` prefix
        // (and any other literal/markup) renders consistently with every
        // other segment.
        let text = seki_core::format::render_one(&self.cfg.format, "path", &path);
        if text.is_empty() {
            return Ok(None);
        }
        Ok(Some(Segment::new("directory").push(StyledFragment::new(
            text,
            self.cfg.style.resolve(),
        ))))
    }
}

/// Pure helper — exposed so tests can drive it without a
/// [`RenderContext`].
pub fn display_path(
    cwd: &Path,
    home: Option<&Path>,
    home_symbol: &str,
    truncation: usize,
    truncation_symbol: &str,
) -> String {
    let display_cwd: String = match home {
        Some(h) if cwd.starts_with(h) => {
            let rest = cwd.strip_prefix(h).unwrap_or(cwd);
            if rest.as_os_str().is_empty() {
                home_symbol.to_owned()
            } else {
                let mut out = home_symbol.to_owned();
                out.push('/');
                out.push_str(&rest.display().to_string());
                out
            }
        }
        _ => cwd.display().to_string(),
    };

    if truncation == 0 {
        return display_cwd;
    }

    let parts: Vec<&str> = display_cwd.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() <= truncation {
        return display_cwd;
    }
    // Truncated: prepend the truncation marker (default "…/") so a short
    // path on a narrow pane is never mistaken for an absolute/whole path.
    let tail = &parts[parts.len() - truncation..];
    let mut out = String::with_capacity(truncation_symbol.len() + display_cwd.len());
    out.push_str(truncation_symbol);
    out.push_str(&tail.join("/"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn truncates_long_paths_to_tail_with_marker() {
        let cwd = PathBuf::from("/a/b/c/d/e/f");
        // Truncated → the "…/" marker signals the elision.
        assert_eq!(display_path(&cwd, None, "~", 3, "…/"), "…/d/e/f");
    }

    #[test]
    fn no_truncation_when_short_enough() {
        let cwd = PathBuf::from("/a/b");
        assert_eq!(display_path(&cwd, None, "~", 3, "…/"), "/a/b");
    }

    #[test]
    fn substitutes_home_for_tilde() {
        let cwd = PathBuf::from("/home/op/code");
        let home = PathBuf::from("/home/op");
        assert_eq!(display_path(&cwd, Some(&home), "~", 5, "…/"), "~/code");
    }

    #[test]
    fn renders_path_through_format() {
        let mut cfg = DirectoryConfig::default();
        cfg.format = "📁 [$path]($style)".to_owned();
        let module = DirectoryModule::new(cfg);
        let ctx = RenderContext::from_env()
            .with_cwd("/tmp/seki-test-abc")
            .with_colors(false);
        let seg = module.render(&ctx).unwrap().unwrap();
        assert_eq!(seg.module, "directory");
        // The format's `📁 ` literal renders, and the path follows.
        assert!(seg.fragments[0].text.starts_with("📁 "));
        assert!(seg.fragments[0].text.contains("seki-test-abc"));
    }
}
