//! `oculus project`.

use super::*;
use crate::*;

impl Ctx {
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
        let rows = self
            .rt
            .block_on(projects::projects(&pool, filter, status))?;

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
            entries.push(Entry {
                project: p,
                tasks_total: total,
                tasks_done: done,
            });
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
                    &format!(
                        "{:<12}",
                        truncate(e.project.subject_code.as_deref().unwrap_or("personal"), 12)
                    ),
                    DIM
                ),
                paint(
                    &format!("{:>7}", format!("{}/{}", e.tasks_done, e.tasks_total)),
                    DIM
                ),
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
            starts_at: args
                .starts
                .as_deref()
                .map(projects::check_iso8601)
                .transpose()?,
            due_at: args
                .due
                .as_deref()
                .map(projects::check_iso8601)
                .transpose()?,
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
                "nothing to change: pass --name, --due, --starts, --brief, --tags or --status"
                    .into(),
            );
        }
        self.rt
            .block_on(projects::update_project(&pool, args.id, &patch))?;
        let updated = self
            .rt
            .block_on(projects::project(&pool, args.id))?
            .ok_or_else(|| format!("project {} does not exist", args.id))?;
        if self.json {
            return self.emit(&updated);
        }
        println!(
            "{} {}",
            paint(&format!("project {}", updated.id), BOLD),
            updated.name
        );
        Ok(())
    }
}
