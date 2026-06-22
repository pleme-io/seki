//! seki — typed prompt renderer CLI.
//!
//! Mirrors the parts of starship's CLI that actually matter:
//!
//! ```text
//! seki init <SHELL>     # emit shell-init snippet
//! seki prompt           # render the prompt
//! seki module <NAME>    # render one module — for debugging
//! seki config-show TIER # shikumi prime directive
//! ```
//!
//! Per the fleet rule: `anyhow` is reserved for this `main.rs` glue,
//! every other crate uses `thiserror`. The shell-init snippet is
//! built from the typed [`seki_core::InitScript`] (`Display` impl)
//! — no `format!()` of shell-script syntax here.

use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use seki_core::{InitScript, RenderContext, Shell, render_prompt};
use seki_modules::default_registry;
use seki_shikumi::TieredSekiConfig;
use shikumi::cli::ConfigShowCommand;

#[cfg(unix)]
mod daemon;

#[derive(Parser, Debug)]
#[command(name = "seki", version, about = "席 — typed prompt renderer")]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Emit shell-init snippet for the requested shell.
    Init(InitArgs),
    /// Render the prompt for the current process environment.
    Prompt(PromptArgs),
    /// Render a single module — for debugging.
    Module(ModuleArgs),
    /// Show the materialized config at a tier (bare/default/…).
    ConfigShow(ConfigShowCommand),
    /// Run the refresh daemon — an FS-watch hot status cache that keeps
    /// git status fresh + instant. Start it once per session (e.g. from
    /// the shell rc, or via `SEKI_DAEMON=auto`); `seki prompt` reads it
    /// when present and falls back to a live fork otherwise.
    Daemon,
}

#[derive(clap::Args, Debug)]
struct InitArgs {
    /// One of `bash`, `zsh`, `fish`, `nu`, `frostmourne`.
    shell: String,
}

#[derive(clap::Args, Debug)]
struct PromptArgs {
    /// Last exit code from the calling shell.
    #[arg(long, default_value_t = 0)]
    status: i32,
    /// Hint of which shell is consuming us (informational; affects
    /// per-shell glyph choices in future modules).
    #[arg(long, default_value = "plain")]
    shell: String,
    /// Disable ANSI colour even when stdout is a terminal.
    #[arg(long)]
    no_color: bool,
}

#[derive(clap::Args, Debug)]
struct ModuleArgs {
    name: String,
    #[arg(long)]
    no_color: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        None => {
            // Bare `seki` with no subcommand — print help and exit 0.
            // Operators running on a fresh host (no config, no env)
            // get a clean response, never a "missing subcommand"
            // error. `--help` works identically.
            use clap::CommandFactory;
            Cli::command().print_help().ok();
            println!();
            Ok(())
        }
        Some(Commands::Init(args)) => run_init(&args),
        Some(Commands::Prompt(args)) => run_prompt(&args),
        Some(Commands::Module(args)) => run_module(&args),
        Some(Commands::ConfigShow(cmd)) => {
            cmd.run::<TieredSekiConfig>("SEKI_TIER")?;
            Ok(())
        }
        Some(Commands::Daemon) => run_daemon(),
    }
}

#[cfg(unix)]
fn run_daemon() -> Result<()> {
    daemon::run()
}

#[cfg(not(unix))]
fn run_daemon() -> Result<()> {
    Err(anyhow!("the seki refresh daemon requires a unix platform"))
}

fn run_init(args: &InitArgs) -> Result<()> {
    let shell = Shell::parse(&args.shell).ok_or_else(|| anyhow!("unknown shell: {}", args.shell))?;
    if matches!(shell, Shell::Plain) {
        return Err(anyhow!("`plain` is not a real shell to init"));
    }
    let script = InitScript::canonical(shell);
    print!("{script}");
    Ok(())
}

fn run_prompt(args: &PromptArgs) -> Result<()> {
    let cfg = seki_shikumi::load_from_env();
    let registry = default_registry(&cfg);
    let shell = Shell::parse(&args.shell).unwrap_or(Shell::Plain);
    let ctx = RenderContext::from_env()
        .with_exit_code(args.status)
        .with_shell(shell)
        .with_colors(!args.no_color);
    let rendered = render_prompt(&cfg, &registry, &ctx)?;
    print!("{}", rendered.raw);
    // Opt-in (`SEKI_DAEMON=auto`): after the first prompt renders live,
    // bring up the FS-watch hot-cache daemon so every subsequent prompt
    // this session reads a hot, never-stale status instantly.
    #[cfg(unix)]
    daemon::maybe_autostart();
    Ok(())
}

fn run_module(args: &ModuleArgs) -> Result<()> {
    let cfg = seki_shikumi::load_from_env();
    let registry = default_registry(&cfg);
    let ctx = RenderContext::from_env().with_colors(!args.no_color);
    if let Some(module) = registry.get(&args.name) {
        if let Some(segment) = module.render(&ctx)? {
            for fragment in &segment.fragments {
                print!(
                    "{}",
                    seki_core::style::apply(&fragment.text, &fragment.style, ctx.enable_colors),
                );
            }
            println!();
        }
    } else {
        anyhow::bail!("unknown module: {}", args.name);
    }
    Ok(())
}
