//! `oculus project` and `oculus task`.

use super::*;

impl Ctx {
    // ── projects and tasks ───────────────────────────────────────────────────

    fn planning_db(&self) -> Result<SqlitePool, String> {
        self.db().ok_or_else(|| "projects live in the database".to_string())
    }

    /// One subject id from a code (prefix match, as `run` uses). A code that
    /// matches two terms prefers the current one; any other tie is an error.
    fn one_subject(&self, pool: &SqlitePool, code: &str) -> Result<i64, String> {
        let subjects = self.rt.block_on(store::subjects(pool))?;
        let matched = filter_subjects(&subjects, &[code.to_string()], false)?;
        if matched.len() == 1 {
            return Ok(matched[0].id);
        }
        let current: Vec<&store::SubjectRow> =
            matched.iter().filter(|s| s.is_current).collect();
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

    pub(crate) fn project_list(&self, args: &ProjectListArgs) -> Result<(), String> {
        let pool = self.planning_db()?;
        let filter = if args.personal {
            projects::SubjectFilter::Personal
        } else if let Some(code) = &args.subject {
            projects::SubjectFilter::Subject(self.one_subject(&pool, code)?)
        } else {
            projects::SubjectFilter::Any
        };
        let status = if args.archived { "archived" } else { "active" };
        let rows = self.rt.block_on(projects::projects(&pool, filter, status))?;

        #[derive(Serialize)]
        struct Entry<'a> {
            #[serde(flatten)]
            project: &'a projects::Project,
            tasks_total: i64,
            tasks_done: i64,
        }
        let mut entries: Vec<Entry> = Vec::with_capacity(rows.len());
        for p in &rows {
            let (total, done) = self.rt.block_on(projects::task_counts(&pool, p.id))?;
            entries.push(Entry { project: p, tasks_total: total, tasks_done: done });
        }

        if self.json {
            return self.emit(&entries);
        }
        if entries.is_empty() {
            println!("{}", paint(&format!("no {status} projects"), DIM));
            return Ok(());
        }
        for e in &entries {
            println!(
                "{} {:<34} {} {} {}",
                paint(&format!("{:>4}", e.project.id), DIM),
                truncate(&e.project.name, 34),
                paint(
                    &format!("{:<12}", truncate(e.project.subject_code.as_deref().unwrap_or("personal"), 12)),
                    DIM
                ),
                paint(&format!("{:>7}", format!("{}/{}", e.tasks_done, e.tasks_total)), DIM),
                match &e.project.due_at {
                    Some(d) => paint(&format!("  due {d}"), YELLOW),
                    None => String::new(),
                }
            );
        }
        Ok(())
    }

    pub(crate) fn project_show(&self, args: &ProjectShowArgs) -> Result<(), String> {
        let pool = self.planning_db()?;
        let project = self
            .rt
            .block_on(projects::project(&pool, args.id))?
            .ok_or_else(|| format!("project {} does not exist", args.id))?;
        let tasks = self.rt.block_on(projects::tasks(&pool, args.id))?;

        if self.json {
            return self.emit(&serde_json::json!({ "project": project, "tasks": tasks }));
        }
        println!(
            "{} {}{}",
            paint(&format!("#{}", project.id), DIM),
            paint(&project.name, BOLD),
            match &project.subject_code {
                Some(c) => paint(&format!("  {c}"), DIM),
                None => String::new(),
            }
        );
        let mut meta: Vec<String> = vec![project.status.clone()];
        if let Some(d) = &project.starts_at {
            meta.push(format!("starts {d}"));
        }
        if let Some(d) = &project.due_at {
            meta.push(format!("due {d}"));
        }
        if !project.tags.is_empty() {
            meta.push(project.tags.join(", "));
        }
        // Said, not resolved: the CLI has no reader for the calendar grid.
        if project.event_id.is_some() {
            meta.push("pinned to a calendar event".to_string());
        }
        println!("{}", paint(&meta.join("  ·  "), DIM));
        if let Some(brief) = project.brief.as_deref().filter(|b| !b.trim().is_empty()) {
            println!("\n{brief}");
        }
        println!();
        if tasks.is_empty() {
            println!(
                "{}",
                paint(
                    &format!("no tasks yet — oculus task add -p {} --batch -", project.id),
                    DIM
                )
            );
            return Ok(());
        }
        print_board(&project, &tasks);
        Ok(())
    }

    pub(crate) fn project_create(&self, args: &ProjectCreateArgs) -> Result<(), String> {
        let pool = self.planning_db()?;
        let subject_id = match &args.subject {
            Some(code) => Some(self.one_subject(&pool, code)?),
            None => None,
        };
        let input = projects::NewProject {
            name: args.name.trim().to_string(),
            subject_id,
            brief: args.brief.clone(),
            starts_at: args.starts.as_deref().map(projects::check_iso8601).transpose()?,
            due_at: args.due.as_deref().map(projects::check_iso8601).transpose()?,
            tags: split_tags(args.tags.as_deref()),
            source: AGENT_SOURCE.to_string(),
        };
        if input.name.is_empty() {
            return Err("a project needs a name".to_string());
        }
        let id = self.rt.block_on(projects::create_project(&pool, &input))?;
        let created = self
            .rt
            .block_on(projects::project(&pool, id))?
            .ok_or("the project was written but could not be read back")?;
        if self.json {
            return self.emit(&created);
        }
        println!("{} {}", paint(&format!("project {id}"), BOLD), created.name);
        Ok(())
    }

    pub(crate) fn project_update(&self, args: &ProjectUpdateArgs) -> Result<(), String> {
        let pool = self.planning_db()?;
        let patch = projects::ProjectPatch {
            name: args.name.clone(),
            subject_id: None,
            brief: nullable_text(args.brief.as_ref()),
            status: args.status.clone(),
            starts_at: nullable_date(args.starts.as_ref())?,
            due_at: nullable_date(args.due.as_ref())?,
            // `--tags ""` clears, like the other "" flags: `Some(vec![])`, not `None`.
            tags: args.tags.as_deref().map(|t| split_tags(Some(t))),
        };
        if patch.name.is_none()
            && patch.brief.is_none()
            && patch.status.is_none()
            && patch.starts_at.is_none()
            && patch.due_at.is_none()
            && patch.tags.is_none()
        {
            return Err(
                "nothing to change: pass --name, --due, --starts, --brief, --tags or --status".into(),
            );
        }
        self.rt.block_on(projects::update_project(&pool, args.id, &patch))?;
        let updated = self
            .rt
            .block_on(projects::project(&pool, args.id))?
            .ok_or_else(|| format!("project {} does not exist", args.id))?;
        if self.json {
            return self.emit(&updated);
        }
        println!("{} {}", paint(&format!("project {}", updated.id), BOLD), updated.name);
        Ok(())
    }

    pub(crate) fn task_list(&self, args: &TaskListArgs) -> Result<(), String> {
        let pool = self.planning_db()?;
        let Some(project_id) = args.project else {
            return self.task_list_across(&pool, args);
        };
        let project = self
            .rt
            .block_on(projects::project(&pool, project_id))?
            .ok_or_else(|| format!("project {project_id} does not exist"))?;
        let mut tasks = self.rt.block_on(projects::tasks(&pool, project_id))?;

        if let Some(column) = &args.column {
            if !project.columns.iter().any(|c| &c.id == column) {
                let known: Vec<&str> = project.columns.iter().map(|c| c.id.as_str()).collect();
                return Err(format!(
                    "project {} has no column \"{column}\" (has: {})",
                    project.id,
                    known.join(", ")
                ));
            }
            tasks.retain(|t| &t.column_id == column);
        }
        if let Some(before) = &args.due_before {
            let before = projects::check_iso8601(before)?;
            tasks.retain(|t| t.due_at.as_deref().is_some_and(|d| d < before.as_str()));
        }

        if self.json {
            return self.emit(&tasks);
        }
        if tasks.is_empty() {
            println!("{}", paint("no matching tasks", DIM));
            return Ok(());
        }
        print_board(&project, &tasks);
        Ok(())
    }

    /// `task list` with no `-p`: every task, or only the unfiled ones, one board
    /// per project in `projects::all_tasks` order. `--column` needs `--project`
    /// because a column id only means something against one board.
    fn task_list_across(&self, pool: &SqlitePool, args: &TaskListArgs) -> Result<(), String> {
        let scope = if args.unfiled {
            projects::TaskScope::Unfiled
        } else {
            projects::TaskScope::All
        };
        let mut tasks = self.rt.block_on(projects::all_tasks(pool, scope))?;
        if let Some(before) = &args.due_before {
            let before = projects::check_iso8601(before)?;
            tasks.retain(|t| t.due_at.as_deref().is_some_and(|d| d < before.as_str()));
        }
        if self.json {
            return self.emit(&tasks);
        }
        if tasks.is_empty() {
            println!(
                "{}",
                paint(
                    if args.unfiled {
                        "nothing unfiled \u{2014} every task you have belongs to a project"
                    } else {
                        "no tasks yet \u{2014} oculus task add \"something to do\""
                    },
                    DIM
                )
            );
            return Ok(());
        }

        // A task carries its project's id, not its board. `"all"`: an archived
        // project's tasks still count.
        let all = self.rt.block_on(projects::projects(pool, projects::SubjectFilter::Any, "all"))?;

        let mut written = false;
        let unfiled: Vec<&projects::Task> =
            tasks.iter().filter(|t| t.project_id.is_none()).collect();
        if !unfiled.is_empty() {
            println!("{}", paint("Unfiled", BOLD));
            // The default board, which an unfiled task's column is checked against.
            let rows: Vec<projects::Task> = unfiled.into_iter().cloned().collect();
            print_columns(&projects::default_columns(), &rows);
            written = true;
        }
        for project in &all {
            let here: Vec<projects::Task> = tasks
                .iter()
                .filter(|t| t.project_id == Some(project.id))
                .cloned()
                .collect();
            if here.is_empty() {
                continue;
            }
            if written {
                println!();
            }
            written = true;
            println!(
                "{} {}{}",
                paint(&format!("#{}", project.id), DIM),
                paint(&project.name, BOLD),
                match &project.subject_code {
                    Some(c) => paint(&format!("  {c}"), DIM),
                    None => String::new(),
                }
            );
            print_board(project, &here);
        }
        Ok(())
    }

    pub(crate) fn task_add(&self, args: &TaskAddArgs) -> Result<(), String> {
        let pool = self.planning_db()?;
        let mut items: Vec<projects::NewTask> = match &args.batch {
            Some(source) => {
                let text = read_input(source, "batch")?;
                if text.trim().is_empty() {
                    return Err("--batch got an empty input".to_string());
                }
                serde_json::from_str(&text)
                    .map_err(|e| format!("--batch wants a JSON array of tasks: {e}"))?
            }
            None => {
                let title = args
                    .title
                    .clone()
                    .ok_or("give a TITLE, or --batch - to read a JSON array of tasks from stdin")?;
                vec![projects::NewTask {
                    title,
                    column: args.column.clone(),
                    parent: args.parent.map(projects::ParentRef::Id),
                    body: args.body.clone(),
                    due: args.due.clone(),
                    starts: args.starts.clone(),
                    estimate: args.estimate,
                    key: None,
                }]
            }
        };

        // Dates are validated and stored verbatim.
        let many = items.len() > 1;
        for (n, item) in items.iter_mut().enumerate() {
            let at = |e: String| if many { format!("task {}: {e}", n + 1) } else { e };
            if let Some(due) = &item.due {
                item.due = Some(projects::check_iso8601(due).map_err(at)?);
            }
            if let Some(starts) = &item.starts {
                item.starts = Some(projects::check_iso8601(starts).map_err(at)?);
            }
        }

        let ids = self
            .rt
            .block_on(projects::create_tasks(&pool, args.project, &items, AGENT_SOURCE))?;
        let mut created: Vec<projects::Task> = Vec::with_capacity(ids.len());
        for id in &ids {
            if let Some(task) = self.rt.block_on(projects::task(&pool, *id))? {
                created.push(task);
            }
        }
        if self.json {
            return self.emit(&created);
        }
        for task in &created {
            println!(
                "{} {}",
                paint(&format!("task {}", task.id), BOLD),
                truncate(&task.title, 60)
            );
        }
        Ok(())
    }

    pub(crate) fn task_update(&self, args: &TaskUpdateArgs) -> Result<(), String> {
        let pool = self.planning_db()?;
        let patch = projects::TaskPatch {
            title: args.title.clone().filter(|t| !t.trim().is_empty()),
            body: nullable_text(args.body.as_ref()),
            parent_id: None,
            starts_at: nullable_date(args.starts.as_ref())?,
            due_at: nullable_date(args.due.as_ref())?,
            estimate_minutes: nullable_minutes(args.estimate.as_ref())?,
        };
        if patch.title.is_none()
            && patch.body.is_none()
            && patch.starts_at.is_none()
            && patch.due_at.is_none()
            && patch.estimate_minutes.is_none()
        {
            return Err("nothing to change: pass --title, --body, --due, --starts or --estimate".into());
        }
        self.rt.block_on(projects::update_task(&pool, args.id, &patch))?;
        self.print_task(&pool, args.id, None)
    }

    pub(crate) fn task_move(&self, args: &TaskMoveArgs) -> Result<(), String> {
        let pool = self.planning_db()?;
        self.rt.block_on(projects::move_task(
            &pool,
            args.id,
            &args.column,
            args.after,
            args.before,
        ))?;
        self.print_task(&pool, args.id, Some(&args.column))
    }

    pub(crate) fn task_refile(&self, args: &TaskRefileArgs) -> Result<(), String> {
        let pool = self.planning_db()?;
        // No default: a bare `task refile 12` could only guess which was meant.
        let destination = match (args.project, args.unfiled) {
            (Some(id), false) => Some(id),
            (None, true) => None,
            _ => {
                return Err(
                    "say where: -p <PROJECT_ID>, or --unfiled to take it out of every project"
                        .to_string(),
                )
            }
        };
        let rows = self
            .rt
            .block_on(projects::refile_task(&pool, args.id, destination))?;

        // The name confirms the id that was typed.
        let label = match destination {
            Some(id) => self
                .rt
                .block_on(projects::project(&pool, id))?
                .map(|p| p.name)
                .unwrap_or_else(|| format!("project {id}")),
            None => "unfiled".to_string(),
        };
        if self.json {
            return self.emit(&serde_json::json!({
                "refiled": args.id,
                "project_id": destination,
                "project": label,
                "rows": rows,
            }));
        }
        if rows == 0 {
            println!("{}", paint(&format!("task {} is already there", args.id), DIM));
            return Ok(());
        }
        self.print_task(&pool, args.id, Some(&label))?;
        if rows > 1 {
            println!(
                "{}",
                paint(&format!("  {} subtask(s) came with it", rows - 1), DIM)
            );
        }
        Ok(())
    }

    pub(crate) fn task_rm(&self, args: &TaskRmArgs) -> Result<(), String> {
        let pool = self.planning_db()?;
        let rows = self.rt.block_on(projects::delete_task(&pool, args.id))?;
        if self.json {
            return self.emit(&serde_json::json!({ "deleted": args.id, "rows": rows }));
        }
        println!(
            "deleted task {}{}",
            args.id,
            match rows {
                1 => String::new(),
                n => format!(" and {} subtask(s)", n - 1),
            }
        );
        Ok(())
    }

    /// Report the row as it now stands, not the arguments sent.
    fn print_task(&self, pool: &SqlitePool, id: i64, moved_to: Option<&str>) -> Result<(), String> {
        let task = self
            .rt
            .block_on(projects::task(pool, id))?
            .ok_or_else(|| format!("task {id} does not exist"))?;
        if self.json {
            return self.emit(&task);
        }
        let where_ = match moved_to {
            Some(column) => format!(" → {column}"),
            None => String::new(),
        };
        println!(
            "{}{} {}{}",
            paint(&format!("task {}", task.id), BOLD),
            paint(&where_, DIM),
            truncate(&task.title, 56),
            match &task.done_at {
                Some(at) => paint(&format!("  done {at}"), GREEN),
                None => String::new(),
            }
        );
        Ok(())
    }
}

// ── Planning helpers ─────────────────────────────────────────────────────────

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
        Some(v) => v
            .trim()
            .parse::<i64>()
            .map(|n| Some(Some(n)))
            .map_err(|_| format!("--estimate takes whole minutes, or \"\" to clear (got {v:?})")),
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
        if task.done_at.is_some() { paint("\u{2713}", GREEN) } else { " ".to_string() },
        truncate(&task.title, 54 - depth * 4),
        paint(&trail, DIM)
    );
}
