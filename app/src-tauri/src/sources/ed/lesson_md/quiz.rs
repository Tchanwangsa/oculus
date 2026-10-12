//! A quiz slide's questions.

use super::super::document::document_md;
use super::body::ed_md;

/// A question, its answers as unticked boxes in Ed's order, and — only when
/// Ed sent a `solution`, which it does once solutions are released — the
/// answer and explanation, collapsed.
pub(super) fn question_md(n: usize, q: &serde_json::Value) -> String {
    let data = &q["data"];
    let mut out = format!("### Question {n}\n\n");
    let content = ed_md(data["content"].as_str().unwrap_or(""));
    if !content.is_empty() {
        out.push_str(&content);
        out.push_str("\n\n");
    }
    let answers: Vec<String> = data["answers"]
        .as_array()
        .into_iter()
        .flatten()
        .map(answer_line)
        .collect();
    for a in &answers {
        out.push_str(&format!("- [ ] {a}\n"));
    }

    let Some(solution) = data.get("solution") else {
        return out;
    };
    let chosen: Vec<String> = match solution {
        serde_json::Value::Array(items) => {
            items.iter().filter_map(|s| named(s, &answers)).collect()
        }
        single => named(single, &answers).into_iter().collect(),
    };
    let explanation = ed_md(data["explanation"].as_str().unwrap_or(""));
    if chosen.is_empty() && explanation.is_empty() {
        return out;
    }
    let mut body = String::new();
    match chosen.as_slice() {
        [] => {}
        [one] => body.push_str(&format!("**Answer:** {one}\n\n")),
        many => {
            body.push_str("**Answers:**\n\n");
            for a in many {
                body.push_str(&format!("- {a}\n"));
            }
            body.push('\n');
        }
    }
    body.push_str(&explanation);
    out.push_str(&format!(
        "\n<details>\n<summary>Solution</summary>\n\n{}\n\n</details>\n",
        body.trim()
    ));
    out
}

/// A solution entry: an index into the answers, or the answer itself (a
/// short-answer question's accepted text).
fn named(solution: &serde_json::Value, answers: &[String]) -> Option<String> {
    match solution {
        serde_json::Value::Number(n) => n.as_u64().and_then(|i| answers.get(i as usize)).cloned(),
        serde_json::Value::String(s) if !s.trim().is_empty() => Some(answer_line(solution)),
        _ => None,
    }
}

/// An answer on one line: Ed sends each as `<document>` XML.
fn answer_line(answer: &serde_json::Value) -> String {
    let md = document_md(answer.as_str().unwrap_or(""));
    md.split_whitespace().collect::<Vec<_>>().join(" ")
}
