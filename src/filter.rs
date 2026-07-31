// New module: src/filter.rs
// Implements whitelist/blacklist filtering for channel/device names
use serde::{Deserialize, Serialize};
use std::{fs, io, path::Path};
use regex::Regex;

#[derive(Debug, Serialize, Deserialize, Default)]
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

/// Decide whether to show a channel with `name` based on the config.
///
/// Behavior:
/// - If whitelist non-empty: show only items matching whitelist (whitelist wins).
/// - Else if blacklist non-empty: hide items matching blacklist.
/// - Else: show all.
pub fn should_show(name: &str, cfg: &FilterConfig) -> bool {
    if !cfg.whitelist.is_empty() {
        return cfg
            .whitelist
            .iter()
            .any(|p| matches_pattern(name, p, cfg.use_regex));
    }
    if !cfg.blacklist.is_empty() {
        return !cfg
            .blacklist
            .iter()
            .any(|p| matches_pattern(name, p, cfg.use_regex));
    }
    true
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
        assert!(should_show("Headphones", &cfg));
        assert!(!should_show("Bluetooth Speaker", &cfg));
    }

    #[test]
    fn blacklist_hides() {
        let cfg = FilterConfig {
            whitelist: vec![],
            blacklist: vec!["Internal Mic".into()],
            use_regex: false,
        };
        assert!(!should_show("Internal Mic 1", &cfg));
        assert!(should_show("External Mic", &cfg));
    }

    #[test]
    fn regex_matching() {
        let cfg = FilterConfig {
            whitelist: vec!["^USB.*".into()],
            blacklist: vec![],
            use_regex: true,
        };
        assert!(should_show("USB Audio Device", &cfg));
        assert!(!should_show("Bluetooth Speaker", &cfg));
    }
}
