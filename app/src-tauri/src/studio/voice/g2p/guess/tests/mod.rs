mod golden;
mod model;
mod vocab;

/// The pinned pack's `g2p-config.json`, so encoding is tested without the pack.
const CONFIG: &str = include_str!("g2p-config.json");
