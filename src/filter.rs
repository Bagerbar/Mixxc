// New module: src/filter.rs
// Implements whitelist/blacklist filtering for channel/device names
use serde::{Deserialize, Serialize};
use std::{fs, io, path::Path};
use regex::Regex;

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct FilterConfig {
    /// If non-empty, only these names (or patterns) are shown.
    pub whitelist: Vec<String>,
    /// If non-empty and whitelist is empty, these names (or patterns) are hidden.
    pub blacklist: Vec<String>,
    /// If true, treat entries as regex patterns. Otherwise do substring match.
    pub use_regex: bool,
}

pub fn load_config(path: &Path) -> io::Result<FilterConfig> {
    let s = fs::read_to_string(path)?;
    let cfg: FilterConfig =
        toml::from_str(&s).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(cfg)
}

fn matches_pattern(name: &str, pat: &str, use_regex: bool) -> bool {
    if use_regex {
        match Regex::new(pat) {
            Ok(re) => re.is_match(name),
            Err(_) => false,
        }
    } else {
        name.contains(pat)
    }
}

/// Check whether a single pattern matches any of the provided fields.
fn pattern_matches_any_field(
    pat: &str,
    use_regex: bool,
    name: &str,
    application: Option<&str>,
    node_name: Option<&str>,
    node_description: Option<&str>,
) -> bool {
    if matches_pattern(name, pat, use_regex) {
        return true;
    }
    if let Some(app) = application {
        if matches_pattern(app, pat, use_regex) {
            return true;
        }
    }
    if let Some(n) = node_name {
        if matches_pattern(n, pat, use_regex) {
            return true;
        }
    }
    if let Some(desc) = node_description {
        if matches_pattern(desc, pat, use_regex) {
            return true;
        }
    }
    false
}

/// Decide whether to show a channel/item based on the config using multiple text fields.
///
/// Parameters:
/// - name: original device/channel name (existing callers pass this)
/// - application: optional application name producing the node (e.g. process or app label)
/// - node_name: optional node name
/// - node_description: optional node description
/// - cfg: FilterConfig
///
/// Behavior:
/// - If whitelist non-empty: show only items matching whitelist (whitelist wins).
/// - Else if blacklist non-empty: hide items matching blacklist.
/// - Else: show all.
pub fn should_show_with_fields(
    name: &str,
    application: Option<&str>,
    node_name: Option<&str>,
    node_description: Option<&str>,
    cfg: &FilterConfig,
) -> bool {
    if !cfg.whitelist.is_empty() {
        return cfg.whitelist.iter().any(|p| {
            pattern_matches_any_field(
                p,
                cfg.use_regex,
                name,
                application,
                node_name,
                node_description,
            )
        });
    }
    if !cfg.blacklist.is_empty() {
        return !cfg.blacklist.iter().any(|p| {
            pattern_matches_any_field(
                p,
                cfg.use_regex,
                name,
                application,
                node_name,
                node_description,
            )
        });
    }
    true
}

/// Backwards-compatible function: keep the original signature.
/// This delegates to should_show_with_fields with no extra fields.
pub fn should_show(name: &str, cfg: &FilterConfig) -> bool {
    should_show_with_fields(name, None, None, None, cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitelist_wins() {
        let cfg = FilterConfig {
            whitelist: vec!["Headphones".into()],
            blacklist: vec!["Bluetooth".into()],
            use_regex: false,
        };
        // original name match
        assert!(should_show_with_fields("Headphones", None, None, None, &cfg));
        // does not match whitelist -> hidden even though blacklist contains "Bluetooth"
        assert!(!should_show_with_fields("Bluetooth Speaker", None, None, None, &cfg));
    }

    #[test]
    fn blacklist_hides() {
        let cfg = FilterConfig {
            whitelist: vec![],
            blacklist: vec!["Internal Mic".into()],
            use_regex: false,
        };
        assert!(!should_show_with_fields("Internal Mic 1", None, None, None, &cfg));
        assert!(should_show_with_fields("External Mic", None, None, None, &cfg));
    }

    #[test]
    fn regex_matching() {
        let cfg = FilterConfig {
            whitelist: vec!["^USB.*".into()],
            blacklist: vec![],
            use_regex: true,
        };
        assert!(should_show_with_fields("USB Audio Device", None, None, None, &cfg));
        assert!(!should_show_with_fields("Bluetooth Speaker", None, None, None, &cfg));
    }

    #[test]
    fn matches_application_node_description() {
        let cfg = FilterConfig {
            whitelist: vec![],
            blacklist: vec!["MyApp".into(), "node-xyz".into(), "unwanted-desc".into()],
            use_regex: false,
        };
        // If any field matches the blacklist, the item should be hidden
        assert!(!should_show_with_fields(
            "Some Device",
            Some("MyApp Client"),
            None,
            None,
            &cfg
        ));
        assert!(!should_show_with_fields(
            "Some Device",
            None,
            Some("node-xyz-123"),
            None,
            &cfg
        ));
        assert!(!should_show_with_fields(
            "Some Device",
            None,
            None,
            Some("this has unwanted-desc inside"),
            &cfg
        ));

        // An unrelated item is shown
        assert!(should_show_with_fields(
            "Some Device",
            Some("OtherApp"),
            Some("node-abc"),
            Some("fine description"),
            &cfg
        ));
    }

    #[test]
    fn whitelist_can_match_other_fields() {
        let cfg = FilterConfig {
            whitelist: vec!["^MyApp.*".into()],
            blacklist: vec![],
            use_regex: true,
        };
        // whitelist regex matches application name
        assert!(should_show_with_fields(
            "Some Device",
            Some("MyAppService"),
            None,
            None,
            &cfg
        ));
        // not matching -> hidden
        assert!(!should_show_with_fields("Some Device", Some("OtherApp"), None, None, &cfg));
    }
}
