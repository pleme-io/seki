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

/// Apply a typed [`Style`] to a piece of text, returning an
/// ANSI-escaped string. Honours `seki`'s `ascii_only` mode: when
/// `enable_colors == false`, returns the raw text untouched.
pub fn apply(text: &str, style: &Style, enable_colors: bool) -> String {
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

    text.paint(ystyle).to_string()
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
        let out = apply("hello", &Style::fg(Color::Red).with_bold(), false);
        assert_eq!(out, "hello");
    }

    #[test]
    fn apply_with_color_wraps_text() {
        let out = apply("hi", &Style::fg(Color::Red), true);
        assert!(out.contains("hi"));
        // ANSI escape introducer.
        assert!(out.starts_with("\x1b["));
    }
}
