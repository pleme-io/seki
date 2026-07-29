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
//!   The `custom.tear_pane` tag (`· <id>`) is deliberately untouched:
//!   the pane *id* is machine-assigned and disambiguates split panes,
//!   which the name cannot.

use seki_core::SekiConfig;

use crate::blzsh_parity::blzsh_parity_config;

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

    #[test]
    fn fleet_base_leaves_the_parity_reference_untouched() {
        // The deviation lives HERE, never upstream in the pinned mirror
        // of blzsh's starship.toml.
        assert!(
            blzsh_parity_config().env_var.entries[session_name()].enabled,
            "blzsh_parity is a pinned reference — it must keep rendering \
             the session name exactly as blzsh's starship.toml did",
        );
    }

    #[test]
    fn fleet_base_changes_nothing_else() {
        // Guards against this layer quietly becoming a second theme.
        // Everything except the session-name flag is byte-identical to
        // the parity base.
        let mut parity = blzsh_parity_config();
        parity
            .env_var
            .entries
            .get_mut(session_name())
            .expect("parity declares the session entry")
            .enabled = false;
        assert_eq!(parity, fleet_base_config());
    }
}
