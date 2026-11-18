//! The one-shot move of a library from the old layout (`layout::LEGACY`)
//! into the Drive tree, run at launch by the app and the CLI before anything
//! else touches the library. docs/library-move.md describes it.
//!
//! 1. **Plan** every step from the folders on disk and the database
//!    (`plan`), and write it to the journal (`journal`).
//! 2. **Move** with `rename` only, logging each step done as it lands
//!    (`apply`); a crash resumes from the journal.
//! 3. **Rewrite** every stored path in one database transaction (`db`), then
//!    the paths inside files (`contents`, `links`).
//!
//! A marker then makes it a no-op; a library with no old folder only writes
//! the marker. A lock keeps two processes from running it at once.

mod apply;
mod contents;
mod db;
mod journal;
mod links;
pub(crate) mod plan;
mod resolve;
mod rules;
mod startup;
pub mod whiteboard;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub use apply::Hooks;
pub use journal::{Kind, Op, Plan, Step};
pub use resolve::{record_moves, resolve_moved, OwnRule};
pub use rules::Rule;
pub use startup::refuse_to_start;

use journal::Phase;

/// What a run did.
#[derive(Debug)]
pub enum Outcome {
    /// An earlier run finished the move.
    AlreadyDone,
    /// No old folder anywhere: a new library, or one already moved.
    NothingToMove,
    Moved(Summary),
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
            .map(|(kind, n)| format!("{n} {}", kind_label(*kind)))
            .collect();
        let mut out = format!(
            "moved the library into the Drive tree: {}",
            steps.join(", ")
        );
        if let Some(db) = &self.db {
            out.push_str(&format!(
                "; {} file rows, {} sync-history paths, {} lectures, {} other rows rewritten, {} old-path rules kept",
                db.files, db.run_files, db.lectures, db.other, db.path_moves
            ));
        }
        out.push_str(&format!(
            "; {} links in {} files re-pointed",
            self.contents.links, self.contents.notes
        ));
        if self.resumed {
            out.push_str(" (resumed)");
        }
        out
    }
}

pub fn kind_label(kind: Kind) -> &'static str {
    match kind {
        Kind::Debris => "empty scratch folders deleted",
        Kind::Synced => "synced folders and files into Course",
        Kind::Media => "videos and captions into Media",
        Kind::Derived => "parse records into .oculus/derived",
        Kind::Own => "own files and folders to the subject root",
        Kind::NoteAssets => "notes' picture folders renamed",
        Kind::Recording => "lecture recordings into Recordings",
        Kind::Agents => "agent folder entries into .oculus/agents",
        Kind::AgentNotes => "agent-written notes into Agent notes",
        Kind::Partial => "cut-short lecture downloads deleted",
        Kind::Cleanup => "emptied folders, stale links and .DS_Store removed",
        Kind::Whiteboard => "canvases renamed to whiteboards",
        Kind::DrawingRecords => "drawing page pictures and drafts folders renamed",
    }
}

/// The launch check, in the app and the CLI: the move into the Drive tree,
/// then the whiteboard rename (`whiteboard`), each nothing once done, else
/// run whole and logged in one line.
pub fn at_startup(data_dir: &Path) -> Result<(), String> {
    if !journal::is_done(data_dir) {
        if let Outcome::Moved(summary) = run(data_dir, &Hooks::from_env())? {
            elog!("[oculus] {}", summary.line());
            for left in &summary.left {
                elog!("[oculus] library move left {left}");
            }
        }
    }
    whiteboard::at_startup(data_dir)
}

/// Run (or resume) the move. Blocks: it runs on a thread of its own with a
/// runtime of its own, so any caller, async or not, may call it.
pub fn run(data_dir: &Path, hooks: &Hooks) -> Result<Outcome, String> {
    let (data_dir, hooks) = (data_dir.to_path_buf(), hooks.clone());
    on_own_thread(move || async move { run_async(&data_dir, &hooks).await })
}

/// What a run would do, touching nothing: `None` when the move is done or
/// there is nothing to move. A journal left by a cut-short run is shown as
/// it stands.
pub fn dry_run(data_dir: &Path) -> Result<Option<Plan>, String> {
    let data_dir = data_dir.to_path_buf();
    on_own_thread(move || async move {
        if journal::is_done(&data_dir) {
            return Ok(None);
        }
        if let Some(plan) = journal::load(&data_dir)? {
            return Ok(Some(plan));
        }
        plan::refuse(&data_dir)?;
        if !plan::has_legacy(&data_dir) {
            return Ok(None);
        }
        let rows = match open(&data_dir, true).await? {
            Some(pool) => db::rows(&pool).await?,
            None => db::Rows::default(),
        };
        Ok(Some(plan::build(&data_dir, &rows)))
    })
}

fn on_own_thread<T, F, Fut>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<T, String>>,
{
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        runtime.block_on(f())
    })
    .join()
    .map_err(|_| "the library move panicked".to_string())?
}

/// The database, when there is one. Read-only for a dry run.
async fn open(data_dir: &Path, read_only: bool) -> Result<Option<sqlx::SqlitePool>, String> {
    let path: PathBuf = crate::library::paths::db_path(data_dir);
    if !path.is_file() {
        return Ok(None);
    }
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(false)
        .read_only(read_only)
        .busy_timeout(std::time::Duration::from_secs(15));
    sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map(Some)
        .map_err(|e| format!("open {}: {e}", path.display()))
}

async fn run_async(data_dir: &Path, hooks: &Hooks) -> Result<Outcome, String> {
    let _lock = journal::lock(data_dir)?;
    // Another process may have finished it while this one waited.
    if journal::is_done(data_dir) {
        return Ok(Outcome::AlreadyDone);
    }
    let pool = open(data_dir, false).await?;
    let (mut plan, resumed) = match journal::load(data_dir)? {
        Some(plan) => (plan, true),
        None => {
            plan::refuse(data_dir)?;
            if !plan::has_legacy(data_dir) {
                journal::mark_done(data_dir)?;
                return Ok(Outcome::NothingToMove);
            }
            if let Some(pool) = &pool {
                db::ensure(pool).await?;
            }
            let rows = match &pool {
                Some(pool) => db::rows(pool).await?,
                None => db::Rows::default(),
            };
            let plan = plan::build(data_dir, &rows);
            journal::save(data_dir, &plan)?;
            (plan, false)
        }
    };
    if plan.phase == Phase::Moving {
        let log = journal::DoneLog::open(data_dir)?;
        let moved = apply::execute(data_dir, &mut plan.steps, log, hooks);
        if moved.is_err() {
            // The states so far go into the journal; the done log holds them
            // already, this only saves replaying it.
            journal::save(data_dir, &plan).ok();
        }
        moved?;
        plan.phase = Phase::Moved;
        journal::save(data_dir, &plan)?;
    }
    let rules = rules::Rules::new(&plan.rules);
    let linker = links::Linker::new(data_dir, &rules);
    let db_counts = match &pool {
        Some(pool) => {
            db::ensure(pool).await?;
            Some(db::rewrite(pool, data_dir, &plan, &linker).await?)
        }
        None => None,
    };
    let content_counts = contents::rewrite(data_dir, &plan, &rules, &linker)?;
    regenerate_opencode_config(data_dir);
    journal::mark_done(data_dir)?;
    Ok(Outcome::Moved(Summary {
        steps: plan.counts(),
        db: db_counts,
        contents: content_counts,
        left: plan.left.clone(),
        resumed,
    }))
}

/// opencode's config names the library's folders in its permission globs;
/// rendered again only when opencode has been used here. Its next server
/// start renders it anyway, so a failure is logged, not fatal.
fn regenerate_opencode_config(data_dir: &Path) {
    let config = crate::harness::thread_cwd(data_dir).join(crate::harness::opencode::CONFIG_NAME);
    if !config.is_file() {
        return;
    }
    if let Err(e) = crate::harness::write_opencode_config(data_dir) {
        elog!("[oculus] library move: could not render opencode's config again: {e}");
    }
}
