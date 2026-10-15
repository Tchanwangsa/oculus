//! Full task reads, including the library paths needed to manage related files.

use super::*;
use crate::*;

impl Ctx {
    pub(crate) fn task_show(&self, args: &TaskShowArgs) -> Result<(), String> {
        let pool = self.planning_db()?;
        let task = self
            .rt
            .block_on(projects::task(&pool, args.id))?
            .ok_or_else(|| format!("task {} does not exist", args.id))?;
        let files = self.rt.block_on(projects::task_files(&pool, args.id))?;
        let registry = self.rt.block_on(projects::list_tags(&pool))?;
        let tags: Vec<_> = registry
            .iter()
            .filter(|t| task.tag_ids.contains(&t.id))
            .collect();
        let siblings = match task.project_id {
            Some(id) => self.rt.block_on(projects::tasks(&pool, id))?,
            None => self
                .rt
                .block_on(projects::all_tasks(&pool, projects::TaskScope::Unfiled))?,
        };
        let subtasks: Vec<_> = siblings
            .into_iter()
            .filter(|t| t.parent_id == Some(task.id))
            .collect();
        if self.json {
            return self.emit(&serde_json::json!({ "task": task, "tags": tags, "files": files, "subtasks": subtasks }));
        }
        println!("#{} {}", task.id, task.title);
        println!(
            "project: {}  column: {}",
            task.project_id
                .map(|p| p.to_string())
                .unwrap_or("unfiled".into()),
            task.column_id
        );
        if let Some(parent) = task.parent_id {
            println!("parent: {parent}");
        }
        if let Some(due) = &task.due_at {
            println!("due: {due}");
        }
        if let Some(starts) = &task.starts_at {
            println!("starts: {starts}");
        }
        if let Some(estimate) = task.estimate_minutes {
            println!("estimate: {estimate}m");
        }
        if let Some(done) = &task.done_at {
            println!("done: {done}");
        }
        for tag in &tags {
            println!("tag: {}", tag.name);
        }
        if let Some(body) = &task.body {
            println!("\n{body}");
        }
        println!("\nRelated files ({})", files.len());
        for file in files {
            println!(
                "  {}  {}",
                file.relative_path,
                file.subject_code.as_deref().unwrap_or("personal")
            );
        }
        if !subtasks.is_empty() {
            println!("\nSubtasks");
            for child in subtasks {
                print_task_line(&child, 0, &registry);
            }
        }
        Ok(())
    }
}
