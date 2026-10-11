//! `oculus search`.

use crate::*;

impl Ctx {
    /// Rank pages by meaning.
    ///
    /// An empty result must say why, or a caller concludes the library has no
    /// answer: an empty index names `index`, and a retired model's vectors are
    /// named as such. Both are errors, never a silent zero-hit success.
    pub(crate) fn search(&self, args: &SearchArgs) -> Result<(), String> {
        let pool = self
            .db()
            .ok_or("the retrieval index lives in the database")?;
        let subjects = self.rt.block_on(store::subjects(&pool))?;
        let ids = subject_ids(&subjects, args.subject.as_slice())?;
        let codes: HashMap<i64, String> = subjects.iter().map(|s| (s.id, s.code.clone())).collect();

        let db_file = app_lib::library::paths::db_path(&self.data_dir);
        let stats = self
            .rt
            .block_on(app_lib::pages::retrieval::stats(&db_file))?;
        if stats.pages_embedded == 0 {
            // "Nothing searchable" and "nothing stored" both count zero but need
            // different actions.
            if stats.pages_stale > 0 {
                return Err(format!(
                    "{} page(s) are stored, but they were embedded by {} and cannot be\n       \
                     compared against a query from {}. Re-run `oculus index` to rebuild them.",
                    stats.pages_stale,
                    if stats.stale_models.is_empty() {
                        "a retired model".to_string()
                    } else {
                        stats.stale_models.join(", ")
                    },
                    stats.model.as_deref().unwrap_or("the current model"),
                ));
            }
            return Err(
                "nothing is indexed yet, so there is nothing to rank.\n       \
                 Run `oculus index` over PDFs already on record, or `oculus run -s` to scrape."
                    .to_string(),
            );
        }

        let hits = self.rt.block_on(app_lib::pages::retrieval::search_in(
            &db_file,
            args.query.clone(),
            args.limit,
            &ids,
        ))?;

        #[derive(Serialize)]
        struct Hit {
            score: f32,
            subject: String,
            path: String,
            filename: String,
            page_no: i64,
            markdown: String,
        }
        let hits: Vec<Hit> = hits
            .into_iter()
            .map(|h| Hit {
                score: h.score,
                subject: codes.get(&h.subject_id).cloned().unwrap_or_default(),
                path: h.relative_path,
                filename: h.filename,
                page_no: h.page_no,
                markdown: h.markdown,
            })
            .collect();

        if self.json {
            return self.emit(&hits);
        }
        if hits.is_empty() {
            println!("{}", paint("no matching pages", DIM));
            return Ok(());
        }
        for h in &hits {
            // Subject plus in-course path: a filename is not unique across courses.
            let short = h.path.splitn(3, '/').nth(2).unwrap_or(&h.path);
            println!(
                "{} {} {} {}",
                paint(&format!("{:.3}", h.score), BOLD),
                paint(&format!("{:<20}", truncate(&h.subject, 20)), DIM),
                truncate(short, 44),
                paint(&format!("p{}", h.page_no), DIM)
            );
            if args.full {
                println!("{}\n", h.markdown);
            } else {
                println!("      {}", snippet(&h.markdown, 96));
            }
        }
        if !args.full {
            if let Some(top) = hits.first() {
                println!();
                println!(
                    "{} oculus read {} --pages {}",
                    paint("read one:", DIM),
                    shell_quote(&top.path),
                    top.page_no
                );
            }
        }
        Ok(())
    }
}
