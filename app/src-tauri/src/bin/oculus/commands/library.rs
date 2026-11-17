//! `oculus library migrate`: the move into the Drive tree, run by hand or
//! previewed.

use crate::*;
use app_lib::library::migrate::{self, kind_label, Hooks, Op, Outcome, Plan};

impl Ctx {
    pub(crate) fn library_migrate(&self, args: &LibraryMigrateArgs) -> Result<(), String> {
        if args.dry_run {
            let plan = migrate::dry_run(&self.data_dir)?;
            return self.print_plan(plan.as_ref(), args.show);
        }
        match migrate::run(&self.data_dir, &Hooks::from_env())? {
            Outcome::AlreadyDone => self.say("the library is already in the Drive tree"),
            Outcome::NothingToMove => self.say("no old folders to move; marked as done"),
            Outcome::Moved(summary) => {
                if self.json {
                    return self.emit(&serde_json::json!({
                        "moved": summary.line(),
                        "left": summary.left,
                    }));
                }
                println!("{}", summary.line());
                for left in &summary.left {
                    println!("  {} {left}", paint("left", DIM));
                }
                Ok(())
            }
        }
    }

    fn say(&self, message: &str) -> Result<(), String> {
        if self.json {
            return self.emit(&serde_json::json!({ "message": message }));
        }
        println!("{message}");
        Ok(())
    }

    fn print_plan(&self, plan: Option<&Plan>, show: usize) -> Result<(), String> {
        let Some(plan) = plan else {
            return self.say("nothing to move: the library is in the Drive tree");
        };
        let pending: Vec<_> = plan.steps.iter().filter(|s| !s.done).collect();
        let rules = plan.rules.len();
        if self.json {
            let counts: serde_json::Map<String, serde_json::Value> = plan
                .counts()
                .into_iter()
                .map(|(kind, n)| (format!("{kind:?}"), n.into()))
                .collect();
            return self.emit(&serde_json::json!({
                "data_dir": self.data_dir,
                "steps": counts,
                "pending": pending.len(),
                "lectures": plan.lectures.len(),
                "markdown_to_repoint": plan.notes.len(),
                "path_moves": rules,
                "left": plan.left,
                "first": pending.iter().take(show).map(|s| &s.op).collect::<Vec<_>>(),
            }));
        }
        println!("data dir: {}", self.data_dir.display());
        println!(
            "{} steps ({} still to do), {} lectures, {} markdown files to re-point, {} path_moves rules",
            plan.steps.len(),
            pending.len(),
            plan.lectures.len(),
            plan.notes.len(),
            rules
        );
        for (kind, n) in plan.counts() {
            println!("  {n:>6}  {}", kind_label(kind));
        }
        for left in &plan.left {
            println!("  {} {left}", paint("left", DIM));
        }
        println!("first {} steps:", show.min(pending.len()));
        for step in pending.iter().take(show) {
            let line = match &step.op {
                Op::Rename { from, to } => format!("{from} → {to}"),
                Op::RemoveEmptyTree { path } => format!("delete empty {path}"),
                Op::RemoveFile { path } => format!("delete {path}"),
                Op::RemoveDirIfEmpty { path } => format!("remove {path} once empty"),
            };
            println!("  {line}");
        }
        Ok(())
    }
}
