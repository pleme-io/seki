//! Runtime carrier handed to every [`crate::Module::render`] call.
//!
//! Modules receive a borrowed [`RenderContext`] containing the
//! current working directory, the last exit status, the operator's
//! username, and the seki-resolved typed config. Modules are
//! responsible for performing any filesystem / process / network
//! sniffing they need themselves (the context is intentionally
//! small — extension is by detect helpers, not by stuffing
//! everything into the context).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderContext {
    pub cwd: PathBuf,
    pub home: Option<PathBuf>,
    pub user: String,
    pub last_exit_code: i32,
    pub last_pipe_status: Vec<i32>,
    pub shell: Shell,
    /// Whether ANSI colour escapes should be emitted. False when
    /// the output is being captured (e.g. `seki prompt | cat`).
    pub enable_colors: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    Nu,
    Frostmourne,
    Plain,
}

impl Shell {
    pub fn as_str(self) -> &'static str {
        match self {
            Shell::Bash => "bash",
            Shell::Zsh => "zsh",
            Shell::Fish => "fish",
            Shell::Nu => "nu",
            Shell::Frostmourne => "frostmourne",
            Shell::Plain => "plain",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "bash" => Shell::Bash,
            "zsh" => Shell::Zsh,
            "fish" => Shell::Fish,
            "nu" => Shell::Nu,
            "frostmourne" | "frost" => Shell::Frostmourne,
            "plain" => Shell::Plain,
            _ => return None,
        })
    }
}

impl Default for Shell {
    fn default() -> Self {
        Shell::Plain
    }
}

impl RenderContext {
    /// Construct a context from the current process environment —
    /// the canonical entrypoint when invoked as `seki prompt`.
    pub fn from_env() -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let user = std::env::var("USER").unwrap_or_default();
        Self {
            cwd,
            home,
            user,
            last_exit_code: 0,
            last_pipe_status: Vec::new(),
            shell: Shell::Plain,
            enable_colors: true,
        }
    }

    pub fn with_cwd<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.cwd = path.as_ref().to_path_buf();
        self
    }

    pub fn with_shell(mut self, shell: Shell) -> Self {
        self.shell = shell;
        self
    }

    pub fn with_exit_code(mut self, code: i32) -> Self {
        self.last_exit_code = code;
        self
    }

    pub fn with_colors(mut self, enable: bool) -> Self {
        self.enable_colors = enable;
        self
    }
}
