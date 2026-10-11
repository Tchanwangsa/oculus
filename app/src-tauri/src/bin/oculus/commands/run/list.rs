//! `oculus list`.

use crate::*;

impl Ctx {
    pub(crate) fn list(&self, args: ListArgs) -> Result<(), String> {
        if args.lectures {
            return self.list_lectures(&args.codes);
        }
        self.list_subjects(args.refresh)
    }

    fn list_subjects(&self, refresh: bool) -> Result<(), String> {
        let pool = self.db();

        // Fetch live when asked, or when the database is empty.
        let mut rows = match &pool {
            Some(p) => self.rt.block_on(store::subjects(p))?,
            None => Vec::new(),
        };
        if refresh || rows.is_empty() {
            let engine = self.engine(false);
            engine.canvas.check_keyd()?;
            let courses = engine.list_courses()?;
            if let Some(p) = &pool {
                self.rt.block_on(store::upsert_subjects(p, &courses))?;
                rows = self.rt.block_on(store::subjects(p))?;
            } else {
                for c in &courses {
                    println!(
                        "{:<24} {:<12} {}",
                        c.code,
                        c.term.clone().unwrap_or_default(),
                        c.name
                    );
                }
                return Ok(());
            }
        }

        if self.json {
            #[derive(Serialize)]
            struct Subject<'a> {
                id: i64,
                code: &'a str,
                name: &'a str,
                term: Option<&'a str>,
                current: bool,
                selected: bool,
                last_synced_at: Option<&'a str>,
            }
            let out: Vec<Subject> = rows
                .iter()
                .map(|s| Subject {
                    id: s.id,
                    code: &s.code,
                    name: &s.name,
                    term: s.term_name.as_deref(),
                    current: s.is_current,
                    selected: s.selected,
                    last_synced_at: s.last_synced_at.as_deref(),
                })
                .collect();
            return self.emit(&out);
        }

        if rows.is_empty() {
            println!("{}", paint("no subjects", DIM));
            return Ok(());
        }
        for s in &rows {
            let mark = if s.is_current {
                paint("●", GREEN)
            } else {
                paint("○", DIM)
            };
            let synced = s
                .last_synced_at
                .clone()
                .map(|t| t.chars().take(10).collect::<String>())
                .unwrap_or_else(|| "never".into());
            println!(
                "{mark} {:<24} {:<14} {} {}",
                s.code,
                s.term_name.clone().unwrap_or_default(),
                paint(&format!("{synced:<11}"), DIM),
                s.name
            );
        }
        Ok(())
    }

    fn list_lectures(&self, codes: &[String]) -> Result<(), String> {
        let pool = self.db().ok_or("lectures live in the database")?;
        let subjects = self.rt.block_on(store::subjects(&pool))?;
        let wanted = filter_subjects(&subjects, codes, true)?;

        if self.json {
            #[derive(Serialize)]
            struct Lecture<'a> {
                id: &'a str,
                subject: &'a str,
                title: &'a str,
                date: &'a str,
                duration_seconds: i64,
                has_video: bool,
                has_transcript: bool,
            }
            let mut out: Vec<Lecture> = Vec::new();
            let rows: Vec<(String, Vec<store::LectureRow>)> = wanted
                .iter()
                .map(|s| {
                    Ok((
                        s.code.clone(),
                        self.rt.block_on(store::lectures(&pool, s.id))?,
                    ))
                })
                .collect::<Result<_, String>>()?;
            for (code, lectures) in &rows {
                out.extend(lectures.iter().map(|l| Lecture {
                    id: &l.id,
                    subject: code,
                    title: &l.title,
                    date: &l.date,
                    duration_seconds: l.duration_seconds,
                    has_video: l.has_video,
                    has_transcript: l.has_transcript,
                }));
            }
            return self.emit(&out);
        }

        for s in &wanted {
            let rows = self.rt.block_on(store::lectures(&pool, s.id))?;
            println!(
                "{}  {}",
                paint(&s.code, BOLD),
                paint(&format!("{} lectures", rows.len()), DIM)
            );
            for l in &rows {
                let mins = l.duration_seconds / 60;
                let marks = format!(
                    "{}{}",
                    if l.has_video { "video" } else { "     " },
                    if l.has_transcript { " transcript" } else { "" }
                );
                // The id leads, dimmed: its first eight characters are what `oculus lecture`
                // takes as a prefix.
                println!(
                    "  {} {:<11} {mins:>4}m  {} {}",
                    paint(&l.id.chars().take(8).collect::<String>(), DIM),
                    l.date.chars().take(10).collect::<String>(),
                    paint(&format!("{marks:<22}"), DIM),
                    l.title
                );
            }
        }
        Ok(())
    }
}
