//! Typed config for the `git_status` segment.
//!
//! Mirrors starship's `[git_status]` table in full so blzsh-parity
//! values render verbatim. The M1 module impl reads a coarse
//! Clean/Modified/Conflicted classification — additional symbols
//! (`ahead`, `behind`, `stashed`, …) become live as the M2 module
//! grows. Storing them now means the typed config is right even
//! when the rendering surface is still partial.

use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GitStatusConfig {
    pub enabled: bool,
    /// Format string. Default: `"[$all_status$ahead_behind]($style)"`.
    pub format: String,
    pub style: StyleSpec,
    pub stashed: String,
    pub ahead: String,
    pub behind: String,
    pub diverged: String,
    pub conflicted: String,
    pub deleted: String,
    pub renamed: String,
    pub modified: String,
    pub staged: String,
    pub untracked: String,
    pub up_to_date: String,
    /// Rendered when the branch has NO upstream — distinct from
    /// `up_to_date`, which claims agreement with a remote that does not
    /// exist. Default `⇡?`: the ahead glyph (work only you have) plus a
    /// question mark (nowhere to send it).
    pub no_upstream: String,
    /// Symbol shown when no other status applies; convenience for
    /// the M1 coarse classifier — starship doesn't model this
    /// explicitly. Falls back to `up_to_date` when empty.
    pub clean_symbol: String,
    pub prefix: String,
    pub suffix: String,
}

impl Default for GitStatusConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            format: "([\\[$all_status$ahead_behind\\]]($style) )".to_owned(),
            style: StyleSpec::new("red bold"),
            stashed: r"\$".to_owned(),
            ahead: "⇡".to_owned(),
            behind: "⇣".to_owned(),
            diverged: "⇕".to_owned(),
            conflicted: "=".to_owned(),
            deleted: "✘".to_owned(),
            renamed: "»".to_owned(),
            modified: "!".to_owned(),
            staged: "+".to_owned(),
            untracked: "?".to_owned(),
            up_to_date: String::new(),
            no_upstream: "⇡?".to_owned(),
            clean_symbol: "✓".to_owned(),
            prefix: "[".to_owned(),
            suffix: "] ".to_owned(),
        }
    }
}

impl GitStatusConfig {
    pub fn bare() -> Self {
        Self {
            enabled: false,
            no_upstream: String::new(),
            format: String::new(),
            style: StyleSpec::default(),
            stashed: String::new(),
            ahead: String::new(),
            behind: String::new(),
            diverged: String::new(),
            conflicted: String::new(),
            deleted: String::new(),
            renamed: String::new(),
            modified: String::new(),
            staged: String::new(),
            untracked: String::new(),
            up_to_date: String::new(),
            clean_symbol: String::new(),
            prefix: String::new(),
            suffix: String::new(),
        }
    }
}
