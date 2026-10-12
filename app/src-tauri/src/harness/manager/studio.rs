//! The Studio section of a thread's brief: which video the conversation is
//! making, read off the video's folder on every send like the lecture's row.

use std::path::{Path, PathBuf};

use crate::db::store::StudioLink;

/// What the brief says about the video a thread makes.
#[derive(Debug, Clone, PartialEq)]
pub struct StudioBrief {
    pub slug: String,
    /// The video's folder, absolute.
    pub dir: PathBuf,
    /// From video.json; `None` when it does not load, which the agent finds
    /// out by linting.
    pub theme: Option<String>,
    pub subject: Option<String>,
    /// 1080x1920: a reel rather than a landscape video.
    pub portrait: Option<bool>,
    /// Narration length asked for, in seconds.
    pub length_secs: Option<i64>,
}

impl StudioBrief {
    /// The brief for `link`'s video under `videos`.
    pub fn read(videos: &Path, link: &StudioLink) -> Self {
        let dir = videos.join(&link.slug);
        let video = std::fs::read_to_string(dir.join("video.json"))
            .ok()
            .and_then(|text| serde_json::from_str::<studio_core::Video>(&text).ok());
        Self {
            slug: link.slug.clone(),
            theme: video.as_ref().map(|v| v.theme.clone()),
            subject: video.as_ref().and_then(|v| v.subject.clone()),
            portrait: video.as_ref().map(|v| v.size.height > v.size.width),
            length_secs: link.length_secs,
            dir,
        }
    }
}

/// What the agent is told about the video. The folder exists already, so
/// `oculus studio new` is ruled out; the draft is the hand-back, and the
/// final waits for the student.
pub fn section(video: &StudioBrief) -> String {
    let StudioBrief {
        slug,
        dir,
        theme,
        subject,
        portrait,
        length_secs,
    } = video;
    let mut s = format!(
        "\n## The video being made\n\n\
         This conversation is making the Studio video **{slug}**. The app made its \
         folder, `{}/`, with a `video.json` holding one title scene, an empty `script.md` \
         and empty `components/` and `assets/`. Edit those; do not run `oculus studio \
         new`. Every `oculus studio` command takes `{slug}` where it says DIR.\n\n",
        dir.display()
    );
    match portrait {
        Some(true) => s.push_str("- Format: portrait, 1080x1920 (a reel).\n"),
        Some(false) => s.push_str("- Format: landscape, 1920x1080.\n"),
        None => {}
    }
    if let Some(theme) = theme {
        s.push_str(&format!(
            "- Theme: `{theme}` (video.json's `theme`); keep it unless the student asks.\n"
        ));
    }
    if let Some(code) = subject {
        s.push_str(&format!(
            "- Subject: `{code}`; its library, `../studio/subjects/{code}/`, is read \
             before the global one.\n"
        ));
    }
    if let Some(secs) = length_secs {
        s.push_str(&format!(
            "- Length: about {secs} seconds of narration, which is the target for the \
             script, not a limit to pad up to.\n"
        ));
    }
    s.push_str(&format!(
        "\nUse the `oculus-studio` skill as its director; the folder is its step 1, done. \
         Render the draft with `oculus studio render {slug}` and say it is ready: the \
         app plays `renders/draft.mp4` beside this conversation. Ask before rendering \
         the final with `--final`.\n"
    ));
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;

    fn link(slug: &str, length_secs: Option<i64>) -> StudioLink {
        StudioLink {
            slug: slug.into(),
            thread_id: Some(1),
            length_secs,
        }
    }

    #[test]
    fn the_brief_names_the_folder_and_reads_the_format_from_video_json() {
        let root = Scratch::new("studio-brief");
        let dir = root.join("bayes");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("video.json"),
            r#"{"size":{"width":1080,"height":1920},"theme":"warm","subject":"MAST20004",
                "scenes":[{"component":"TitleCard","hold":2}]}"#,
        )
        .unwrap();

        let brief = StudioBrief::read(&root, &link("bayes", Some(60)));
        assert_eq!(brief.theme.as_deref(), Some("warm"));
        assert_eq!(brief.portrait, Some(true));
        let text = section(&brief);
        assert!(text.contains(&format!("`{}/`", dir.display())), "{text}");
        assert!(text.contains("portrait, 1080x1920"), "{text}");
        assert!(text.contains("Theme: `warm`"));
        assert!(text.contains("`../studio/subjects/MAST20004/`"));
        assert!(text.contains("about 60 seconds"));
        assert!(text.contains("oculus studio render bayes"));
        assert!(text.contains("Ask before rendering the final"));
    }

    #[test]
    fn a_video_json_that_does_not_load_leaves_those_lines_out() {
        let root = Scratch::new("studio-brief-broken");
        std::fs::create_dir_all(root.join("broken")).unwrap();
        std::fs::write(root.join("broken/video.json"), "{").unwrap();
        let text = section(&StudioBrief::read(&root, &link("broken", None)));
        assert!(text.contains("**broken**"));
        assert!(!text.contains("Format:") && !text.contains("Theme:"));
        assert!(!text.contains("Length:"));
    }
}
