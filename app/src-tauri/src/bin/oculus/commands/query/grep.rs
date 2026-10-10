//! `oculus grep`.

use crate::*;

impl Ctx {
    /// Pattern search across both halves of the library.
    ///
    /// Scans in path order and stops at the limit, so PDF text and markdown
    /// interleave instead of one half crowding out the other.
    pub(crate) fn grep(&self, args: &GrepArgs) -> Result<(), String> {
        let pool = self.db().ok_or("the library index lives in the database")?;
        let ids = self.subject_ids(&pool, &args.subject)?;
        let files = filter_categories(self.library_files(&pool, &ids)?, &args.category)?;
        let pages = self.page_text(&pool, &ids)?;
        let re = build_regex(&args.pattern, args.fixed, args.case_sensitive)?;
        let limit = args.limit.max(1);

        #[derive(Serialize)]
        struct Match {
            subject: String,
            path: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            page_no: Option<i64>,
            #[serde(skip_serializing_if = "Option::is_none")]
            line_no: Option<usize>,
            line: String,
        }

        let mut hits: Vec<Match> = Vec::new();
        let mut truncated = false;

        'files: for f in &files {
            // PDF markdown lives in pages; other text lives on disk. Feed
            // both through the same limit and matching rules in file order.
            let disk = if pages.contains_key(&f.id) || !is_text_file(&f.relative_path) {
                None
            } else {
                std::fs::read_to_string(self.data_dir.join(&f.relative_path)).ok()
            };
            let chunks = pages
                .get(&f.id)
                .into_iter()
                .flatten()
                .map(|(page, text)| (Some(*page), text.as_str()))
                .chain(disk.as_deref().map(|text| (None, text)));
            for (page_no, text) in chunks {
                for (i, line) in text.lines().enumerate() {
                    if !re.is_match(line) {
                        continue;
                    }
                    if hits.len() >= limit {
                        truncated = true;
                        break 'files;
                    }
                    hits.push(Match {
                        subject: f.code.clone(),
                        path: f.relative_path.clone(),
                        page_no,
                        line_no: page_no.is_none().then_some(i + 1),
                        line: line.trim().to_string(),
                    });
                }
            }
        }

        if args.files_with_matches {
            let mut seen: Vec<&str> = Vec::new();
            for h in &hits {
                if !seen.contains(&h.path.as_str()) {
                    seen.push(&h.path);
                }
            }
            if self.json {
                return self.emit(&seen);
            }
            for p in seen {
                println!("{p}");
            }
            return Ok(());
        }

        if self.json {
            return self.emit(&hits);
        }
        if hits.is_empty() {
            println!("{}", paint("no matches", DIM));
            return Ok(());
        }
        for h in &hits {
            let at = match (h.page_no, h.line_no) {
                (Some(p), _) => format!("p{p}"),
                (_, Some(l)) => format!("L{l}"),
                _ => String::new(),
            };
            println!(
                "{}{} {}",
                h.path,
                paint(&format!(":{at}:"), DIM),
                truncate(&h.line, 140)
            );
        }
        if truncated {
            println!();
            println!(
                "{}",
                paint(
                    &format!("stopped at {limit} matches — raise it with -n"),
                    DIM
                )
            );
        }
        Ok(())
    }
}
