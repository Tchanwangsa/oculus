//! Naming a thread from its first exchange: the prompt the namer is given
//! and the cleaning of what it answers.

/// Long enough for a cold CLI start, short enough that a wedged one does not
/// leave a thread forever "being named".
pub(super) const NAMING_TIMEOUT_SECS: u64 = 90;

pub(super) const NAMING_INSTRUCTIONS: &str =
    "You name conversations. Reply with the name alone — never a sentence about it.";

/// How much of the exchange the namer sees. A name comes from what was asked
/// and the shape of the answer; the rest is tokens.
const NAMING_CLIP: usize = 800;

pub(super) fn naming_prompt(first_message: &str, reply: &str) -> String {
    let clip = |s: &str| -> String {
        let t: String = s.chars().take(NAMING_CLIP).collect();
        if s.chars().count() > NAMING_CLIP {
            format!("{t}…")
        } else {
            t
        }
    };
    format!(
        "Name this conversation between a university student and their study assistant.\n\n\
         Reply with the name and nothing else: three to six words, sentence case, no quotes and \
         no full stop. Name what the conversation is *about* — the topic, the subject, the \
         artefact — not what happened in it. Do not write \"the student asks\" or \"discussion \
         of\".\n\n\
         <student>\n{}\n</student>\n\n<assistant>\n{}\n</assistant>",
        clip(first_message.trim()),
        clip(reply.trim()),
    )
}

/// What survives from a naming reply: the first non-empty line, stripped of
/// labels and quoting. Anything long enough to be prose is refused, so the
/// first-line title stays instead.
pub(super) fn clean_title(raw: &str) -> Option<String> {
    let line = raw.lines().find(|l| !l.trim().is_empty())?.trim();
    let line = line
        .strip_prefix("Title:")
        .or_else(|| line.strip_prefix("Name:"))
        .unwrap_or(line);
    let line = line
        .trim()
        .trim_matches(|c| matches!(c, '"' | '\'' | '`' | '*' | '#'))
        .trim();
    let line = line.trim_end_matches(['.', '!']).trim();
    if line.is_empty() || line.chars().count() > 60 {
        return None;
    }
    Some(line.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_taken_out_of_whatever_the_model_wrapped_it_in() {
        assert_eq!(
            clean_title("Dijkstra worked example").as_deref(),
            Some("Dijkstra worked example")
        );
        assert_eq!(
            clean_title("\"Week 6 tutorial questions\"\n").as_deref(),
            Some("Week 6 tutorial questions")
        );
        assert_eq!(
            clean_title("Title: **Semaphores and deadlock**").as_deref(),
            Some("Semaphores and deadlock")
        );
        assert_eq!(
            clean_title("Assignment 2 marking scheme.").as_deref(),
            Some("Assignment 2 marking scheme")
        );
        assert_eq!(clean_title(""), None);
        assert_eq!(clean_title("   \n\n "), None);
        assert_eq!(
            clean_title("The student asks about the difficulty of the week 6 lecture and the assistant replies"),
            None,
            "prose is refused rather than becoming the name"
        );
    }
}
