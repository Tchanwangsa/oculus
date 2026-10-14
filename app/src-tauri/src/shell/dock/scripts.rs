//! The scripts a dock page gets. Init scripts run in the main frame at
//! document start, each as one function made of a prelude (constants from
//! Rust), `scripts/post.js` (the `dock` handler poster) and its body, so no
//! helper leaks onto `window`.
//!
//! - Read-only, on the services that need them: the Notification shim (all)
//!   and the chat-list row watcher. They change nothing the page shows.
//! - Page-modifying, only where the service has the data for it: the one-hop
//!   guard (`rules`, Instagram) and the trim sheet (`trim_css`, Instagram and
//!   Facebook), and the thread-list scrollbar (Instagram). WhatsApp and
//!   Discord forbid client modification, so they get none of these.
//!
//! `leave_script` is not an init script: `page.rs` evaluates it when a page
//! is parked.

use super::services::DockService;

const POST: &str = include_str!("scripts/post.js");
const NOTIFICATION: &str = include_str!("scripts/notification.js");
const ROWS: &str = include_str!("scripts/rows.js");
const HOP: &str = include_str!("scripts/hop.js");
const GUARD: &str = include_str!("scripts/guard.js");
const STYLE: &str = include_str!("scripts/style.js");
const LEAVE: &str = include_str!("scripts/leave.js");
const INSTAGRAM_SCROLL: &str = include_str!("scripts/instagram-scroll.js");

fn wrap(prelude: &str, body: &str) -> String {
    format!("(() => {{\n'use strict';\n{prelude}{POST}\n{body}\n}})();\n")
}

/// `RULES` (the table, verbatim) and the matcher every rule-driven script uses.
fn hop_prelude(rules: &str) -> String {
    format!("const RULES = {rules};\n{HOP}\n")
}

/// The Notification shim for every service, the row watcher for the
/// services that have a chat-list config, the trim sheet and the guard
/// where the service has them.
pub(super) fn init_scripts(service: &DockService) -> Vec<String> {
    let mut scripts = vec![wrap("", NOTIFICATION)];
    if service.watches_rows {
        let prelude = format!("const SERVICE = {};\n", serde_json::json!(service.id));
        scripts.push(wrap(&prelude, ROWS));
    }
    if let Some(css) = service.trim_css {
        let prelude = format!("const CSS = {};\n", serde_json::json!(css));
        scripts.push(wrap(&prelude, STYLE));
    }
    if let Some(rules) = service.rules {
        scripts.push(wrap(&hop_prelude(rules), GUARD));
    }
    if service.id == "instagram" {
        scripts.push(wrap("", INSTAGRAM_SCROLL));
    }
    scripts
}

/// Sends a parked page back to its inbox, for services that mark an open
/// thread read (`park_path`). Needs the rule table to tell a thread from a
/// login page.
pub(super) fn leave_script(service: &DockService) -> Option<String> {
    let (path, rules) = (service.park_path?, service.rules?);
    let prelude = format!(
        "const PARK_PATH = {};\n{}",
        serde_json::json!(path),
        hop_prelude(rules)
    );
    Some(format!(
        "(() => {{\n'use strict';\n{prelude}\n{LEAVE}\n}})();\n"
    ))
}

#[cfg(test)]
mod tests {
    use super::super::rules::Rules;
    use super::super::services::{service, SERVICES};
    use super::*;

    fn has(scripts: &[String], marker: &str) -> bool {
        scripts.iter().any(|s| s.contains(marker))
    }

    #[test]
    fn every_service_gets_the_shim_and_row_services_the_watcher() {
        for s in &SERVICES {
            let scripts = init_scripts(s);
            assert!(scripts[0].contains("class DockNotification"), "{}", s.id);
            assert_eq!(has(&scripts, "const sites"), s.watches_rows, "{}", s.id);
            assert_eq!(has(&scripts, "let scroller = null"), s.id == "instagram");
        }
        let whatsapp = init_scripts(service("whatsapp").unwrap());
        assert!(has(&whatsapp, "const SERVICE = \"whatsapp\";"));
        assert!(!has(
            &init_scripts(service("discord").unwrap()),
            "const sites"
        ));
    }

    /// WhatsApp and Discord forbid client modification: only the read-only
    /// scripts reach them.
    #[test]
    fn only_instagram_and_facebook_are_modified() {
        let modified = |id: &str| {
            let scripts = init_scripts(service(id).unwrap());
            (
                has(&scripts, "const CSS = "),
                has(&scripts, "const RULES = "),
            )
        };
        assert_eq!(modified("whatsapp"), (false, false));
        assert_eq!(modified("discord"), (false, false));
        assert_eq!(modified("messenger"), (true, false));
        assert_eq!(modified("instagram"), (true, true));
        assert_eq!(init_scripts(service("whatsapp").unwrap()).len(), 2);
        assert_eq!(init_scripts(service("discord").unwrap()).len(), 1);

        let instagram = init_scripts(service("instagram").unwrap());
        assert!(has(&instagram, "data-visualcompletion"));
        assert!(has(&instagram, "history.pushState"));
        let messenger = init_scripts(service("messenger").unwrap());
        assert!(has(&messenger, "--header-height"));
    }

    #[test]
    fn only_instagram_leaves_the_thread_when_parked() {
        for s in &SERVICES {
            assert_eq!(leave_script(s).is_some(), s.id == "instagram", "{}", s.id);
        }
        let leave = leave_script(service("instagram").unwrap()).unwrap();
        assert!(leave.contains("const PARK_PATH = \"/direct/inbox/\";"));
        assert!(leave.contains("const hop = "));
    }

    /// The guard's table parses in Rust too, and the park path is one of its
    /// home pages, so leaving a thread passes the guard.
    #[test]
    fn rule_tables_parse_and_park_paths_are_home() {
        for s in SERVICES.iter() {
            let Some(rules) = s.rules else { continue };
            let rules = Rules::parse(rules).unwrap();
            let home: url::Url = s.home.parse().unwrap();
            assert_eq!(rules.classify(&home), super::super::rules::Place::Home);
            if let Some(path) = s.park_path {
                let parked = home.join(path).unwrap();
                assert_eq!(rules.classify(&parked), super::super::rules::Place::Home);
            }
        }
    }

    /// Every service with a row watcher has a config in `rows.js`.
    #[test]
    fn row_services_have_a_config() {
        for s in SERVICES.iter().filter(|s| s.watches_rows) {
            assert!(ROWS.contains(&format!("\n  {}: {{", s.id)), "{}", s.id);
        }
    }
}
