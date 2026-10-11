//! `oculus calendar`.

use crate::*;

impl Ctx {
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
            let rows = sqlx::query(&sql)
                .fetch_all(&pool)
                .await
                .map_err(|e| e.to_string())?;
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
                if e.kind == "due" {
                    paint("due  ", YELLOW)
                } else {
                    paint("class", DIM)
                },
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
}
