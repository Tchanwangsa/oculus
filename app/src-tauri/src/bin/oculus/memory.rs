//! `oculus memory`.

use super::*;
use app_lib::memory;

impl Ctx {
    // ── memory ───────────────────────────────────────────────────────────────

    /// A subject flag to its bucket's course folder, `None` for the cross-subject
    /// one. Filesystem only, so these commands work where the database cannot open.
    fn memory_bucket(&self, subject: &Option<String>) -> Result<Option<String>, String> {
        match subject {
            Some(code) => memory::resolve_subject(&self.data_dir, code).map(Some),
            None => Ok(None),
        }
    }

    pub(crate) fn memory_list(&self, args: &MemoryListArgs) -> Result<(), String> {
        let scope: Vec<Option<String>> = if args.all {
            memory::buckets(&self.data_dir)
        } else {
            vec![self.memory_bucket(&args.subject)?]
        };

        let mut all: Vec<memory::Entry> = Vec::new();
        for bucket in &scope {
            let mut rows = memory::list(&self.data_dir, bucket.as_deref())?;
            if let Some(kind) = &args.r#type {
                rows.retain(|e| e.front.meta("type") == Some(kind.as_str()));
            }
            all.extend(rows);
        }

        if self.json {
            return self.emit(&all);
        }
        if all.is_empty() {
            println!("nothing filed here yet");
            return Ok(());
        }
        let mut current: Option<Option<String>> = None;
        for entry in &all {
            if current.as_ref() != Some(&entry.subject) {
                if current.is_some() {
                    println!();
                }
                let name = entry
                    .subject
                    .clone()
                    .unwrap_or_else(|| "across subjects".into());
                println!("{}", paint(&name, BOLD));
                current = Some(entry.subject.clone());
            }
            let kind = entry.front.meta("type").unwrap_or("—");
            let (created, updated) = entry.dates();
            let created = created.unwrap_or_default();
            // Shown only when it differs from the created date in the column.
            let revised = match &updated {
                Some(u) if *u != created => format!("  {}", paint(&format!("updated {u}"), DIM)),
                _ => String::new(),
            };
            // The name, not the title: it is what the next command takes, and a title
            // slugged by hand misses the filename as often as it hits.
            println!(
                "  {}  {}  {}{}\n      {}",
                paint(&format!("{created:<10}"), DIM),
                paint(&format!("[{kind}]"), DIM),
                entry.front.name,
                revised,
                paint(entry.front.description.trim(), DIM)
            );
        }
        Ok(())
    }

    pub(crate) fn memory_read(&self, args: &MemoryReadArgs) -> Result<(), String> {
        let bucket = self.memory_bucket(&args.subject)?;
        let entry = memory::find(&self.data_dir, &args.name, bucket.as_deref())?;
        if self.json {
            return self.emit(&entry);
        }
        println!("{}", paint(&entry.front.display(), BOLD));
        if let (Some(created), updated) = entry.dates() {
            let revised = match updated {
                Some(u) if u != created => format!(", updated {u}"),
                _ => String::new(),
            };
            println!("{}", paint(&format!("written {created}{revised}"), DIM));
        }
        println!("{}", paint(&entry.path, DIM));
        println!();
        println!("{}", entry.body.as_deref().unwrap_or_default().trim());
        Ok(())
    }

    pub(crate) fn memory_write(&self, args: &MemoryWriteArgs) -> Result<(), String> {
        let bucket = self.memory_bucket(&args.subject)?;

        let body = match (&args.text, &args.body) {
            (Some(text), _) => Some(text.clone()),
            (None, Some(source)) => Some(if source == "-" {
                let mut buffer = String::new();
                std::io::Read::read_to_string(&mut std::io::stdin(), &mut buffer)
                    .map_err(|e| format!("reading the memory from stdin: {e}"))?;
                buffer
            } else {
                std::fs::read_to_string(source).map_err(|e| format!("reading {source}: {e}"))?
            }),
            (None, None) => None,
        };

        let mut extra = Vec::new();
        for pair in &args.meta {
            let (k, v) = pair
                .split_once('=')
                .ok_or_else(|| format!("--meta wants key=value, got {pair}"))?;
            extra.push((memory::slug(k).replace('-', "_"), v.to_string()));
        }

        let spec = memory::WriteSpec {
            name: args.name.clone(),
            title: args.title.clone(),
            description: args.about.clone(),
            kind: args.r#type.clone(),
            body,
            why: args.why.clone(),
            how: args.how.clone(),
            source: args.source.clone(),
            links: args.link.clone(),
            extra,
        };
        let written = memory::write(&self.data_dir, bucket.as_deref(), spec)?;

        if self.json {
            return self.emit(&written);
        }
        let where_ = written
            .subject
            .clone()
            .unwrap_or_else(|| "across subjects".into());
        println!(
            "{} {} in {where_} ({} filed there now)",
            paint(if written.created { "wrote" } else { "updated" }, GREEN),
            written.name,
            written.indexed
        );
        Ok(())
    }

    pub(crate) fn memory_rm(&self, args: &MemoryRmArgs) -> Result<(), String> {
        let bucket = self.memory_bucket(&args.subject)?;
        let entry = memory::remove(&self.data_dir, &args.name, bucket.as_deref())?;
        if self.json {
            return self
                .emit(&serde_json::json!({ "deleted": entry.front.name, "path": entry.path }));
        }
        println!("{} {}", paint("deleted", YELLOW), entry.front.name);
        Ok(())
    }

    pub(crate) fn memory_move(&self, args: &MemoryMoveArgs) -> Result<(), String> {
        if args.subject.is_none() && !args.global {
            return Err("say where it goes: -s <CODE>, or --global".into());
        }
        let to = self.memory_bucket(&args.subject)?;
        let entry = memory::find(&self.data_dir, &args.name, None)?;
        let moved = memory::relocate(&self.data_dir, &entry, to.as_deref())?;
        if self.json {
            return self.emit(&moved);
        }
        println!(
            "{} {} to {}",
            paint("moved", GREEN),
            moved.name,
            moved
                .subject
                .clone()
                .unwrap_or_else(|| "across subjects".into())
        );
        Ok(())
    }

    pub(crate) fn memory_reindex(&self, args: &MemoryReindexArgs) -> Result<(), String> {
        let scope: Vec<Option<String>> = if args.all {
            memory::buckets(&self.data_dir)
        } else {
            vec![self.memory_bucket(&args.subject)?]
        };
        let mut done: Vec<(String, usize)> = Vec::new();
        for bucket in &scope {
            let n = memory::reindex(&self.data_dir, bucket.as_deref())?;
            done.push((
                bucket.clone().unwrap_or_else(|| "across subjects".into()),
                n,
            ));
        }
        if self.json {
            return self.emit(
                &done
                    .iter()
                    .map(|(b, n)| serde_json::json!({ "bucket": b, "entries": n }))
                    .collect::<Vec<_>>(),
            );
        }
        for (bucket, n) in &done {
            println!("{bucket}: {n} indexed");
        }
        Ok(())
    }

    /// A memory into a `TASTE.md` bullet, and out of the store.
    ///
    /// Goes under the named heading; a missing heading is appended, not refused.
    pub(crate) fn memory_promote(&self, args: &MemoryPromoteArgs) -> Result<(), String> {
        let bucket = self.memory_bucket(&args.subject)?;
        let entry = memory::find(&self.data_dir, &args.name, bucket.as_deref())?;
        let line = args
            .text
            .clone()
            .unwrap_or_else(|| entry.front.description.trim().to_string());
        if line.trim().is_empty() {
            return Err("nothing to write — pass --as".into());
        }

        let path = agents::agents_dir(&self.data_dir).join("TASTE.md");
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let heading = format!("## {}", memory::humanize(&args.section));
        let bullet = format!("- {}", line.trim());
        if text.contains(&bullet) {
            return Err(format!("{} already says that", path.display()));
        }

        let updated = match text.find(&heading) {
            Some(at) => {
                // The end of that section: the next heading, or the file's.
                let after = at + heading.len();
                let end = text[after..]
                    .find("\n## ")
                    .map(|i| after + i)
                    .unwrap_or(text.len());
                let section = text[after..end].trim_end();
                // The first bullet takes a blank line after the heading, as the stub does.
                let body = if section.trim().is_empty() {
                    format!("\n\n{bullet}\n")
                } else if section.lines().any(|l| l.trim_start().starts_with("- ")) {
                    format!("{section}\n{bullet}\n")
                } else {
                    format!("{section}\n\n{bullet}\n")
                };
                format!("{}{body}{}", &text[..after], &text[end..])
            }
            None => format!("{}\n\n{heading}\n\n{bullet}\n", text.trim_end()),
        };
        std::fs::write(&path, &updated)
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;

        let removed = if args.keep {
            None
        } else {
            Some(memory::remove(
                &self.data_dir,
                &entry.front.name,
                entry.subject.as_deref(),
            )?)
        };

        if self.json {
            return self.emit(&serde_json::json!({
                "taste": path.to_string_lossy(),
                "section": args.section,
                "bullet": bullet,
                "memory_removed": removed.is_some(),
            }));
        }
        println!(
            "{} {bullet}",
            paint(&format!("TASTE.md / {}:", args.section), GREEN)
        );
        if removed.is_some() {
            println!(
                "{} {} (it is a preference now)",
                paint("deleted", DIM),
                entry.front.name
            );
        }
        Ok(())
    }
}
