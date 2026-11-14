//! Streamed answer text a run has not committed as a row yet, kept so a
//! restart loses none of it (`store::recover`). Deltas gather here by the
//! message they belong to and reach `harness_stream_drafts` at most once per
//! [`INTERVAL`], as the whole uncommitted text.

use std::collections::HashMap;
use std::time::{Duration, Instant};

pub(super) const INTERVAL: Duration = Duration::from_millis(50);

/// One message's uncommitted text, under the provider's id for it.
pub(super) type Part = (Option<String>, String);

#[derive(Default)]
struct Draft {
    /// In first-seen order. A provider that names no message has one part.
    parts: Vec<Part>,
    /// Text arrived since the last write.
    dirty: bool,
    written: Option<Instant>,
}

impl Draft {
    fn text(&self) -> String {
        self.parts.iter().map(|(_, text)| text.as_str()).collect()
    }
}

#[derive(Default)]
pub(super) struct Drafts {
    runs: HashMap<i64, Draft>,
}

impl Drafts {
    /// A delta of `run`'s message `item`. Returns the draft to store now; a
    /// delta inside the interval waits for [`Self::due`].
    pub fn push(
        &mut self,
        run: i64,
        item: Option<&str>,
        text: &str,
        now: Instant,
    ) -> Option<String> {
        let draft = self.runs.entry(run).or_default();
        match draft.parts.iter_mut().find(|(id, _)| id.as_deref() == item) {
            Some((_, part)) => part.push_str(text),
            None => draft.parts.push((item.map(String::from), text.to_string())),
        }
        let waited = draft
            .written
            .is_none_or(|at| now.saturating_duration_since(at) >= INTERVAL);
        draft.dirty = !waited;
        if waited {
            draft.written = Some(now);
        }
        waited.then(|| draft.text())
    }

    /// `item` became a row, so its text is a draft no longer; text under no
    /// id goes with any message, since nothing says which one it was. Returns
    /// the draft to store now (empty: none is left), or `None` for a run that
    /// has none.
    pub fn commit(&mut self, run: i64, item: Option<&str>, now: Instant) -> Option<String> {
        let draft = self.runs.get_mut(&run)?;
        draft
            .parts
            .retain(|(id, _)| id.is_some() && id.as_deref() != item);
        let text = draft.text();
        if text.is_empty() {
            self.runs.remove(&run);
        } else {
            draft.dirty = false;
            draft.written = Some(now);
        }
        Some(text)
    }

    /// The drafts whose interval has passed, to store.
    pub fn due(&mut self, now: Instant) -> Vec<(i64, String)> {
        let mut out = Vec::new();
        for (run, draft) in &mut self.runs {
            let waited = draft
                .written
                .is_none_or(|at| now.saturating_duration_since(at) >= INTERVAL);
            if draft.dirty && waited {
                draft.dirty = false;
                draft.written = Some(now);
                out.push((*run, draft.text()));
            }
        }
        out
    }

    /// Every draft still waiting, to store: the bus has closed.
    pub fn waiting(&mut self) -> Vec<(i64, String)> {
        let mut out = Vec::new();
        for (run, draft) in &mut self.runs {
            if std::mem::take(&mut draft.dirty) {
                out.push((*run, draft.text()));
            }
        }
        out
    }

    /// When the next waiting draft is due.
    pub fn next_due(&self) -> Option<Instant> {
        self.runs
            .values()
            .filter(|d| d.dirty)
            .filter_map(|d| d.written.map(|at| at + INTERVAL))
            .min()
    }

    /// The run is over: its uncommitted parts, with nothing left behind.
    pub fn take(&mut self, run: i64) -> Vec<Part> {
        self.runs.remove(&run).map(|d| d.parts).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deltas_inside_the_interval_are_stored_as_one_write() {
        let mut drafts = Drafts::default();
        let start = Instant::now();
        assert_eq!(
            drafts.push(1, Some("m"), "Hel", start).as_deref(),
            Some("Hel")
        );
        let soon = start + Duration::from_millis(10);
        assert_eq!(drafts.push(1, Some("m"), "lo", soon), None);
        assert_eq!(drafts.push(1, Some("m"), " there", soon), None);
        assert!(drafts.due(soon).is_empty(), "the interval has not passed");
        assert_eq!(drafts.next_due(), Some(start + INTERVAL));
        assert_eq!(
            drafts.due(start + INTERVAL),
            vec![(1, "Hello there".to_string())],
            "one write, holding the whole text"
        );
        assert_eq!(drafts.next_due(), None);
        assert!(drafts.due(start + INTERVAL * 4).is_empty(), "nothing new");
    }

    #[test]
    fn a_committed_message_leaves_the_others_text_in_the_draft() {
        let mut drafts = Drafts::default();
        let now = Instant::now();
        drafts.push(1, Some("a"), "first", now);
        drafts.push(1, Some("b"), "second", now);
        assert_eq!(drafts.commit(1, Some("a"), now).as_deref(), Some("second"));
        assert_eq!(drafts.commit(1, Some("b"), now).as_deref(), Some(""));
        assert_eq!(drafts.commit(1, Some("b"), now), None, "no draft left");
    }

    #[test]
    fn text_under_no_id_goes_with_any_committed_message() {
        let mut drafts = Drafts::default();
        let now = Instant::now();
        drafts.push(1, None, "streamed", now);
        drafts.push(1, None, " more", now);
        assert_eq!(drafts.commit(1, Some("uuid/0"), now).as_deref(), Some(""));
    }

    #[test]
    fn a_finished_run_hands_back_its_parts_in_order() {
        let mut drafts = Drafts::default();
        let now = Instant::now();
        drafts.push(1, Some("a"), "one", now);
        drafts.push(1, Some("b"), "two", now);
        drafts.push(2, None, "other run", now);
        assert_eq!(
            drafts.take(1),
            vec![
                (Some("a".to_string()), "one".to_string()),
                (Some("b".to_string()), "two".to_string())
            ]
        );
        assert!(drafts.take(1).is_empty());
        assert_eq!(drafts.take(2).len(), 1);
    }
}
