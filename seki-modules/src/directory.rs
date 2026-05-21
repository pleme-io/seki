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
        );
        let text = format_with_suffix(&path, &self.cfg.suffix);
        Ok(Some(Segment::new("directory").push(StyledFragment::new(
            text,
            self.cfg.style.resolve(),
        ))))
    }
}

fn format_with_suffix(path: &str, suffix: &str) -> String {
    let mut out = String::with_capacity(path.len() + suffix.len());
    out.push_str(path);
    out.push_str(suffix);
    out
}

/// Pure helper — exposed so tests can drive it without a
/// [`RenderContext`].
pub fn display_path(
    cwd: &Path,
    home: Option<&Path>,
    home_symbol: &str,
    truncation: usize,
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
    let tail = &parts[parts.len() - truncation..];
    tail.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn truncates_long_paths_to_tail() {
        let cwd = PathBuf::from("/a/b/c/d/e/f");
        assert_eq!(display_path(&cwd, None, "~", 3), "d/e/f");
    }

    #[test]
    fn no_truncation_when_short_enough() {
        let cwd = PathBuf::from("/a/b");
        assert_eq!(display_path(&cwd, None, "~", 3), "/a/b");
    }

    #[test]
    fn substitutes_home_for_tilde() {
        let cwd = PathBuf::from("/home/op/code");
        let home = PathBuf::from("/home/op");
        assert_eq!(display_path(&cwd, Some(&home), "~", 5), "~/code");
    }

    #[test]
    fn renders_segment_with_suffix() {
        let module = DirectoryModule::new(DirectoryConfig::default());
        let ctx = RenderContext::from_env()
            .with_cwd("/tmp/seki-test-abc")
            .with_colors(false);
        let seg = module.render(&ctx).unwrap().unwrap();
        assert_eq!(seg.module, "directory");
        assert!(seg.fragments[0].text.ends_with(' '));
    }
}
