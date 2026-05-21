//! `nix_shell` segment — reads `IN_NIX_SHELL` / `NIX_BUILD_CORES`
//! to decide whether we're in a nix shell (pure / impure / unknown).

use seki_core::{
    Module, RenderContext, Segment, SekiResult,
    config::nix_shell::NixShellConfig,
    segment::StyledFragment,
};

pub struct NixShellModule {
    cfg: NixShellConfig,
}

impl NixShellModule {
    pub fn new(cfg: NixShellConfig) -> Self {
        Self { cfg }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NixState {
    None,
    Pure,
    Impure,
    Unknown,
}

pub fn detect_state(in_nix_shell: Option<&str>) -> NixState {
    match in_nix_shell {
        None | Some("") => NixState::None,
        Some("pure") => NixState::Pure,
        Some("impure") => NixState::Impure,
        Some(_) => NixState::Unknown,
    }
}

impl Module for NixShellModule {
    fn name(&self) -> &'static str {
        "nix_shell"
    }

    fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    fn render(&self, _ctx: &RenderContext) -> SekiResult<Option<Segment>> {
        let env = std::env::var("IN_NIX_SHELL").ok();
        let state = detect_state(env.as_deref());
        let label = match state {
            NixState::None => return Ok(None),
            NixState::Pure => &self.cfg.pure_format,
            NixState::Impure => &self.cfg.impure_format,
            NixState::Unknown => &self.cfg.unknown_format,
        };
        let mut text = String::new();
        text.push_str(&self.cfg.prefix);
        text.push_str(&self.cfg.symbol);
        text.push_str(label);
        text.push_str(&self.cfg.suffix);
        Ok(Some(Segment::new("nix_shell").push(StyledFragment::new(
            text,
            self.cfg.style.resolve(),
        ))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_pure_impure_unknown_none() {
        assert_eq!(detect_state(Some("pure")), NixState::Pure);
        assert_eq!(detect_state(Some("impure")), NixState::Impure);
        assert_eq!(detect_state(Some("weird")), NixState::Unknown);
        assert_eq!(detect_state(None), NixState::None);
        assert_eq!(detect_state(Some("")), NixState::None);
    }
}
