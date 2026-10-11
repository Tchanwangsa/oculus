//! `oculus files`.

use crate::*;

impl Ctx {
    pub(crate) fn files(&self, args: &FilesArgs) -> Result<(), String> {
        let pool = self.db().ok_or("the library index lives in the database")?;
        let ids = self.subject_ids(&pool, &args.codes)?;

        let needle = args.r#match.as_ref().map(|m| m.to_lowercase());
        let all = filter_categories(self.library_files(&pool, &ids)?, &args.category)?;
        let rows: Vec<&LibFile> = all
            .iter()
            .filter(|f| {
                args.r#type
                    .as_ref()
                    .is_none_or(|t| f.file_type.eq_ignore_ascii_case(t))
                    && needle
                        .as_ref()
                        .is_none_or(|n| f.relative_path.to_lowercase().contains(n))
                    && (!args.indexed || f.indexed_pages > 0)
            })
            .take(args.limit.max(1))
            .collect();

        if self.json {
            return self.emit(&rows);
        }

        if rows.is_empty() {
            println!("{}", paint("no matching files", DIM));
            return Ok(());
        }
        let mut current = "";
        for f in &rows {
            if f.code != current {
                current = &f.code;
                println!("{}", paint(current, BOLD));
            }
            let indexed = if f.indexed_pages > 0 {
                format!("{}p", f.indexed_pages)
            } else {
                "-".to_string()
            };
            // The course prefix is already the section header.
            let short = f
                .relative_path
                .splitn(3, '/')
                .nth(2)
                .unwrap_or(&f.relative_path);
            println!(
                "  {} {} {} {}",
                paint(&format!("{:<5}", truncate(&f.file_type, 5)), DIM),
                paint(&format!("{indexed:>5}"), DIM),
                paint(
                    &format!("{:>9}", human_bytes(f.size_bytes.unwrap_or(0) as u64)),
                    DIM
                ),
                short
            );
        }
        Ok(())
    }
}
