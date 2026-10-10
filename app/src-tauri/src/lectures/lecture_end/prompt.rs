//! The brief and the per-lecture prompt.

use super::window::{clock, transcript_lines, Line};
use super::TAIL_SECS;

/// The one-off brief: Claude's appended system prompt, Codex's developer
/// instructions, opencode's `oculus-lecture-end` agent, ahead of agy's message.
pub const INSTRUCTIONS: &str = "You find where a university lecture recording's planned content \
ends.\n\n\
You are shown the last 15 minutes of an automatic transcript, one line per cue: \
`second  clock  speaker: text`. A speaker is named only where the voice changes, and not at \
all when only one is heard. The second and the clock are the same moment.\n\n\
Recordings keep running after the lecturer finishes — students come to the lectern with \
questions, people pack up and chat — and the microphone keeps transcribing. The end is the line \
where the lecturer finishes the lecture: their closing words or wrap-up. It is not a student \
thanking them, and not \"I'll stop the recording\" said after a Q&A that followed an earlier \
wrap-up — then the end is that earlier wrap-up. A Q&A after the close is not lecture, even if it \
is long. A note that the projector goes black says the lecture is over by then; the end is still \
a line of the transcript. If the lecturer is still teaching at the last line, the recording was \
cut off and there is no end.\n\n\
Reply with only JSON — no prose, no tools:\n\
{\"ends_at\": <that line's first column, as a bare number like 1911>, \"quote\": \"<3 to 12 \
words copied exactly from that line>\"}\n\
or, when there is no end:\n\
{\"ends_at\": null, \"quote\": null}";

pub struct Prompt<'a> {
    pub title: &'a str,
    pub code: Option<&'a str>,
    pub length: u32,
    pub black_from: Option<u32>,
    pub lines: &'a [Line],
}

/// The turn's message: the lecture, its length, the optional black-projector
/// line, then the window's transcript inline — no files to open.
pub fn prompt(p: &Prompt) -> String {
    let mut out = format!("Lecture: {}\n", p.title);
    if let Some(code) = p.code {
        out.push_str(&format!("Course: {code}\n"));
    }
    out.push_str(&format!(
        "Recording length: {} ({} s)\n",
        clock(p.length),
        p.length
    ));
    if let Some(at) = p.black_from {
        out.push_str(&format!(
            "The projector goes black from {} ({at}) to the end of the recording.\n",
            clock(at)
        ));
    }
    let heading = if p.length > TAIL_SECS {
        "The transcript's last 15 minutes:"
    } else {
        "The whole transcript:"
    };
    out.push_str(&format!("\n{heading}\n\n{}\n", transcript_lines(p.lines)));
    out
}
