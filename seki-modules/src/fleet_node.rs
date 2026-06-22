//! `fleet_node` segment — surfaces current node identity + cluster role.
//!
//! Pleme-io-native (Tier 2 per `docs/PLEME-IO-SEGMENTS.md`). Reads
//! the typed kindling node manifest at
//! `~/.config/kindling/node.yaml` and renders the node name + cluster
//! when present. Tells the operator at a glance which fleet node +
//! cluster role they're driving.
//!
//! ## Theme
//!
//! Nord-snow `#D8DEE9` dimmed — quiet, persistent context.
//!
//! ## Probe budget
//!
//! Filesystem read only — bypasses `scan_timeout_ms`. The path is
//! built from `$HOME` + the manifest_path (no shell expansion, no
//! path-traversal; we only join $HOME with the configured relative
//! path).

use seki_core::{
    Module, RenderContext, Segment, SekiResult,
    config::fleet_node::FleetNodeConfig,
    segment::StyledFragment,
};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Mutex;

/// Typed kindling node manifest. Only the three fields we render
/// are deserialized; extra fields are ignored.
#[derive(Debug, Clone, Deserialize)]
pub struct NodeManifest {
    pub node_name: String,
    pub cluster: String,
    #[serde(default)]
    pub role: String,
}

pub struct FleetNodeModule {
    cfg: FleetNodeConfig,
    /// Parsed manifest, cached across renders (the file rarely
    /// changes within a shell session — re-parsing is wasted work).
    cache: Mutex<Option<NodeManifest>>,
}

impl FleetNodeModule {
    pub fn new(cfg: FleetNodeConfig) -> Self {
        Self {
            cfg,
            cache: Mutex::new(None),
        }
    }
}

impl Module for FleetNodeModule {
    fn name(&self) -> &'static str {
        "fleet_node"
    }

    fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    fn render(&self, ctx: &RenderContext) -> SekiResult<Option<Segment>> {
        let cached = self.cache.lock().ok().and_then(|g| g.clone());
        let manifest = match cached {
            Some(m) => m,
            None => {
                let home = match ctx.home.as_ref() {
                    Some(h) => h,
                    None => return Ok(None),
                };
                let path = resolve_manifest_path(home, &self.cfg.manifest_path);
                match load_manifest(&path) {
                    Some(m) => {
                        if let Ok(mut g) = self.cache.lock() {
                            *g = Some(m.clone());
                        }
                        m
                    }
                    None => return Ok(None),
                }
            }
        };
        let text = seki_core::format::render(&self.cfg.format, |__n| match __n {
            "node" => Some(manifest.node_name.to_owned()),
            "cluster" => Some(manifest.cluster.to_owned()),
            "role" => Some(manifest.role.to_owned()),
            _ => None,
        });
        Ok(Some(
            Segment::new("fleet_node").push(StyledFragment::new(text, self.cfg.style.resolve())),
        ))
    }
}

/// Join `$HOME` with the manifest path. Rejects absolute paths +
/// `..` segments to prevent path-traversal escape from $HOME.
pub fn resolve_manifest_path(home: &std::path::Path, rel: &str) -> PathBuf {
    let p = PathBuf::from(rel);
    if p.is_absolute() {
        // Configured value is absolute — caller knows what they're
        // doing; trust it but never blindly concatenate with $HOME.
        return p;
    }
    // Reject any `..` segment as a defensive measure.
    if p.components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return home.join("__rejected_traversal__");
    }
    home.join(p)
}

/// Read + parse the typed kindling node manifest. Returns `None` for
/// missing / unreadable / unparseable files.
pub fn load_manifest(path: &std::path::Path) -> Option<NodeManifest> {
    let body = std::fs::read_to_string(path).ok()?;
    parse_manifest(&body)
}

pub fn parse_manifest(body: &str) -> Option<NodeManifest> {
    serde_yaml::from_str(body).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp_dir(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("seki-fleet-node-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn parses_typed_manifest() {
        let body = "node_name: rio\ncluster: home\nrole: server\n";
        let m = parse_manifest(body).unwrap();
        assert_eq!(m.node_name, "rio");
        assert_eq!(m.cluster, "home");
        assert_eq!(m.role, "server");
    }

    #[test]
    fn parses_manifest_with_default_role() {
        let body = "node_name: cid\ncluster: laptop\n";
        let m = parse_manifest(body).unwrap();
        assert_eq!(m.node_name, "cid");
        assert_eq!(m.cluster, "laptop");
        assert_eq!(m.role, "");
    }

    #[test]
    fn parses_manifest_rejects_garbage() {
        assert!(parse_manifest("not yaml: ::\n").is_none());
    }

    #[test]
    fn render_format_substitutes_all_three() {
        // Format mirrors shikumi_tier's `[..]($style)` shape: parens
        // are reserved for the style marker. Author-visible role
        // goes between non-paren delimiters.
        let out = seki_core::format::render("[$node/$cluster $role]($style)", |__n| match __n {
            "node" => Some("n".to_owned()),
            "cluster" => Some("c".to_owned()),
            "role" => Some("r".to_owned()),
            _ => None,
        });
        assert_eq!(out, "n/c r");
    }

    #[test]
    fn resolve_manifest_path_joins_home() {
        let home = PathBuf::from("/home/op");
        let p = resolve_manifest_path(&home, ".config/kindling/node.yaml");
        assert_eq!(p, PathBuf::from("/home/op/.config/kindling/node.yaml"));
    }

    #[test]
    fn resolve_manifest_path_rejects_traversal() {
        let home = PathBuf::from("/home/op");
        let p = resolve_manifest_path(&home, "../../etc/passwd");
        // Marker path; the open will fail downstream.
        assert!(p.to_str().unwrap().contains("__rejected_traversal__"));
    }

    #[test]
    fn resolve_manifest_path_passes_absolute_through() {
        let home = PathBuf::from("/home/op");
        let p = resolve_manifest_path(&home, "/etc/seki/node.yaml");
        assert_eq!(p, PathBuf::from("/etc/seki/node.yaml"));
    }

    #[test]
    fn bare_config_is_disabled() {
        let cfg = FleetNodeConfig::bare();
        assert!(!cfg.enabled);
        assert_eq!(cfg.manifest_path, "");
    }

    #[test]
    fn default_uses_nord_snow_dimmed() {
        let cfg = FleetNodeConfig::default();
        assert_eq!(cfg.style.as_str(), "dimmed #D8DEE9");
        assert_eq!(cfg.manifest_path, ".config/kindling/node.yaml");
    }

    #[test]
    fn renders_segment_from_present_manifest() {
        let dir = tmp_dir("present");
        let cfg_dir = dir.join(".config").join("kindling");
        fs::create_dir_all(&cfg_dir).unwrap();
        fs::write(
            cfg_dir.join("node.yaml"),
            "node_name: rio\ncluster: home\nrole: server\n",
        )
        .unwrap();
        let module = FleetNodeModule::new(FleetNodeConfig {
            enabled: true,
            ..FleetNodeConfig::default()
        });
        let mut ctx = RenderContext::from_env().with_colors(false);
        ctx.home = Some(dir);
        let seg = module.render(&ctx).unwrap().expect("segment");
        assert_eq!(seg.module, "fleet_node");
        assert_eq!(seg.fragments[0].text, "rio/home");
    }

    #[test]
    fn renders_nothing_when_manifest_missing() {
        let dir = tmp_dir("missing");
        let module = FleetNodeModule::new(FleetNodeConfig {
            enabled: true,
            ..FleetNodeConfig::default()
        });
        let mut ctx = RenderContext::from_env().with_colors(false);
        ctx.home = Some(dir);
        assert!(module.render(&ctx).unwrap().is_none());
    }
}
