//! Storyboard entries and hand-offs, notes, scene addressing, and a whole
//! brief over a small narrated video.

use std::path::Path;

use serde_json::json;
use studio_core::Project;

use super::storyboard::{entry, handoff, note_paths};
use super::*;
use crate::test_support::Scratch;

const BOARD: &str = "# Title

## Scenes

### intro (component Intro) — scene 0
Carries in: nothing. Hands on: `dots` to `count`.

---

### count
**Carries in:** `dots` from `intro`.
**Hands on:** nothing.
See notes/01-a.md and (notes/02-b.md), notes/01-a.md again.

```tsx
### count
---
```
#### Beats
| Said | On screen |
| --- | --- |
## Appendix
### countdown (component Later)
";

#[test]
fn an_entry_runs_from_its_heading_to_the_next_heading_or_rule() {
    let intro = entry(BOARD, "intro").unwrap();
    assert_eq!(
        intro,
        "### intro (component Intro) — scene 0\nCarries in: nothing. Hands on: `dots` to `count`."
    );
    let count = entry(BOARD, "count").unwrap();
    assert!(count.starts_with("### count\n"), "{count}");
    assert!(count.contains("```tsx\n### count\n---\n```"), "fences kept");
    assert!(
        count.ends_with("| --- | --- |"),
        "a level-4 heading stays inside: {count}"
    );
    assert!(entry(BOARD, "countdown")
        .unwrap()
        .ends_with("(component Later)"));
    assert_eq!(entry(BOARD, "coun"), None, "an id is a whole word");
    assert_eq!(entry(BOARD, "Beats"), None);
    assert_eq!(entry("```\n### fenced\n```\n", "fenced"), None);
}

#[test]
fn handoffs_read_on_one_line_or_two_with_or_without_bold() {
    let intro = entry(BOARD, "intro").unwrap();
    assert_eq!(
        handoff(&intro),
        (Some("nothing.".into()), Some("`dots` to `count`.".into()))
    );
    let count = entry(BOARD, "count").unwrap();
    assert_eq!(
        handoff(&count),
        (Some("`dots` from `intro`.".into()), Some("nothing.".into()))
    );
    assert_eq!(handoff("### x\nIdea: none"), (None, None));
}

#[test]
fn notes_are_named_once_each_in_order() {
    let count = entry(BOARD, "count").unwrap();
    assert_eq!(note_paths(&count), ["notes/01-a.md", "notes/02-b.md"]);
    assert!(note_paths("see mynotes/x.md or notes/").is_empty());
}

/// A three-scene video: a silent title, a narrated `count` and a narrated
/// `outro` whose clip is for older text. Only `Intro`'s component exists.
fn video(root: &Path) -> std::path::PathBuf {
    let dir = root.join("counting");
    let write = |rel: &str, text: &str| {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    };
    write(
        "video.json",
        &json!({
            "size": { "width": 64, "height": 36 },
            "theme": "default",
            "scenes": [
                { "id": "intro", "component": "Intro", "hold": 2 },
                { "id": "count", "component": "Count", "props": { "n": 3 } },
                { "id": "outro", "component": "Outro" }
            ]
        })
        .to_string(),
    );
    write(
        "components/Intro.tsx",
        "export const props = {};\nexport default () => <Box />;\n",
    );
    write(
        "script.md",
        "## count\nFifteen is odd. Is it?\n\n## outro\nBye now.\n",
    );
    let word =
        |t: &str, s: f64, e: f64| json!({ "text": t, "start": s, "end": e, "confidence": -1.0 });
    write(
        "narration/manifest.json",
        &json!({ "version": 1, "scenes": {
            "count": { "key": "k", "text": "Fifteen is odd. Is it?", "audio": "k.wav", "duration": 1.6,
                "words": [word("Fifteen", 0.28, 0.7), word("is", 0.71, 0.8), word("odd.", 0.8, 1.1),
                          word("Is", 1.2, 1.3), word("it?", 1.3, 1.5)] },
            "outro": { "key": "o", "text": "Bye.", "audio": "o.wav", "duration": 0.5, "words": [] }
        }})
        .to_string(),
    );
    write("storyboard.md", BOARD);
    write("notes/01-a.md", "a");
    write("notes/03-c.md", "c");
    dir
}

#[test]
fn a_brief_has_the_scene_its_neighbours_entry_notes_words_and_commands() {
    let root = Scratch::new("brief");
    let dir = video(&root);
    let project = Project::load(&dir).unwrap();
    assert_eq!(scene_index(&project, "count"), Ok(1));
    assert_eq!(scene_index(&project, "2"), Ok(2));
    assert!(scene_index(&project, "3").unwrap_err().contains("0 to 2"));
    assert!(scene_index(&project, "nope")
        .unwrap_err()
        .contains("intro, count, outro"));

    let b = build(&project, 1, &dir, "counting");
    let prev = b.previous.as_ref().unwrap();
    assert!(prev.written && prev.entry);
    assert_eq!(prev.hands_on.as_deref(), Some("`dots` to `count`."));
    let next = b.next.as_ref().unwrap();
    assert!(!next.written && !next.entry);
    assert_eq!(b.notes.paths, ["notes/01-a.md", "notes/02-b.md"]);
    assert_eq!(b.notes.missing, ["notes/02-b.md"]);
    let text = b.to_string();
    assert!(
        text.starts_with("Scene count (1 of 3), component Count, props {\"n\":3}\n"),
        "{text}"
    );
    // The theme's 0.25 s lead is in every time.
    assert!(
        text.contains("\n0.53 Fifteen  0.96 is  1.05 odd.\n1.45 Is  1.55 it?\n"),
        "{text}"
    );
    assert!(
        text.contains("Next: outro (2), component Outro (not written yet)\n  no storyboard entry")
    );
    let out = dir.join("out");
    assert!(text.contains(&format!(
        "oculus studio strip counting --scene 1 --n 16 --cols 4 -o {}",
        out.join("count-strip.png").display()
    )));
    assert!(text.contains("oculus studio gallery counting Count --sheet"));

    let silent = build(&project, 0, &dir, "counting");
    assert!(matches!(silent.narration, Narration::Silent { seconds } if seconds == 2.0));
    assert!(silent.to_string().contains("Previous: none (first scene)"));
    assert!(!silent.notes.named, "the intro names no notes: all of them");
    assert_eq!(silent.notes.paths, ["notes/01-a.md", "notes/03-c.md"]);

    let stale = build(&project, 2, &dir, "counting");
    let Narration::Unavailable { error, script } = &stale.narration else {
        panic!("{:?}", stale.narration);
    };
    assert!(error.contains("older text"), "{error}");
    assert_eq!(script.as_deref(), Some("Bye now."));
    assert_eq!(
        stale.storyboard_missing.as_deref(),
        Some("storyboard.md has no `### outro` entry")
    );
}

#[test]
fn commands_name_a_video_by_slug_only_where_it_lives() {
    let root = Scratch::new("brief-target");
    let videos = root.join("videos");
    std::fs::create_dir_all(videos.join("shor")).unwrap();
    assert_eq!(target(&videos.join("shor"), &videos), "shor");
    let elsewhere = root.join("my videos").join("shor");
    assert_eq!(
        target(&elsewhere, &videos),
        format!("'{}'", elsewhere.display())
    );
}
