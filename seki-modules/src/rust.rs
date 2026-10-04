//! `rust` segment — present when `Cargo.toml` / `rust-toolchain*`
//! lives in CWD. The channel is read from `rust-toolchain.toml`'s
//! `[toolchain] channel`, walking CWD up to filesystem root the way
//! rustup resolves an override; with no toolchain file anywhere the
//! version is the one the `rustc` on `PATH` reports (the compiler a
//! build here would use: a nix devshell's pin, or the profile's), and
//! with neither the segment is symbol-only.
//!
//! **Never a value nothing measured.** It used to print the literal
//! `"stable"`, so a repo pinned by nix with no `rust-toolchain.toml`
//! rendered `via 🦀 stable` having consulted no toolchain at all. The
//! version shown is read from a file on disk or from the compiler
//! itself; when neither answers, nothing is shown.

use seki_core::{
    Module, RenderContext, Segment, SekiResult, config::rust::RustConfig, segment::StyledFragment,
};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;

const RUSTC_BUDGET: Duration = Duration::from_millis(500);

type Probe = Box<dyn Fn(&Path) -> Option<String> + Send + Sync>;

pub struct RustModule {
    cfg: RustConfig,
    search_path: Option<OsString>,
    probe: Probe,
    compiler: Mutex<Option<(PathBuf, Option<String>)>>,
}

fn rustc_version(rustc: &Path) -> Option<String> {
    seki_core::exec::bounded_stdout(Command::new(rustc).arg("--version"), RUSTC_BUDGET)
        .and_then(|out| parse_rustc_version(&out))
}

impl RustModule {
    pub fn new(cfg: RustConfig) -> Self {
        Self {
            cfg,
            search_path: None,
            probe: Box::new(rustc_version),
            compiler: Mutex::new(None),
        }
    }

    pub fn with_search_path(cfg: RustConfig, search_path: impl Into<OsString>) -> Self {
        Self {
            cfg,
            search_path: Some(search_path.into()),
            probe: Box::new(rustc_version),
            compiler: Mutex::new(None),
        }
    }

    #[must_use]
    pub fn probing_with(
        mut self,
        probe: impl Fn(&Path) -> Option<String> + Send + Sync + 'static,
    ) -> Self {
        self.probe = Box::new(probe);
        self
    }

    fn compiler_version(&self) -> Option<String> {
        let search = self
            .search_path
            .clone()
            .or_else(|| std::env::var_os("PATH"))?;
        let rustc = find_on(&search, "rustc")?;
        let mut cache = self.compiler.lock().ok()?;
        if let Some((path, version)) = cache.as_ref()
            && *path == rustc
        {
            return version.clone();
        }
        let version = (self.probe)(&rustc);
        *cache = Some((rustc, version.clone()));
        version
    }
}

pub fn find_on(search_path: &OsString, name: &str) -> Option<PathBuf> {
    std::env::split_paths(search_path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
        .and_then(|found| fs::canonicalize(found).ok())
}

pub fn parse_rustc_version(output: &str) -> Option<String> {
    output
        .split_whitespace()
        .nth(1)
        .filter(|v| v.starts_with(|c: char| c.is_ascii_digit()))
        .map(str::to_owned)
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
        // ── ★ A BARE SYMBOL IS A READING, NOT A FABRICATION ─────────────
        // The `text.is_empty()` guard that used to sit below could never
        // fire — `text` already held prefix + symbol ("via ⊿") by then.
        // So I replaced it with a gate on `read_toolchain_channel`
        // returning `Some`, reasoning that `via ⊿` with no version was
        // the same "presented as measured, never measured" defect as
        // `stable`.
        //
        // Two tests went red and they were right. The symbol means "this
        // is a Rust project", which IS measured — `detected` just proved
        // it — and unlike `stable` it asserts no version. Suppressing it
        // deletes a true signal to avoid a false one that is not there.
        //
        // The distinction worth keeping: `stable` was a VALUE nothing
        // declared; a bare symbol is a CATEGORY the filesystem declares.
        // Only the first is an invention. The dead guard is gone rather
        // than repaired, because there is nothing for it to catch.
        let mut text = String::new();
        text.push_str(&self.cfg.prefix);
        text.push_str(&self.cfg.symbol);
        if let Some(version) = read_toolchain_channel(&ctx.cwd).or_else(|| self.compiler_version())
        {
            text.push_str(&version);
        }
        // The symbol carries its own trailing space so `⊿ 1.89.0` reads
        // right; with no channel that space would dangle before the suffix.
        let mut text = text.trim_end().to_owned();
        text.push_str(&self.cfg.suffix);
        Ok(Some(
            Segment::new("rust")
                .push(StyledFragment::new(text, self.cfg.style.resolve()))
                // `git_branch` does not always end in a space (it leans on
                // `git_status`, which is absent in a clean repo), so this
                // segment asks not to be run into: `mainvia ⊿` -> `main via ⊿`.
                .with_leading_space(),
        ))
    }
}

/// Is `cwd` inside a Rust project?
///
/// ── ★ THE WALK IS THE POINT ─────────────────────────────────────────────
/// This checked `cwd` alone, which made it the THIRD instance of the
/// blind spot behind the `stable` bug — and the one that hid the other
/// two. Standing in `seki/seki-modules/src` there is no `Cargo.toml`, so
/// the whole segment returned `None` before the toolchain lookup ran; the
/// walk added to `read_toolchain_channel` was unreachable from a
/// subdirectory, which is where a shell spends its life.
///
/// Fixing the reader without fixing the detector produces a fix that
/// passes its tests and changes nothing on screen. Both walk now.
pub fn detected(cwd: &Path, files: &[String], folders: &[String]) -> bool {
    let mut cur = cwd;
    loop {
        if files.iter().any(|f| cur.join(f).is_file())
            || folders.iter().any(|d| cur.join(d).is_dir())
        {
            return true;
        }
        match cur.parent() {
            Some(p) => cur = p,
            None => return false,
        }
    }
}

/// Resolve the toolchain channel the way rustup resolves an override:
/// walk from `cwd` up to filesystem root, and at each directory prefer
/// `rust-toolchain.toml` over the legacy plain `rust-toolchain`. The
/// nearest directory holding either file wins.
///
/// Reading only `cwd` — which is what this did — meant every subcrate
/// of a workspace whose ROOT carries the pin resolved to nothing, and
/// the caller's fallback then invented an answer. The walk is what
/// makes the pin visible from where the operator actually stands.
/// The channel from the nearest `rust-toolchain.toml` at or above `cwd`.
///
/// The walk lives in `seki_core::project` because `nix_flake_drift` needed
/// the identical thing and had gone without it — silent in every
/// subdirectory of every flake repo. Two copies of walk-up-to-a-marker,
/// one right and one missing, is where the helper earns its keep.
pub fn read_toolchain_channel(cwd: &Path) -> Option<String> {
    // BOTH spellings, one walk. Searching for `rust-toolchain.toml` alone
    // made the extension-less `rust-toolchain` invisible — a regression
    // this repo's own test caught on the first build after the extraction.
    let root = seki_core::project::find_root_containing_any(
        cwd,
        &["rust-toolchain.toml", "rust-toolchain"],
    )?;
    read_toolchain_channel_in(&root)
}

pub fn read_toolchain_channel_in(dir: &Path) -> Option<String> {
    let toml_path = dir.join("rust-toolchain.toml");
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
    let plain_path = dir.join("rust-toolchain");
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
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

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

    /// A workspace pins its toolchain at the ROOT; the operator stands
    /// in a subcrate. Red run: restore the CWD-only lookup and this
    /// returns `None`.
    #[test]
    fn finds_channel_in_a_parent_directory() {
        let root = td("parent");
        fs::write(
            root.join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"1.89.0\"\n",
        )
        .unwrap();
        let sub = root.join("crates").join("deep");
        fs::create_dir_all(&sub).unwrap();
        assert_eq!(read_toolchain_channel(&sub).as_deref(), Some("1.89.0"));
        let _ = fs::remove_dir_all(&root);
    }

    /// Nearest wins — a subcrate overriding the workspace pin is the
    /// answer, not the root's.
    #[test]
    fn nearest_toolchain_file_wins_over_an_ancestor() {
        let root = td("nearest");
        fs::write(
            root.join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"1.80.0\"\n",
        )
        .unwrap();
        let sub = root.join("crates").join("nightly-only");
        fs::create_dir_all(&sub).unwrap();
        fs::write(
            sub.join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"nightly-2026-01-01\"\n",
        )
        .unwrap();
        assert_eq!(
            read_toolchain_channel(&sub).as_deref(),
            Some("nightly-2026-01-01")
        );
        let _ = fs::remove_dir_all(&root);
    }

    /// rustup's own precedence within one directory.
    #[test]
    fn toolchain_toml_beats_plain_file_in_the_same_directory() {
        let dir = td("precedence");
        fs::write(
            dir.join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"1.89.0\"\n",
        )
        .unwrap();
        fs::write(dir.join("rust-toolchain"), "1.70.0\n").unwrap();
        assert_eq!(read_toolchain_channel(&dir).as_deref(), Some("1.89.0"));
        let _ = fs::remove_dir_all(&dir);
    }

    /// The defect this module shipped: a Cargo project with NO
    /// toolchain file anywhere rendered `via 🦀 stable`, a channel
    /// nothing on disk had said. It must render symbol-only instead.
    /// Red run: restore `.unwrap_or_else(|| "stable".to_owned())` and
    /// the `contains("stable")` assertion fires.
    #[test]
    fn an_unknown_channel_renders_symbol_only_and_never_says_stable() {
        let dir = td("unknown");
        fs::write(dir.join("Cargo.toml"), "[package]\nname='x'\n").unwrap();
        let cfg = RustConfig::default();
        let (prefix, symbol, suffix) = (cfg.prefix.clone(), cfg.symbol.clone(), cfg.suffix.clone());
        let module = RustModule::with_search_path(cfg, "");
        let ctx = RenderContext::from_env().with_cwd(&dir);

        let segment = module.render(&ctx).unwrap().expect("segment renders");
        let text: String = segment.fragments.iter().map(|f| f.text.as_str()).collect();

        assert!(
            !text.contains("stable"),
            "prompt invented a channel no file declared: {text:?}"
        );
        // Symbol-only, with the symbol's dangling space collapsed so
        // the segment does not pad itself twice.
        assert_eq!(text, format!("{prefix}{}{suffix}", symbol.trim_end()));
        let _ = fs::remove_dir_all(&dir);
    }

    /// The channel IS printed when a file declares it — the honest
    /// half of the same gate, so "print nothing" can't pass by
    /// printing nothing always.
    #[test]
    fn a_declared_channel_is_rendered() {
        let dir = td("declared");
        fs::write(dir.join("Cargo.toml"), "[package]\nname='x'\n").unwrap();
        fs::write(
            dir.join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"1.89.0\"\n",
        )
        .unwrap();
        let (_, probe) = counting(Some("1.98.1"));
        let module = RustModule::with_search_path(RustConfig::default(), rustc_on_path(&dir))
            .probing_with(probe);
        let ctx = RenderContext::from_env().with_cwd(&dir);

        let segment = module.render(&ctx).unwrap().expect("segment renders");
        let text: String = segment.fragments.iter().map(|f| f.text.as_str()).collect();

        assert!(
            text.ends_with("1.89.0 "),
            "the toolchain file outranks the compiler: {text:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    fn rustc_on_path(dir: &Path) -> PathBuf {
        let bin = dir.join("bin");
        fs::create_dir_all(&bin).unwrap();
        fs::write(bin.join("rustc"), "").unwrap();
        bin
    }

    fn counting(
        says: Option<&'static str>,
    ) -> (
        Arc<AtomicUsize>,
        impl Fn(&Path) -> Option<String> + Send + Sync,
    ) {
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&calls);
        let probe = move |_: &Path| {
            seen.fetch_add(1, Ordering::SeqCst);
            says.map(str::to_owned)
        };
        (calls, probe)
    }

    #[test]
    fn with_no_toolchain_file_the_compiler_on_path_names_the_version_once() {
        let dir = td("compiler");
        fs::write(dir.join("Cargo.toml"), "[package]\nname='x'\n").unwrap();
        let (calls, probe) = counting(Some("1.98.1"));
        let module = RustModule::with_search_path(RustConfig::default(), rustc_on_path(&dir))
            .probing_with(probe);
        let ctx = RenderContext::from_env().with_cwd(&dir);

        for _ in 0..3 {
            let segment = module.render(&ctx).unwrap().expect("segment renders");
            let text: String = segment.fragments.iter().map(|f| f.text.as_str()).collect();
            assert!(text.ends_with("1.98.1 "), "unexpected render: {text:?}");
        }
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "rustc is probed once per toolchain path"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_compiler_that_reports_no_version_renders_symbol_only() {
        let dir = td("mute");
        fs::write(dir.join("Cargo.toml"), "[package]\nname='x'\n").unwrap();
        let cfg = RustConfig::default();
        let (prefix, symbol, suffix) = (cfg.prefix.clone(), cfg.symbol.clone(), cfg.suffix.clone());
        let (_, probe) = counting(None);
        let module = RustModule::with_search_path(cfg, rustc_on_path(&dir)).probing_with(probe);
        let ctx = RenderContext::from_env().with_cwd(&dir);
        let segment = module.render(&ctx).unwrap().expect("segment renders");
        let text: String = segment.fragments.iter().map(|f| f.text.as_str()).collect();
        assert_eq!(text, format!("{prefix}{}{suffix}", symbol.trim_end()));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_real_probe_reads_the_rustc_on_path() {
        let Some(rustc) = std::env::var_os("PATH").and_then(|p| find_on(&p, "rustc")) else {
            return;
        };
        let version = rustc_version(&rustc).expect("the rustc on PATH reports a version");
        assert!(version.starts_with(|c: char| c.is_ascii_digit()), "{version:?}");
    }

    #[test]
    fn rustc_version_output_is_parsed_strictly() {
        assert_eq!(
            parse_rustc_version("rustc 1.98.1 (48a229cea 2026-09-01)").as_deref(),
            Some("1.98.1")
        );
        assert_eq!(
            parse_rustc_version("rustc 1.99.0-nightly (abc 2026-10-01)").as_deref(),
            Some("1.99.0-nightly")
        );
        assert_eq!(parse_rustc_version("rustc unknown"), None);
        assert_eq!(parse_rustc_version(""), None);
    }
}
