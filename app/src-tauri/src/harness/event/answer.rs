//! Which of a turn's replies is its answer (docs/harness.md, "One answer per
//! run"). Settlement picks a stored row by it; one-off turns and job runs
//! pick their collected text by it, so neither ever joins replies together.

use super::{HarnessEvent, Phase, Source};

/// One reply's say in the choice, in provider order: its native phase
/// (`final_answer`, `commentary`, anything else) and whether it has text.
pub struct Candidate<'a> {
    pub phase: Option<&'a str>,
    pub empty: bool,
}

/// The answer's index in `replies`, and why. A Codex `final_answer` wins;
/// otherwise the last reply not marked commentary, even one before a tool.
/// Length and wording never decide.
pub fn select(replies: &[Candidate]) -> Option<(usize, &'static str)> {
    let eligible = || replies.iter().enumerate().rev().filter(|(_, r)| !r.empty);
    if let Some((i, _)) = eligible().find(|(_, r)| r.phase == Some("final_answer")) {
        return Some((i, "native_final"));
    }
    eligible()
        .find(|(_, r)| r.phase != Some("commentary"))
        .map(|(i, _)| (i, "terminal_fallback"))
}

/// A turn's replies off the event stream, for a caller with no rows: the
/// same source again replaces its text, as the store's upsert does.
#[derive(Default)]
pub struct Replies {
    replies: Vec<(Option<String>, Phase, String)>,
}

impl Replies {
    /// Take in one event; anything but an `AssistantMessage` is ignored.
    pub fn observe(&mut self, ev: &HarnessEvent) {
        let HarnessEvent::AssistantMessage { text, source } = ev else {
            return;
        };
        let id = source.as_ref().map(|s| s.id.clone());
        let phase = source.as_ref().map_or(Phase::Unknown, |s: &Source| s.phase);
        match self
            .replies
            .iter_mut()
            .find(|(known, ..)| id.is_some() && *known == id)
        {
            Some(reply) => *reply = (id, phase, text.clone()),
            None => self.replies.push((id, phase, text.clone())),
        }
    }

    /// The selected reply's text, or `""` when there is none.
    pub fn answer(&self) -> &str {
        let candidates: Vec<Candidate> = self
            .replies
            .iter()
            .map(|(_, phase, text)| Candidate {
                phase: Some(phase.as_str()),
                empty: text.trim().is_empty(),
            })
            .collect();
        select(&candidates).map_or("", |(i, _)| self.replies[i].2.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(text: &str, id: &str, phase: Phase) -> HarnessEvent {
        HarnessEvent::AssistantMessage {
            text: text.into(),
            source: Some(Source {
                id: id.into(),
                phase,
                partial: false,
            }),
        }
    }

    #[test]
    fn a_one_off_answer_is_one_reply_never_the_replies_joined() {
        let mut replies = Replies::default();
        replies.observe(&message("Let me think.", "a", Phase::Unknown));
        replies.observe(&message("Week 3 notes", "b", Phase::Unknown));
        assert_eq!(replies.answer(), "Week 3 notes");
    }

    #[test]
    fn the_same_source_again_replaces_its_text() {
        let mut replies = Replies::default();
        replies.observe(&message("Week", "a", Phase::Unknown));
        replies.observe(&message("Week 3 notes", "a", Phase::Unknown));
        assert_eq!(replies.answer(), "Week 3 notes");
        assert_eq!(replies.replies.len(), 1);
    }

    #[test]
    fn a_native_final_answer_wins_over_later_commentary() {
        let mut replies = Replies::default();
        replies.observe(&message("The answer", "a", Phase::FinalAnswer));
        replies.observe(&message("Checking", "b", Phase::Commentary));
        assert_eq!(replies.answer(), "The answer");
    }

    #[test]
    fn nothing_said_is_no_answer() {
        let mut replies = Replies::default();
        replies.observe(&message("  ", "a", Phase::Unknown));
        assert_eq!(replies.answer(), "");
    }
}
