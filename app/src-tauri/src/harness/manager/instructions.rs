//! The brief appended to every provider's own system prompt, with the
//! library's real paths in it, and the working directory every thread runs
//! from.

use std::path::{Path, PathBuf};

use super::options::LectureBrief;
use super::studio::{self, StudioBrief};
use crate::library::layout;

const INSTRUCTIONS_TEMPLATE: &str = include_str!("../../../templates/HARNESS.template.md");

/// The thread's working directory: the library's `agents/` folder.
pub fn thread_cwd(data_dir: &Path) -> PathBuf {
    crate::agents::agents_dir(data_dir)
}

/// The instructions appended to the provider's own system prompt, with the
/// library's real paths in them. `scope` (the subject folder), `lecture` and
/// `studio` only say what the questions are about — they never narrow what
/// the agent may reach — and are appended to the library-wide brief, not
/// substituted.
pub fn instructions(
    data_dir: &Path,
    scope: Option<&str>,
    lecture: Option<&LectureBrief>,
    studio: Option<&StudioBrief>,
) -> String {
    let mut courses: Vec<String> = std::fs::read_dir(layout::courses_dir(data_dir))
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().is_dir())
                .filter_map(|e| e.file_name().to_str().map(String::from))
                .filter(|n| !n.starts_with('.'))
                .collect()
        })
        .unwrap_or_default();
    courses.sort();
    let courses = if courses.is_empty() {
        "none synced yet".to_string()
    } else {
        courses
            .iter()
            .map(|c| format!("`{c}`"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let base = INSTRUCTIONS_TEMPLATE
        .replace("{{DATA_DIR}}", &data_dir.display().to_string())
        .replace("{{COURSES}}", &courses);
    format!("{base}{}", thread_sections(scope, lecture, studio))
}

/// The part of the brief about *this thread* (subject, lecture, Studio
/// video). Split out for the bridges with no per-thread system prompt —
/// opencode and Antigravity — which send it ahead of the session's first
/// message.
pub fn thread_sections(
    scope: Option<&str>,
    lecture: Option<&LectureBrief>,
    studio: Option<&StudioBrief>,
) -> String {
    let mut out = String::new();
    if let Some(code) = scope {
        let folder = layout::course_rel(code);
        let agents = layout::agents_rel(code);
        out.push_str(&format!(
            "\n\n## This conversation\n\n\
             It is scoped to **{code}** — the folder `../{folder}/`. Unless the \
             student names another subject, answer from that folder, and pass `{code}` \
             as the subject to the CLI. Subject-specific guidance, if present, is at \
             `../{agents}/INSTRUCTIONS.md`.\n"
        ));
    }
    if let Some(lec) = lecture {
        out.push_str(&lecture_section(lec, scope));
    }
    if let Some(video) = studio {
        out.push_str(&studio::section(video));
    }
    out
}

/// What the agent is told about the recording the student is watching.
/// Paths are relative to `agents/`, like the rest of the brief. Chapters are
/// inlined (short; fetching them costs a tool call); the transcript is only
/// named. The date, the VTT's shape and the deck-finding commands with their
/// real flags are spelled out because an agent otherwise spends its turn
/// rediscovering them.
fn lecture_section(lec: &LectureBrief, scope: Option<&str>) -> String {
    let dir = lec.folder.as_deref().map(|folder| format!("../{folder}"));
    let mut s = format!(
        "\n## The lecture being watched\n\n\
         The student is watching **{}**, recorded **{}**.{} Lecture titles here come \
         from the timetable, so the date is what says which one this is.\n\n",
        lec.title,
        lec.date,
        dir.as_deref()
            .map(|dir| format!(" Its recording folder is `{dir}/`."))
            .unwrap_or_default()
    );
    if let (true, Some(dir)) = (lec.has_transcript, dir) {
        s.push_str(&format!(
            "- `{dir}/transcript.vtt` — the whole transcript, WebVTT, with timestamps. \
             It is long (an hour of speech) and every cue is followed by a `NOTE CONF` \
             line of recogniser confidence numbers, which is noise — skip those. Do not \
             read it from the top: find the span you want by its timestamp \
             (`grep -n \"00:14:\" {dir}/transcript.vtt` gives you the line number, then \
             read from there).\n"
        ));
    }
    if let Some(code) = scope {
        s.push_str(&format!(
            "- `../{}/` — the course folder: the slide deck for this lecture, \
             and everything else the subject has.\n",
            layout::course_rel(code)
        ));
        s.push_str(&format!(
            "\nThe deck is not linked to the recording, so finding it is a step: \
             `oculus files {code} --type pdf` lists them, and the date above is what \
             picks the week out of `Lecture_1`, `Lecture_2`… Then `oculus grep \"<a phrase \
             off the slide>\" -s {code}` says which deck and page it is on, and \
             `oculus read <FILE> --pages N` prints that page. Note the flags: `oculus \
             files` takes the subject code as a bare argument, every other command takes \
             it as `-s`, and a file as its only argument.\n"
        ));
    }
    if !lec.chapters.is_empty() {
        s.push_str("\nIts chapters:\n\n");
        for c in &lec.chapters {
            s.push_str(&format!(
                "- {} — {} ({})\n",
                crate::lectures::chapters::hms(c.start_seconds),
                c.title,
                c.summary
            ));
        }
    }
    s.push_str(
        "\nThe student is watching this lecture, and a message may carry the moment it was \
         sent at — a timestamp, the last minute of transcript, and a frame of every stream \
         the capture has — appended under a heading after their own words. When it is \
         there, \"this\", \"that slide\" and \"what he just said\" mean that moment. It is \
         usually enough on its own: open every frame and read the transcript it carries \
         before going looking for more, and go to the deck when the question needs the \
         exact notation rather than as a matter of course.\n\
         \nTwo frames are two cameras on the same second, not two moments. Echo360 \
         numbers the streams rather than naming them and either can be the one being \
         taught from: a whiteboard derivation is often only on the room camera while the \
         screen capture holds the theatre's idle splash for the hour, and the slides are \
         only on the capture. Look at all of them before saying a frame shows nothing.\n",
    );
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The scope is appended to the library brief, not substituted.
    #[test]
    fn a_scoped_thread_keeps_the_library_brief_and_names_its_folder() {
        let root = crate::test_support::Scratch::new("harness-scope");
        std::fs::create_dir_all(root.join("courses/COMP30026_2026_SM2")).unwrap();

        let general = instructions(&root, None, None, None);
        assert!(
            general.contains("`COMP30026_2026_SM2`"),
            "the course list is filled in"
        );
        assert!(
            !general.contains("This conversation"),
            "no scope section on a general thread"
        );
        assert!(
            general.contains("./AGENTS.md"),
            "the shared library instructions are referenced"
        );

        let scoped = instructions(&root, Some("COMP30026_2026_SM2"), None, None);
        assert!(
            scoped.starts_with(&general),
            "the scope is appended to the same brief"
        );
        assert!(scoped.contains("`../courses/COMP30026_2026_SM2/`"));
        assert!(scoped.contains("../courses/COMP30026_2026_SM2/.oculus/agents/INSTRUCTIONS.md"));
        assert!(
            !scoped.contains("../courses/COMP30026_2026_SM2/.oculus/agents/memories"),
            "the unwritable path is not offered as a place to write"
        );
    }

    /// The lecture is a third layer on the same brief, with paths as typed
    /// from `agents/` and the chapters inline.
    #[test]
    fn a_lecture_thread_keeps_both_briefs_and_names_the_recording() {
        let root = crate::test_support::Scratch::new("harness-lecture");
        std::fs::create_dir_all(root.join("courses/COMP30026_2026_SM2")).unwrap();

        let scoped = instructions(&root, Some("COMP30026_2026_SM2"), None, None);
        let lecture = LectureBrief {
            id: "abc-123".into(),
            title: "Lecture 14".into(),
            date: "2026-09-08".into(),
            folder: Some(
                "courses/COMP30026_2026_SM2/Course/Recordings/2026-09-08-lecture-14".into(),
            ),
            has_transcript: true,
            chapters: vec![crate::lectures::chapters::Chapter {
                start_seconds: 1382,
                title: "Resolution".into(),
                summary: "Unification, worked".into(),
            }],
        };
        let full = instructions(&root, Some("COMP30026_2026_SM2"), Some(&lecture), None);

        assert!(
            full.starts_with(&scoped),
            "the lecture is appended to the subject's brief"
        );
        assert!(
            full.contains(
                "`../courses/COMP30026_2026_SM2/Course/Recordings/2026-09-08-lecture-14/`"
            ),
            "the folder as the agent would type it"
        );
        assert!(full.contains(
            "`../courses/COMP30026_2026_SM2/Course/Recordings/2026-09-08-lecture-14/transcript.vtt`"
        ));
        assert!(
            full.contains("00:23:02 — Resolution"),
            "chapters are inline"
        );
        assert!(full.contains("the moment it was sent at"));
        assert!(full.contains("2026-09-08"), "the recording's date is named");
        assert!(
            full.contains("oculus files COMP30026_2026_SM2 --type pdf"),
            "the deck hunt is written out with its real flags"
        );

        let no_transcript = instructions(
            &root,
            Some("COMP30026_2026_SM2"),
            Some(&LectureBrief {
                has_transcript: false,
                chapters: vec![],
                ..lecture
            }),
            None,
        );
        assert!(
            !no_transcript.contains("transcript.vtt`"),
            "a transcript that is not on disk is not named"
        );
    }

    /// The video is a layer on the same brief, for every provider.
    #[test]
    fn a_studio_thread_keeps_the_brief_and_names_its_video() {
        let root = crate::test_support::Scratch::new("harness-studio");
        let video = StudioBrief {
            slug: "bayes".into(),
            dir: root.join("studio/videos/bayes"),
            theme: Some("default".into()),
            subject: None,
            portrait: Some(false),
            length_secs: Some(90),
        };
        let plain = instructions(&root, None, None, None);
        let full = instructions(&root, None, None, Some(&video));
        assert!(full.starts_with(&plain), "appended, not substituted");
        assert!(full.contains("## The video being made"));
        assert_eq!(
            thread_sections(None, None, Some(&video)),
            full[plain.len()..],
            "the bridges without a system prompt send the same section"
        );
        assert!(!plain.contains("The video being made"));
    }
}
