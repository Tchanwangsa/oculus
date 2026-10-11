//! The `transcribe` settings row and the engines it configures.

use std::path::{Path, PathBuf};

use super::engine::{Engine, ENGINES, NO_ENGINE};
use super::engines::{apple, groq, whisper};

/// The `transcribe` settings row, which Settings → Transcription writes
/// (`parseTranscribeSettings` in `app/src/lib/lectures/transcribe/settings.ts` mirrors it):
///
/// `{"order": [...], "language": string, "groq": {"enabled"},
///   "whisper": {"enabled", "model"}, "apple": {"enabled"}}`
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Settings {
    /// Each of [`ENGINES`] once, in the order a run tries them.
    pub order: Vec<&'static str>,
    pub language: Language,
    pub groq: bool,
    pub whisper: bool,
    /// A catalogue id; `None` lets `whisper_models::pick` choose.
    pub whisper_model: Option<String>,
    pub apple: bool,
}

/// The one language every engine transcribes in: `auto`, a locale such as
/// `en_AU`, or a bare language code; `None` is the default, English in the
/// Mac's region.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Language(Option<String>);

impl Language {
    /// Whisper's `-l`: the locale's language part, or `auto`. The default is
    /// English because auto-detection judges from the first 30 s heard, which
    /// is easily noise or someone else's chatter.
    pub fn whisper(&self) -> String {
        match &self.0 {
            None => "en".into(),
            Some(l) => l.split(['_', '-']).next().unwrap_or(l).to_lowercase(),
        }
    }

    /// Groq's `language` field, an ISO-639-1 code; `None` lets it detect.
    pub fn groq(&self) -> Option<String> {
        Some(self.whisper()).filter(|code| code != "auto")
    }

    /// The helper's `--locale`. `None` is its default — English in the Mac's
    /// region — which also stands in for auto-detect, since Apple has none.
    pub fn apple(&self) -> Option<String> {
        self.0
            .clone()
            .filter(|l| l != "auto" && !l.eq_ignore_ascii_case("en"))
    }
}

/// A missing row or key, or a value of the wrong type, reads as its default:
/// the default order, every engine on, the default language, the model
/// `pick` chooses. Without a top-level `language`, the older per-engine
/// `apple.locale`, then `whisper.language`, stands in for it.
pub(crate) fn settings_from(row: Option<&str>) -> Settings {
    let row = row.and_then(|row| serde_json::from_str::<serde_json::Value>(row).ok());
    let at = |path: &[&str]| {
        path.iter()
            .try_fold(row.as_ref()?, |value, key| value.get(key))
    };
    let text = |path: &[&str]| {
        at(path)
            .and_then(|v| v.as_str())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    let enabled = |engine: &str| {
        at(&[engine, "enabled"])
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
    };
    let language = text(&["language"])
        .or_else(|| text(&["apple", "locale"]))
        .or_else(|| text(&["whisper", "language"]))
        .map(|l| {
            if l.eq_ignore_ascii_case("auto") {
                "auto".into()
            } else {
                l
            }
        });
    Settings {
        order: order_from(at(&["order"])),
        language: Language(language),
        groq: enabled("groq"),
        whisper: enabled("whisper"),
        whisper_model: text(&["whisper", "model"]),
        apple: enabled("apple"),
    }
}

/// Known names in the stored order, each once, then any left out in the
/// default order.
fn order_from(stored: Option<&serde_json::Value>) -> Vec<&'static str> {
    let mut order: Vec<&'static str> = Vec::new();
    let named = stored
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str());
    for name in named.chain(ENGINES) {
        if let Some(&engine) = ENGINES.iter().find(|&&e| e == name) {
            if !order.contains(&engine) {
                order.push(engine);
            }
        }
    }
    order
}

pub(in crate::transcribe) const NO_GROQ_KEY: &str =
    "No Groq API key — add one in Settings → Transcription";

/// Groq as Settings has it: through keyd when it is installed (in
/// `data_dir`), else with the keychain's key from `key`. Nothing is asked
/// while Groq is off, so a switched-off engine never touches keyd or the
/// keychain.
fn groq_engine(
    settings: &Settings,
    data_dir: &Path,
    key: impl FnOnce() -> Result<Option<String>, String>,
) -> Result<groq::Groq, String> {
    if !settings.groq {
        return Err("Groq is turned off in Settings → Transcription".into());
    }
    let broker = crate::providers::credentials::Credentialed::at(data_dir);
    let auth = match broker.has(crate::providers::groq::SECRET) {
        Ok(true) => groq::Auth::Keyd(broker),
        Ok(false) => return Err(NO_GROQ_KEY.into()),
        Err(crate::providers::credentials::KeydError::Absent) => {
            let key = key()
                .map_err(|e| format!("The keychain refused to give out the Groq API key ({e})"))?
                .ok_or(NO_GROQ_KEY)?;
            groq::Auth::Direct(key)
        }
        Err(crate::providers::credentials::KeydError::Keychain(e)) => {
            return Err(format!(
                "The keychain refused to give out the Groq API key ({e})"
            ))
        }
        Err(e) => {
            return Err(format!(
                "oculus-keyd, which holds the Groq API key, could not check it: {e}"
            ))
        }
    };
    Ok(groq::Groq::new(auth, settings.language.groq()))
}

/// The configured engines in the order set in Settings, or `only` that one.
/// With none, the error says what to set up.
pub(super) fn engines(
    resource_dir: Option<PathBuf>,
    ffmpeg: &Path,
    only: Option<&str>,
) -> Result<Vec<Box<dyn Engine>>, String> {
    if let Some(name) = only.filter(|name| !ENGINES.contains(name)) {
        return Err(format!(
            "no engine called {name} — it is {}",
            ENGINES.join(", ")
        ));
    }
    let settings = settings_from(crate::db::store::setting_blocking("transcribe").as_deref());
    let mut found: Vec<Box<dyn Engine>> = Vec::new();
    let mut missing = Vec::new();
    for &name in settings
        .order
        .iter()
        .filter(|&&name| only.is_none_or(|only| only == name))
    {
        let engine: Result<Box<dyn Engine>, String> = match name {
            "groq" => groq_engine(
                &settings,
                &crate::library::paths::data_dir(),
                crate::providers::groq::fetch_api_key,
            )
            .map(|e| Box::new(e) as Box<dyn Engine>),
            "whisper" => whisper::Whisper::configured(&settings, resource_dir.clone(), ffmpeg)
                .map(|e| Box::new(e) as Box<dyn Engine>),
            _ => apple::Apple::configured(&settings, resource_dir.clone())
                .map(|e| Box::new(e) as Box<dyn Engine>),
        };
        match engine {
            Ok(engine) => found.push(engine),
            Err(reason) => missing.push(reason),
        }
    }
    match (found.is_empty(), only) {
        (false, _) => Ok(found),
        (true, Some(_)) => Err(missing.join("; ")),
        (true, None) => Err(NO_ENGINE.into()),
    }
}

#[cfg(test)]
mod tests;
