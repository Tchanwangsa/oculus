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
                let id=self.rt.block_on(revision::add(&pool,Entity::Artifact,&json!({"title":a.title,"program":program,"state":state,"subject_id":subject,"source_ref":a.source,"topic_ids":topic_ids(a.topics.as_deref())?})))?;
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
