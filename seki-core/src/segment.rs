//! Typed result of a single [`Module::render`] call.
//!
//! A [`Segment`] is the pure-data carrier handed back to the
//! renderer — it knows nothing about the terminal. Conversion to
//! ANSI-escaped output happens in [`crate::render`] via
//! [`crate::style::apply`]. Empty segments (`fragments == []`) are
//! filtered out of the prompt — that's how a module "decides not to
//! render itself".

use crate::style::Style;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyledFragment {
    pub text: String,
    pub style: Style,
}

impl StyledFragment {
    pub fn new<S: Into<String>>(text: S, style: Style) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }

    pub fn plain<S: Into<String>>(text: S) -> Self {
        Self::new(text, Style::default())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Segment {
    /// Module name (`directory`, `git_branch`, …) — purely for
    /// debugging + the `seki module <NAME>` subcommand.
    pub module: String,
    pub fragments: Vec<StyledFragment>,
    /// ── ★ "I MUST NOT ABUT THE SEGMENT BEFORE ME" ────────────────────
    /// Segments are concatenated with no separator, so each one owns its
    /// own trailing space by convention. That convention has a hole: it
    /// only works if every segment that MIGHT precede you actually
    /// rendered. `git_branch` emitted `main` with no trailing space and
    /// relied on `git_status` to supply one — and `git_status` correctly
    /// renders nothing in a clean repo, so the prompt read `mainvia ⊿`
    /// in every clean Rust repo in the fleet.
    ///
    /// A blanket "always separate" rule in the renderer is wrong: `main`
    /// and `!?` abut ON PURPOSE, and forcing a space there would be a
    /// regression. So separation is DECLARED by the segment that needs
    /// it, and applied only when the accumulated output does not already
    /// end in whitespace — which makes it idempotent and leaves every
    /// deliberate adjacency untouched.
    #[serde(default)]
    pub needs_leading_space: bool,
}

impl Segment {
    /// Declare that this segment must not run into whatever precedes it.
    #[must_use]
    pub fn with_leading_space(mut self) -> Self {
        self.needs_leading_space = true;
        self
    }

    pub fn new<S: Into<String>>(module: S) -> Self {
        Self {
            module: module.into(),
            fragments: Vec::new(),
            // Off by default: abutting is the existing behaviour and some
            // of it is deliberate (`main` + `!?`). A segment opts in.
            needs_leading_space: false,
        }
    }

    pub fn push(mut self, fragment: StyledFragment) -> Self {
        self.fragments.push(fragment);
        self
    }

    /// Total visible-character count — useful for prompts that want
    /// to right-align or line-wrap. Uses `unicode-width` so wide
    /// CJK glyphs and emoji count correctly.
    pub fn visible_width(&self) -> usize {
        use unicode_width::UnicodeWidthStr;
        self.fragments.iter().map(|f| f.text.width()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.fragments.is_empty()
    }
}
