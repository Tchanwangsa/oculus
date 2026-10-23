//! `oculus calendar`.

use crate::*;

impl Ctx {
    pub(crate) fn calendar(&self, args: &CalendarArgs) -> Result<(), String> {
        let pool = self.db().ok_or("the calendar lives in the database")?;
        let ids = self.subject_ids(&pool, &args.codes)?;

        let sql = calendar_sql(&ids, args.due, args.past, args.days);

        #[derive(Serialize)]
        struct Event {
            id: String,
            subject: Option<String>,
            kind: String,
            title: String,
            start_at: String,
            end_at: Option<String>,
            notes: Option<String>,
            all_day: bool,
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
                        id: r.get("id"),
                        subject: r.try_get("code").unwrap_or_default(),
                        kind: r.try_get("kind").unwrap_or_default(),
                        title: r.try_get("title").unwrap_or_default(),
                        start_at: r.try_get("start_at").unwrap_or_default(),
                        end_at: r.get("end_at"),
                        notes: r.get("notes"),
                        all_day: r.get::<i64, _>("all_day") != 0,
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
                "{} {} {} {:<20} {}{}",
                e.id,
                paint(&e.starts_local, DIM),
                if e.kind == "due" {
                    paint("due  ", YELLOW)
                } else {
                    paint(&format!("{:<5}", e.kind), DIM)
                },
                truncate(e.subject.as_deref().unwrap_or("Personal"), 20),
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

fn calendar_sql(ids: &[i64], due: bool, past: bool, days: i64) -> String {
    let mut clauses: Vec<String> = Vec::new();
    if !ids.is_empty() {
        let list: Vec<String> = ids.iter().map(|i| i.to_string()).collect();
        clauses.push(format!("e.subject_id IN ({})", list.join(",")));
    }
    if due {
        clauses.push("e.kind = 'due'".to_string());
    }
    // Compare instants across offsets; date-only events stay visible on their local day.
    if !past {
        clauses.push("(julianday(e.start_at) >= julianday('now') OR (length(e.start_at) = 10 AND e.start_at = date('now','localtime')))".to_string());
    }
    clauses.push(format!(
        "julianday(e.start_at) < julianday('now','+{} days')",
        days.max(0)
    ));
    format!(
        r#"WITH events AS (
            SELECT id, subject_id, kind, title, start_at, end_at, all_day, NULL AS notes, location, url FROM calendar_events
            UNION ALL
            SELECT 'local_' || id, subject_id, kind, title, start_at, end_at, all_day, notes, NULL, NULL FROM local_events
        )
        SELECT e.id AS id, s.code AS code, e.kind AS kind, e.title AS title,
              e.start_at AS start_at, e.end_at, e.all_day, e.notes, e.location AS location, e.url AS url,
              CASE WHEN length(e.start_at)=10 THEN e.start_at ELSE strftime('%Y-%m-%d %H:%M', e.start_at, 'localtime') END AS local_at
           FROM events e LEFT JOIN subjects s ON s.id = e.subject_id
           WHERE {}
           ORDER BY julianday(e.start_at), e.id"#,
        clauses.join(" AND ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn calendar_includes_personal_ids_and_filters_by_instants() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::raw_sql("CREATE TABLE subjects(id INTEGER,code TEXT); CREATE TABLE calendar_events(id TEXT,subject_id INTEGER,kind TEXT,title TEXT,start_at TEXT,end_at TEXT,all_day INTEGER,location TEXT,url TEXT); CREATE TABLE local_events(id INTEGER,subject_id INTEGER,kind TEXT,title TEXT,start_at TEXT,end_at TEXT,all_day INTEGER,notes TEXT); INSERT INTO subjects VALUES(1,'TEST10001');
        INSERT INTO local_events VALUES(1,NULL,'note','All day',date('now','localtime'),NULL,1,'Read chapter 3'),(2,NULL,'class','Past',strftime('%Y-%m-%dT%H:%M:%SZ','now','-1 hour'),NULL,0,NULL),(3,1,'due','Offset first',date('now','+2 days')||'T01:00:00+11:00',date('now','+2 days')||'T02:00:00+11:00',0,'Bring notes');
        INSERT INTO calendar_events VALUES('canvas',1,'due','UTC second',date('now','+2 days')||'T00:30:00Z',NULL,0,NULL,NULL);").execute(&pool).await.unwrap();
        let rows = sqlx::query(&calendar_sql(&[], false, false, 7))
            .fetch_all(&pool)
            .await
            .unwrap();
        let ids: Vec<String> = rows.iter().map(|r| r.get("id")).collect();
        assert!(ids.contains(&"local_1".into()));
        assert!(!ids.contains(&"local_2".into()));
        assert!(ids.iter().position(|s| s == "local_3") < ids.iter().position(|s| s == "canvas"));
        let personal = rows
            .iter()
            .find(|r| r.get::<String, _>("id") == "local_1")
            .unwrap();
        assert_eq!(personal.get::<Option<String>, _>("code"), None);
        assert_eq!(personal.get::<String, _>("notes"), "Read chapter 3");
        assert_eq!(
            personal.get::<String, _>("local_at"),
            personal.get::<String, _>("start_at")
        );
        assert_eq!(personal.get::<i64, _>("all_day"), 1);
        let own = rows
            .iter()
            .find(|r| r.get::<String, _>("id") == "local_3")
            .unwrap();
        assert!(own.get::<String, _>("end_at").ends_with("T02:00:00+11:00"));
        assert_eq!(own.get::<String, _>("notes"), "Bring notes");
        let canvas = rows
            .iter()
            .find(|r| r.get::<String, _>("id") == "canvas")
            .unwrap();
        assert_eq!(canvas.get::<Option<String>, _>("notes"), None);
        let rows = sqlx::query(&calendar_sql(&[], false, true, 7))
            .fetch_all(&pool)
            .await
            .unwrap();
        assert!(rows.iter().any(|r| r.get::<String, _>("id") == "local_2"));
        let rows = sqlx::query(&calendar_sql(&[1], true, false, 7))
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(rows.len(), 2);
    }
}
