//! The read-only library commands — search, grep, read, files, calendar —
//! and the file lookup they share.

use super::*;

impl Ctx {
    // ── search ───────────────────────────────────────────────────────────────

    /// Rank pages by meaning.
    ///
    /// An empty result must say why, or a caller concludes the library has no
    /// answer: an empty index names `index`, and a retired model's vectors are
    /// named as such. Both are errors, never a silent zero-hit success.
    pub(crate) fn search(&self, args: &SearchArgs) -> Result<(), String> {
        let pool = self.db().ok_or("the retrieval index lives in the database")?;
        let subjects = self.rt.block_on(store::subjects(&pool))?;
        let ids = subject_ids(&subjects, args.subject.as_slice())?;
        let codes: HashMap<i64, String> =
            subjects.iter().map(|s| (s.id, s.code.clone())).collect();

        let db_file = app_lib::paths::db_path(&self.data_dir);
        let stats = self.rt.block_on(app_lib::retrieval::stats(&db_file))?;
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

        let hits = self.rt.block_on(app_lib::retrieval::search_in(
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

    // ── grep ─────────────────────────────────────────────────────────────────

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
            let chunks = pages.get(&f.id).into_iter().flatten()
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
                paint(&format!("stopped at {limit} matches — raise it with -n"), DIM)
            );
        }
        Ok(())
    }

    // ── read ─────────────────────────────────────────────────────────────────

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
        if stored.is_none() && app_lib::paths::doc_pdf_rel(&file.relative_path).is_some() {
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
                    .map(|(n, md)| Page { page_no: *n, markdown: md.clone() })
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
            None if app_lib::paths::is_sheet(&file.relative_path) => {
                let md = self.data_dir.join(app_lib::sheets::md_rel(&file.relative_path));
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
                let text = std::fs::read_to_string(&abs)
                    .map_err(|e| format!("{}: {e}", abs.display()))?;
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

    // ── files ────────────────────────────────────────────────────────────────

    pub(crate) fn files(&self, args: &FilesArgs) -> Result<(), String> {
        let pool = self.db().ok_or("the library index lives in the database")?;
        let ids = self.subject_ids(&pool, &args.codes)?;

        let needle = args.r#match.as_ref().map(|m| m.to_lowercase());
        let all = filter_categories(self.library_files(&pool, &ids)?, &args.category)?;
        let rows: Vec<&LibFile> = all
            .iter()
            .filter(|f| {
                args.r#type.as_ref().is_none_or(|t| f.file_type.eq_ignore_ascii_case(t))
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
            let short = f.relative_path.splitn(3, '/').nth(2).unwrap_or(&f.relative_path);
            println!(
                "  {} {} {} {}",
                paint(&format!("{:<5}", truncate(&f.file_type, 5)), DIM),
                paint(&format!("{indexed:>5}"), DIM),
                paint(&format!("{:>9}", human_bytes(f.size_bytes.unwrap_or(0) as u64)), DIM),
                short
            );
        }
        Ok(())
    }

    // ── calendar ─────────────────────────────────────────────────────────────

    pub(crate) fn calendar(&self, args: &CalendarArgs) -> Result<(), String> {
        let pool = self.db().ok_or("the calendar lives in the database")?;
        let ids = self.subject_ids(&pool, &args.codes)?;

        let mut clauses: Vec<String> = Vec::new();
        if !ids.is_empty() {
            let list: Vec<String> = ids.iter().map(|i| i.to_string()).collect();
            clauses.push(format!("e.subject_id IN ({})", list.join(",")));
        }
        if args.due {
            clauses.push("e.kind = 'due'".to_string());
        }
        // ISO-8601 UTC, so string comparison against SQLite's clock is the filter.
        if !args.past {
            clauses.push("e.start_at >= strftime('%Y-%m-%dT%H:%M:%SZ','now')".to_string());
        }
        clauses.push(format!(
            "e.start_at < strftime('%Y-%m-%dT%H:%M:%SZ','now','+{} days')",
            args.days.max(0)
        ));
        let sql = format!(
            r#"SELECT s.code AS code, e.kind AS kind, e.title AS title,
                      e.start_at AS start_at, e.location AS location, e.url AS url,
                      strftime('%Y-%m-%d %H:%M', e.start_at, 'localtime') AS local_at
               FROM calendar_events e JOIN subjects s ON s.id = e.subject_id
               WHERE {}
               ORDER BY e.start_at"#,
            clauses.join(" AND ")
        );

        #[derive(Serialize)]
        struct Event {
            subject: String,
            kind: String,
            title: String,
            start_at: String,
            starts_local: String,
            location: Option<String>,
            url: Option<String>,
        }
        let events: Vec<Event> = self.rt.block_on(async {
            let rows = sqlx::query(&sql).fetch_all(&pool).await.map_err(|e| e.to_string())?;
            Ok::<_, String>(
                rows.iter()
                    .map(|r| Event {
                        subject: r.try_get("code").unwrap_or_default(),
                        kind: r.try_get("kind").unwrap_or_default(),
                        title: r.try_get("title").unwrap_or_default(),
                        start_at: r.try_get("start_at").unwrap_or_default(),
                        starts_local: r.try_get("local_at").unwrap_or_default(),
                        location: r.try_get("location").ok().flatten(),
                        url: r.try_get("url").ok().flatten(),
                    })
                    .collect(),
            )
        })?;

        if self.json {
            return self.emit(&events);
        }
        if events.is_empty() {
            println!(
                "{}",
                paint(
                    &format!("nothing in the next {} day(s)", args.days.max(0)),
                    DIM
                )
            );
            return Ok(());
        }
        for e in &events {
            println!(
                "{} {} {:<20} {}{}",
                paint(&e.starts_local, DIM),
                if e.kind == "due" { paint("due  ", YELLOW) } else { paint("class", DIM) },
                truncate(&e.subject, 20),
                truncate(&e.title, 44),
                match e.location.as_deref().filter(|l| !l.is_empty()) {
                    Some(l) => paint(&format!("  {}", truncate(l, 34)), DIM),
                    None => String::new(),
                }
            );
        }
        Ok(())
    }

    // ── shared loaders ───────────────────────────────────────────────────────

    /// Subject-prefix selection shared by the read commands and indexing.
    pub(super) fn subject_ids(&self, pool: &SqlitePool, codes: &[String]) -> Result<Vec<i64>, String> {
        subject_ids(&self.rt.block_on(store::subjects(pool))?, codes)
    }

    /// Every library file for the given subjects (all when empty), in the order
    /// `grep` scans and `files` prints: subject, then path.
    fn library_files(&self, pool: &SqlitePool, ids: &[i64]) -> Result<Vec<LibFile>, String> {
        let filter = if ids.is_empty() {
            String::new()
        } else {
            let list: Vec<String> = ids.iter().map(|i| i.to_string()).collect();
            format!(" WHERE f.subject_id IN ({})", list.join(","))
        };
        let sql = format!(
            r#"SELECT f.id AS id, s.code AS code, f.filename AS filename,
                      f.relative_path AS relative_path, f.file_type AS file_type,
                      f.category AS category, f.size_bytes AS size_bytes,
                      f.parse_status AS parse_status,
                      (SELECT COUNT(*) FROM pages p
                        WHERE p.file_id = f.id AND p.markdown != '') AS indexed_pages
               FROM files f JOIN subjects s ON s.id = f.subject_id{filter}
               ORDER BY s.code, f.relative_path"#
        );
        self.rt.block_on(async {
            let rows = sqlx::query(&sql).fetch_all(pool).await.map_err(|e| e.to_string())?;
            Ok(rows
                .iter()
                .map(|r| LibFile {
                    id: r.try_get("id").unwrap_or_default(),
                    code: r.try_get("code").unwrap_or_default(),
                    filename: r.try_get("filename").unwrap_or_default(),
                    relative_path: r.try_get("relative_path").unwrap_or_default(),
                    file_type: r.try_get("file_type").unwrap_or_default(),
                    category: r.try_get("category").ok().flatten(),
                    size_bytes: r.try_get("size_bytes").ok().flatten(),
                    parse_status: r.try_get("parse_status").ok().flatten(),
                    indexed_pages: r.try_get("indexed_pages").unwrap_or_default(),
                })
                .collect())
        })
    }

    /// One file's parsed pages, in order.
    fn pages_of(&self, pool: &SqlitePool, file_id: i64) -> Result<Vec<(i64, String)>, String> {
        self.rt.block_on(async {
            let rows = sqlx::query(
                "SELECT page_no, markdown FROM pages
                  WHERE file_id = ?1 AND markdown != '' ORDER BY page_no",
            )
            .bind(file_id)
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;
            Ok(rows
                .iter()
                .map(|r| {
                    (
                        r.try_get("page_no").unwrap_or_default(),
                        r.try_get("markdown").unwrap_or_default(),
                    )
                })
                .collect())
        })
    }

    /// Parsed page markdown, keyed by file and ordered by page number.
    ///
    /// Loaded whole in one query rather than one query per file.
    fn page_text(
        &self,
        pool: &SqlitePool,
        ids: &[i64],
    ) -> Result<HashMap<i64, Vec<(i64, String)>>, String> {
        let filter = if ids.is_empty() {
            String::new()
        } else {
            let list: Vec<String> = ids.iter().map(|i| i.to_string()).collect();
            format!(" AND f.subject_id IN ({})", list.join(","))
        };
        let sql = format!(
            r#"SELECT p.file_id AS file_id, p.page_no AS page_no, p.markdown AS markdown
               FROM pages p JOIN files f ON f.id = p.file_id
               WHERE p.markdown != ''{filter}
               ORDER BY p.file_id, p.page_no"#
        );
        self.rt.block_on(async {
            let rows = sqlx::query(&sql).fetch_all(pool).await.map_err(|e| e.to_string())?;
            let mut out: HashMap<i64, Vec<(i64, String)>> = HashMap::new();
            for r in &rows {
                let id: i64 = r.try_get("file_id").unwrap_or_default();
                let page: i64 = r.try_get("page_no").unwrap_or_default();
                let md: String = r.try_get("markdown").unwrap_or_default();
                out.entry(id).or_default().push((page, md));
            }
            Ok(out)
        })
    }
}

// ── Library lookup ───────────────────────────────────────────────────────────

/// One row of `files` with its subject code and how much of it is searchable.
#[derive(Serialize)]
pub(crate) struct LibFile {
    #[serde(skip)]
    id: i64,
    #[serde(rename = "subject")]
    code: String,
    #[serde(rename = "path")]
    relative_path: String,
    filename: String,
    file_type: String,
    category: Option<String>,
    size_bytes: Option<i64>,
    parse_status: Option<String>,
    indexed_pages: i64,
}

/// Extensions whose bytes are worth reading as text.
pub(crate) const TEXT_EXTS: &[&str] = &["md", "txt", "csv", "json", "html", "htm", "vtt", "srt"];

pub(crate) fn is_text_file(rel: &str) -> bool {
    let lower = rel.to_ascii_lowercase();
    TEXT_EXTS.iter().any(|e| lower.ends_with(&format!(".{e}")))
}

/// Find the one file a caller meant.
///
/// Tiered, not fuzzy: exact path, then exact filename, then fragment; only the
/// best tier that matched counts, and a tie within it is reported, not guessed.
pub(crate) fn resolve_file<'a>(files: &'a [LibFile], target: &str) -> Result<&'a LibFile, String> {
    let needle = target.to_lowercase();
    let tiers: [Box<dyn Fn(&LibFile) -> bool>; 4] = [
        Box::new(|f: &LibFile| f.relative_path == target),
        Box::new(|f: &LibFile| f.filename == target),
        Box::new(|f: &LibFile| f.filename.to_lowercase() == needle),
        Box::new(|f: &LibFile| f.relative_path.to_lowercase().contains(&needle)),
    ];

    for matches in tiers {
        let hits: Vec<&LibFile> = files.iter().filter(|f| matches(f)).collect();
        match hits.len() {
            0 => continue,
            1 => return Ok(hits[0]),
            _ => {
                let mut message = format!("{} matches {} files:\n", target, hits.len());
                for f in hits.iter().take(12) {
                    message.push_str(&format!("       {}\n", f.relative_path));
                }
                if hits.len() > 12 {
                    message.push_str(&format!("       … and {} more\n", hits.len() - 12));
                }
                message.push_str("       Name one of them, or narrow it with --subject.");
                return Err(message);
            }
        }
    }
    Err(format!(
        "no library file matches {target} — `oculus files -m {}` to look",
        shell_quote(target)
    ))
}

/// `12`, `12-15`, `12,14,20-22`, `30-` (to the end), `-4` (from the start).
pub(crate) fn parse_page_spec(spec: &str) -> Result<Vec<(i64, i64)>, String> {
    let mut ranges = Vec::new();
    for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let bad = || format!("not a page range: {part}");
        let (lo, hi) = match part.split_once('-') {
            None => {
                let n: i64 = part.parse().map_err(|_| bad())?;
                (n, n)
            }
            Some((from, to)) => {
                let lo = if from.trim().is_empty() { 1 } else { from.trim().parse().map_err(|_| bad())? };
                let hi = if to.trim().is_empty() { i64::MAX } else { to.trim().parse().map_err(|_| bad())? };
                (lo, hi)
            }
        };
        if lo > hi {
            return Err(format!("empty page range: {part}"));
        }
        ranges.push((lo, hi));
    }
    if ranges.is_empty() {
        return Err("no pages given".to_string());
    }
    Ok(ranges)
}

pub(crate) fn page_wanted(ranges: &[(i64, i64)], page: i64) -> bool {
    ranges.iter().any(|(lo, hi)| page >= *lo && page <= *hi)
}

pub(crate) fn build_regex(pattern: &str, fixed: bool, case_sensitive: bool) -> Result<regex::Regex, String> {
    let body = if fixed { regex::escape(pattern) } else { pattern.to_string() };
    regex::RegexBuilder::new(&body)
        .case_insensitive(!case_sensitive)
        .build()
        .map_err(|e| format!("bad pattern: {e}"))
}

/// A page of markdown flattened to one line of prose, for a result list.
pub(crate) fn snippet(markdown: &str, max: usize) -> String {
    let flat: Vec<&str> = markdown.split_whitespace().collect();
    truncate(&flat.join(" "), max)
}

/// Quote a suggested command argument so a name with spaces pastes intact.
pub(crate) fn shell_quote(s: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || "._-/".contains(c)) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', r"'\''"))
    }
}

#[cfg(test)]
mod query_tests {
    use super::*;

    #[test]
    fn query_scopes_include_every_term_and_never_hide_an_unknown_code() {
        let subjects: Vec<store::SubjectRow> = ["COMP10001_2026_SM1", "COMP10001_2026_SM2", "MULT20015_2026_SM2"]
            .iter().enumerate().map(|(n, code)| store::SubjectRow {
                id: n as i64 + 1, code: code.to_string(), name: String::new(),
                term_name: None, is_current: n != 0, selected: true, last_synced_at: None,
            }).collect();
        assert!(subject_ids(&subjects, &[]).unwrap().is_empty());
        assert_eq!(subject_ids(&subjects, &["COMP10001".into()]).unwrap(), [1, 2]);
        assert_eq!(subject_ids(&subjects, &["COMP10001_2026_SM2".into()]).unwrap(), [2]);
        assert!(subject_ids(&subjects, &["MISS10000".into()]).unwrap_err().contains("no subject matched"));
    }

    #[test]
    fn page_specs_cover_points_ranges_and_open_ends() {
        let spec = parse_page_spec("3,7-9,20-").unwrap();
        for wanted in [3, 7, 8, 9, 20, 4000] {
            assert!(page_wanted(&spec, wanted), "{wanted} should be selected");
        }
        for unwanted in [2, 4, 6, 10, 19] {
            assert!(!page_wanted(&spec, unwanted), "{unwanted} should not be");
        }
        assert!(parse_page_spec("9-4").is_err());
        assert!(parse_page_spec("twelve").is_err());
        assert!(parse_page_spec("").is_err());
    }

    #[test]
    fn fixed_strings_do_not_read_as_patterns() {
        assert!(build_regex("a.c", false, false).unwrap().is_match("abc"));
        assert!(!build_regex("a.c", true, false).unwrap().is_match("abc"));
        assert!(build_regex("a.c", true, false).unwrap().is_match("A.C"));
        assert!(!build_regex("a.c", true, true).unwrap().is_match("A.C"));
    }

    fn file(code: &str, rel: &str) -> LibFile {
        LibFile {
            id: 0,
            code: code.to_string(),
            filename: rel.rsplit('/').next().unwrap().to_string(),
            relative_path: rel.to_string(),
            file_type: "pdf".to_string(),
            category: None,
            size_bytes: None,
            parse_status: None,
            indexed_pages: 0,
        }
    }

    #[test]
    fn file_json_keeps_the_public_names_without_a_database_id() {
        let row = file("COMP30026", "courses/COMP30026/files/week-01.pdf");
        assert_eq!(serde_json::to_value(&row).unwrap(), serde_json::json!({
            "subject": "COMP30026",
            "path": "courses/COMP30026/files/week-01.pdf",
            "filename": "week-01.pdf",
            "file_type": "pdf",
            "category": null,
            "size_bytes": null,
            "parse_status": null,
            "indexed_pages": 0
        }));
    }

    /// A filename shared by two subjects resolves by full path, and is reported
    /// rather than guessed otherwise.
    #[test]
    fn resolution_prefers_the_most_exact_tier() {
        let files = vec![
            file("COMP30026", "courses/COMP30026/files/week-01.pdf"),
            file("MULT20015", "courses/MULT20015/files/week-01.pdf"),
            file("MULT20015", "courses/MULT20015/files/notes.pdf"),
        ];
        assert_eq!(
            resolve_file(&files, "courses/MULT20015/files/week-01.pdf").unwrap().code,
            "MULT20015"
        );
        assert_eq!(resolve_file(&files, "notes.pdf").unwrap().code, "MULT20015");
        assert_eq!(resolve_file(&files, "NOTES.PDF").unwrap().code, "MULT20015");
        assert_eq!(resolve_file(&files, "COMP30026/files/week").unwrap().code, "COMP30026");
        assert!(resolve_file(&files, "week-01.pdf").is_err());
        assert!(resolve_file(&files, "nothing-like-this").is_err());
    }

    fn filed(rel: &str, category: &str) -> LibFile {
        LibFile { category: Some(category.to_string()), ..file("COMP30026", rel) }
    }

    /// A category narrows before the match limit.
    #[test]
    fn categories_narrow_the_scan() {
        let rows = || {
            vec![
                filed("courses/COMP30026/ed/0001-teams.md", "ed"),
                filed("courses/COMP30026/announcements/2026-07-14-welcome.md", "announcement"),
                filed("courses/COMP30026/files/week-01.pdf", "file"),
            ]
        };

        let kept = filter_categories(rows(), &["ed".into()]).ok().unwrap();
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].category.as_deref(), Some("ed"));

        // Repeatable, and case is not the caller's problem.
        let pair = filter_categories(rows(), &["ED".into(), "announcement".into()]).ok().unwrap();
        assert_eq!(pair.len(), 2);

        // No flag is every category, not none.
        assert_eq!(filter_categories(rows(), &[]).ok().unwrap().len(), 3);
    }

    /// A word that is not a category is refused; a real category these rows lack
    /// is an empty answer.
    #[test]
    fn a_typo_is_refused_but_an_honest_miss_is_empty() {
        let rows = || vec![filed("courses/COMP30026/ed/0001-teams.md", "ed")];

        let err = filter_categories(rows(), &["eds".into()]).err().unwrap();
        assert!(err.contains("no category \"eds\""), "{err}");
        // The refusal names the real ones, from the one list that defines them.
        assert!(err.contains("announcement"), "{err}");
        assert!(err.contains("quiz"), "{err}");

        // Real category, none in these rows: empty, not an error.
        assert!(filter_categories(rows(), &["quiz".into()]).ok().unwrap().is_empty());

        // An empty row set does not excuse the typo.
        assert!(filter_categories(vec![], &["eds".into()]).is_err());
        assert!(filter_categories(vec![], &["quiz".into()]).ok().unwrap().is_empty());
    }

    /// Both commands must accept the same words and offer the same list.
    #[test]
    fn the_category_help_lists_what_the_flag_accepts() {
        let help = category_help();
        for c in paths::CATEGORIES {
            assert!(help.contains(c), "{c:?} missing from {help:?}");
        }
    }
}

// ── Subject filtering ────────────────────────────────────────────────────────

/// `MULT20015` matches `MULT20015_2026_SM2`.
pub(crate) fn matches_code(code: &str, wanted: &str) -> bool {
    let (code, wanted) = (code.to_uppercase(), wanted.to_uppercase());
    code == wanted || code.starts_with(&format!("{wanted}_"))
}

/// Narrow a file list to the categories asked for, and refuse a word that is
/// not a category: a silent empty result from a typo reads as "the library does
/// not cover that". Validity is `paths::CATEGORIES`, not the rows' own
/// categories, so a subject with no quizzes answers `--category quiz` with
/// nothing rather than an error. Shared by `grep` and `files`.
pub(crate) fn filter_categories(files: Vec<LibFile>, wanted: &[String]) -> Result<Vec<LibFile>, String> {
    if wanted.is_empty() {
        return Ok(files);
    }
    for c in wanted {
        if !paths::CATEGORIES.iter().any(|k| k.eq_ignore_ascii_case(c)) {
            return Err(format!(
                "no category {c:?} — the categories are: {}",
                paths::CATEGORIES.join(", ")
            ));
        }
    }
    Ok(files
        .into_iter()
        .filter(|f| {
            f.category
                .as_deref()
                .is_some_and(|k| wanted.iter().any(|c| k.eq_ignore_ascii_case(c)))
        })
        .collect())
}

/// The `--category` help, built from the list the flag validates against.
pub(crate) fn category_help() -> String {
    format!("Only these categories ({}). Repeatable", paths::CATEGORIES.join(", "))
}

pub(crate) fn filter_subjects(
    subjects: &[store::SubjectRow],
    codes: &[String],
    current_only_when_empty: bool,
) -> Result<Vec<store::SubjectRow>, String> {
    let picked: Vec<store::SubjectRow> = subjects
        .iter()
        .filter(|s| {
            if codes.is_empty() {
                !current_only_when_empty || s.is_current
            } else {
                codes.iter().any(|w| matches_code(&s.code, w))
            }
        })
        .cloned()
        .collect();

    if picked.is_empty() && !codes.is_empty() {
        return Err(format!("no subject matched {}", codes.join(", ")));
    }
    Ok(picked)
}

/// An empty set means all subjects to query loaders; a bare code includes every term.
fn subject_ids(subjects: &[store::SubjectRow], codes: &[String]) -> Result<Vec<i64>, String> {
    if codes.is_empty() { return Ok(Vec::new()); }
    Ok(filter_subjects(subjects, codes, false)?.iter().map(|s| s.id).collect())
}
