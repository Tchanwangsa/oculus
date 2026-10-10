//! What a command did, for JS: `FieldStep`'s JSON.

use oculus_math_edit::{Change, Direction, Effect, Field, Outcome, utf16::Units};
use serde::Serialize;

#[derive(Serialize)]
struct Json<'a> {
    changes: Vec<ChangeJson<'a>>,
    isolate: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    rewrite: Option<Vec<ChangeJson<'a>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    effect: Option<&'static str>,
}

#[derive(Serialize)]
struct ChangeJson<'a> {
    from: u32,
    to: u32,
    insert: &'a str,
}

fn changes<'a>(source: &str, changes: &'a [Change]) -> Vec<ChangeJson<'a>> {
    let units = Units::new(source);
    changes
        .iter()
        .map(|change| ChangeJson {
            from: units.of(change.from),
            to: units.of(change.to),
            insert: &change.insert,
        })
        .collect()
}

/// `source` with `changes` (sorted, in its bytes) applied.
fn applied(source: &str, changes: &[Change]) -> String {
    let mut source = source.to_owned();
    for change in changes.iter().rev() {
        source.replace_range(change.from..change.to, &change.insert);
    }
    source
}

const fn effect(effect: Effect) -> &'static str {
    match effect {
        Effect::Leave(Direction::Left) => "leaveLeft",
        Effect::Leave(Direction::Right) => "leaveRight",
        Effect::Leave(Direction::Up) => "leaveUp",
        Effect::Leave(Direction::Down) => "leaveDown",
        Effect::RemoveMaths => "removeMaths",
    }
}

/// `outcome` of a command run on `before`, as JSON.
///
/// `changes` are UTF-16 offsets of `before`'s source and `rewrite` of the
/// source after them, so a caller applies each in reverse order, as the
/// model's are.
#[must_use]
pub fn step(before: &Field, outcome: &Outcome) -> String {
    // The model leaves the source between the two steps implicit.
    let rewrite = outcome.rewrite.as_deref().map(|rewrite| {
        let middle = applied(before.source(), &outcome.changes);
        changes(&middle, rewrite)
    });
    let json = Json {
        changes: changes(before.source(), &outcome.changes),
        isolate: outcome.isolate,
        rewrite,
        effect: outcome.effect.map(effect),
    };
    serde_json::to_string(&json).unwrap_or_default()
}
