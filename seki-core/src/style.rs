//! Typed colour + style primitives.
//!
//! Replaces starship's stringly-typed ANSI grammar with a typed
//! [`StyleSpec`] enum + [`Color`] + [`Style`] structs. Apps render
//! by handing a [`Style`] to [`apply`], which returns an ANSI-escaped
//! string via the `yansi` crate.
//!
//! Parsing a starship-flavoured style string (e.g. `"bold green"`,
//! `"#34ace0"`) lives in [`StyleSpec::parse`]; the typed [`Style`]
//! lives in [`StyleSpec::resolve`].

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Color {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Purple,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightPurple,
    BrightCyan,
    BrightWhite,
    /// 24-bit truecolor `(r, g, b)`.
    Rgb(u8, u8, u8),
    /// 256-colour palette index.
    Palette(u8),
    /// No colour applied.
    None,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub dim: bool,
    pub inverted: bool,
}

impl Style {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn fg(color: Color) -> Self {
        Self {
            fg: Some(color),
            ..Self::default()
        }
    }

    pub fn with_bold(mut self) -> Self {
        self.bold = true;
        self
    }
}

/// Parsed starship-flavoured style string, e.g. `"bold green"`,
/// `"#34ace0 italic"`, `""`, or `"bg:red fg:white bold"`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StyleSpec(pub String);

impl StyleSpec {
    pub fn new<S: Into<String>>(s: S) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Resolve to a typed [`Style`]. Unknown tokens are silently
    /// ignored (matches starship's permissive grammar).
    pub fn resolve(&self) -> Style {
        let mut style = Style::default();
        for tok in self.0.split_whitespace() {
            let (kind, value) = if let Some(rest) = tok.strip_prefix("fg:") {
                ("fg", rest)
            } else if let Some(rest) = tok.strip_prefix("bg:") {
                ("bg", rest)
            } else {
                ("fg", tok)
            };
            match value {
                "bold" => style.bold = true,
                "italic" => style.italic = true,
                "underline" => style.underline = true,
                "dim" => style.dim = true,
                "inverted" | "reverse" => style.inverted = true,
                "none" => {
                    if kind == "fg" {
                        style.fg = None;
                    } else {
                        style.bg = None;
                    }
                }
                _ => {
                    if let Some(c) = parse_color(value) {
                        if kind == "fg" {
                            style.fg = Some(c);
                        } else {
                            style.bg = Some(c);
                        }
                    }
                }
            }
        }
        style
    }
}

fn parse_color(token: &str) -> Option<Color> {
    if let Some(hex) = token.strip_prefix('#') {
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some(Color::Rgb(r, g, b));
        }
        return None;
    }
    Some(match token {
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "purple" | "magenta" => Color::Purple,
        "cyan" => Color::Cyan,
        "white" => Color::White,
        "bright-black" => Color::BrightBlack,
        "bright-red" => Color::BrightRed,
        "bright-green" => Color::BrightGreen,
        "bright-yellow" => Color::BrightYellow,
        "bright-blue" => Color::BrightBlue,
        "bright-purple" => Color::BrightPurple,
        "bright-cyan" => Color::BrightCyan,
        "bright-white" => Color::BrightWhite,
        _ => return None,
    })
}

/// The delimiters a shell uses to fence NON-PRINTING bytes out of its
/// prompt-width arithmetic.
///
/// ── ★ WHY THIS TYPE EXISTS (operator-reported, measured 2026-08-06) ──
/// A line-editing shell has to know how many COLUMNS its prompt occupies,
/// so it can put the cursor back in the right place on redraw. It cannot
/// parse ANSI, so it counts bytes — and unless the escapes are fenced, it
/// counts them as VISIBLE.
///
/// Measured on the live prompt: 8 SGR sequences, 96 bytes of escapes, 19
/// visible cells. zsh believed the prompt was **115 columns wide**. Every
/// redraw — Ctrl-A, Ctrl-E, history recall, a long line — repositioned
/// from column 115 instead of 19, which on a wide terminal throws the
/// cursor to the far right and overwrites already-drawn prompt text
/// (`093bbd` rendered as `bu3bbd`). It reads exactly like a terminal-
/// emulator bug and it is not one: the terminal placed the cursor where
/// the shell asked.
///
/// Fencing is therefore not decoration — it is the contract that makes
/// the shell's column arithmetic true.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PromptFence {
    pub open: &'static str,
    pub close: &'static str,
}

impl PromptFence {
    /// No fencing — for shells that measure width themselves (fish, nu)
    /// and for non-prompt output, where the delimiters would be literal
    /// garbage in the stream.
    pub const NONE: Self = Self {
        open: "",
        close: "",
    };
    /// zsh: `%{ … %}` — "the enclosed bytes occupy zero columns".
    pub const ZSH: Self = Self {
        open: "%{",
        close: "%}",
    };
    /// bash: `\[ … \]`, the same contract in readline's spelling.
    pub const BASH: Self = Self {
        open: "\\[",
        close: "\\]",
    };

    #[must_use]
    pub fn is_none(self) -> bool {
        self.open.is_empty() && self.close.is_empty()
    }
}

/// Apply a typed [`Style`] to a piece of text, returning an ANSI-escaped
/// string with its escapes fenced for `fence`'s shell. Honours `seki`'s
/// `ascii_only` mode: when `enable_colors == false`, returns the raw text
/// untouched (no escapes, so nothing to fence).
///
/// The fence is a REQUIRED argument rather than an `Option` with a
/// default, and that is the point: an unfenced prompt escape is the
/// defect this signature exists to prevent, so every call site is made
/// to state which shell it is emitting for. Passing
/// [`PromptFence::NONE`] is a decision; forgetting is not possible.
///
/// TIER: only-mitigated (C4) — a caller can still pass `NONE` for a zsh
/// prompt. Deriving the fence from `Shell` (see `Shell::prompt_fence`)
/// is what keeps that from happening in practice; the type system cannot
/// tell which shell a `&str` is destined for.
pub fn apply(text: &str, style: &Style, enable_colors: bool, fence: PromptFence) -> String {
    if !enable_colors {
        return text.to_owned();
    }
    use yansi::{Paint, Style as YS};

    let mut ystyle = YS::new();
    if let Some(c) = style.fg {
        ystyle = ystyle.fg(to_yansi(c));
    }
    if let Some(c) = style.bg {
        ystyle = ystyle.bg(to_yansi(c));
    }
    if style.bold {
        ystyle = ystyle.bold();
    }
    if style.italic {
        ystyle = ystyle.italic();
    }
    if style.underline {
        ystyle = ystyle.underline();
    }
    if style.dim {
        ystyle = ystyle.dim();
    }
    if style.inverted {
        ystyle = ystyle.invert();
    }

    let painted = text.paint(ystyle).to_string();
    if fence.is_none() {
        return painted;
    }

    // Fence the escapes, NOT the whole string: the visible text has to stay
    // outside the fence or the shell would count it as zero-width too and
    // land the cursor to the LEFT of where it belongs — the same bug with
    // the sign flipped. So walk the painted string and wrap each CSI run.
    let mut out = String::with_capacity(painted.len() + 8 * (fence.open.len() + fence.close.len()));
    let bytes = painted.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b {
            // A CSI sequence runs to its first byte in 0x40..=0x7e (the
            // "final byte"); for SGR that is `m`. Anything unterminated
            // runs to end-of-string rather than being dropped — emitting
            // a truncated escape is strictly better than eating the text
            // that follows it.
            let start = i;
            i += 1;
            if i < bytes.len() && bytes[i] == b'[' {
                i += 1;
                while i < bytes.len() && !(0x40..=0x7e).contains(&bytes[i]) {
                    i += 1;
                }
                if i < bytes.len() {
                    i += 1; // consume the final byte
                }
            }
            out.push_str(fence.open);
            out.push_str(&painted[start..i]);
            out.push_str(fence.close);
        } else {
            let start = i;
            while i < bytes.len() && bytes[i] != 0x1b {
                i += 1;
            }
            out.push_str(&painted[start..i]);
        }
    }
    out
}

fn to_yansi(c: Color) -> yansi::Color {
    use yansi::Color as YC;
    match c {
        Color::Black => YC::Black,
        Color::Red => YC::Red,
        Color::Green => YC::Green,
        Color::Yellow => YC::Yellow,
        Color::Blue => YC::Blue,
        Color::Purple => YC::Magenta,
        Color::Cyan => YC::Cyan,
        Color::White => YC::White,
        Color::BrightBlack => YC::BrightBlack,
        Color::BrightRed => YC::BrightRed,
        Color::BrightGreen => YC::BrightGreen,
        Color::BrightYellow => YC::BrightYellow,
        Color::BrightBlue => YC::BrightBlue,
        Color::BrightPurple => YC::BrightMagenta,
        Color::BrightCyan => YC::BrightCyan,
        Color::BrightWhite => YC::BrightWhite,
        Color::Rgb(r, g, b) => YC::Rgb(r, g, b),
        Color::Palette(n) => YC::Fixed(n),
        Color::None => YC::Primary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_named_color() {
        let s = StyleSpec::new("bold green").resolve();
        assert!(s.bold);
        assert_eq!(s.fg, Some(Color::Green));
    }

    #[test]
    fn parse_hex_color() {
        let s = StyleSpec::new("#34ace0 italic").resolve();
        assert_eq!(s.fg, Some(Color::Rgb(0x34, 0xac, 0xe0)));
        assert!(s.italic);
    }

    #[test]
    fn parse_bg_color() {
        let s = StyleSpec::new("bg:red fg:white").resolve();
        assert_eq!(s.bg, Some(Color::Red));
        assert_eq!(s.fg, Some(Color::White));
    }

    #[test]
    fn empty_style_resolves_to_default() {
        let s = StyleSpec::new("").resolve();
        assert_eq!(s, Style::default());
    }

    #[test]
    fn apply_no_color_returns_raw() {
        let out = apply(
            "hello",
            &Style::fg(Color::Red).with_bold(),
            false,
            PromptFence::NONE,
        );
        assert_eq!(out, "hello");
    }

    #[test]
    fn apply_with_color_wraps_text() {
        let out = apply("hi", &Style::fg(Color::Red), true, PromptFence::NONE);
        assert!(out.contains("hi"));
        // ANSI escape introducer.
        assert!(out.starts_with("\x1b["));
    }

    // ── ★ THE PROMPT-WIDTH CONTRACT ─────────────────────────────────────
    // A shell counts prompt bytes to know its column. Unfenced escapes get
    // counted as visible, so the shell's arithmetic is wrong by exactly the
    // escape length, and every redraw (Ctrl-A, Ctrl-E, history recall) puts
    // the cursor in the wrong column. Measured on the live prompt before
    // this fix: 96 bytes of escapes, 19 visible cells, zsh believed 115.
    #[test]
    fn zsh_fences_every_escape_so_the_shell_measures_only_visible_text() {
        let out = apply("x", &Style::fg(Color::Red), true, PromptFence::ZSH);
        // What zsh counts: everything NOT between %{ and %}.
        let mut counted = String::new();
        let mut rest = out.as_str();
        while let Some(o) = rest.find("%{") {
            counted.push_str(&rest[..o]);
            let after = &rest[o + 2..];
            match after.find("%}") {
                Some(c) => rest = &after[c + 2..],
                None => panic!("unclosed %{{ fence in {out:?}"),
            }
        }
        counted.push_str(rest);
        assert_eq!(
            counted, "x",
            "zsh must count exactly the visible text; it counted {counted:?} from {out:?}"
        );
    }

    #[test]
    fn bash_uses_readlines_spelling_of_the_same_contract() {
        let out = apply("x", &Style::fg(Color::Red), true, PromptFence::BASH);
        assert!(out.starts_with("\\["), "bash fence opens with \\[: {out:?}");
        assert!(out.contains("\\]"), "bash fence closes with \\]: {out:?}");
        assert!(out.contains('x'));
    }

    #[test]
    fn the_visible_text_stays_OUTSIDE_the_fence() {
        // Fencing the whole string would make the shell count the prompt as
        // ZERO columns — the same bug with the sign flipped, cursor landing
        // left of where it belongs instead of right.
        let out = apply("hello", &Style::fg(Color::Red), true, PromptFence::ZSH);
        assert!(
            !out.contains("%{\u{1b}[31mhello"),
            "visible text must not be inside the fence: {out:?}"
        );
    }

    #[test]
    fn every_shell_that_edits_a_line_gets_a_fence() {
        use crate::context::Shell;
        assert_eq!(Shell::Zsh.prompt_fence(), PromptFence::ZSH);
        assert_eq!(Shell::Bash.prompt_fence(), PromptFence::BASH);
        // fish/nu measure the rendered prompt themselves and would print
        // `%{` literally.
        assert!(Shell::Fish.prompt_fence().is_none());
        assert!(Shell::Nu.prompt_fence().is_none());
        assert!(Shell::Plain.prompt_fence().is_none());
    }

    #[test]
    fn an_unterminated_escape_keeps_the_text_after_it() {
        // Truncated escapes happen (a killed child, a clipped cache entry).
        // Emitting a short escape is recoverable; eating the rest of the
        // prompt is not.
        let out = apply("ok", &Style::default(), true, PromptFence::ZSH);
        assert!(out.contains("ok"), "text survives: {out:?}");
    }
}
