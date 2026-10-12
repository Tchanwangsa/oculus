//! What a dock page may say over the `dock` channel, read as hostile input:
//! a dictionary with a string `kind`, string fields only, every string
//! clamped. Anything else is dropped.

/// Longest raw string read from a body, in UTF-16 units, before clamping.
/// A longer one is dropped rather than copied out of WebKit.
pub(super) const MAX_RAW: usize = 8192;
pub(super) const MAX_SENDER: usize = 120;
pub(super) const MAX_PREVIEW: usize = 400;
/// Only used as a dedupe key (Discord's is a message id).
pub(super) const MAX_TAG: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Message {
    /// From the Notification shim: the site raised a web notification.
    Notification {
        sender: String,
        preview: String,
        tag: String,
    },
    /// From the chat-list row watcher: a row became unread, or its unread
    /// preview changed. `sender` is the row's name (a person or a group).
    Row { sender: String, preview: String },
}

impl Message {
    /// Builds a message from a body's string fields; `field` returns `None`
    /// for a missing or non-string field.
    pub(super) fn parse(field: impl Fn(&str) -> Option<String>) -> Option<Self> {
        let text = |key: &str, max: usize| clamp(&field(key).unwrap_or_default(), max);
        let message = match field("kind")?.as_str() {
            "notification" => Message::Notification {
                sender: text("title", MAX_SENDER),
                preview: text("body", MAX_PREVIEW),
                tag: text("tag", MAX_TAG),
            },
            "row" => Message::Row {
                sender: text("name", MAX_SENDER),
                preview: text("preview", MAX_PREVIEW),
            },
            _ => return None,
        };
        let (sender, preview) = message.parts();
        match message {
            Message::Row { .. } if sender.is_empty() => None,
            _ if sender.is_empty() && preview.is_empty() => None,
            _ => Some(message),
        }
    }

    pub(super) fn parts(&self) -> (&str, &str) {
        match self {
            Message::Notification {
                sender, preview, ..
            }
            | Message::Row { sender, preview } => (sender, preview),
        }
    }
}

/// One line of at most `max` characters: control characters (newlines
/// included) become spaces, runs of whitespace collapse, ends are trimmed.
pub(super) fn clamp(raw: &str, max: usize) -> String {
    let mut out = String::new();
    let mut count = 0;
    for word in raw
        .split(|c: char| c.is_whitespace() || c.is_control())
        .filter(|w| !w.is_empty())
    {
        if count > 0 {
            // A separator needs room for at least one character after it.
            if count + 2 > max {
                break;
            }
            out.push(' ');
            count += 1;
        }
        for c in word.chars() {
            if count == max {
                break;
            }
            out.push(c);
            count += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn parse(pairs: &[(&str, &str)]) -> Option<Message> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Message::parse(|key| map.get(key).cloned())
    }

    #[test]
    fn reads_a_discord_notification() {
        assert_eq!(
            parse(&[
                ("kind", "notification"),
                ("title", "b0nniee150"),
                ("body", "IM BORED"),
                ("tag", "1558705393918681110"),
                ("icon", "data:image/png;base64,AAAA"),
            ]),
            Some(Message::Notification {
                sender: "b0nniee150".into(),
                preview: "IM BORED".into(),
                tag: "1558705393918681110".into(),
            })
        );
    }

    #[test]
    fn reads_a_row() {
        assert_eq!(
            parse(&[
                ("kind", "row"),
                ("name", "INFOSEC Group"),
                ("preview", "~Paige | Thank you guys for that!"),
            ]),
            Some(Message::Row {
                sender: "INFOSEC Group".into(),
                preview: "~Paige | Thank you guys for that!".into(),
            })
        );
    }

    #[test]
    fn drops_unknown_or_empty_messages() {
        assert_eq!(parse(&[]), None);
        assert_eq!(parse(&[("title", "x")]), None);
        assert_eq!(parse(&[("kind", "boot"), ("title", "x")]), None);
        assert_eq!(parse(&[("kind", "Notification"), ("title", "x")]), None);
        assert_eq!(parse(&[("kind", "notification")]), None);
        assert_eq!(parse(&[("kind", "notification"), ("title", " \n ")]), None);
        assert_eq!(parse(&[("kind", "row"), ("preview", "hi")]), None);
        assert!(parse(&[("kind", "notification"), ("body", "no title")]).is_some());
        assert!(parse(&[("kind", "row"), ("name", "Ann")]).is_some());
    }

    #[test]
    fn clamps_every_string() {
        let long = "é".repeat(5000);
        let Some(Message::Notification {
            sender,
            preview,
            tag,
        }) = parse(&[
            ("kind", "notification"),
            ("title", &long),
            ("body", &long),
            ("tag", &long),
        ])
        else {
            panic!("expected a notification");
        };
        assert_eq!(sender.chars().count(), MAX_SENDER);
        assert_eq!(preview.chars().count(), MAX_PREVIEW);
        assert_eq!(tag.chars().count(), MAX_TAG);
    }

    #[test]
    fn clamp_makes_one_trimmed_line() {
        assert_eq!(clamp("  hi\n\tthere\u{0}you  ", 100), "hi there you");
        assert_eq!(clamp("abc def", 5), "abc d");
        assert_eq!(clamp("abc def", 3), "abc");
        assert_eq!(clamp("abc def", 4), "abc");
        assert_eq!(clamp("", 4), "");
        assert_eq!(clamp("🎱🎱🎱", 2), "🎱🎱");
    }
}
