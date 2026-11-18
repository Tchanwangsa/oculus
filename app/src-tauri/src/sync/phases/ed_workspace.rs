//! A challenge slide's workspace folder, `<NN>-<slide-slug>/` beside the slide
//! file: a read-only mirror of the student's workspace on Ed, fetched every
//! sync. Ed's files are written at Ed's own paths and everything else in the
//! folder is deleted, but only once a download has come in and unpacked.

use crate::library::layout;
use crate::library::paths;
use crate::sources::ed::workspace::{self, Keep, Pruned};
use crate::sources::ed::{self, LinkedFile, SlideExtras};
use crate::sync::render::rel_within_course;
use crate::sync::{Engine, FileEvent, Subject};

/// A workspace text file up to this size is also shown in the lesson file.
const INLINE_TEXT_BYTES: usize = 20 * 1024;

/// Data-dir folder of per-challenge records, `<challenge_id>.json`, each
/// marking a workspace whose folder may hold local edits. The mirror
/// replaces them, so a run counts them for one log line, then deletes it.
pub(in crate::sync) const EDIT_RECORDS: &str = "ed-workspaces";

impl Engine {
    /// The student's current workspace for a challenge, mirrored into the
    /// slide's folder. Fetched each sync: Ed gives no version for a
    /// workspace, and the byte compare keeps unchanged files unchanged.
    pub(super) fn lesson_workspace(
        &self,
        c: &Subject,
        challenge_id: i64,
        slide_title: &str,
        slide_stem: &str,
    ) -> SlideExtras {
        // `fetch_bytes` names neither the URL nor the ticket in its errors.
        let zip = self
            .ed
            .workspace_zip(challenge_id, ed::open_live_workspace, |url| {
                self.fetch_bytes(url)
            });
        let records = self.data_dir.join(EDIT_RECORDS);
        let had_edits = records.join(format!("{challenge_id}.json")).is_file();
        self.mirror_workspace(c, slide_title, slide_stem, zip, had_edits)
    }

    /// Make the slide's folder hold exactly the files of Ed's `zip`: the
    /// rest deleted, then each of Ed's written. A failed download or a zip
    /// that will not unpack leaves the folder as it is. `had_edits` counts
    /// what this replaced towards the run's line about local edits.
    pub(super) fn mirror_workspace(
        &self,
        c: &Subject,
        slide_title: &str,
        slide_stem: &str,
        zip: Result<Vec<u8>, String>,
        had_edits: bool,
    ) -> SlideExtras {
        let say = |level: &str, line: &str| {
            self.reporter.log(
                level,
                &c.code,
                &format!("ed workspace {slide_title}: {line}"),
            )
        };
        let failed = |e: &str| {
            say("warning", e);
            SlideExtras {
                note: Some("The challenge's files could not be fetched.".into()),
                ..SlideExtras::default()
            }
        };
        let (files, skipped) = match zip.and_then(|zip| ed::unpack_workspace(&zip)) {
            Ok(unpacked) => unpacked,
            Err(e) => return failed(&e),
        };
        let Some(dir) = paths::course_rel_path(&c.code, slide_stem) else {
            return failed(&format!("invalid path: {slide_stem}"));
        };
        for line in &skipped {
            say("info", &line.to_string());
        }
        if files.is_empty() && skipped.is_empty() {
            say("info", "Ed's workspace is empty, so its folder is too");
        }

        // A file left out of the zip's unpack is still Ed's, so it stays.
        let ed_paths = files
            .iter()
            .map(|f| f.path.as_str())
            .chain(skipped.iter().map(|s| s.path.as_str()));
        let pruned = self.prune_workspace(&dir, ed_paths);
        for (path, e) in &pruned.failed {
            say("warning", &format!("{path}: could not delete it: {e}"));
        }
        for path in &pruned.removed {
            say("info", &format!("{path} deleted: Ed's workspace has none"));
            let rel = format!("{dir}/{path}");
            self.reporter.file(&FileEvent {
                subject_id: c.id,
                code: c.code.clone(),
                category: layout::category_of(&rel).to_string(),
                relative_path: rel,
                size_bytes: 0,
                canvas_id: None,
                source_url: None,
                source_dates: Default::default(),
                action: "removed",
            });
        }
        let mut replaced = pruned.removed.iter().filter(|p| is_edit(p)).count();

        let mut linked = Vec::new();
        for f in files {
            match self.store_workspace_file(c, &dir, &f.path, &f.bytes) {
                Ok((rel, action)) => {
                    replaced += usize::from(action == paths::WriteAction::Updated);
                    linked.push(LinkedFile {
                        text: inline_text(&f.bytes),
                        rel: rel_within_course(&rel),
                        path: f.path,
                    });
                }
                Err(e) => say("warning", &format!("{}: {e}", f.path)),
            }
        }
        if had_edits {
            self.replaced_edits
                .set(self.replaced_edits.get() + replaced);
        }
        SlideExtras {
            files: linked,
            ..SlideExtras::default()
        }
    }

    /// `workspace::prune` over the slide's folder `dir` (data-dir-relative),
    /// keeping Ed's files and the parse artifacts each carries. A deleted
    /// file's artifacts go with it, those outside the folder too.
    fn prune_workspace<'a>(&self, dir: &str, ed_paths: impl Iterator<Item = &'a str>) -> Pruned {
        let mut keep = Keep::default();
        for path in ed_paths {
            let rel = format!("{dir}/{path}");
            keep.prefixes
                .extend(paths::parse_artifacts(&self.data_dir, &rel));
            keep.files.insert(self.data_dir.join(rel));
        }
        let pruned = workspace::prune(&self.data_dir.join(dir), &keep);
        for path in &pruned.removed {
            for artifact in paths::parse_artifacts(&self.data_dir, &format!("{dir}/{path}")) {
                if !keep.holds(&artifact) {
                    let _ = if artifact.is_dir() {
                        std::fs::remove_dir_all(&artifact)
                    } else {
                        std::fs::remove_file(&artifact)
                    };
                }
            }
        }
        pruned
    }

    /// Store one workspace file at Ed's own path under the slide's folder
    /// `dir`, through the workspace guard (`workspace::resolve`). Returns its
    /// data-dir-relative path and what the write did.
    fn store_workspace_file(
        &self,
        c: &Subject,
        dir: &str,
        path: &str,
        bytes: &[u8],
    ) -> Result<(String, paths::WriteAction), String> {
        let root = self.data_dir.join(dir);
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let target = workspace::resolve(&root, path)?;
        let action = match std::fs::read(&target) {
            Ok(existing) if existing == bytes => paths::WriteAction::Unchanged,
            Ok(_) => paths::WriteAction::Updated,
            Err(_) => paths::WriteAction::New,
        };
        if action != paths::WriteAction::Unchanged {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(&target, bytes).map_err(|e| e.to_string())?;
        }
        let rel = format!("{dir}/{path}");
        if action == paths::WriteAction::Updated {
            paths::purge_parse_artifacts(&self.data_dir, &rel);
        }
        self.reporter.file(&FileEvent {
            subject_id: c.id,
            code: c.code.clone(),
            relative_path: rel.clone(),
            size_bytes: bytes.len() as u64,
            category: layout::category_of(&rel).to_string(),
            canvas_id: None,
            source_url: None,
            source_dates: Default::default(),
            action: action.as_str(),
        });
        if self.parse_pdfs && paths::is_pdf(&rel) {
            self.trigger_parse(&rel, c.id);
        }
        Ok((rel, action))
    }

    /// Once a run has mirrored the lessons, say how many local edits it
    /// replaced and delete the edit records (`EDIT_RECORDS`).
    pub(in crate::sync) fn forget_edit_records(&self) {
        let records = self.data_dir.join(EDIT_RECORDS);
        if !self.options.ed_lessons || !records.is_dir() {
            return;
        }
        let n = self.replaced_edits.get();
        if n > 0 {
            let edits = if n == 1 { "edit was" } else { "edits were" };
            self.reporter.log(
                "info",
                "",
                &format!("{n} local {edits} replaced by Ed's copy: challenge files now mirror Ed"),
            );
        }
        if let Err(e) = std::fs::remove_dir_all(&records) {
            elog!("[oculus] {}: could not delete: {e}", records.display());
        }
    }
}

/// Whether a deleted workspace file could have been the student's edit: not
/// a hidden file and not a conflict copy (`<stem>.ed-server.<ext>`).
fn is_edit(path: &str) -> bool {
    !path.split('/').any(|p| p.starts_with('.')) && !path.contains(".ed-server")
}

/// A small UTF-8 text file's contents, for showing inline.
fn inline_text(bytes: &[u8]) -> Option<String> {
    if bytes.len() > INLINE_TEXT_BYTES || bytes.contains(&0) {
        return None;
    }
    std::str::from_utf8(bytes).ok().map(str::to_string)
}
