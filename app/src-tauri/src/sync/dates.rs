//! Source timestamps are UTC RFC 3339; missing or invalid dates stay unknown.

use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct SourceDates {
    pub source_created_at: Option<String>,
    pub source_updated_at: Option<String>,
}

fn timestamp(value: &Value) -> Option<String> {
    DateTime::parse_from_rfc3339(value.as_str()?)
        .ok()
        .map(|date| {
            date.with_timezone(&Utc)
                .to_rfc3339_opts(SecondsFormat::Millis, true)
        })
}

impl SourceDates {
    pub(super) fn file(item: &Value) -> Self {
        Self {
            source_created_at: timestamp(&item["created_at"]).max(timestamp(&item["unlock_at"])),
            source_updated_at: timestamp(&item["modified_at"])
                .or_else(|| timestamp(&item["updated_at"])),
        }
    }

    pub(super) fn page(item: &Value) -> Self {
        Self {
            source_created_at: timestamp(&item["created_at"]),
            source_updated_at: timestamp(&item["updated_at"]),
        }
    }

    pub(super) fn announcement(item: &Value) -> Self {
        let posted = timestamp(&item["posted_at"]).or_else(|| timestamp(&item["created_at"]));
        Self {
            source_created_at: posted.clone(),
            source_updated_at: posted,
        }
    }

    pub(super) fn assignment(item: &Value) -> Self {
        Self {
            source_created_at: timestamp(&item["unlock_at"])
                .or_else(|| timestamp(&item["created_at"])),
            source_updated_at: timestamp(&item["updated_at"]),
        }
    }

    pub(super) fn thread(listing: &Value, detail: &Value) -> Self {
        let thread = if detail["thread"].is_object() {
            &detail["thread"]
        } else {
            listing
        };
        let created =
            timestamp(&thread["created_at"]).or_else(|| timestamp(&listing["created_at"]));
        Self {
            source_created_at: created,
            source_updated_at: latest_reply(thread),
        }
    }
}

/// Only reply creation counts as discussion activity, not edits or endorsements.
fn latest_reply(item: &Value) -> Option<String> {
    ["answers", "comments"]
        .into_iter()
        .flat_map(|key| item[key].as_array().into_iter().flatten())
        .filter_map(|reply| timestamp(&reply["created_at"]).max(latest_reply(reply)))
        .max()
}
