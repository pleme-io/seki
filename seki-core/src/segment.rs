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
}

impl Segment {
    pub fn new<S: Into<String>>(module: S) -> Self {
        Self {
            module: module.into(),
            fragments: Vec::new(),
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
