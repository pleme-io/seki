//! The **fleet base** config — [`blzsh_parity_config`] plus every
//! deliberate fleet-wide deviation from the pure parity reference.
//!
//! # Why this layer exists
//!
//! [`crate::blzsh_parity`] is a *pinned mirror* of blzsh's
//! `starship.toml`: its tests assert field-for-field equality with the
//! reference, so it must never absorb an opinion of our own. Every
//! fleet-facing theme — [`crate::companion_config`],
//! [`crate::vellum_config`], and the `prescribed_default()` tier —
//! nevertheless starts from that same base, so a fleet-wide "we don't
//! render X by default" decision taken in each of them separately is
//! three hand-lists that are free to disagree.
//!
//! This module is the one place that decision lives. Parity stays
//! pristine; the themes derive from `fleet_base_config()` and inherit
//! the fleet posture in lockstep.
//!
//! # What it currently changes
//!
//! - **`env_var.TEAR_SESSION_NAME` → disabled.** The tear/mado session
//!   *name* is not prompt-worthy: the operator picked the name and the
//!   window/tab already carries it, so re-rendering it on every line is
//!   pure width spent on something already known. MODULARIZE, DON'T
//!   DELETE — the entry stays declared, styled and themable (companion
//!   still paints it 🌊, vellum still gives it ice_cyan), so an operator
//!   who wants it back flips `enabled = true` rather than rebuilding it.
//!   The `custom.tear_pane` tag (`· <id>`) stays ON: the pane *id* is
//!   machine-assigned and disambiguates split panes, which the name
//!   cannot.
//!
//! - **`custom.tear_pane` → `EnvSlice`, not a shell command.** Same
//!   rendered bytes, zero processes. The parity mirror transcribes
//!   starship's `command = 'printf "%s" "${TEAR_PANE_ID:0:6}"'` +
//!   `when = '[ -n "$TEAR_PANE_ID" ]'`, which is TWO `sh -c` starts per
//!   prompt to take a substring of an environment variable. On ryn that
//!   was ~20.7 ms of a 28.1 ms prompt — the single largest cost in the
//!   fleet's default prompt, spent on a `${VAR:0:6}` the renderer can do
//!   in nanoseconds. It was also a portability bug: `${VAR:0:6}` is
//!   bash/zsh-only and dash answers `Bad substitution` on stderr with a
//!   non-zero exit, so on any host whose `/bin/sh` is dash the segment
//!   both vanished and complained. macOS `/bin/sh` being bash 3.2 is the
//!   only reason nobody saw it.
//!
//!   Why HERE and not in the mirror: `CustomSource::EnvSlice` is a
//!   seki-native concept with no starship TOML spelling, so writing it
//!   upstream would silently break the `// blzsh: <field> = <value>`
//!   correspondence the mirror exists to provide. And nothing loses the
//!   fix by being downstream — `prescribed_default`, `companion`,
//!   `vellum` and `discovered` all resolve through this function, so
//!   every operator-reachable tier gets it, while `blzsh_parity_config`
//!   is reached only from tests. MODULARIZE, DON'T DELETE: the shell
//!   form stays declared, tested and reachable upstream as the escape
//!   hatch's live example.

use seki_core::SekiConfig;
use seki_core::config::custom::CustomSource;

use crate::blzsh_parity::blzsh_parity_config;

/// The environment variable the pane tag reads, and how much of it fits.
///
/// Six characters is what blzsh's `${TEAR_PANE_ID:0:6}` took — enough to
/// tell two split panes apart without spending prompt width on a full
/// 16-character id.
const TEAR_PANE_VAR: &str = "TEAR_PANE_ID";
const TEAR_PANE_CHARS: usize = 6;

/// The blzsh-parity base with the fleet's own default posture applied.
///
/// Every fleet-facing theme builds on this, never on
/// [`blzsh_parity_config`] directly.
#[must_use]
pub fn fleet_base_config() -> SekiConfig {
    let mut c = blzsh_parity_config();

    // The session NAME is operator-chosen and already on the window —
    // don't spend prompt width restating it. Declared-but-off, not
    // removed (see the module doc).
    if let Some(session) = c
        .env_var
        .entries
        .get_mut(ishou_tokens::FleetStateVar::TearSessionName.name())
    {
        session.enabled = false;
    }

    // The pane tag is a substring of an env var. That is not a reason to
    // start a shell — twice — before every prompt the operator sees.
    // Byte-identical output; ~20.7 ms cheaper; portable off bash.
    if let Some(pane) = c.custom.entries.get_mut("tear_pane") {
        pane.source = CustomSource::EnvSlice {
            var: TEAR_PANE_VAR.to_owned(),
            chars: TEAR_PANE_CHARS,
        };
    }

    c
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session_name() -> &'static str {
        ishou_tokens::FleetStateVar::TearSessionName.name()
    }

    #[test]
    fn fleet_base_hides_the_session_name() {
        assert!(
            !fleet_base_config().env_var.entries[session_name()].enabled,
            "the fleet default must not render the tear session name",
        );
    }

    #[test]
    fn fleet_base_keeps_the_session_entry_declared() {
        // MODULARIZE, DON'T DELETE: turning the segment back on must be
        // a one-field flip, not a re-declaration. The entry — and its
        // variable binding, format and style — survives being disabled.
        let c = fleet_base_config();
        let entry = &c.env_var.entries[session_name()];
        assert_eq!(entry.variable.as_deref(), Some(session_name()));
        assert!(!entry.format.is_empty());
        assert!(!entry.style.as_str().is_empty());
    }

    #[test]
    fn fleet_base_keeps_the_pane_tag() {
        // Only the NAME is suppressed. The machine-assigned pane id
        // still disambiguates split panes, which the name cannot.
        assert!(fleet_base_config().custom.entries["tear_pane"].enabled);
    }

    /// The performance deviation, stated as what it IS rather than as a
    /// timing: the pane tag reads the environment and starts nothing.
    #[test]
    fn fleet_base_renders_the_pane_tag_without_a_process() {
        let entry = &fleet_base_config().custom.entries["tear_pane"];
        assert!(
            !entry.source.spawns(),
            "the fleet's one default custom entry must not spawn a shell",
        );
        assert_eq!(
            entry.source,
            CustomSource::EnvSlice {
                var: TEAR_PANE_VAR.to_owned(),
                chars: TEAR_PANE_CHARS,
            },
        );
    }

    /// The deviation must be output-preserving, not merely cheaper. Six
    /// characters of the id is exactly what `${TEAR_PANE_ID:0:6}`
    /// produced, so an operator sees the same `· 42032e` they always did.
    #[test]
    fn the_pane_tag_renders_the_same_six_characters_as_the_shell_form() {
        let entry = &fleet_base_config().custom.entries["tear_pane"];
        assert_eq!(
            entry
                .source
                .from_env(|var| (var == TEAR_PANE_VAR).then(|| "42032e03aa3e69d8".to_owned())),
            Some("42032e".to_owned()),
        );
        // And an absent variable is silence — what the retired
        // `when = '[ -n "$TEAR_PANE_ID" ]'` guard bought, for free.
        assert_eq!(entry.source.from_env(|_| None), None);
    }

    #[test]
    fn fleet_base_leaves_the_parity_reference_untouched() {
        // The deviations live HERE, never upstream in the pinned mirror
        // of blzsh's starship.toml.
        let parity = blzsh_parity_config();
        assert!(
            parity.env_var.entries[session_name()].enabled,
            "blzsh_parity is a pinned reference — it must keep rendering \
             the session name exactly as blzsh's starship.toml did",
        );
        assert!(
            parity.custom.entries["tear_pane"].source.spawns(),
            "blzsh_parity is a pinned reference — it must keep transcribing \
             starship's `command = printf …` form, bashism and all",
        );
    }

    #[test]
    fn fleet_base_changes_nothing_else() {
        // Guards against this layer quietly becoming a second theme.
        // Everything except the two declared deviations — the
        // session-name flag and the pane tag's source — is byte-identical
        // to the parity base. Reconstructing the deviations here (rather
        // than relaxing the comparison) keeps the guard exact: a third,
        // undeclared change still fails.
        let mut parity = blzsh_parity_config();
        parity
            .env_var
            .entries
            .get_mut(session_name())
            .expect("parity declares the session entry")
            .enabled = false;
        parity
            .custom
            .entries
            .get_mut("tear_pane")
            .expect("parity declares the pane entry")
            .source = CustomSource::EnvSlice {
            var: TEAR_PANE_VAR.to_owned(),
            chars: TEAR_PANE_CHARS,
        };
        assert_eq!(parity, fleet_base_config());
    }
}
