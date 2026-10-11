//! The renderer: the katex fork behind KaTeX JS's `renderToString`, plus
//! `parseError`, the editor's parse gate.
//!
//! Options are read by hand, with KaTeX JS's names and defaults (`strict`
//! is "warn", not the fork's "ignore"), and only those the app passes, plus
//! our `sourceMap` for the edit field.
//! Unknown keys are ignored; a known key of the wrong type throws a
//! `TypeError`. `settings` mirrors the display oracle's, so its parity holds.
//! No DOM `render`: the app only sets `innerHTML`, and web-sys stays out.
//!
//! Strict-"warn" messages go to `eprintln!`, a no-op on wasm32, so KaTeX JS's
//! console warning is dropped. A panic, or nesting a few hundred levels deep
//! (1 MiB stack), traps and leaves the instance unusable.

use std::sync::OnceLock;

use js_sys::{Array, Object, Reflect, TypeError};
use katex::{
    KatexContext, ParseError,
    macro_expander::MacroMap,
    macros::MacroDefinition,
    render_to_string,
    types::{OutputFormat, Settings, StrictMode, StrictSetting},
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const OPTIONS: &str = r#"
/** The KaTeX options the app uses, with KaTeX JS's defaults. */
export interface MathOptions {
  /** Default false. */
  displayMode?: boolean;
  /** Default true: a parse error throws a `ParseError`. False draws KaTeX's red `katex-error` span. */
  throwOnError?: boolean;
  /** Default "warn" (accept, as KaTeX JS; the console warning is dropped). */
  strict?: boolean | "ignore" | "warn" | "error";
  /** Macro name (with its backslash) to its expansion. */
  macros?: Record<string, string>;
  /** Default "htmlAndMathml". */
  output?: "htmlAndMathml" | "html" | "mathml";
  /** Not KaTeX's. Default false. True maps the HTML to the source for the
   *  edit field: each node's element gets `data-s`/`data-e`, its range as
   *  UTF-16 offsets into `tex`; glyphs of different nodes are not merged;
   *  an empty group draws an `oc-placeholder` glyph with a zero-width
   *  range. Display output must leave it off. */
  sourceMap?: boolean;
}
"#;

/// The function and symbol tables, built on first use and shared by every
/// call; per-call state lives in `Settings`. The first call also installs the
/// panic hook (about 2 KB), which a `#[wasm_bindgen(start)]` would export.
fn context() -> &'static KatexContext {
    static CONTEXT: OnceLock<KatexContext> = OnceLock::new();
    CONTEXT.get_or_init(|| {
        console_error_panic_hook::set_once();
        KatexContext::default()
    })
}

fn type_error(message: &str) -> JsValue {
    TypeError::new(message).into()
}

/// KaTeX JS's `ParseError`: `message` is `KaTeX parse error: …` and
/// `String(e)` is `ParseError: KaTeX parse error: …`.
pub fn parse_error(error: &ParseError) -> JsValue {
    let e = js_sys::Error::new(&error.to_string());
    e.set_name("ParseError");
    e.into()
}

/// An own property of `options`, or `None` when it is absent, `undefined` or
/// `null` (KaTeX JS ignores inherited keys too).
fn own(options: &Object, key: &str) -> Option<JsValue> {
    let key = JsValue::from_str(key);
    if !Object::has_own(options, &key) {
        return None;
    }
    Reflect::get(options, &key)
        .ok()
        .filter(|v| !v.is_undefined() && !v.is_null())
}

fn bool_option(options: &Object, key: &str) -> Result<Option<bool>, JsValue> {
    own(options, key)
        .map(|v| {
            v.as_bool()
                .ok_or_else(|| type_error(&format!("option '{key}' must be a boolean")))
        })
        .transpose()
}

fn strict(options: &Object) -> Result<StrictSetting, JsValue> {
    let Some(value) = own(options, "strict") else {
        return Ok(StrictSetting::Mode(StrictMode::Warn));
    };
    if let Some(b) = value.as_bool() {
        return Ok(StrictSetting::Bool(b));
    }
    match value.as_string().as_deref() {
        Some("ignore") => Ok(StrictSetting::Mode(StrictMode::Ignore)),
        Some("warn") => Ok(StrictSetting::Mode(StrictMode::Warn)),
        Some("error") => Ok(StrictSetting::Mode(StrictMode::Error)),
        _ => Err(type_error(
            "option 'strict' must be a boolean, \"ignore\", \"warn\" or \"error\"",
        )),
    }
}

fn output(options: &Object) -> Result<OutputFormat, JsValue> {
    let Some(value) = own(options, "output") else {
        return Ok(OutputFormat::HtmlAndMathml);
    };
    match value.as_string().as_deref() {
        Some("htmlAndMathml") => Ok(OutputFormat::HtmlAndMathml),
        Some("html") => Ok(OutputFormat::Html),
        Some("mathml") => Ok(OutputFormat::Mathml),
        _ => Err(type_error(
            "option 'output' must be \"htmlAndMathml\", \"html\" or \"mathml\"",
        )),
    }
}

fn macros(options: &Object) -> Result<MacroMap, JsValue> {
    let mut map = MacroMap::default();
    let Some(value) = own(options, "macros") else {
        return Ok(map);
    };
    if !value.is_object() || Array::is_array(&value) {
        return Err(type_error("option 'macros' must be a plain object"));
    }
    let object = Object::from(value);
    for key in Object::keys(&object).iter() {
        let body = Reflect::get(&object, &key).ok().and_then(|v| v.as_string());
        let name = key.as_string().unwrap_or_default();
        let body = body
            .ok_or_else(|| type_error(&format!("option 'macros': {name} must map to a string")))?;
        map.insert(name, MacroDefinition::String(body));
    }
    Ok(map)
}

/// Fresh per call: the expander writes `\gdef`s into `Settings::macros`.
fn settings(options: Option<Object>) -> Result<Settings, JsValue> {
    let options = &options.unwrap_or_else(Object::new);
    if !options.is_object() {
        return Err(type_error("options must be an object"));
    }
    Ok(Settings::builder()
        .display_mode(bool_option(options, "displayMode")?.unwrap_or(false))
        .throw_on_error(bool_option(options, "throwOnError")?.unwrap_or(true))
        .strict(strict(options)?)
        .output(output(options)?)
        .macros(macros(options)?)
        .source_map(bool_option(options, "sourceMap")?.unwrap_or(false))
        .build())
}

/// KaTeX JS's `katex.renderToString`. Throws a `ParseError` when the formula
/// does not render and `throwOnError` is not false, and a `TypeError` for a
/// malformed option.
#[wasm_bindgen(js_name = renderToString)]
pub fn render(
    tex: &str,
    #[wasm_bindgen(unchecked_optional_param_type = "MathOptions")] options: Option<Object>,
) -> Result<String, JsValue> {
    let ctx = context();
    let settings = settings(options)?;
    render_to_string(ctx, tex, &settings).map_err(|e| parse_error(&e))
}

/// Why `tex` does not render: the message `renderToString` would throw.
///
/// `throwOnError` is forced on; `undefined` when the formula renders. It runs
/// the whole render, since some errors only surface while building. Throws a
/// `TypeError` for a malformed option.
#[wasm_bindgen(js_name = parseError)]
pub fn check(
    tex: &str,
    #[wasm_bindgen(unchecked_optional_param_type = "MathOptions")] options: Option<Object>,
) -> Result<Option<String>, JsValue> {
    let ctx = context();
    let mut settings = settings(options)?;
    settings.throw_on_error = true;
    Ok(render_to_string(ctx, tex, &settings)
        .err()
        .map(|e| e.to_string()))
}
