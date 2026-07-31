## Add whitelist/blacklist filtering support

This PR adds a small filtering module (src/filter.rs) that provides a FilterConfig (whitelist, blacklist, use_regex) and a should_show helper to decide whether a channel/device slider should be shown. It also adds a docs/CONFIG.md with an example configuration and updates Cargo.toml with the required dependencies (serde, toml, regex).

Integration notes are included in the commit message and the PR description.
