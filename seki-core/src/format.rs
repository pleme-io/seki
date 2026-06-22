//! The ONE typed format-string interpreter every segment shares.
//!
//! Before this module existed, ~23 segment impls each carried a
//! near-identical private `render_format()` copy and four more
//! (`git_branch`, `rust`, `nix_shell`, `git_status`) ignored their
//! typed `format` entirely and hand-rolled `prefix + symbol + suffix`.
//! That is the exact "solve the same problem in N places" debt the
//! prime directive forbids (Pillar 12 — generation over composition):
//! a recurring impl shape that should be one typed surface. This is
//! that surface. Every module renders its typed `format` field through
//! [`render`]; the per-module copies are deleted.
//!
//! ## Grammar (a faithful subset of starship's format language)
//!
//! | Syntax        | Meaning                                                   |
//! |---------------|-----------------------------------------------------------|
//! | `$name`       | variable — expands via the supplied lookup                |
//! | `${name}`     | braced variable — lets `${count}3` parse cleanly          |
//! | `[ … ]`       | text group — brackets are stripped, inner always emitted  |
//! | `[ … ](spec)` | the `(spec)` directly after `]` is a *style spec*, skipped |
//! | `( … )`       | *conditional* group — emitted only if a contained variable |
//! |               | expanded to a non-empty string (starship's `(…)`-optional) |
//! | `\c`          | literal escape — the next char is emitted verbatim         |
//!
//! Colour is NOT encoded here — a [`crate::Segment`] carries one
//! [`crate::Style`] resolved from the module's typed `style` field, so
//! the `($style)` markup is parsed-and-discarded (the style lives on
//! the fragment, not in the text). Unknown variables expand to empty,
//! matching starship's permissive behaviour.
//!
//! No `format!()` here — every byte is composed via `push`/`push_str`,
//! per the crate's TYPED EMISSION rule.

/// Render a typed format string against a variable `lookup`.
///
/// `lookup(name)` returns `Some(value)` for a known variable (empty
/// `value` is allowed and counts as "present but empty" for the
/// conditional-group emptiness test) or `None` for an unknown name.
pub fn render(fmt: &str, lookup: impl Fn(&str) -> Option<String>) -> String {
    let chars: Vec<char> = fmt.chars().collect();
    let mut i = 0;
    render_seq(&chars, &mut i, &lookup, None).0
}

/// Convenience for the common single-variable segment (`directory`,
/// `hostname`, `cmd_duration`, every `custom`/`env_var`-shaped module).
pub fn render_one(fmt: &str, name: &str, value: &str) -> String {
    render(fmt, |n| (n == name).then(|| value.to_owned()))
}

/// Convenience for a fixed set of `(name, value)` pairs.
pub fn render_vars(fmt: &str, vars: &[(&str, &str)]) -> String {
    render(fmt, |n| {
        vars.iter()
            .find(|(k, _)| *k == n)
            .map(|(_, v)| (*v).to_owned())
    })
}

/// Render the sequence of chars starting at `*i` until either `stop`
/// (the closing `]` / `)` of an enclosing group) or end-of-input.
///
/// Returns the rendered text plus whether **any** variable inside
/// expanded to a non-empty value — the signal a conditional `(…)`
/// group uses to decide whether it renders at all.
fn render_seq(
    chars: &[char],
    i: &mut usize,
    lookup: &impl Fn(&str) -> Option<String>,
    stop: Option<char>,
) -> (String, bool) {
    let mut out = String::new();
    let mut any_nonempty = false;
    while *i < chars.len() {
        let c = chars[*i];
        if Some(c) == stop {
            *i += 1; // consume the closer
            return (out, any_nonempty);
        }
        match c {
            '\\' => {
                // Literal escape: emit the next char verbatim.
                *i += 1;
                if *i < chars.len() {
                    out.push(chars[*i]);
                    *i += 1;
                }
            }
            '$' => {
                *i += 1;
                let name = read_var_name(chars, i);
                if let Some(value) = lookup(&name) {
                    if !value.is_empty() {
                        any_nonempty = true;
                    }
                    out.push_str(&value);
                }
            }
            '[' => {
                // Text group: strip the brackets, always emit the
                // inner content, then skip an immediately-following
                // `(style spec)`. Propagate the inner emptiness so an
                // enclosing conditional group can see it.
                *i += 1;
                let (inner, inner_any) = render_seq(chars, i, lookup, Some(']'));
                skip_style_spec(chars, i);
                out.push_str(&inner);
                any_nonempty |= inner_any;
            }
            '(' => {
                // Conditional group: emit the inner content only when a
                // contained variable expanded non-empty.
                *i += 1;
                let (inner, inner_any) = render_seq(chars, i, lookup, Some(')'));
                if inner_any {
                    out.push_str(&inner);
                    any_nonempty = true;
                }
            }
            _ => {
                out.push(c);
                *i += 1;
            }
        }
    }
    (out, any_nonempty)
}

/// Read a variable name at `*i` — either `$name` (`[A-Za-z0-9_]+`) or
/// the braced `${name}` form. `*i` is advanced past the name (and the
/// closing `}` for the braced form).
fn read_var_name(chars: &[char], i: &mut usize) -> String {
    let mut name = String::new();
    if *i < chars.len() && chars[*i] == '{' {
        *i += 1; // consume '{'
        while *i < chars.len() && chars[*i] != '}' {
            name.push(chars[*i]);
            *i += 1;
        }
        if *i < chars.len() {
            *i += 1; // consume '}'
        }
    } else {
        while *i < chars.len() {
            let c = chars[*i];
            if c.is_ascii_alphanumeric() || c == '_' {
                name.push(c);
                *i += 1;
            } else {
                break;
            }
        }
    }
    name
}

/// If a balanced `(…)` style spec sits at `*i` (i.e. immediately after
/// a closed `]` text group), consume it. Colour markup like
/// `(bold #EBCB8B)` is discarded — the style is carried on the
/// [`crate::Segment`], not in the text.
fn skip_style_spec(chars: &[char], i: &mut usize) {
    if *i >= chars.len() || chars[*i] != '(' {
        return;
    }
    let mut depth = 0usize;
    while *i < chars.len() {
        let c = chars[*i];
        *i += 1;
        if c == '(' {
            depth += 1;
        } else if c == ')' {
            depth -= 1;
            if depth == 0 {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Parity with the deleted per-module `render_format` copies ──

    #[test]
    fn single_var_strips_markup() {
        // custom / env_var / hostname / cmd_duration shape.
        assert_eq!(render_one("[· $output]($style) ", "output", "abcdef"), "· abcdef ");
        assert_eq!(render_one("[$duration]($style) ", "duration", "5s"), "5s ");
        assert_eq!(
            render_one("[$hostname](dimmed $style) · ", "hostname", "rio"),
            "rio · "
        );
    }

    #[test]
    fn escaped_brackets_are_literal() {
        // env_var WORKSPACE shape: `[\[$env_value\]]($style) `.
        assert_eq!(
            render_one("[\\[$env_value\\]]($style) ", "env_value", "pleme-io"),
            "[pleme-io] "
        );
    }

    #[test]
    fn multi_var_lookup() {
        // tear shape: session + pane.
        assert_eq!(
            render_vars("[~ $session·$pane]($style) ", &[("session", "main"), ("pane", "ab")]),
            "~ main·ab "
        );
    }

    #[test]
    fn braced_var_with_trailing_text() {
        // git_status ahead glyph: `⇡${count}`.
        assert_eq!(render_one("⇡${count}", "count", "3"), "⇡3");
        assert_eq!(
            render_vars("⇕${ahead_count}⇣${behind_count}", &[("ahead_count", "2"), ("behind_count", "1")]),
            "⇕2⇣1"
        );
    }

    #[test]
    fn unknown_var_expands_empty() {
        assert_eq!(render_one("a$nope b", "x", "1"), "a b");
    }

    // ── Conditional groups (the behaviour the copies got wrong) ──

    #[test]
    fn conditional_group_renders_when_var_present() {
        // nix_shell shape: `via [$symbol$state( \($name\))]($style) `.
        let fmt = "via [$symbol$state( \\($name\\))]($style) ";
        assert_eq!(
            render_vars(fmt, &[("symbol", "❄ "), ("state", "pure"), ("name", "devshell")]),
            "via ❄ pure (devshell) "
        );
    }

    #[test]
    fn conditional_group_drops_when_var_empty() {
        let fmt = "via [$symbol$state( \\($name\\))]($style) ";
        // No `$name` → the ` (…)` group disappears entirely.
        assert_eq!(
            render_vars(fmt, &[("symbol", "❄ "), ("state", "pure"), ("name", "")]),
            "via ❄ pure "
        );
    }

    #[test]
    fn git_status_default_format_is_empty_when_clean() {
        // starship's git_status default — the outer `(…)` swallows the
        // literal `[]` + trailing space when every status var is empty.
        let fmt = "([\\[$all_status$ahead_behind\\]]($style) )";
        assert_eq!(
            render_vars(fmt, &[("all_status", ""), ("ahead_behind", "")]),
            ""
        );
    }

    #[test]
    fn git_status_default_format_renders_when_dirty() {
        let fmt = "([\\[$all_status$ahead_behind\\]]($style) )";
        assert_eq!(
            render_vars(fmt, &[("all_status", "🟡🟢"), ("ahead_behind", "⇡1")]),
            "[🟡🟢⇡1] "
        );
    }

    #[test]
    fn nested_groups() {
        // A conditional inside a text group, both populated.
        assert_eq!(
            render_vars("[$a($b)]", &[("a", "x"), ("b", "y")]),
            "xy"
        );
        // Inner conditional empty → dropped, text group keeps `$a`.
        assert_eq!(render_vars("[$a($b)]", &[("a", "x"), ("b", "")]), "x");
    }

    #[test]
    fn plain_text_passes_through() {
        assert_eq!(render("hello world", |_| None), "hello world");
    }
}
