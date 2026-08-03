//! Typed config for the `gitops` reconciler-health segment.

use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GitopsConfig {
    pub enabled: bool,
    /// Where the reconciler publishes its heartbeat + receipt chain. The
    /// segment renders nothing when this is not a directory, which is how a
    /// non-gitops host stays silent without needing to say so.
    pub state_dir: String,
    /// Fallback poll interval, used ONLY when the heartbeat file does not
    /// carry one. It is a BOUND on how long silence is tolerated, never a
    /// preference — raising it cannot change any verdict except how long a
    /// stopped loop goes unreported.
    pub assumed_poll_seconds: u64,
    /// A stopped loop is the loudest state: the node is not reconciling at
    /// all and nothing else will say so.
    pub stopped_style: StyleSpec,
    /// Ticking but failing — degraded, not silent.
    pub failing_style: StyleSpec,
    pub prefix: String,
    pub suffix: String,
}

impl Default for GitopsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            state_dir: "/var/log/pleme-gitops".to_owned(),
            assumed_poll_seconds: 60,
            stopped_style: StyleSpec::new("bold red"),
            failing_style: StyleSpec::new("bold yellow"),
            prefix: String::new(),
            suffix: " ".to_owned(),
        }
    }
}

impl GitopsConfig {
    pub fn bare() -> Self {
        Self {
            enabled: false,
            state_dir: String::new(),
            assumed_poll_seconds: 60,
            stopped_style: StyleSpec::default(),
            failing_style: StyleSpec::default(),
            prefix: String::new(),
            suffix: String::new(),
        }
    }
}
