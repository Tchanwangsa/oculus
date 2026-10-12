//! The heteronym table: a part of speech for a word whose gold entry has
//! more than one reading, from its neighbour words alone (no tagger). Port
//! of `data/evals/studio-g2p/tools/heteronyms.py`'s `decide`. The tag it
//! returns is resolved by the dictionary as misaki's `lookup` does (`VB`
//! reads `VERB`, `NN` `NOUN`, `JJ` `ADJ`; `VBD`/`VBN` as they are); `None`
//! leaves the `DEFAULT` reading. `lead` the metal is a reading misaki's
//! dictionary lacks, so the table gives its phonemes outright.

/// What the table decides for one word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Read the entry by this tag.
    Tag(&'static str),
    /// Read these phonemes, which the dictionary does not have.
    Phonemes(&'static str),
}

const MODALS: &[&str] = &[
    "to", "will", "would", "can", "could", "should", "must", "may", "might", "shall", "cannot",
    "can't", "won't", "don't", "doesn't", "didn't", "do", "does", "did", "please", "let's",
    "we'll", "i'll", "you'll", "they'll", "we'd", "i'd", "you'd",
];
const SUBJ: &[&str] = &["i", "we", "you", "they"];
const DET: &[&str] = &[
    "the",
    "a",
    "an",
    "this",
    "that",
    "these",
    "those",
    "my",
    "your",
    "his",
    "her",
    "its",
    "our",
    "their",
    "no",
    "any",
    "some",
    "each",
    "every",
    "of",
    "whose",
    "good",
    "new",
    "great",
    "full",
    "main",
    "real",
    "first",
    "last",
    "best",
    "same",
    "whole",
    "public",
    "permanent",
    "official",
    "written",
    "historical",
    "criminal",
    "single",
    "track",
];
const COPULA: &[&str] = &[
    "is",
    "are",
    "was",
    "were",
    "be",
    "been",
    "being",
    "seem",
    "seems",
    "seemed",
    "feel",
    "feels",
    "felt",
    "look",
    "looks",
    "very",
    "so",
    "too",
    "quite",
    "really",
    "perfectly",
    "more",
    "most",
    "remain",
    "remains",
    "stay",
    "stays",
    "am",
    "'m",
    "'re",
    "become",
    "became",
    "get",
    "got",
    "fairly",
    "pretty",
];
const PERFECT: &[&str] = &[
    "have", "has", "had", "'ve", "i've", "we've", "you've", "they've", "having",
];
const PASSIVE: &[&str] = &[
    "is", "are", "was", "were", "be", "been", "being", "get", "gets", "got",
];
/// Adverbs skipped when looking left from the word ("has not read").
pub const SKIP_ADV: &[&str] = &[
    "not", "never", "already", "just", "also", "all", "ever", "recently", "often", "always",
    "usually", "first",
];
const PAST_CUES: &[&str] = &[
    "yesterday",
    "ago",
    "last",
    "earlier",
    "already",
    "once",
    "previously",
];
const LIVE_ADJ_LEFT: &[&str] = &[
    "streamed",
    "broadcast",
    "broadcasting",
    "go",
    "goes",
    "went",
    "going",
    "gone",
    "is",
    "are",
    "was",
    "were",
    "be",
];
const LIVE_ADJ_RIGHT: &[&str] = &[
    "stream",
    "streams",
    "streaming",
    "broadcast",
    "demo",
    "demos",
    "audience",
    "music",
    "wire",
    "wires",
    "coding",
    "session",
    "sessions",
    "event",
    "events",
    "recording",
    "show",
    "feed",
    "version",
    "data",
    "performance",
    "chat",
    "on",
];
const LEAD_METAL_RIGHT: &[&str] = &[
    "pipe",
    "pipes",
    "apron",
    "aprons",
    "paint",
    "poisoning",
    "shot",
    "weight",
    "weights",
    "crystal",
    "glass",
    "ore",
    "sheet",
    "sheets",
    "shielding",
    "lining",
    "acid",
    "levels",
    "exposure",
    "free",
];
const LEAD_METAL: &str = "lˈɛd";

fn is(list: &[&str], w: &str) -> bool {
    list.contains(&w)
}

/// After a modal, `to` or a subject pronoun, or in "let me/us/them".
fn verb_ctx(l1: &str, l2: &str) -> bool {
    is(MODALS, l1) || is(SUBJ, l1) || (l2 == "let" && matches!(l1, "me" | "us" | "them"))
}

/// The reading for `word` (lower case) from its left words `l1`, `l2` (the
/// nearest first, `""` past a clause edge), its right word `r1`, and the
/// unit's words `sent` (all lower case). `has_adj` says whether the entry
/// has an `ADJ` reading.
pub fn decide(
    word: &str,
    l1: &str,
    l2: &str,
    r1: &str,
    sent: &[String],
    has_adj: bool,
) -> Option<Decision> {
    use Decision::{Phonemes, Tag};
    let tag = |t| Some(Tag(t));
    match word {
        "read" => {
            if is(PERFECT, l1) || is(PASSIVE, l1) {
                tag("VBN")
            } else if is(MODALS, l1) || (l2 == "let" && matches!(l1, "me" | "us" | "them")) {
                tag("VB")
            } else if sent.iter().any(|w| is(PAST_CUES, w)) {
                tag("VBD")
            } else {
                None
            }
        }
        "lead" => {
            let made_of = l1 == "of" && matches!(l2, "made" | "lined" | "built" | "full");
            (is(LEAD_METAL_RIGHT, r1) || made_of).then_some(Phonemes(LEAD_METAL))
        }
        "live" => {
            let after_go = is(LIVE_ADJ_LEFT, l1)
                && (r1.is_empty()
                    || is(LIVE_ADJ_RIGHT, r1)
                    || matches!(l1, "streamed" | "broadcast"));
            let before_noun =
                r1 != "on" && is(LIVE_ADJ_RIGHT, r1) && (is(DET, l1) || l1.is_empty());
            // misaki's DEFAULT is the adjective; the verb is far commoner.
            tag(if after_go || before_noun { "JJ" } else { "VB" })
        }
        // DEFAULT is the adjective; the noun is far commoner.
        "content" => tag(if is(COPULA, l1) { "JJ" } else { "NN" }),
        "used" => (r1 == "to" && !is(PASSIVE, l1)).then_some(Tag("VBD")),
        "that" => matches!(r1, "" | "is" | "was" | "'s").then_some(Tag("DT")),
        "close" => {
            if verb_ctx(l1, l2) {
                tag("VB")
            } else if l1 == "the" && r1 == "of" {
                tag("NN")
            } else {
                None
            }
        }
        _ => {
            if verb_ctx(l1, l2) {
                tag("VB")
            } else if is(DET, l1) {
                tag("NN")
            } else if is(COPULA, l1) && has_adj {
                tag("JJ")
            } else {
                None
            }
        }
    }
}
