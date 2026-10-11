//! `oculus read`.

use crate::*;

impl Ctx {
    /// Print one file's text: page markdown for a parsed document, the bytes
    /// on disk for anything else.
    pub(crate) fn read(&self, args: &ReadArgs) -> Result<(), String> {
        let pool = self.db().ok_or("the library index lives in the database")?;
        let ids = self.subject_ids(&pool, args.subject.as_slice())?;
        let files = self.library_files(&pool, &ids)?;
        let file = resolve_file(&files, &args.file)?;

        let wanted = args.pages.as_deref().map(parse_page_spec).transpose()?;

        #[derive(Serialize)]
        struct Page {
            page_no: i64,
            markdown: String,
        }
        #[derive(Serialize)]
        struct Document<'a> {
            path: &'a str,
            filename: &'a str,
            subject: &'a str,
            file_type: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            pages: Option<Vec<Page>>,
            #[serde(skip_serializing_if = "Option::is_none")]
            text: Option<String>,
        }

        let stored = self.pages_of(&pool, file.id)?;
        let stored = (!stored.is_empty()).then_some(stored);

        // A document that should have pages but has none is an error, not silence.
        if stored.is_none() && app_lib::library::paths::doc_pdf_rel(&file.relative_path).is_some() {
            return Err(format!(
                "{} has not been parsed, so there is no text to read.\n       \
                 Run `oculus index {}` with the Oculus app open.",
                file.relative_path, file.code
            ));
        }

        let doc = match &stored {
            Some(rows) => {
                let selected: Vec<Page> = rows
                    .iter()
                    .filter(|(n, _)| wanted.as_ref().is_none_or(|w| page_wanted(w, *n)))
                    .map(|(n, md)| Page {
                        page_no: *n,
                        markdown: md.clone(),
                    })
                    .collect();
                if selected.is_empty() {
                    let last = rows.last().map(|(n, _)| *n).unwrap_or(0);
                    return Err(format!(
                        "no such page — {} has pages 1-{last}",
                        file.relative_path
                    ));
                }
                Document {
                    path: &file.relative_path,
                    filename: &file.filename,
                    subject: &file.code,
                    file_type: &file.file_type,
                    pages: Some(selected),
                    text: None,
                }
            }
            // A spreadsheet without page rows still has its text beside it.
            None if app_lib::library::paths::is_sheet(&file.relative_path) => {
                let md = self
                    .data_dir
                    .join(app_lib::pages::sheets::md_rel(&file.relative_path));
                let text = std::fs::read_to_string(&md).map_err(|_| {
                    format!(
                        "{} has not been converted to text yet.\n       \
                         Open the Oculus app, or run `oculus run -s {}`, to convert it.",
                        file.relative_path, file.code
                    )
                })?;
                Document {
                    path: &file.relative_path,
                    filename: &file.filename,
                    subject: &file.code,
                    file_type: &file.file_type,
                    pages: None,
                    text: Some(text),
                }
            }
            None => {
                let abs = self.data_dir.join(&file.relative_path);
                if !is_text_file(&file.relative_path) {
                    return Err(format!(
                        "{} is not text — it is on disk at {}",
                        file.relative_path,
                        abs.display()
                    ));
                }
                let text =
                    std::fs::read_to_string(&abs).map_err(|e| format!("{}: {e}", abs.display()))?;
                Document {
                    path: &file.relative_path,
                    filename: &file.filename,
                    subject: &file.code,
                    file_type: &file.file_type,
                    pages: None,
                    text: Some(text),
                }
            }
        };

        if self.json {
            return self.emit(&doc);
        }
        match (&doc.pages, &doc.text) {
            (Some(pages), _) => {
                println!(
                    "{}  {}",
                    paint(doc.path, BOLD),
                    paint(&format!("{} page(s)", pages.len()), DIM)
                );
                for p in pages {
                    println!();
                    println!("{}", paint(&format!("── page {} ──", p.page_no), DIM));
                    println!("{}", p.markdown);
                }
            }
            (_, Some(text)) => {
                println!("{}", paint(doc.path, BOLD));
                println!();
                print!("{text}");
            }
            _ => {}
        }
        Ok(())
    }
}
