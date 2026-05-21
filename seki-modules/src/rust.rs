//! `rust` segment — present when `Cargo.toml` / `rust-toolchain*`
//! lives in CWD. Reads `rust-toolchain.toml`'s `[toolchain]
//! channel` if present, otherwise falls back to the symbol-only
//! rendering.

use seki_core::{
    Module, RenderContext, Segment, SekiResult,
    config::rust::RustConfig,
    segment::StyledFragment,
};
use std::fs;
use std::path::Path;

pub struct RustModule {
    cfg: RustConfig,
}

impl RustModule {
    pub fn new(cfg: RustConfig) -> Self {
        Self { cfg }
    }
}

impl Module for RustModule {
    fn name(&self) -> &'static str {
        "rust"
    }

    fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    fn render(&self, ctx: &RenderContext) -> SekiResult<Option<Segment>> {
        if !detected(&ctx.cwd, &self.cfg.detect_files, &self.cfg.detect_folders) {
            return Ok(None);
        }
        let channel = read_toolchain_channel(&ctx.cwd).unwrap_or_else(|| "stable".to_owned());
        let mut text = String::new();
        text.push_str(&self.cfg.prefix);
        text.push_str(&self.cfg.symbol);
        text.push_str(&channel);
        text.push_str(&self.cfg.suffix);
        Ok(Some(Segment::new("rust").push(StyledFragment::new(
            text,
            self.cfg.style.resolve(),
        ))))
    }
}

pub fn detected(cwd: &Path, files: &[String], folders: &[String]) -> bool {
    for f in files {
        if cwd.join(f).is_file() {
            return true;
        }
    }
    for d in folders {
        if cwd.join(d).is_dir() {
            return true;
        }
    }
    false
}

pub fn read_toolchain_channel(cwd: &Path) -> Option<String> {
    let toml_path = cwd.join("rust-toolchain.toml");
    if let Ok(content) = fs::read_to_string(&toml_path) {
        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("channel") {
                let rest = rest.trim_start_matches([' ', '=', '"']);
                let value = rest.trim_end_matches('"').trim();
                if !value.is_empty() {
                    return Some(value.to_owned());
                }
            }
        }
    }
    let plain_path = cwd.join("rust-toolchain");
    if let Ok(content) = fs::read_to_string(&plain_path) {
        let value = content.trim();
        if !value.is_empty() {
            return Some(value.to_owned());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn td(tag: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("seki-rust-{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn detects_cargo_toml() {
        let dir = td("cargo");
        fs::write(dir.join("Cargo.toml"), "[package]\nname='x'\n").unwrap();
        assert!(detected(&dir, &["Cargo.toml".into()], &[]));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reads_channel_from_toolchain_toml() {
        let dir = td("toml");
        fs::write(
            dir.join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"nightly-2026-01-01\"\n",
        )
        .unwrap();
        assert_eq!(
            read_toolchain_channel(&dir).as_deref(),
            Some("nightly-2026-01-01")
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reads_channel_from_plain_rust_toolchain() {
        let dir = td("plain");
        fs::write(dir.join("rust-toolchain"), "1.89.0\n").unwrap();
        assert_eq!(read_toolchain_channel(&dir).as_deref(), Some("1.89.0"));
        let _ = fs::remove_dir_all(&dir);
    }
}
