//! `oculus task`.

use super::*;
use crate::*;

impl Ctx {
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
        let all = self.rt.block_on(projects::projects(
            pool,
            projects::SubjectFilter::Any,
            "all",
        ))?;

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
            let at = |e: String| {
                if many {
                    format!("task {}: {e}", n + 1)
                } else {
                    e
                }
            };
            if let Some(due) = &item.due {
                item.due = Some(projects::check_iso8601(due).map_err(at)?);
            }
            if let Some(starts) = &item.starts {
                item.starts = Some(projects::check_iso8601(starts).map_err(at)?);
            }
        }

        let ids = self.rt.block_on(projects::create_tasks(
            &pool,
            args.project,
            &items,
            AGENT_SOURCE,
        ))?;
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
            return Err(
                "nothing to change: pass --title, --body, --due, --starts or --estimate".into(),
            );
        }
        self.rt
            .block_on(projects::update_task(&pool, args.id, &patch))?;
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
            println!(
                "{}",
                paint(&format!("task {} is already there", args.id), DIM)
            );
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
