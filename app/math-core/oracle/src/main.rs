//! The Rust half of the display oracle: reads one JSON request per stdin
//! line, renders it with the katex fork, and prints one JSON answer per line.
//!
//! In:  `{"id":…, "tex":"…", "display":bool, "options":{…}}`
//! Out: `{"id":…, "html":"…"}`, `{"id":…, "error":"…"}`, or
//!      `{"id":…, "panic":"…"}` when the fork panics.
//!
//! Options are KaTeX JS's names and defaults (`strict` defaults to "warn",
//! as in KaTeX JS; the fork's own default is "ignore"), plus the binding's
//! `sourceMap`. `options.displayMode` wins over the top-level `display`.
//! `oracle/render.ts` drives this.
//!
//! `oracle --prefixes` is the typing probe: it renders every char-boundary
//! prefix of each request's `tex` with `throwOnError` forced on and answers
//! `{"id":…, "prefixes":n, "panics":[{"len":bytes, "panic":"…"}]}`.

use std::{
    any::Any,
    collections::BTreeMap,
    io::{self, BufRead as _, Write as _},
    panic::{self, AssertUnwindSafe},
};

use katex::{
    KatexContext,
    macro_expander::MacroMap,
    macros::MacroDefinition,
    render_to_string,
    types::{OutputFormat, Settings, StrictMode, StrictSetting},
};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
struct Request {
    id: Value,
    tex: String,
    #[serde(default)]
    display: bool,
    #[serde(default)]
    options: Options,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Options {
    display_mode: Option<bool>,
    throw_on_error: Option<bool>,
    strict: Option<Value>,
    macros: Option<BTreeMap<String, String>>,
    output: Option<String>,
    source_map: Option<bool>,
}

fn strict(value: Option<&Value>) -> Result<StrictSetting, String> {
    match value {
        None => Ok(StrictSetting::Mode(StrictMode::Warn)),
        Some(Value::Bool(b)) => Ok(StrictSetting::Bool(*b)),
        Some(Value::String(s)) => match s.as_str() {
            "ignore" => Ok(StrictSetting::Mode(StrictMode::Ignore)),
            "warn" => Ok(StrictSetting::Mode(StrictMode::Warn)),
            "error" => Ok(StrictSetting::Mode(StrictMode::Error)),
            other => Err(format!("unknown strict mode {other:?}")),
        },
        Some(other) => Err(format!("unsupported strict value {other}")),
    }
}

fn output(value: Option<&str>) -> Result<OutputFormat, String> {
    match value {
        None | Some("htmlAndMathml") => Ok(OutputFormat::HtmlAndMathml),
        Some("html") => Ok(OutputFormat::Html),
        Some("mathml") => Ok(OutputFormat::Mathml),
        Some(other) => Err(format!("unknown output {other:?}")),
    }
}

fn settings(request: &Request) -> Result<Settings, String> {
    let options = &request.options;
    let mut macros = MacroMap::default();
    for (name, body) in options.macros.iter().flatten() {
        macros.insert(name.clone(), MacroDefinition::String(body.clone()));
    }
    Ok(Settings::builder()
        .display_mode(options.display_mode.unwrap_or(request.display))
        .throw_on_error(options.throw_on_error.unwrap_or(true))
        .strict(strict(options.strict.as_ref())?)
        .output(output(options.output.as_deref())?)
        .macros(macros)
        .source_map(options.source_map.unwrap_or(false))
        .build())
}

fn panic_message(p: &(dyn Any + Send)) -> String {
    p.downcast_ref::<String>()
        .cloned()
        .or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .unwrap_or_else(|| "panic".to_owned())
}

fn answer(ctx: &KatexContext, line: &str) -> Value {
    let request: Request = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(e) => return json!({ "id": null, "panic": format!("bad request: {e}") }),
    };
    let settings = match settings(&request) {
        Ok(s) => s,
        Err(e) => return json!({ "id": request.id, "panic": e }),
    };
    let rendered = panic::catch_unwind(AssertUnwindSafe(|| {
        render_to_string(ctx, &request.tex, &settings)
    }));
    match rendered {
        Ok(Ok(html)) => json!({ "id": request.id, "html": html }),
        Ok(Err(e)) => json!({ "id": request.id, "error": e.to_string() }),
        Err(p) => json!({ "id": request.id, "panic": panic_message(p.as_ref()) }),
    }
}

/// Every prefix the user passes through while typing `tex`, rendered as the
/// editor's parse gate would; only the panics are kept.
fn prefixes(ctx: &KatexContext, line: &str) -> Value {
    let mut request: Request = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(e) => return json!({ "id": null, "panic": format!("bad request: {e}") }),
    };
    request.options.throw_on_error = Some(true);
    let tex = std::mem::take(&mut request.tex);
    let ends = tex
        .char_indices()
        .map(|(i, _)| i)
        .skip(1)
        .chain(std::iter::once(tex.len()));
    let mut count = 0;
    let mut panics = Vec::new();
    for end in ends {
        count += 1;
        // Fresh settings per render: the expander writes `\gdef`s into them.
        let settings = match settings(&request) {
            Ok(s) => s,
            Err(e) => return json!({ "id": request.id, "panic": e }),
        };
        let prefix = &tex[..end];
        if let Err(p) = panic::catch_unwind(AssertUnwindSafe(|| {
            render_to_string(ctx, prefix, &settings)
        })) {
            panics.push(json!({ "len": end, "panic": panic_message(p.as_ref()) }));
        }
    }
    json!({ "id": request.id, "prefixes": count, "panics": panics })
}

fn main() -> io::Result<()> {
    // A panic is reported as an answer; keep the default hook off stderr.
    panic::set_hook(Box::new(|_| {}));
    let probe = std::env::args().any(|a| a == "--prefixes");
    let ctx = KatexContext::default();
    let stdin = io::stdin();
    let mut out = io::BufWriter::new(io::stdout().lock());
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = if probe {
            prefixes(&ctx, &line)
        } else {
            answer(&ctx, &line)
        };
        writeln!(out, "{reply}")?;
        out.flush()?;
    }
    Ok(())
}
