//! Inline completions for the document editor: one tool-less turn per pause
//! in typing ([`Harness::one_off`]) on the `documentSuggestions` job's model.
//!
//! Only the newest request is wanted. A new one, or `document_suggest_cancel`,
//! stops the turn in flight — killed where the CLI is a process per turn,
//! interrupted on a server — so a stale turn does not keep spending, and the
//! stopped call answers empty. Claude and Antigravity read the prompt off
//! stdin, so one process is spawned ahead of the next request and kept warm;
//! a different selection discards it. See docs/harness.md.

use std::sync::Arc;

use super::jobs::JobSelection;
use super::{opencode, Handle, Harness, OneOff, Provider};

/// The brief: Claude's appended system prompt, Codex's developer
/// instructions, opencode's `oculus-writer` agent, ahead of agy's message.
pub const INSTRUCTIONS: &str = "You are the inline autocomplete in a university student's markdown \
note editor. You are shown the note with the caret marked <CARET/>, and you reply with only the \
text to insert there — usually a few words, at most one sentence of about 25 words.\n\n\
- Continue the sentence or line the caret is on, naturally, in the note's own language, voice and \
markdown. Maths is written $…$ inline and $$…$$ for display, like the rest of the note.\n\
- Never repeat text that is already before the caret, and never restate what follows it.\n\
- If the caret is in the middle of a word, continue that word with no leading space. If your text \
starts a new word, begin it with a single space.\n\
- No preamble, no explanation, no quotes or code fences around the text, and no tool calls.\n\
- If there is nothing useful to add, reply with nothing at all.";

/// Where the caret is, in the prompt. Never in the note itself.
const CARET: &str = "<CARET/>";
/// How much of the note the prompt carries either side of the caret; the
/// webview already sends about this much.
const BEFORE_CHARS: usize = 4000;
const AFTER_CHARS: usize = 1500;
/// The longest suggestion kept: the brief asks for one sentence.
const MAX_WORDS: usize = 30;
const MAX_CHARS: usize = 240;

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

impl Harness {
    /// One completion for the caret between `before` and `after` in the note
    /// at `path` (library-relative). Empty when there is nothing to add, or
    /// when a newer request or a cancel superseded this one.
    pub fn suggest(
        self: &Arc<Self>,
        request_id: u64,
        sel: &JobSelection,
        path: &str,
        before: &str,
        after: &str,
    ) -> Result<String, String> {
        let stale = {
            let mut s = self.suggest.lock().unwrap();
            // An older request arriving after a newer one is already stale.
            if s.wanted.is_some_and(|w| w > request_id) {
                return Ok(String::new());
            }
            s.wanted = Some(request_id);
            s.running.take()
        };
        // Outside the lock: on Codex this is a request to the server.
        if let Some((_, h)) = stale {
            let _ = h.cancel();
        }
        let Some(prompt) = prompt(path, before, after) else {
            self.release(request_id);
            return Ok(String::new());
        };

        let key = Key::of(sel);
        let turn = match self.take_warm(&key) {
            Some(t) => t,
            None => match self.one_off(sel, INSTRUCTIONS, opencode::WRITER_AGENT) {
                Ok(t) => t,
                Err(e) => {
                    self.release(request_id);
                    return Err(e);
                }
            },
        };
        {
            let mut s = self.suggest.lock().unwrap();
            if s.wanted != Some(request_id) {
                // Never prompted, so it is still a spare.
                drop(s);
                self.keep_warm(key, turn);
                return Ok(String::new());
            }
            s.running = Some((request_id, turn.handle.clone()));
        }

        let sent = turn.handle.send(&prompt);
        // A cancel that landed while the prompt was going out can miss a
        // turn the server had not started yet, so it is repeated here.
        if self.suggest.lock().unwrap().wanted != Some(request_id) {
            let _ = turn.handle.cancel();
        }
        let reply = sent.map(|()| turn.wait(None, "suggesting"));
        turn.close();
        let current = {
            let mut s = self.suggest.lock().unwrap();
            if s.running.as_ref().is_some_and(|(id, _)| *id == request_id) {
                s.running = None;
            }
            s.wanted == Some(request_id)
        };
        self.release(request_id);
        self.refill_warm(sel);
        if !current {
            return Ok(String::new());
        }
        let reply = reply?;
        if let Some(e) = reply.failed {
            return Err(e);
        }
        let raw = if reply.streamed.trim().is_empty() {
            &reply.message
        } else {
            &reply.streamed
        };
        Ok(clean(raw, before, after))
    }

    /// Stop whatever suggestion is in flight; its call answers empty. The
    /// warm spare stays.
    pub fn cancel_suggestion(&self) {
        let running = {
            let mut s = self.suggest.lock().unwrap();
            s.wanted = None;
            s.running.take()
        };
        if let Some((_, h)) = running {
            let _ = h.cancel();
        }
    }

    /// The turn in flight and the spare, on quit.
    pub(super) fn drop_suggestions(&self) {
        self.cancel_suggestion();
        let warm = self.suggest.lock().unwrap().warm.take();
        if let Some(w) = warm {
            w.turn.close();
        }
    }

    /// This request is over; a later one with a lower id (a reloaded page
    /// counts from 1 again) is no longer refused.
    fn release(&self, request_id: u64) {
        let mut s = self.suggest.lock().unwrap();
        if s.wanted == Some(request_id) {
            s.wanted = None;
        }
    }

    /// The spare, if it was started under this selection and is still alive.
    /// Any other spare is closed.
    fn take_warm(&self, key: &Key) -> Option<OneOff> {
        let warm = self.suggest.lock().unwrap().warm.take()?;
        if warm.key == *key && warm.turn.handle.is_alive() {
            return Some(warm.turn);
        }
        warm.turn.close();
        None
    }

    /// Keep `turn` as the spare when there is none, else close it.
    fn keep_warm(&self, key: Key, turn: OneOff) {
        let extra = {
            let mut s = self.suggest.lock().unwrap();
            let have = s.warm.as_ref().is_some_and(|w| w.turn.handle.is_alive());
            if warmable(key.provider) && !have {
                s.warm.replace(Warm { key, turn }).map(|w| w.turn)
            } else {
                Some(turn)
            }
        };
        if let Some(t) = extra {
            t.close();
        }
    }

    /// Start the next request's process now, off the caller's thread.
    fn refill_warm(self: &Arc<Self>, sel: &JobSelection) {
        if !warmable(sel.provider) {
            return;
        }
        let key = Key::of(sel);
        {
            let s = self.suggest.lock().unwrap();
            if s.warm.as_ref().is_some_and(|w| w.key == key && w.turn.handle.is_alive()) {
                return;
            }
        }
        let (h, sel) = (self.clone(), sel.clone());
        std::thread::spawn(move || match h.one_off(&sel, INSTRUCTIONS, opencode::WRITER_AGENT) {
            Ok(turn) => h.keep_warm(key, turn),
            Err(e) => eprintln!("[oculus] suggestion warm-up: {e}"),
        });
    }
}

// ── The prompt ───────────────────────────────────────────────────────────────

/// The user turn, or None when no turn should be spent: the caret sits inside
/// a word (letters on both sides), where any insertion splits it.
fn prompt(path: &str, before: &str, after: &str) -> Option<String> {
    let last = before.chars().next_back();
    let next = after.chars().next();
    if last.is_some_and(char::is_alphanumeric) && next.is_some_and(char::is_alphanumeric) {
        return None;
    }
    let (title, subject) = note_names(path);
    let subject = subject.map(|c| format!(", in the subject {c}")).unwrap_or_default();
    let (before, after) = (tail(before, BEFORE_CHARS), head(after, AFTER_CHARS));
    Some(format!(
        "Complete the student's note at the caret.\n\n\
         The note is \"{title}\"{subject} (`{path}`). {}\n\n\
         <note>\n{before}{CARET}{after}\n</note>\n\n\
         Reply with only the text to insert at {CARET}.",
        caret_note(&before),
    ))
}

/// The note's title (its file stem) and subject code, from a path like
/// `courses/<code>/documents/<name>.md`.
fn note_names(path: &str) -> (String, Option<String>) {
    let mut parts = path.split('/');
    let subject = match (parts.next(), parts.next()) {
        (Some("courses"), Some(code)) if !code.is_empty() && path.matches('/').count() >= 2 => {
            Some(code.to_string())
        }
        _ => None,
    };
    let name = path.rsplit('/').next().unwrap_or(path);
    let stem = name.strip_suffix(".md").unwrap_or(name);
    (stem.replace('_', " "), subject)
}

/// What the text just before the caret means for the first character of
/// the reply — the one thing a model cannot see from the marker alone.
fn caret_note(before: &str) -> String {
    let Some(last) = before.chars().next_back() else {
        return "The caret is at the start of the note.".into();
    };
    if before.trim().is_empty() {
        return "The caret is at the start of the note.".into();
    }
    if last == '\n' {
        return "The caret is at the start of a new line.".into();
    }
    if last.is_whitespace() {
        return "The caret follows a space, so do not start with one.".into();
    }
    if last.is_alphanumeric() {
        let fragment: String = {
            let rev: Vec<char> = before.chars().rev().take_while(|c| c.is_alphanumeric()).take(40).collect();
            rev.into_iter().rev().collect()
        };
        return format!(
            "The caret is directly after \"{fragment}\", with no space. If that word is unfinished, \
             continue it with no leading space; if it is complete, start with a space."
        );
    }
    format!("The caret is directly after \"{last}\". Start with a space if the next word needs one.")
}

/// The last `n` characters of `s`, marked when cut.
fn tail(s: &str, n: usize) -> String {
    match s.char_indices().rev().nth(n.saturating_sub(1)) {
        Some((i, _)) if i > 0 => format!("…{}", &s[i..]),
        _ => s.to_string(),
    }
}

/// The first `n` characters of `s`, marked when cut.
fn head(s: &str, n: usize) -> String {
    match s.char_indices().nth(n) {
        Some((i, _)) => format!("{}…", &s[..i]),
        None => s.to_string(),
    }
}

// ── The reply ────────────────────────────────────────────────────────────────

/// The text to insert, out of whatever the model wrapped it in: no quotes or
/// fences, no echo of the text either side of the caret, one line, one
/// sentence's length, and spacing that fits the caret. Empty means none.
fn clean(raw: &str, before: &str, after: &str) -> String {
    let raw = raw.replace(CARET, "");
    let (lead, body) = unwrap(&raw);
    // A suggestion continues the line it is on.
    let body = body.split('\n').next().unwrap_or("").trim_end_matches('\r');
    let mut s = strip_echo(&lead, body, before);
    s = strip_after_echo(&s, after);

    // Spacing: never double the space before the caret, and leave one
    // between the suggestion and a word right after it.
    let at_gap = before.is_empty() || before.ends_with(char::is_whitespace);
    let s = if at_gap { s.trim_start().to_string() } else { collapse_lead(&s) };
    let mut out = cap(s.trim_end());
    if out.trim().is_empty() {
        return String::new();
    }
    if after.starts_with(|c: char| c.is_alphanumeric()) && !out.ends_with(char::is_whitespace) {
        out.push(' ');
    }
    out
}

/// The reply with a label, code fence or matching quotes taken off, and its
/// leading whitespace apart, since that decides the spacing.
fn unwrap(raw: &str) -> (String, String) {
    let mut t = raw.trim();
    let lead: String = raw.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
    // A model that set the text on a line of its own meant a new word.
    let lead = if lead.is_empty() && raw.starts_with(['\n', '\r']) && !t.is_empty() {
        " ".to_string()
    } else {
        lead
    };
    for label in ["completion:", "suggestion:", "continuation:"] {
        if t.get(..label.len()).is_some_and(|h| h.eq_ignore_ascii_case(label)) {
            t = t[label.len()..].trim();
        }
    }
    if let Some(rest) = t.strip_prefix("```") {
        // The fence's own line may name a language.
        let inner = match rest.split_once('\n') {
            Some((_, body)) => body,
            None => rest,
        };
        let inner = inner.trim_end();
        let inner = inner.strip_suffix("```").unwrap_or(inner);
        return (lead, inner.trim().to_string());
    }
    for (open, close) in [('"', '"'), ('“', '”'), ('\'', '\''), ('«', '»')] {
        let mut cs = t.chars();
        if t.chars().count() >= 2 && cs.next() == Some(open) && cs.next_back() == Some(close) {
            let inner = cs.as_str();
            if !inner.contains(open) && !inner.contains(close) {
                return (lead, inner.trim().to_string());
            }
        }
    }
    (lead, t.to_string())
}

/// Drop a restatement of the text before the caret from the reply's start:
/// the longest run that `before` ends with, starting on a word boundary
/// there. A reply that opened with a space meant a new word, so only a long
/// overlap counts as an echo then.
fn strip_echo(lead: &str, body: &str, before: &str) -> String {
    let min = if lead.is_empty() { 3 } else { 12 };
    let mut best: Option<usize> = None;
    for (end, _) in body.char_indices().skip(1).chain(std::iter::once((body.len(), ' '))) {
        let piece = &body[..end];
        if piece.chars().count() < min || !before.ends_with(piece) {
            continue;
        }
        let start = before.len() - piece.len();
        let boundary = before[..start].chars().next_back().is_none_or(|c| !c.is_alphanumeric());
        if boundary {
            best = Some(end);
        }
    }
    match best {
        Some(end) => body[end..].to_string(),
        None => format!("{lead}{body}"),
    }
}

/// Drop a restatement of the text after the caret from the reply's end.
fn strip_after_echo(s: &str, after: &str) -> String {
    let after = after.trim_start();
    let trimmed = s.trim_end();
    let mut cut = None;
    for (start, _) in trimmed.char_indices() {
        let piece = &trimmed[start..];
        if piece.trim().chars().count() >= 4 && after.starts_with(piece.trim_start()) {
            cut = Some(start);
            break;
        }
    }
    match cut {
        Some(start) => trimmed[..start].to_string(),
        None => s.to_string(),
    }
}

/// At most one space ahead of the text.
fn collapse_lead(s: &str) -> String {
    let trimmed = s.trim_start();
    if trimmed.len() == s.len() {
        s.to_string()
    } else {
        format!(" {trimmed}")
    }
}

/// The first [`MAX_WORDS`] words and [`MAX_CHARS`] characters, cut at a word.
fn cap(s: &str) -> String {
    let mut words = 0;
    let mut in_word = false;
    let mut end = s.len();
    for (i, c) in s.char_indices() {
        if c.is_whitespace() {
            in_word = false;
        } else if !in_word {
            in_word = true;
            words += 1;
            if words > MAX_WORDS {
                end = i;
                break;
            }
        }
    }
    let s = &s[..end];
    if s.chars().count() <= MAX_CHARS {
        return s.trim_end().to_string();
    }
    let limit = s.char_indices().nth(MAX_CHARS).map(|(i, _)| i).unwrap_or(s.len());
    let cut = s[..limit].rfind(char::is_whitespace).filter(|i| *i > 0).unwrap_or(limit);
    s[..cut].trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapping_quotes_and_fences_come_off() {
        assert_eq!(clean("\"the shortest path\"", "Dijkstra finds ", ""), "the shortest path");
        assert_eq!(clean("“the shortest path”", "Dijkstra finds ", ""), "the shortest path");
        assert_eq!(clean("```\nthe shortest path\n```", "Dijkstra finds ", ""), "the shortest path");
        assert_eq!(clean("```markdown\nthe shortest path\n```", "Dijkstra finds ", ""), "the shortest path");
        assert_eq!(clean("Completion: the shortest path", "Dijkstra finds ", ""), "the shortest path");
        assert_eq!(clean("```the shortest path```", "Dijkstra finds ", ""), "the shortest path");
        // A short non-ASCII reply is not sliced through while looking for a label.
        assert_eq!(clean("é", "caf", ""), "é");
    }

    #[test]
    fn a_word_is_continued_without_a_space_and_a_new_one_gets_one() {
        assert_eq!(clean("thm runs in $O(n^2)$", "The algori", ""), "thm runs in $O(n^2)$");
        assert_eq!(clean(" theory of computation", "the", ""), " theory of computation");
        assert_eq!(clean("   of computation", "theory", ""), " of computation");
        // The caret already follows a space.
        assert_eq!(clean(" the cat", "I saw ", ""), "the cat");
        assert_eq!(clean("\nthe cat", "", ""), "the cat");
    }

    #[test]
    fn a_restatement_of_the_text_before_the_caret_is_dropped() {
        assert_eq!(clean("algorithm runs in", "The algori", ""), "thm runs in");
        assert_eq!(
            clean("Dijkstra's algorithm finds the shortest path.", "Dijkstra's algorithm finds the", ""),
            " shortest path."
        );
        assert_eq!(clean(" quick brown fox jumps", "the quick brown fox", ""), " jumps");
        // Starts on a word boundary only: "cat" is not an echo of "concat".
        assert_eq!(clean("cat", "concat", ""), "cat");
        // Too short to be an echo once the reply opened with a space.
        assert_eq!(clean(" the end", "of the", ""), " the end");
    }

    #[test]
    fn a_restatement_of_the_text_after_the_caret_is_dropped() {
        assert_eq!(clean("big and the rest.", "A ", " and the rest."), "big");
    }

    #[test]
    fn a_word_right_after_the_caret_keeps_its_space() {
        assert_eq!(clean("big", "The ", "cat sat"), "big ");
    }

    #[test]
    fn only_the_first_line_and_one_sentence_s_length_survive() {
        assert_eq!(clean("the shortest path\nSecond paragraph", "Finds ", ""), "the shortest path");
        let long = (1..=60).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" ");
        let out = clean(&long, "x ", "");
        assert_eq!(out.split_whitespace().count(), MAX_WORDS);
        assert!(out.starts_with("w1 w2"));
        let wordy = "abcdefghij ".repeat(40);
        assert!(clean(&wordy, "x ", "").chars().count() <= MAX_CHARS);
    }

    #[test]
    fn nothing_useful_is_nothing() {
        assert_eq!(clean("", "x ", ""), "");
        assert_eq!(clean("  \n ", "x ", ""), "");
        assert_eq!(clean("\"\"", "x ", ""), "");
        assert_eq!(clean("<CARET/>", "x ", ""), "");
    }

    #[test]
    fn the_prompt_names_the_note_and_marks_the_caret() {
        let p = prompt("courses/COMP3121/documents/week_3_notes.md", "Greedy algorithms ", "\n\nNext").unwrap();
        assert!(p.contains("\"week 3 notes\", in the subject COMP3121"));
        assert!(p.contains("`courses/COMP3121/documents/week_3_notes.md`"));
        assert!(p.contains("Greedy algorithms <CARET/>\n\nNext"));
        assert!(p.contains("follows a space"));
        let mid = prompt("documents/x.md", "The algori", "").unwrap();
        assert!(mid.contains("directly after \"algori\""));
        assert!(!mid.contains("in the subject"));
    }

    #[test]
    fn no_turn_is_spent_inside_a_word() {
        assert!(prompt("courses/X/documents/a.md", "algo", "rithm").is_none());
        assert!(prompt("courses/X/documents/a.md", "algo", " rithm").is_some());
    }

    #[test]
    fn the_prompt_carries_only_the_text_near_the_caret() {
        let before = format!("START{}", "a".repeat(BEFORE_CHARS));
        let after = format!("{}END", "b".repeat(AFTER_CHARS));
        let p = prompt("courses/X/documents/a.md", &format!("{before} "), &format!(" {after}")).unwrap();
        assert!(!p.contains("START") && !p.contains("END"));
        assert!(p.contains("…a") && p.contains("b…"));
        // Characters, not bytes: a cut never lands inside one.
        assert_eq!(tail("éééé", 2), "…éé");
        assert_eq!(head("éééé", 2), "éé…");
        assert_eq!(tail("éé", 5), "éé");
    }
}
