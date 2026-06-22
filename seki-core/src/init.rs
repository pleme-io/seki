//! Typed shell-init snippets — `seki init <shell>`.
//!
//! Each supported shell maps to a single [`InitScript`] value. The
//! script body is built from typed pieces (the shell name, the seki
//! binary name, a documentation header) and rendered through
//! [`InitScript::render`] — no `format!()` of shell syntax outside
//! this module's `Display` impls, per the fleet-wide typed-emission
//! rule.
//!
//! Two integration shapes are emitted today:
//!
//! 1. **frostmourne** — emits a `(defprompt :command "seki prompt …")`
//!    Lisp form. frostmourne's `defprompt` already supports
//!    `:command`; the form synthesizes a precmd hook that captures
//!    `seki prompt`'s stdout into `PS1` on every redraw (see
//!    `frostmourne/lisp/63-tools-starship.lisp` for the pre-existing
//!    starship integration this mirrors).
//!
//! 2. **bash / zsh / fish** — emits the equivalent native hook
//!    (`PROMPT_COMMAND` / `precmd_functions` / `fish_prompt`) so seki
//!    works on hosts not running frostmourne. Always passes
//!    `--status "$?"` (or fish's `$status`) so the typed
//!    `RenderContext::last_exit_code` is populated.
//!
//! Adding a new shell = one variant + one `render_*` arm. No string
//! munging at the call site.

use crate::context::Shell;
use std::fmt;

/// Operator-facing render of a shell-init script. Implements
/// [`fmt::Display`] so callers can `println!("{}", script)` or
/// write to any `io::Write` via `script.to_string()`.
pub struct InitScript {
    shell: Shell,
    /// Binary name to invoke — usually `"seki"` but injectable for
    /// tests / wrappers / nix derivations.
    bin: String,
}

impl InitScript {
    /// Construct an init script for `shell`, invoking `bin` per
    /// prompt redraw. `bin` is typically the bare string `"seki"`;
    /// pass an absolute path when wiring through a wrapper that
    /// can't rely on `$PATH`.
    pub fn new<S: Into<String>>(shell: Shell, bin: S) -> Self {
        Self {
            shell,
            bin: bin.into(),
        }
    }

    /// Convenience: render the canonical `seki` invocation for the
    /// shell — equivalent to `InitScript::new(shell, "seki")`.
    pub fn canonical(shell: Shell) -> Self {
        Self::new(shell, "seki")
    }

    /// The shell this script targets.
    pub fn shell(&self) -> Shell {
        self.shell
    }
}

impl fmt::Display for InitScript {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.shell {
            Shell::Frostmourne => render_frostmourne(f, &self.bin),
            Shell::Bash => render_bash(f, &self.bin),
            Shell::Zsh => render_zsh(f, &self.bin),
            Shell::Fish => render_fish(f, &self.bin),
            Shell::Nu => render_nu(f, &self.bin),
            Shell::Plain => render_plain(f, &self.bin),
        }
    }
}

fn render_frostmourne(f: &mut fmt::Formatter<'_>, bin: &str) -> fmt::Result {
    writeln!(f, ";; seki :: prompt integration (auto-generated)")?;
    writeln!(f, ";; Synthesizes a `precmd` hook that captures")?;
    writeln!(f, ";; `{bin} prompt`'s stdout into PS1 each redraw.")?;
    writeln!(f, ";; Last `defprompt` form loaded wins — drop")?;
    writeln!(f, ";; lisp/10-prompt.lisp from your layer to use this.")?;
    writeln!(f, ";;")?;
    writeln!(f, ";; Never-stale refresh: SEKI_DAEMON=auto brings up the")?;
    writeln!(f, ";; FS-watch hot-status daemon after the first prompt, so")?;
    writeln!(f, ";; git status stays fresh + instant fleet-wide. `seki")?;
    writeln!(f, ";; prompt` falls back to a live fork whenever it's absent.")?;
    writeln!(f, "(defenv :name \"SEKI_DAEMON\" :value \"auto\" :export #t)")?;
    writeln!(f, "(defprompt")?;
    writeln!(
        f,
        "  :command \"{bin} prompt --status=\\\"$?\\\" --shell=frostmourne\")"
    )
}

fn render_bash(f: &mut fmt::Formatter<'_>, bin: &str) -> fmt::Result {
    writeln!(f, "# seki :: prompt integration (auto-generated)")?;
    writeln!(f, "# Add to your ~/.bashrc:  eval \"$({bin} init bash)\"")?;
    writeln!(f, "__seki_prompt() {{")?;
    writeln!(f, "  local _seki_status=$?")?;
    writeln!(
        f,
        "  PS1=\"$({bin} prompt --status=\"$_seki_status\" --shell=bash)\""
    )?;
    writeln!(f, "}}")?;
    writeln!(f, "case \"$PROMPT_COMMAND\" in")?;
    writeln!(f, "  *__seki_prompt*) ;;")?;
    writeln!(
        f,
        "  *) PROMPT_COMMAND=\"__seki_prompt${{PROMPT_COMMAND:+;$PROMPT_COMMAND}}\" ;;"
    )?;
    writeln!(f, "esac")
}

fn render_zsh(f: &mut fmt::Formatter<'_>, bin: &str) -> fmt::Result {
    writeln!(f, "# seki :: prompt integration (auto-generated)")?;
    writeln!(f, "# Add to your ~/.zshrc:  eval \"$({bin} init zsh)\"")?;
    writeln!(f, "__seki_prompt() {{")?;
    writeln!(f, "  local _seki_status=$?")?;
    writeln!(
        f,
        "  PROMPT=\"$({bin} prompt --status=\"$_seki_status\" --shell=zsh)\""
    )?;
    writeln!(f, "}}")?;
    writeln!(f, "autoload -Uz add-zsh-hook")?;
    writeln!(f, "add-zsh-hook precmd __seki_prompt")
}

fn render_fish(f: &mut fmt::Formatter<'_>, bin: &str) -> fmt::Result {
    writeln!(f, "# seki :: prompt integration (auto-generated)")?;
    writeln!(
        f,
        "# Add to your config.fish:  {bin} init fish | source"
    )?;
    writeln!(f, "function fish_prompt")?;
    writeln!(f, "    set -l _seki_status $status")?;
    writeln!(
        f,
        "    {bin} prompt --status=\"$_seki_status\" --shell=fish"
    )?;
    writeln!(f, "end")
}

fn render_nu(f: &mut fmt::Formatter<'_>, bin: &str) -> fmt::Result {
    writeln!(f, "# seki :: prompt integration (auto-generated)")?;
    writeln!(f, "# Add to your config.nu:  source ({bin} init nu | save -f /tmp/seki-init.nu; '/tmp/seki-init.nu')")?;
    writeln!(f, "$env.PROMPT_COMMAND = {{|| ")?;
    writeln!(
        f,
        "    {bin} prompt --status=($env.LAST_EXIT_CODE? | default 0) --shell=nu"
    )?;
    writeln!(f, "}}")
}

fn render_plain(f: &mut fmt::Formatter<'_>, bin: &str) -> fmt::Result {
    writeln!(f, "# seki :: plain shell (no integration hook).")?;
    writeln!(f, "# Invoke directly:  {bin} prompt")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frostmourne_script_emits_defprompt_command() {
        let s = InitScript::canonical(Shell::Frostmourne).to_string();
        assert!(s.contains("(defprompt"), "missing defprompt form: {s}");
        assert!(s.contains(":command"), "missing :command slot: {s}");
        assert!(s.contains("seki prompt"), "missing renderer cmd: {s}");
        assert!(
            s.contains("--shell=frostmourne"),
            "missing shell hint: {s}"
        );
    }

    #[test]
    fn bash_script_wires_prompt_command_idempotently() {
        let s = InitScript::canonical(Shell::Bash).to_string();
        assert!(s.contains("__seki_prompt"), "missing hook fn: {s}");
        assert!(s.contains("PROMPT_COMMAND"), "missing PROMPT_COMMAND: {s}");
        assert!(
            s.contains("*__seki_prompt*"),
            "must guard against double-install: {s}"
        );
    }

    #[test]
    fn zsh_script_uses_add_zsh_hook_precmd() {
        let s = InitScript::canonical(Shell::Zsh).to_string();
        assert!(s.contains("add-zsh-hook precmd"), "missing zsh hook: {s}");
    }

    #[test]
    fn fish_script_defines_fish_prompt_function() {
        let s = InitScript::canonical(Shell::Fish).to_string();
        assert!(s.contains("function fish_prompt"), "missing fish fn: {s}");
        assert!(s.contains("set -l _seki_status $status"));
    }

    #[test]
    fn custom_bin_substituted_into_all_shells() {
        for shell in [Shell::Bash, Shell::Zsh, Shell::Fish, Shell::Frostmourne] {
            let s = InitScript::new(shell, "/nix/store/abc/bin/seki").to_string();
            assert!(
                s.contains("/nix/store/abc/bin/seki"),
                "{shell:?} script missing custom bin path: {s}",
            );
        }
    }
}
