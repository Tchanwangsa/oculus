//! Inline completions for the document editor: one tool-less turn per pause
//! in typing (`Harness::one_off`) on the `documentSuggestions` job's model.
//!
//! Only the newest request is wanted. A new one, or `document_suggest_cancel`,
//! stops the turn in flight — killed where the CLI is a process per turn,
//! interrupted on a server — so a stale turn does not keep spending, and the
//! stopped call answers empty. Claude and Antigravity read the prompt off
//! stdin, so one process is spawned ahead of the next request and kept warm;
//! a different selection discards it. See docs/harness.md.

mod prompt;
mod reply;
mod request;

use super::jobs::JobSelection;
use super::manager::{Handle, OneOff};
use super::Provider;

/// The brief: Claude's appended system prompt, Codex's developer
/// instructions, opencode's `oculus-writer` agent, ahead of agy's message.
pub const INSTRUCTIONS: &str =
    "You are the inline autocomplete in a university student's markdown \
note editor. You are shown the note with the caret marked <CARET/>, and you reply with only the \
text to insert there — usually a few words, at most one sentence of about 25 words.\n\n\
- Continue the sentence or line the caret is on, naturally, in the note's own language, voice and \
markdown. Maths is written $…$ inline and $$…$$ for display, like the rest of the note.\n\
- Never repeat text that is already before the caret, and never restate what follows it.\n\
- If the caret is in the middle of a word, continue that word with no leading space. If your text \
starts a new word, begin it with a single space.\n\
- No preamble, no explanation, no quotes or code fences around the text, and no tool calls.\n\
- If there is nothing useful to add, reply with nothing at all.";

/// The turn in flight and the warm spare. `wanted` is the one request whose
/// answer is still worth returning.
#[derive(Default)]
pub(super) struct Suggestions {
    wanted: Option<u64>,
    running: Option<(u64, Handle)>,
    warm: Option<Warm>,
}

/// A spawned, unprompted session for the selection it was started under.
struct Warm {
    key: Key,
    turn: OneOff,
}

#[derive(Clone, PartialEq)]
struct Key {
    provider: Provider,
    model: String,
    effort: Option<String>,
}

impl Key {
    fn of(sel: &JobSelection) -> Key {
        Key {
            provider: sel.provider,
            model: sel.model.clone(),
            effort: sel.reasoning_effort.clone(),
        }
    }
}

/// The CLIs that spawn a process per session and wait on stdin for the
/// prompt, so a spare can be started early. Codex and opencode keep a server
/// up already; a session there is one cheap request.
fn warmable(provider: Provider) -> bool {
    matches!(provider, Provider::Claude | Provider::Antigravity)
}
