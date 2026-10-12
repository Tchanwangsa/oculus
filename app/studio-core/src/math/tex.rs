//! LaTeX -> KaTeX's HTML tree, through the maths engine's KaTeX port
//! (`app/math-core/katex`, `render_to_html_tree`). One `KatexContext` (the
//! function, symbol and environment tables) serves every call; settings are
//! per call, since the macro expander writes into them.

use katex::dom_tree::DomSpan;
use katex::macro_expander::MacroMap;
use katex::macros::MacroDefinition;
use katex::types::{OutputFormat, Settings, StrictMode, StrictSetting, TrustSetting};
use katex::{KatexContext, ParseError};
use std::sync::LazyLock;

static CONTEXT: LazyLock<KatexContext> = LazyLock::new(KatexContext::default);

/// `\k{name}{…}` names a group of terms for magic move (see `keys`); it is
/// `\htmlData`, which writes `data-k="name"` on the group's span. Trust is
/// on so `\htmlData` runs; nothing here reaches a browser.
const KEY_MACRO: (&str, &str) = ("\\k", "\\htmlData{k=#1}{#2}");

fn settings(display: bool) -> Settings {
    let mut macros = MacroMap::default();
    macros.insert(
        KEY_MACRO.0.to_owned(),
        MacroDefinition::String(KEY_MACRO.1.to_owned()),
    );
    Settings::builder()
        .display_mode(display)
        .output(OutputFormat::Html)
        .throw_on_error(true)
        .strict(StrictSetting::Mode(StrictMode::Ignore))
        .trust(TrustSetting::Bool(true))
        .macros(macros)
        .build()
}

/// KaTeX's HTML tree for `tex`, or its parse error as KaTeX words it,
/// with the position (`… at position 9: \frac{a}{̲`).
pub fn html_tree(tex: &str, display: bool) -> Result<DomSpan, String> {
    katex::render_to_html_tree(&CONTEXT, tex, &settings(display)).map_err(|e| message(&e))
}

fn message(e: &ParseError) -> String {
    let s = e.to_string();
    s.strip_prefix("KaTeX parse error: ")
        .map_or_else(|| s.clone(), str::to_owned)
}
