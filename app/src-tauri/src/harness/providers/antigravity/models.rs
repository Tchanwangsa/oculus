//! The model catalogue off `agy models`, and the slug a chosen model and
//! reasoning level fold back into.

use std::process::{Command, Stdio};

/// One model `agy models` printed; the same shape as Codex's `ModelInfo`.
#[derive(serde::Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    pub reasoning_efforts: Vec<String>,
    pub default_reasoning_effort: Option<String>,
}

/// What this account can use, off `agy models` — a listing, nothing billed.
/// No `--json`, so lines are parsed (see [`parse_models`]).
pub fn list_models(
    bin: &std::path::Path,
    env: &[(String, String)],
) -> Result<Vec<ModelInfo>, String> {
    let out = run_models(bin, env)?;
    if !out.success {
        let said = [out.stderr.trim(), out.stdout.trim()]
            .into_iter()
            .find(|s| !s.is_empty())
            .map(String::from);
        return Err(said.unwrap_or_else(|| "`agy models` failed and said nothing".into()));
    }
    Ok(parse_models(&out.stdout))
}

/// What one `agy models` run said.
pub struct ModelsRun {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Enough that a wedged `agy` cannot hold a model picker open.
const MODELS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// `agy models`, killed past [`MODELS_TIMEOUT`].
pub fn run_models(bin: &std::path::Path, env: &[(String, String)]) -> Result<ModelsRun, String> {
    use std::io::Read;
    let mut child = Command::new(bin)
        .arg("models")
        .env_clear()
        .envs(env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run {}: {e}", bin.display()))?;
    // One thread per pipe, or a child filling the unread one blocks.
    fn drain(r: Option<impl Read + Send + 'static>) -> std::thread::JoinHandle<String> {
        std::thread::spawn(move || {
            let mut s = String::new();
            if let Some(mut r) = r {
                let _ = r.read_to_string(&mut s);
            }
            s
        })
    }
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let deadline = std::time::Instant::now() + MODELS_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "`agy models` did not answer within {}s",
                    MODELS_TIMEOUT.as_secs()
                ));
            }
            Err(e) => {
                let _ = child.kill();
                return Err(format!("`agy models`: {e}"));
            }
        }
    };
    Ok(ModelsRun {
        success: status.success(),
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
    })
}

/// Models out of `agy models`' output: the first slug-shaped word per line.
/// The level is baked into the slug (`gemini-3.8-flash-high`), so slugs
/// sharing a base fold into one model whose `reasoning_efforts` are the
/// suffixes; [`model_slug`] rebuilds the slug at spawn.
pub(super) fn parse_models(stdout: &str) -> Vec<ModelInfo> {
    let mut seen = std::collections::HashSet::new();
    let mut models: Vec<ModelInfo> = Vec::new();
    for line in stdout.lines() {
        let Some(word) = line.split_whitespace().next() else {
            continue;
        };
        let word = word.trim_matches(|c: char| !c.is_alphanumeric());
        if !is_slug(word) || !seen.insert(word.to_string()) {
            continue;
        }
        let (base, level) = split_level(word);
        match models.iter_mut().find(|m| m.id == base) {
            Some(m) => {
                if let Some(l) = level {
                    m.reasoning_efforts.push(l.to_string());
                }
            }
            None => models.push(ModelInfo {
                id: base.to_string(),
                display_name: listed_name(line, level).unwrap_or_else(|| display_name(base)),
                reasoning_efforts: level.map(|l| vec![l.to_string()]).unwrap_or_default(),
                default_reasoning_effort: None,
            }),
        }
    }
    // Medium where offered, else the listing's first.
    for m in &mut models {
        m.default_reasoning_effort = m
            .reasoning_efforts
            .iter()
            .find(|l| *l == "medium")
            .or(m.reasoning_efforts.first())
            .cloned();
    }
    models
}

/// The name after the tab in a `<slug>\t<Display Name>` row (agy 1.2.9),
/// minus a folded level's ` (High)`. `None` without a tab.
fn listed_name(line: &str, level: Option<&str>) -> Option<String> {
    let name = line.split_once('\t')?.1.trim();
    let name = match level {
        Some(l) => match name.rsplit_once(" (") {
            Some((head, tail)) if tail.trim_end_matches(')').eq_ignore_ascii_case(l) => head.trim(),
            _ => name,
        },
        None => name,
    };
    (!name.is_empty()).then(|| name.to_string())
}

/// The level suffixes a slug can end in; the names `validate_effort` accepts.
const LEVELS: [&str; 6] = ["minimal", "low", "medium", "high", "xhigh", "max"];

/// `gemini-3.8-flash-high` → (`gemini-3.8-flash`, `high`). A slug that is
/// nothing but a level, or has no level suffix, keeps its whole self.
pub(super) fn split_level(slug: &str) -> (&str, Option<&str>) {
    match slug.rsplit_once('-') {
        Some((base, l)) if !base.is_empty() && LEVELS.contains(&l) => (base, Some(l)),
        _ => (slug, None),
    }
}

/// The inverse of `split_level`. A slug that already ends in a level is
/// passed as it is rather than doubled.
pub(super) fn model_slug(model: &str, effort: Option<&str>) -> String {
    match effort {
        Some(e) if split_level(model).1.is_none() => format!("{model}-{e}"),
        _ => model.to_string(),
    }
}

/// A slug as a person reads it: `claude-opus-4-6-thinking` → "Claude Opus 4.6
/// Thinking", `gpt-oss-120b` → "GPT-OSS 120B".
pub(super) fn display_name(slug: &str) -> String {
    let mut words: Vec<String> = Vec::new();
    let mut prev_number = false;
    for part in slug.split('-').filter(|p| !p.is_empty()) {
        let number = part.chars().all(|c| c.is_ascii_digit());
        if number && prev_number {
            if let Some(last) = words.last_mut() {
                last.push('.');
                last.push_str(part);
            }
            continue;
        }
        prev_number = number;
        let word = match part {
            "gpt" | "oss" => part.to_uppercase(),
            p if p.starts_with(|c: char| c.is_ascii_digit()) => p.to_uppercase(),
            p => {
                let mut c = p.chars();
                c.next()
                    .map(|f| f.to_uppercase().chain(c).collect())
                    .unwrap_or_default()
            }
        };
        match words.last_mut() {
            Some(last) if last == "GPT" => {
                last.push('-');
                last.push_str(&word);
            }
            _ => words.push(word),
        }
    }
    words.join(" ")
}

/// Slug-shaped, strictly: a false positive is an unselectable picker row.
pub(super) fn is_slug(w: &str) -> bool {
    w.len() >= 3
        && w.contains('-')
        && w.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.')
        && w.chars().any(|c| c.is_ascii_alphanumeric())
}
