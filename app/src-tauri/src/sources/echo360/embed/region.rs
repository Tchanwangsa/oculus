//! Echo360's regional hosts. An embed's tag comes from page markdown, so its
//! LTI link and the form Canvas signs for it must name one of these; the
//! launch lands on it, and the player calls after it go to the same host.

const REGIONS: &[&str] = &[
    "echo360.org",
    "echo360.org.uk",
    "echo360.org.au",
    "echo360.net.au",
    "echo360.ca",
];

/// Whether `host` is one of Echo360's regions.
pub fn is_region(host: &str) -> bool {
    REGIONS.contains(&host)
}

/// Whether `host` is a region or a host under one, as a page's iframe may
/// name.
pub fn in_region(host: &str) -> bool {
    REGIONS
        .iter()
        .any(|r| host == *r || host.strip_suffix(r).is_some_and(|sub| sub.ends_with('.')))
}
