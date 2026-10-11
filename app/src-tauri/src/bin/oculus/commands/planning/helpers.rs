//! What `project` and `task` share: the database, subject lookup, patch fields
//! and the board printers.

use crate::*;

impl Ctx {
    pub(super) fn planning_db(&self) -> Result<SqlitePool, String> {
        self.db()
            .ok_or_else(|| "projects live in the database".to_string())
    }

    /// One subject id from a code (prefix match, as `run` uses). A code that
    /// matches two terms prefers the current one; any other tie is an error.
    pub(super) fn one_subject(&self, pool: &SqlitePool, code: &str) -> Result<i64, String> {
        let subjects = self.rt.block_on(store::subjects(pool))?;
        let matched = filter_subjects(&subjects, &[code.to_string()], false)?;
        if matched.len() == 1 {
            return Ok(matched[0].id);
        }
        let current: Vec<&store::SubjectRow> = matched.iter().filter(|s| s.is_current).collect();
        if current.len() == 1 {
            return Ok(current[0].id);
        }
        let codes: Vec<&str> = matched.iter().map(|s| s.code.as_str()).collect();
        Err(format!(
            "{code} matched {} subjects ({}) — pass the full code",
            matched.len(),
            codes.join(", ")
        ))
    }
}

/// `source` lets the app show which cards the agent put there.
pub(crate) const AGENT_SOURCE: &str = "agent";

/// A nullable text field on a patch: absent leaves it, `--flag ""` clears it.
pub(crate) fn nullable_text(value: Option<&String>) -> Option<Option<String>> {
    value.map(|v| Some(v.clone()).filter(|v| !v.trim().is_empty()))
}

/// The same for a date, which is checked but stored verbatim.
pub(crate) fn nullable_date(value: Option<&String>) -> Result<Option<Option<String>>, String> {
    match value {
        None => Ok(None),
        Some(v) if v.trim().is_empty() => Ok(Some(None)),
        Some(v) => Ok(Some(Some(projects::check_iso8601(v)?))),
    }
}

/// A comma-separated `--tags` list, split only: `projects::normalise_tags`
/// owns the rest.
pub(crate) fn split_tags(value: Option<&str>) -> Vec<String> {
    value
        .map(|v| v.split(',').map(str::to_string).collect())
        .unwrap_or_default()
}

pub(crate) fn nullable_minutes(value: Option<&String>) -> Result<Option<Option<i64>>, String> {
    match value {
        None => Ok(None),
        Some(v) if v.trim().is_empty() => Ok(Some(None)),
        Some(v) => {
            v.trim().parse::<i64>().map(|n| Some(Some(n))).map_err(|_| {
                format!("--estimate takes whole minutes, or \"\" to clear (got {v:?})")
            })
        }
    }
}

/// Print tasks under their columns, in the board's own order.
///
/// Headings carry the column id, which `--column` takes. A subtask sits under
/// its parent in a shared column, and on its own otherwise.
pub(crate) fn print_board(project: &projects::Project, tasks: &[projects::Task]) {
    print_columns(&project.columns, tasks);
}

/// The same, given the columns alone — as for an unfiled task, whose board is
/// `projects::default_columns()`.
pub(crate) fn print_columns(columns: &[projects::Column], tasks: &[projects::Task]) {
    for column in columns {
        let here: Vec<&projects::Task> =
            tasks.iter().filter(|t| t.column_id == column.id).collect();
        if here.is_empty() {
            continue;
        }
        println!(
            "{} {}",
            paint(&column.name, BOLD),
            paint(&format!("[{}]", column.id), DIM)
        );
        for task in here.iter().filter(|t| t.parent_id.is_none()) {
            print_task_line(task, 0);
            for child in here.iter().filter(|c| c.parent_id == Some(task.id)) {
                print_task_line(child, 1);
            }
        }
        for orphan in here
            .iter()
            .filter(|t| t.parent_id.is_some() && !here.iter().any(|p| Some(p.id) == t.parent_id))
        {
            print_task_line(orphan, 1);
        }
    }
}

pub(crate) fn print_task_line(task: &projects::Task, depth: usize) {
    let mut trail = String::new();
    if let Some(due) = &task.due_at {
        trail.push_str(&format!("  due {due}"));
    }
    if let Some(minutes) = task.estimate_minutes {
        trail.push_str(&format!("  {minutes}m"));
    }
    println!(
        "  {}{} {} {}{}",
        "    ".repeat(depth),
        paint(&format!("{:>4}", task.id), DIM),
        if task.done_at.is_some() {
            paint("\u{2713}", GREEN)
        } else {
            " ".to_string()
        },
        truncate(&task.title, 54 - depth * 4),
        paint(&trail, DIM)
    );
}
