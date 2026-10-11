//! The four services the dock can host, in rail order.

use super::rules;

/// One messaging web app. `store_id` names its WebKit data store, which holds
/// the login: it is derived from `id` and must never change, or the user is
/// signed out of that service for good.
pub struct DockService {
    pub id: &'static str,
    pub home: &'static str,
    /// Exempted from ITP's 30-operating-day storage wipe (`_setPersistedSites:`).
    /// WebKit matches by registrable domain, so `www.x.com` covers `x.com`.
    pub sites: &'static [&'static str],
    /// Registrable domains whose main frame may post to the `dock` message
    /// handler; a host matches its domain or any subdomain (`channel.rs`).
    pub domains: &'static [&'static str],
    /// Whether the chat-list row watcher runs here (`scripts/rows.js` has a
    /// config for it). Discord has none: its notifications carry the sender.
    pub watches_rows: bool,
    /// The one-hop rule table (`rules.rs`), which turns on the guard script
    /// and the native navigation guard. Only where client modification is
    /// tolerated (`scripts.rs`).
    pub rules: Option<&'static str>,
    /// A stylesheet that hides the page's ways into the rest of the site.
    pub trim_css: Option<&'static str>,
    /// Where the page goes when it is parked, for a service that marks an
    /// open thread read while it is off-screen. Needs `rules`.
    pub park_path: Option<&'static str>,
    pub store_id: [u8; 16],
}

pub const SERVICES: [DockService; 4] = [
    DockService {
        id: "whatsapp",
        home: "https://web.whatsapp.com/",
        sites: &["https://web.whatsapp.com/"],
        domains: &["whatsapp.com"],
        watches_rows: true,
        rules: None,
        trim_css: None,
        park_path: None,
        store_id: store_id("whatsapp"),
    },
    DockService {
        id: "instagram",
        home: "https://www.instagram.com/direct/inbox/",
        sites: &["https://www.instagram.com/"],
        domains: &["instagram.com"],
        watches_rows: true,
        rules: Some(rules::INSTAGRAM),
        trim_css: Some(include_str!("scripts/instagram.css")),
        park_path: Some("/direct/inbox/"),
        store_id: store_id("instagram"),
    },
    // messenger.com no longer keeps a session: its `c_user` cookie lives on
    // facebook.com, so Messenger is hosted there.
    DockService {
        id: "messenger",
        home: "https://www.facebook.com/messages/",
        sites: &["https://www.facebook.com/", "https://www.messenger.com/"],
        domains: &["facebook.com", "messenger.com"],
        watches_rows: true,
        rules: None,
        trim_css: Some(include_str!("scripts/facebook.css")),
        park_path: None,
        store_id: store_id("messenger"),
    },
    DockService {
        id: "discord",
        home: "https://discord.com/channels/@me",
        sites: &["https://discord.com/"],
        domains: &["discord.com"],
        watches_rows: false,
        rules: None,
        trim_css: None,
        park_path: None,
        store_id: store_id("discord"),
    },
];

pub fn service(id: &str) -> Option<&'static DockService> {
    SERVICES.iter().find(|s| s.id == id)
}

/// Two FNV-1a 64-bit hashes of `id` with different offset bases, so the same
/// id always opens the same store and no state is needed to remember it.
pub const fn store_id(id: &str) -> [u8; 16] {
    let bytes = id.as_bytes();
    let mut out = [0u8; 16];
    let mut half = 0;
    while half < 2 {
        let mut h: u64 = if half == 0 {
            0xcbf29ce484222325
        } else {
            0x84222325cbf29ce4
        };
        let mut i = 0;
        while i < bytes.len() {
            h ^= bytes[i] as u64;
            h = h.wrapping_mul(0x100000001b3);
            i += 1;
        }
        let mut b = 0;
        while b < 8 {
            out[half * 8 + b] = (h >> (56 - 8 * b)) as u8;
            b += 1;
        }
        half += 1;
    }
    out
}

/// The store id as WebKit prints it (`NSUUID.UUIDString`), for the log.
pub fn uuid_string(id: &[u8; 16]) -> String {
    let hex: String = id.iter().map(|b| format!("{b:02X}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}
