//! Revision CLI commands share validated JSON writers with the app.
use crate::*;
use app_lib::db::revision::{self, Entity};
use serde_json::{json, Value};

mod topics;

impl Ctx {
    fn revision_subject(
        &self,
        pool: &SqlitePool,
        code: Option<&str>,
    ) -> Result<Option<i64>, String> {
        code.map(|c| self.one_subject(pool, c)).transpose()
    }
    fn revision_output(&self, value: Value) -> Result<(), String> {
        println!(
            "{}",
            serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?
        );
        Ok(())
    }
    fn revision_list(
        &self,
        pool: &SqlitePool,
        entity: Entity,
        a: &RevisionListArgs,
    ) -> Result<Vec<Value>, String> {
        let subject = self.revision_subject(pool, a.subject.as_deref())?;
        if let Some(topic) = a.topic {
            self.rt.block_on(revision::topics::show(pool, topic))?;
        }
        Ok(self
            .rt
            .block_on(revision::list(pool, entity))?
            .into_iter()
            .filter(|v| {
                if a.topic.is_some_and(|id| {
                    !v["topic_ids"]
                        .as_array()
                        .is_some_and(|ids| ids.contains(&json!(id)))
                }) {
                    return false;
                }
                subject.is_none_or(|s| v["subject_id"].as_i64() == Some(s))
            })
            .collect())
    }
    pub(crate) fn cheat(&self, action: CheatAction) -> Result<(), String> {
        let pool = self.planning_db()?;
        match action {
            CheatAction::Add(a) => {
                let subject = self.one_subject(&pool, &a.subject)?;
                let body =
                    super::planning::planning_text(a.body.as_ref(), a.body_file.as_ref(), "body")?
                        .unwrap_or_default();
                let id=self.rt.block_on(revision::add(&pool,Entity::Note,&json!({"title":a.title,"body":body,"subject_id":subject,"tags":tags(a.tags.as_deref()),"source_kind":a.source_kind,"source_ref":a.source,"topic_ids":topic_ids(a.topics.as_deref())?})))?;
                self.revision_output(json!({"id":id}))
            }
            CheatAction::List(a) => {
                self.revision_output(json!(self.revision_list(&pool, Entity::Note, &a)?))
            }
            CheatAction::Show(a) => self.revision_output(self.rt.block_on(revision::show(
                &pool,
                Entity::Note,
                a.id,
            ))?),
            CheatAction::Rm(a) => self
                .rt
                .block_on(revision::remove(&pool, Entity::Note, a.id)),
            CheatAction::Tags => {
                self.revision_output(json!(self.rt.block_on(revision::tags::list(&pool))?))
            }
            CheatAction::Update(a) => {
                let mut patch = json!({});
                if let Some(v) = a.title {
                    patch["title"] = json!(v);
                }
                if let Some(body) =
                    super::planning::planning_text(a.body.as_ref(), a.body_file.as_ref(), "body")?
                {
                    patch["body"] = json!(body);
                }
                if let Some(code) = a.subject {
                    patch["subject_id"] = json!(self.one_subject(&pool, &code)?);
                }
                if let Some(v) = a.tags {
                    patch["tags"] = json!(tags(Some(&v)));
                }
                if let Some(v) = a.topics {
                    patch["topic_ids"] = json!(topic_ids(Some(&v))?);
                }
                if let Some(v) = a.source {
                    patch["source_ref"] = json!(v);
                }
                self.rt
                    .block_on(revision::update(&pool, Entity::Note, a.id, &patch))
            }
        }
    }
    pub(crate) fn quiz(&self, action: QuizAction) -> Result<(), String> {
        let pool = self.planning_db()?;
        match action {
            QuizAction::Add(a) => {
                let subject = self.one_subject(&pool, &a.subject)?;
                let mut values = if let Some(file) = a.batch {
                    let raw = read_input(&file, "batch")?;
                    if raw.trim_start().starts_with('[') {
                        serde_json::from_str::<Vec<Value>>(&raw).map_err(|e| e.to_string())?
                    } else {
                        raw.lines()
                            .filter(|s| !s.trim().is_empty())
                            .map(|s| serde_json::from_str(s).map_err(|e| e.to_string()))
                            .collect::<Result<Vec<_>, _>>()?
                    }
                } else {
                    let mut item = json!({"kind":a.kind.unwrap_or_else(||"card".into()),"prompt":a.prompt.ok_or("--prompt required")?,"answer":a.answer.ok_or("--answer required")?,"level":a.level.unwrap_or_else(||"medium".into()),"topic":tags(a.topic.as_deref()),"style_ref":a.style_ref,"topic_ids":topic_ids(a.topics.as_deref())?});
                    if let Some(v) = a.options {
                        item["options"] = serde_json::from_str(&v).map_err(|e| e.to_string())?;
                    }
                    if let Some(v) = a.rubric {
                        item["rubric"] = serde_json::from_str(&v).map_err(|e| e.to_string())?;
                    }
                    if let Some(v) = a.tolerance {
                        item["tolerance"] = json!(v);
                    }
                    vec![item]
                };
                if values.is_empty() || values.len() > 500 {
                    return Err("Supply 1–500 questions".into());
                }
                for v in &mut values {
                    reject_scheduling_fields(v)?;
                    let obj = v.as_object_mut().ok_or("Each question must be an object")?;
                    obj.insert("status".into(), json!("draft"));
                    obj.insert("created_by".into(), json!("agent"));
                    obj.insert("subject_id".into(), json!(subject));
                }
                let ids = self
                    .rt
                    .block_on(revision::add_batch(&pool, Entity::Item, &values))?;
                self.revision_output(json!({"ids":ids}))
            }
            QuizAction::List(a) => {
                let mut rows = self.revision_list(&pool, Entity::Item, &a.scope)?;
                if a.due {
                    let now = chrono::Utc::now();
                    rows.retain(|v| {
                        v["status"] == "active"
                            && v["due"]
                                .as_str()
                                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                                .is_some_and(|d| d <= now)
                    });
                }
                if let Some(level) = a.level {
                    rows.retain(|v| v["level"] == level);
                }
                if let Some(set) = a.set {
                    let ids = self.rt.block_on(revision::set_items(&pool, set))?;
                    rows.retain(|v| v["id"].as_i64().is_some_and(|id| ids.contains(&id)));
                    rows.sort_by_key(|v| ids.iter().position(|id| Some(*id) == v["id"].as_i64()));
                }
                self.revision_output(json!(rows))
            }
            QuizAction::Show(a) => self.revision_output(self.rt.block_on(revision::show(
                &pool,
                Entity::Item,
                a.id,
            ))?),
            QuizAction::Rm(a) => self
                .rt
                .block_on(revision::remove(&pool, Entity::Item, a.id)),
            QuizAction::Update(a) => {
                let patch = patch_value(&a)?;
                reject_scheduling_fields(&patch)?;
                self.rt
                    .block_on(revision::update(&pool, Entity::Item, a.id, &patch))
            }
            QuizAction::Set { action } => match action {
                QuizSetAction::List(a) => {
                    self.revision_output(json!(self.revision_list(&pool, Entity::Set, &a)?))
                }
                QuizSetAction::Show(a) => self
                    .revision_output(self.rt.block_on(revision::show(&pool, Entity::Set, a.id))?),
                QuizSetAction::Update(a) => self.rt.block_on(revision::update(
                    &pool,
                    Entity::Set,
                    a.id,
                    &patch_value(&a)?,
                )),
                QuizSetAction::Add(a) => {
                    let subject = self.one_subject(&pool, &a.subject)?;
                    let id=self.rt.block_on(revision::add_set(&pool,&json!({"title":a.title,"subject_id":subject,"description":a.description,"topic_ids":topic_ids(a.topics.as_deref())?}),&a.items))?;
                    self.revision_output(json!({"id":id}))
                }
            },
        }
    }
    pub(crate) fn artifact(&self, action: ArtifactAction) -> Result<(), String> {
        let pool = self.planning_db()?;
        match action {
            ArtifactAction::Save(a) => {
                let subject = self.one_subject(&pool, &a.subject)?;
                let program = read_input(&a.program_file, "program")?;
                let state = match a.state {
                    Some(s) => serde_json::from_str::<Value>(&s).map_err(|e| e.to_string())?,
                    None => json!({}),
                };
                let id=self.rt.block_on(revision::add(&pool,Entity::Artifact,&json!({"title":a.title,"program":program,"format":a.format,"state":state,"subject_id":subject,"source_ref":a.source,"topic_ids":topic_ids(a.topics.as_deref())?})))?;
                self.revision_output(json!({"id":id}))
            }
            ArtifactAction::Update(a) => self.rt.block_on(revision::update(
                &pool,
                Entity::Artifact,
                a.id,
                &patch_value(&a)?,
            )),
            ArtifactAction::List(a) => {
                self.revision_output(json!(self.revision_list(&pool, Entity::Artifact, &a)?))
            }
            ArtifactAction::Show(a) => self.revision_output(self.rt.block_on(revision::show(
                &pool,
                Entity::Artifact,
                a.id,
            ))?),
            ArtifactAction::Rm(a) => {
                self.rt
                    .block_on(revision::remove(&pool, Entity::Artifact, a.id))
            }
        }
    }
}
fn patch_value(a: &QuizUpdateArgs) -> Result<Value, String> {
    let raw = super::planning::planning_text(a.patch.as_ref(), a.patch_file.as_ref(), "patch")?
        .ok_or("Supply --patch or --patch-file")?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

fn topic_ids(text: Option<&str>) -> Result<Vec<i64>, String> {
    let mut ids = Vec::new();
    for value in text
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        let id = value
            .parse::<i64>()
            .map_err(|_| format!("Invalid topic id: {value}"))?;
        if id <= 0 {
            return Err("Topic ids must be positive integers".into());
        }
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    Ok(ids)
}

fn tags(text: Option<&str>) -> Vec<String> {
    text.unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Scheduling belongs to a student's review, including when importing drafts.
fn reject_scheduling_fields(value: &Value) -> Result<(), String> {
    for key in [
        "due",
        "stability",
        "fsrs_difficulty",
        "reps",
        "lapses",
        "state",
        "last_review",
        "learning_steps",
    ] {
        if value.get(key).is_some() {
            return Err(format!("Scheduling field {key} is read-only in the CLI"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topic_flags_preserve_unsorted_and_require_real_ids() {
        assert_eq!(topic_ids(None).unwrap(), Vec::<i64>::new());
        assert_eq!(topic_ids(Some("")).unwrap(), Vec::<i64>::new());
        assert_eq!(topic_ids(Some("2, 1,2")).unwrap(), vec![2, 1]);
        for value in ["0", "-1", "name", "1.5"] {
            assert!(topic_ids(Some(value)).is_err());
        }
    }

    /// `artifact save` against a throwaway database: a widget is saved with
    /// its format and state, and one that doesn't compile writes nothing.
    #[test]
    fn widget_artifacts_round_trip_through_the_cli() {
        let dir = std::env::temp_dir().join(format!("oculus-artifact-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ctx = Ctx {
            data_dir: dir.clone(),
            rt: Runtime::new().unwrap(),
            json: true,
        };
        let path = app_lib::library::paths::db_path(&dir);
        ctx.rt.block_on(async {
            // One connection, as the app's migrator uses: each migration sees the last.
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(
                    sqlx::sqlite::SqliteConnectOptions::new()
                        .filename(&path)
                        .create_if_missing(true),
                )
                .await
                .unwrap();
            for migration in app_lib::db::migrations::all() {
                sqlx::raw_sql(migration.sql).execute(&pool).await.unwrap();
            }
            sqlx::query("INSERT INTO subjects(id,code,name) VALUES(1,'MATH1','Maths')")
                .execute(&pool)
                .await
                .unwrap();
            pool.close().await;
        });
        let save = |program: &str| {
            let file = dir.join("widget.tsx");
            std::fs::write(&file, program).unwrap();
            let file = file.to_str().unwrap().to_string();
            let cli = Cli::try_parse_from([
                "oculus",
                "artifact",
                "save",
                "--title",
                "Phase",
                "--subject",
                "MATH1",
                "--format",
                "widget",
                "--state",
                r#"{"angle":1}"#,
                "--program-file",
                &file,
            ])
            .unwrap();
            match cli.command.unwrap() {
                Command::Artifact { action } => ctx.artifact(action),
                _ => panic!("wrong command"),
            }
        };
        let widget = "// title: Phase\nexport default function Phase() { return <p>θ</p>; }\n";
        save(widget).unwrap();
        assert!(save("export default () => <p>")
            .unwrap_err()
            .contains("does not compile"));

        let pool = ctx.planning_db().unwrap();
        let rows = ctx
            .rt
            .block_on(revision::list(&pool, Entity::Artifact))
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["format"], "widget");
        assert_eq!(rows[0]["program"], widget);
        assert_eq!(rows[0]["state"], json!({"angle":1}));
        ctx.rt.block_on(pool.close());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn artifact_format_defaults_to_openui() {
        let parse = |extra: &[&str]| {
            let args = [
                "oculus",
                "artifact",
                "save",
                "--title",
                "A",
                "--subject",
                "M",
            ]
            .into_iter()
            .chain(["--program-file", "-"])
            .chain(extra.iter().copied());
            match Cli::try_parse_from(args).map(|cli| cli.command) {
                Ok(Some(Command::Artifact {
                    action: ArtifactAction::Save(a),
                })) => Ok(a.format),
                Ok(_) => panic!("wrong command"),
                Err(e) => Err(e),
            }
        };
        assert_eq!(parse(&[]).unwrap(), "openui");
        assert_eq!(parse(&["--format", "widget"]).unwrap(), "widget");
        assert!(parse(&["--format", "html"]).is_err());
    }

    #[test]
    fn imported_drafts_cannot_carry_scheduler_state() {
        for key in [
            "due",
            "stability",
            "fsrs_difficulty",
            "reps",
            "lapses",
            "state",
            "last_review",
            "learning_steps",
        ] {
            let mut item = json!({"kind":"card","prompt":"Front","answer":"Back"});
            item[key] = Value::Null;
            assert!(reject_scheduling_fields(&item).unwrap_err().contains(key));
        }
        assert!(reject_scheduling_fields(&json!({"kind":"numeric","prompt":"2 + 2","answer":"4","tolerance":0.1,"status":"draft"})).is_ok());
    }
}
