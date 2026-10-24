use super::*;

impl Ctx {
    pub(crate) fn topic(&self, action: TopicAction) -> Result<(), String> {
        let pool = self.planning_db()?;
        match action {
            TopicAction::Add(a) => {
                let subject = self.one_subject(&pool, &a.subject)?;
                let id = self.rt.block_on(revision::topics::add(
                    &pool,
                    &json!({"name": a.name, "subject_id": subject, "description": a.description}),
                ))?;
                self.revision_output(json!({"id": id}))
            }
            TopicAction::List(a) => {
                let subject = self.revision_subject(&pool, a.subject.as_deref())?;
                let rows: Vec<Value> = self
                    .rt
                    .block_on(revision::topics::list(&pool))?
                    .into_iter()
                    .filter(|v| subject.is_none_or(|s| v["subject_id"].as_i64() == Some(s)))
                    .collect();
                self.revision_output(json!(rows))
            }
            TopicAction::Show(a) => {
                self.revision_output(self.rt.block_on(revision::topics::show(&pool, a.id))?)
            }
            TopicAction::Update(a) => {
                self.rt
                    .block_on(revision::topics::update(&pool, a.id, &patch_value(&a)?))
            }
            TopicAction::Rm(a) => self.rt.block_on(revision::topics::remove(&pool, a.id)),
        }
    }
}
