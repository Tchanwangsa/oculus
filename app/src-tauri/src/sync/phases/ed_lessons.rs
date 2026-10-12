//! Phase: the subject's Ed Lessons, an index `lessons/<id>-<slug>.md` per
//! lesson with a file per slide (a PDF slide is its PDF), its PDFs and its
//! challenge workspaces in `lessons/<id>-<slug>/`.

use crate::library::paths;
use crate::sources::ed::{self, LessonPlace, LinkedFile, SlideExtras, SlideFile};
use crate::sync::render::{rel_within_course, slug};
use crate::sync::{Engine, FileEvent, FileFailed, FileStart, Progress, Subject};

/// A workspace text file up to this size is also shown in the lesson file.
const INLINE_TEXT_BYTES: usize = 20 * 1024;

impl Engine {
    /// Every lesson of the Ed course. A lesson Ed refuses (a password-gated
    /// quiz answers 403) is a warning and the rest carry on; so is a slide
    /// whose PDF, questions or workspace cannot be fetched, which renders a
    /// note in their place.
    pub(in crate::sync) fn scrape_ed_lessons(
        &self,
        c: &Subject,
        course_id: i64,
    ) -> Result<(), String> {
        let list = self.ed.lessons(course_id)?;
        for (i, listing) in list.lessons.iter().enumerate() {
            if self.reporter.cancelled() {
                break;
            }
            let title = listing["title"].as_str().unwrap_or("Lesson");
            self.reporter.progress(&Progress {
                done: i + 1,
                total: list.lessons.len(),
                course: c.code.clone(),
                phase: "ed_lessons".into(),
                label: title.to_string(),
            });
            let Some(id) = listing["id"].as_i64() else {
                continue;
            };
            let module = list.module_name(listing["module_id"].as_i64());
            if let Err(e) = self.scrape_lesson(c, course_id, id, module, i) {
                self.reporter
                    .log("warning", &c.code, &format!("ed lesson {title}: {e}"));
            }
        }
        Ok(())
    }

    /// The lesson's slide files, then its index. Every slide is fetched
    /// before anything is written, so a cancelled sync leaves the last whole
    /// lesson in place.
    fn scrape_lesson(
        &self,
        c: &Subject,
        course_id: i64,
        lesson_id: i64,
        module: Option<&str>,
        order: usize,
    ) -> Result<(), String> {
        let lesson = self.ed.lesson(lesson_id)?;
        let title = lesson["title"].as_str().unwrap_or("Lesson");
        // The id leads, so a renamed lesson keeps its prefix.
        let stem = format!("lessons/{lesson_id}-{}", slug(title));
        let region = self.ed.region();
        let place = LessonPlace {
            stem: &stem,
            course_id,
            lesson_id,
            module,
            order,
            region: region.as_deref(),
        };

        let mut slides = Vec::new();
        for (i, slide) in ed::sorted_slides(&lesson).into_iter().enumerate() {
            if self.reporter.cancelled() {
                return Ok(());
            }
            let Some(slide_id) = slide["id"].as_i64() else {
                continue;
            };
            let slide_title = slide["title"].as_str().unwrap_or("");
            let slide_stem = format!("{stem}/{:02}-{}", i + 1, slug(slide_title));
            let extras = match slide["type"].as_str().unwrap_or("") {
                "pdf" => self.lesson_pdf(c, slide, slide_id, &slide_stem),
                "quiz" => self.lesson_quiz(c, slide_id, slide_title),
                "code" | "jupyter" => match slide["challenge_id"].as_i64() {
                    Some(challenge) => {
                        self.lesson_workspace(c, challenge, slide_title, &slide_stem)
                    }
                    None => SlideExtras::default(),
                },
                _ => SlideExtras::default(),
            };
            slides.push(SlideFile {
                slide,
                id: slide_id,
                index: i + 1,
                stem: slide_stem,
                extras,
            });
        }

        for slide in &slides {
            let Some(md) = ed::slide_md(&lesson, &place, slide) else {
                continue;
            };
            let path = format!("{}.md", slide.stem);
            self.write_from(c, &path, md.as_bytes(), None, place.slide_url(slide.id))?;
        }
        let md = ed::lesson_index_md(&lesson, &place, &slides);
        self.write_from(c, &format!("{stem}.md"), md.as_bytes(), None, place.url())?;
        Ok(())
    }

    /// The slide's PDF beside the lesson. Ed's file URLs name their content,
    /// so the URL is the version: `file-manifest.json` keeps it under
    /// `ed-slide-<id>`, and a match with the file on disk skips the download.
    fn lesson_pdf(
        &self,
        c: &Subject,
        slide: &serde_json::Value,
        slide_id: i64,
        slide_stem: &str,
    ) -> SlideExtras {
        let Some(url) = slide["file_url"].as_str().filter(|u| !u.is_empty()) else {
            return SlideExtras::default();
        };
        let rel_path = format!("{slide_stem}.pdf");
        let Some(rel) = paths::course_rel_path(&c.code, &rel_path) else {
            return SlideExtras::default();
        };
        let key = format!("ed-slide-{slide_id}");
        let known = self
            .manifest
            .borrow()
            .get(&key)
            .is_some_and(|(seen, _)| seen == url);
        let on_disk = std::fs::metadata(self.data_dir.join(&rel)).ok();
        if let (true, Some(meta)) = (known, on_disk.filter(|m| m.is_file())) {
            self.reporter.file(&FileEvent {
                subject_id: c.id,
                code: c.code.clone(),
                relative_path: rel.clone(),
                size_bytes: meta.len(),
                category: paths::category_from_path(&rel_path).to_string(),
                canvas_id: None,
                source_url: None,
                action: "unchanged",
            });
            return SlideExtras {
                pdf: Some(rel_path),
                ..SlideExtras::default()
            };
        }

        self.reporter.file_start(&FileStart {
            subject_id: c.id,
            code: c.code.clone(),
            relative_path: rel.clone(),
            filename: rel.rsplit('/').next().unwrap_or(&rel).to_string(),
            size_bytes: 0,
        });
        let saved = self
            .fetch_bytes(url)
            .map_err(|e| format!("Download failed: {e}."))
            .and_then(|bytes| {
                self.store(c, &rel_path, &bytes, None, None)
                    .map(|_| bytes.len() as u64)
                    .map_err(|e| format!("Could not save the file: {e}."))
            });
        match saved {
            Ok(size) => {
                self.manifest
                    .borrow_mut()
                    .insert(key, (url.to_string(), size));
                SlideExtras {
                    pdf: Some(rel_path),
                    ..SlideExtras::default()
                }
            }
            Err(error) => {
                self.reporter.file_failed(&FileFailed {
                    subject_id: c.id,
                    relative_path: rel,
                    error: error.clone(),
                });
                SlideExtras {
                    note: Some(format!("The PDF was not downloaded. {error}")),
                    ..SlideExtras::default()
                }
            }
        }
    }

    fn lesson_quiz(&self, c: &Subject, slide_id: i64, slide_title: &str) -> SlideExtras {
        match self.ed.quiz_questions(slide_id) {
            Ok(questions) => SlideExtras {
                questions,
                ..SlideExtras::default()
            },
            Err(e) => {
                self.reporter
                    .log("warning", &c.code, &format!("ed quiz {slide_title}: {e}"));
                SlideExtras {
                    note: Some("The quiz questions could not be fetched.".into()),
                    ..SlideExtras::default()
                }
            }
        }
    }

    /// The student's current workspace for a challenge, every kept file
    /// stored under the slide's folder. Fetched each sync: Ed gives no version
    /// for a workspace, and the byte compare keeps unchanged files unchanged.
    fn lesson_workspace(
        &self,
        c: &Subject,
        challenge_id: i64,
        slide_title: &str,
        slide_stem: &str,
    ) -> SlideExtras {
        let warn = |e: &str| {
            self.reporter.log(
                "warning",
                &c.code,
                &format!("ed workspace {slide_title}: {e}"),
            );
            SlideExtras {
                note: Some("The challenge's files could not be fetched.".into()),
                ..SlideExtras::default()
            }
        };
        // `fetch_bytes` names neither the URL nor the ticket in its errors.
        let zip = match self
            .ed
            .workspace_zip(challenge_id, ed::open_live_workspace, |url| {
                self.fetch_bytes(url)
            }) {
            Ok(zip) => zip,
            Err(e) => return warn(&e),
        };
        let (files, skipped) = match ed::unpack_workspace(&zip) {
            Ok(unpacked) => unpacked,
            Err(e) => return warn(&e),
        };
        for line in skipped {
            self.reporter.log(
                "info",
                &c.code,
                &format!("ed workspace {slide_title}: {line}"),
            );
        }

        let mut linked = Vec::new();
        for f in files {
            match self.store(c, &format!("{slide_stem}/{}", f.path), &f.bytes, None, None) {
                Ok((rel, _)) => linked.push(LinkedFile {
                    text: inline_text(&f.bytes),
                    rel: rel_within_course(&rel),
                    path: f.path,
                }),
                Err(e) => self.reporter.log(
                    "warning",
                    &c.code,
                    &format!("ed workspace {slide_title}: {}: {e}", f.path),
                ),
            }
        }
        SlideExtras {
            files: linked,
            ..SlideExtras::default()
        }
    }
}

/// A small UTF-8 text file's contents, for showing inline.
fn inline_text(bytes: &[u8]) -> Option<String> {
    if bytes.len() > INLINE_TEXT_BYTES || bytes.contains(&0) {
        return None;
    }
    std::str::from_utf8(bytes).ok().map(str::to_string)
}
