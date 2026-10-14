//! `oculus tag`.

use super::*;
use crate::*;

impl Ctx {
    pub(crate) fn tag_list(&self, _args: &TagListArgs) -> Result<(), String> {
        let pool = self.planning_db()?;
        let tags = self.rt.block_on(projects::list_tags(&pool))?;
        if self.json {
            return self.emit(&tags);
        }
        if tags.is_empty() {
            println!("{}", paint("no tags yet \u{2014} oculus tag add NAME", DIM));
            return Ok(());
        }
        for tag in &tags {
            println!(
                "{} {:<24} {} {}",
                paint(&format!("{:>4}", tag.id), DIM),
                truncate(&tag.name, 24),
                paint(&format!("{:<7}", tag.color), DIM),
                paint(&format!("{:>3} task(s)", tag.uses), DIM)
            );
        }
        Ok(())
    }

    pub(crate) fn tag_add(&self, args: &TagAddArgs) -> Result<(), String> {
        let pool = self.planning_db()?;
        let before = self.rt.block_on(projects::find_tag(&pool, &args.name))?;
        let tag = self.rt.block_on(projects::create_tag(
            &pool,
            &args.name,
            args.color.as_deref(),
        ))?;
        if self.json {
            return self.emit(&tag);
        }
        println!(
            "{} {} {}{}",
            paint(&format!("tag {}", tag.id), BOLD),
            tag.name,
            paint(&tag.color, DIM),
            if before.is_some() {
                paint("  already there", DIM)
            } else {
                String::new()
            }
        );
        Ok(())
    }

    pub(crate) fn tag_rm(&self, args: &TagRmArgs) -> Result<(), String> {
        let pool = self.planning_db()?;
        let tags = self.rt.block_on(projects::list_tags(&pool))?;
        let tag = tag_named(&tags, &args.name)?;
        let tasks = self.rt.block_on(projects::delete_tag(&pool, tag.id))?;
        if self.json {
            return self.emit(&serde_json::json!({
                "deleted": tag.id, "name": tag.name, "tasks": tasks,
            }));
        }
        println!(
            "deleted tag {} (taken off {tasks} task(s))",
            truncate(&tag.name, 40)
        );
        Ok(())
    }

    pub(crate) fn tag_rename(&self, args: &TagRenameArgs) -> Result<(), String> {
        let pool = self.planning_db()?;
        let tags = self.rt.block_on(projects::list_tags(&pool))?;
        let tag = tag_named(&tags, &args.old)?;
        let renamed =
            self.rt
                .block_on(projects::update_tag(&pool, tag.id, Some(&args.new), None))?;
        self.print_tag(&renamed)
    }

    pub(crate) fn tag_color(&self, args: &TagColorArgs) -> Result<(), String> {
        let pool = self.planning_db()?;
        let tags = self.rt.block_on(projects::list_tags(&pool))?;
        let tag = tag_named(&tags, &args.name)?;
        let recoloured =
            self.rt
                .block_on(projects::update_tag(&pool, tag.id, None, Some(&args.key)))?;
        self.print_tag(&recoloured)
    }

    /// Report the row as it now stands.
    fn print_tag(&self, tag: &projects::TaskTag) -> Result<(), String> {
        if self.json {
            return self.emit(tag);
        }
        println!(
            "{} {} {}",
            paint(&format!("tag {}", tag.id), BOLD),
            tag.name,
            paint(&tag.color, DIM)
        );
        Ok(())
    }
}
