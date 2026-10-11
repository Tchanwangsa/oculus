//! The init scripts a dock page gets, all read-only: nothing they do changes
//! what the page shows. Each runs in the main frame at document start, as
//! one function made of `scripts/post.js` (the `dock` handler poster) and
//! its body, so no helper leaks onto `window`.

use super::services::DockService;

const POST: &str = include_str!("scripts/post.js");
const NOTIFICATION: &str = include_str!("scripts/notification.js");
const ROWS: &str = include_str!("scripts/rows.js");

fn wrap(prelude: &str, body: &str) -> String {
    format!("(() => {{\n'use strict';\n{prelude}{POST}\n{body}\n}})();\n")
}

/// The Notification shim for every service, then the row watcher for the
/// services that have a chat-list config.
pub(super) fn init_scripts(service: &DockService) -> Vec<String> {
    let mut scripts = vec![wrap("", NOTIFICATION)];
    if service.watches_rows {
        let prelude = format!("const SERVICE = {};\n", serde_json::json!(service.id));
        scripts.push(wrap(&prelude, ROWS));
    }
    scripts
}

#[cfg(test)]
mod tests {
    use super::super::services::{service, SERVICES};
    use super::*;

    #[test]
    fn every_service_gets_the_shim_and_row_services_the_watcher() {
        for s in &SERVICES {
            let scripts = init_scripts(s);
            assert!(scripts[0].contains("class DockNotification"), "{}", s.id);
            assert_eq!(
                scripts.len(),
                if s.watches_rows { 2 } else { 1 },
                "{}",
                s.id
            );
        }
        let rows = &init_scripts(service("whatsapp").unwrap())[1];
        assert!(rows.contains("const SERVICE = \"whatsapp\";"));
        assert!(init_scripts(service("discord").unwrap())
            .iter()
            .all(|s| !s.contains("const sites")));
    }

    /// Every service with a row watcher has a config in `rows.js`.
    #[test]
    fn row_services_have_a_config() {
        for s in SERVICES.iter().filter(|s| s.watches_rows) {
            assert!(ROWS.contains(&format!("\n  {}: {{", s.id)), "{}", s.id);
        }
    }
}
