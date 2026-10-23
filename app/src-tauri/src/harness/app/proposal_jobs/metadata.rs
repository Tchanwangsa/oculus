use super::{text, validate};
use serde_json::{json, Value};
#[tauri::command]
pub async fn proposal_job_info(kind: String, state: Value) -> Result<Value, String> {
    validate(&kind, &state)?;
    let pool = crate::db::store::open_pool().await?;
    let description = match kind.as_str() {
        "Sync" => "Sync the selected subjects; this can take several minutes.".into(),
        "Reparse" => "Parse and embed the selected files using Settings → Parsing and Embeddings. This can take several minutes and use credits.".into(),
        "Transcribe" => {
            let settings = crate::db::store::setting(&pool,"transcribe").await?;
            let settings = crate::transcribe::settings_from(settings.as_deref());
            let engines: Vec<&str> = settings.order.iter().copied().filter(|engine| match *engine { "groq" => settings.groq, "whisper" => settings.whisper, "apple" => settings.apple, _ => false }).collect();
            format!("Transcription engine order: {}. {}", engines.iter().map(|engine|crate::transcribe::engine_label(engine)).collect::<Vec<_>>().join(" → "), if engines.contains(&"groq") { "Groq uploads the audio and uses credits; local engines run on this device." } else { "Audio is processed on this device." })
        },
        "LectureChapters" | "LectureEnd" => {
            let job = if kind == "LectureChapters" { crate::harness::jobs::Job::LectureChapters } else { crate::harness::jobs::Job::LectureEnd };
            let chain = crate::harness::jobs::chain(&pool,job).await;
            format!("Uses {}. This job can take several minutes and use credits.",chain.iter().map(|s| format!("{} / {}",s.provider.label(),s.model)).collect::<Vec<_>>().join(" → "))
        },
        "Download" => {
            let item = text(&state,"item")?;
            let bytes = if item == "aligner" { crate::studio::timing::PACK.bytes() } else if item == "voices" { crate::studio::tts::kokoro::packs().iter().map(|p| p.bytes()).sum::<u64>() + ["af_heart","am_michael"].iter().filter_map(|id| crate::studio::tts::kokoro::voices::find(id)).map(|v| v.bytes()).sum::<u64>() } else { crate::studio::tts::kokoro::voices::find(item).unwrap().bytes() };
            format!("Download {item} ({:.1} MB).",bytes as f64 / 1_000_000.0)
        },
        _ => unreachable!(),
    };
    Ok(json!({"description":description}))
}
