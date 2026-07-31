# Mixxc configuration: filtering (whitelist / blacklist)

Place a `config.toml` at $HOME/.config/mixxc/config.toml (or another path you prefer).
Example:

```toml
# Only show these (whitelist wins if not empty)
whitelist = []

# Hide channels that contain these substrings (or regex if use_regex = true)
blacklist = ["Bluetooth", "Internal Mic"]

# Use regex patterns instead of substring matching
use_regex = false
```

Behavior:
- If `whitelist` is non-empty, only entries matching the whitelist are shown.
- Otherwise, if `blacklist` is non-empty, entries matching the blacklist are hidden.
- If both are empty, all sliders are shown.

Tips:
- Use exact stable device IDs instead of human names if available to avoid accidental matches.
- Enable `use_regex = true` only if you’re comfortable writing regex patterns.
