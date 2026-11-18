//! The drawing kind's rename, run at launch right after the Drive move and
//! with its lock: every `.canvas` the student drew becomes `.whiteboard`, the
//! page pictures (`canvas-pages/`) and recovery drafts (`recovery/canvases/`)
//! move to their folders' new names, and the database follows.
//! docs/library-move.md describes it.
//!
//! 1. **Plan** (`plan`) every rename from the Drive tree on disk and the
//!    `files` rows, and write it to its own journal (`journal`).
//! 2. **Move** with `rename` only, through the Drive move's `apply`, each
//!    step logged as it lands; a crash resumes from the journal.
//! 3. **Rewrite** the rows and `path_moves` in one transaction (`db`), then
//!    the pages manifests and the notes that link to a renamed drawing
//!    (`contents`).
//!
//! A drawing keeps the `"type": "oculus-canvas"` it holds until its next
//! save: both readers take it, and rewriting it would rewrite every drawing
//! the student has for nothing a reader needs.

mod contents;
mod db;
mod journal;
mod plan;

use std::collections::BTreeMap;
use std::path::Path;

pub use journal::Plan;

use super::apply::{self, Hooks};
use super::journal::{DoneLog, Kind, Phase};
use super::rules::Rules;

/// The extension a drawing had (`whiteboards::legacy`).
pub const LEGACY_EXTENSION: &str = crate::library::files::whiteboards::legacy::EXTENSION;
/// What a drawing's `files` row said it was, in `file_type` and `category`.
pub const LEGACY_CATEGORY: &str = "canvas";
/// Where a notebook's page pictures were (`library::paths::PAGES_DIR` now).
pub const LEGACY_PAGES_DIR: &str = "canvas-pages";
/// Where the recovery drafts were (`whiteboards::drafts::DIR` now).
pub const LEGACY_DRAFTS_DIR: &str = "recovery/canvases";

#[derive(Debug)]
pub enum Outcome {
    /// An earlier run finished the rename.
    AlreadyDone,
    /// Nothing named for canvases anywhere: a new library, or one renamed.
    NothingToRename,
    Renamed(Summary),
}

#[derive(Debug, Default)]
pub struct Summary {
    pub steps: BTreeMap<Kind, usize>,
    pub db: Option<db::DbCounts>,
    pub contents: contents::ContentCounts,
    pub left: Vec<String>,
    /// The run picked up a journal an earlier one left.
    pub resumed: bool,
}

impl Summary {
    /// The one line the log and the CLI print.
    pub fn line(&self) -> String {
        let steps: Vec<String> = self
            .steps
            .iter()
            .map(|(kind, n)| format!("{n} {}", super::kind_label(*kind)))
            .collect();
        let mut out = format!("renamed canvases to whiteboards: {}", steps.join(", "));
        if let Some(db) = &self.db {
            out.push_str(&format!(
                "; {} file rows, {} other rows, {} saved note versions rewritten, {} old-path rules kept",
                db.files, db.other, db.versions, db.path_moves
            ));
        }
        out.push_str(&format!(
            "; {} page manifests, {} links in {} notes re-pointed",
            self.contents.manifests, self.contents.links, self.contents.notes
        ));
        if self.resumed {
            out.push_str(" (resumed)");
        }
        out
    }
}

/// The launch check, after the Drive move: nothing once the rename is done,
/// else the whole rename, logged in one line.
pub fn at_startup(data_dir: &Path) -> Result<(), String> {
    if journal::is_done(data_dir) {
        return Ok(());
    }
    if let Outcome::Renamed(summary) = run(data_dir, &Hooks::from_env())? {
        elog!("[oculus] {}", summary.line());
        for left in &summary.left {
            elog!("[oculus] whiteboard rename left {left}");
        }
    }
    Ok(())
}

/// Run (or resume) the rename. Blocks, on a thread and runtime of its own.
/// Refused until the Drive move is done: the plan reads the Drive tree.
pub fn run(data_dir: &Path, hooks: &Hooks) -> Result<Outcome, String> {
    let (data_dir, hooks) = (data_dir.to_path_buf(), hooks.clone());
    super::on_own_thread(move || async move { run_async(&data_dir, &hooks).await })
}

/// What a run would do, touching nothing: `None` when the rename is done or
/// there is nothing to rename. A journal left by a cut-short run is shown as
/// it stands.
pub fn dry_run(data_dir: &Path) -> Result<Option<Plan>, String> {
    let data_dir = data_dir.to_path_buf();
    super::on_own_thread(move || async move {
        if journal::is_done(&data_dir) {
            return Ok(None);
        }
        if let Some(plan) = journal::load(&data_dir)? {
            return Ok(Some(plan));
        }
        let rows = match super::open(&data_dir, true).await? {
            Some(pool) => db::rows(&pool).await?,
            None => db::Rows::default(),
        };
        let plan = plan::build(&data_dir, &rows);
        Ok((!plan.is_empty()).then_some(plan))
    })
}

async fn run_async(data_dir: &Path, hooks: &Hooks) -> Result<Outcome, String> {
    let _lock = super::journal::lock(data_dir)?;
    if journal::is_done(data_dir) {
        return Ok(Outcome::AlreadyDone);
    }
    if !super::journal::is_done(data_dir) {
        return Err("the move into the Drive tree has not finished, so whiteboards wait".into());
    }
    let pool = super::open(data_dir, false).await?;
    if let Some(pool) = &pool {
        super::db::ensure(pool).await?;
    }
    let (mut plan, resumed) = match journal::load(data_dir)? {
        Some(plan) => (plan, true),
        None => {
            let rows = match &pool {
                Some(pool) => db::rows(pool).await?,
                None => db::Rows::default(),
            };
            let plan = plan::build(data_dir, &rows);
            if plan.is_empty() {
                journal::mark_done(data_dir)?;
                return Ok(Outcome::NothingToRename);
            }
            journal::save(data_dir, &plan)?;
            (plan, false)
        }
    };
    if plan.phase == Phase::Moving {
        let log = DoneLog::open_at(&journal::done_log_path(data_dir))?;
        let moved = apply::execute(data_dir, &mut plan.steps, log, hooks);
        if moved.is_err() {
            journal::save(data_dir, &plan).ok();
        }
        moved?;
        plan.phase = Phase::Moved;
        journal::save(data_dir, &plan)?;
    }
    let rules = Rules::new(&plan.rules);
    let linker = super::links::Linker::new(data_dir, &rules);
    let db_counts = match &pool {
        Some(pool) => Some(db::rewrite(pool, &plan, &rules, &linker).await?),
        None => None,
    };
    let content_counts = contents::rewrite(data_dir, &plan, &linker)?;
    journal::mark_done(data_dir)?;
    Ok(Outcome::Renamed(Summary {
        steps: plan.counts(),
        db: db_counts,
        contents: content_counts,
        left: plan.left.clone(),
        resumed,
    }))
}
