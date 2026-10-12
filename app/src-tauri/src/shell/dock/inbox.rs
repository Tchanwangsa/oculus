//! The Inbox: every message the dock pages report, newest first. It lives in
//! memory only and is gone on quit, so message text never reaches the disk
//! or `oculus.db`, which agents can read.

use std::collections::{HashMap, VecDeque};

use serde::Serialize;
use tauri::{AppHandle, Emitter, EventTarget};

use super::message::Message;
use super::with_state;
use crate::shell::browser::MAIN;

pub(super) const CAP: usize = 200;
/// A dedupe key seen within this long of its last sighting drops the
/// message, and the repeat refreshes it. Kept across `dock_inbox_clear`, so
/// clearing doesn't let a re-report back in.
const DEDUPE_MS: i64 = 10 * 60 * 1000;
/// A service with both channels can report one message twice (a row and a
/// notification, same sender). The second within this window is dropped.
const CROSS_CHANNEL_MS: i64 = 10 * 1000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxItem {
    pub id: String,
    pub service: String,
    pub sender: String,
    pub preview: String,
    /// Unix milliseconds, when Rust received it.
    pub at: i64,
}

struct Entry {
    item: InboxItem,
    from_row: bool,
}

#[derive(Default)]
pub(super) struct Inbox {
    entries: VecDeque<Entry>,
    /// Dedupe key → when it was last seen.
    seen: HashMap<String, i64>,
    next: u64,
}

impl Inbox {
    /// Records `message` unless it is a duplicate; returns the new item.
    pub(super) fn add(&mut self, service: &str, message: Message, at: i64) -> Option<InboxItem> {
        let (key, from_row) = match &message {
            Message::Notification {
                sender,
                preview,
                tag,
            } if tag.is_empty() => (format!("n\0{service}\0{sender}\0{preview}"), false),
            Message::Notification { tag, .. } => (format!("t\0{service}\0{tag}"), false),
            Message::Row { sender, preview } => {
                (format!("r\0{service}\0{sender}\0{preview}"), true)
            }
        };
        self.seen.retain(|_, when| at - *when < DEDUPE_MS);
        if self.seen.insert(key, at).is_some() {
            return None;
        }
        let (sender, preview) = message.parts();
        let echoed = self.entries.iter().any(|e| {
            e.item.service == service
                && e.from_row != from_row
                && e.item.sender == sender
                && at - e.item.at < CROSS_CHANNEL_MS
        });
        if echoed {
            return None;
        }
        self.next += 1;
        let item = InboxItem {
            id: format!("{service}-{}", self.next),
            service: service.to_string(),
            sender: sender.to_string(),
            preview: preview.to_string(),
            at,
        };
        self.entries.push_front(Entry {
            item: item.clone(),
            from_row,
        });
        self.entries.truncate(CAP);
        Some(item)
    }

    pub(super) fn items(&self) -> Vec<InboxItem> {
        self.entries.iter().map(|e| e.item.clone()).collect()
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Records a message from `service`'s page and sends any new item to the
/// app's webview as `dock-inbox`, never to the pages.
pub(super) fn record(app: &AppHandle, service: &str, message: Message) {
    let item = with_state(app, |s| s.inbox.add(service, message, now_ms()));
    if let Some(item) = item {
        app.emit_to(EventTarget::webview(MAIN), "dock-inbox", item)
            .ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(sender: &str, preview: &str, tag: &str) -> Message {
        Message::Notification {
            sender: sender.into(),
            preview: preview.into(),
            tag: tag.into(),
        }
    }

    fn row(sender: &str, preview: &str) -> Message {
        Message::Row {
            sender: sender.into(),
            preview: preview.into(),
        }
    }

    #[test]
    fn newest_first_and_capped() {
        let mut inbox = Inbox::default();
        for i in 0..CAP + 5 {
            let added = inbox.add(
                "discord",
                note("a", &format!("m{i}"), &i.to_string()),
                i as i64,
            );
            assert!(added.is_some());
        }
        let items = inbox.items();
        assert_eq!(items.len(), CAP);
        assert_eq!(items[0].preview, format!("m{}", CAP + 4));
        assert_eq!(items[CAP - 1].preview, "m5");
        assert!(items.windows(2).all(|w| w[0].at >= w[1].at));
    }

    #[test]
    fn notifications_dedupe_by_tag_then_sender_and_body() {
        let mut inbox = Inbox::default();
        assert!(inbox.add("discord", note("a", "hi", "1"), 0).is_some());
        assert!(inbox.add("discord", note("b", "other", "1"), 1).is_none());
        assert!(inbox.add("discord", note("a", "hi", "2"), 2).is_some());
        assert!(inbox.add("whatsapp", note("a", "hi", "1"), 3).is_some());
        assert!(inbox.add("discord", note("c", "yo", ""), 4).is_some());
        assert!(inbox.add("discord", note("c", "yo", ""), 5).is_none());
        assert!(inbox.add("discord", note("c", "yo again", ""), 6).is_some());
        assert_eq!(inbox.items().len(), 5);
    }

    #[test]
    fn rows_dedupe_by_name_and_preview() {
        let mut inbox = Inbox::default();
        assert!(inbox.add("instagram", row("Ann", "Test"), 0).is_some());
        assert!(inbox.add("instagram", row("Ann", "Test"), 1).is_none());
        assert!(inbox
            .add("instagram", row("Ann", "2 new messages"), 2)
            .is_some());
        assert!(inbox.add("instagram", row("Bo", "Test"), 3).is_some());
        assert!(inbox.add("whatsapp", row("Ann", "Test"), 4).is_some());
    }

    #[test]
    fn a_key_returns_after_the_window_and_survives_clear() {
        let mut inbox = Inbox::default();
        assert!(inbox.add("instagram", row("Ann", "ok"), 0).is_some());
        inbox.clear();
        assert!(inbox.items().is_empty());
        assert!(inbox.add("instagram", row("Ann", "ok"), 1).is_none());
        assert!(inbox
            .add("instagram", row("Ann", "ok"), 1 + DEDUPE_MS)
            .is_some());
    }

    #[test]
    fn one_message_through_both_channels_is_one_item() {
        let mut inbox = Inbox::default();
        assert!(inbox.add("whatsapp", row("Ann", "hi"), 0).is_some());
        assert!(inbox
            .add("whatsapp", note("Ann", "Ann: hi", "x"), 500)
            .is_none());
        assert!(inbox
            .add("whatsapp", note("Ann", "later", "y"), CROSS_CHANNEL_MS)
            .is_some());
        assert!(inbox.add("whatsapp", note("Bo", "hi", "z"), 600).is_some());
        assert_eq!(inbox.items().len(), 3);
    }

    #[test]
    fn ids_are_unique_and_serde_is_camel_case() {
        let mut inbox = Inbox::default();
        let a = inbox.add("discord", note("a", "1", "1"), 0).unwrap();
        let b = inbox.add("discord", note("a", "2", "2"), 0).unwrap();
        assert_ne!(a.id, b.id);
        let json = serde_json::to_value(&a).unwrap();
        let mut keys: Vec<_> = json.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, ["at", "id", "preview", "sender", "service"]);
    }
}
