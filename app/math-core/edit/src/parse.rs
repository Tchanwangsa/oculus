//! Parsing a formula for editing.

use std::sync::OnceLock;

use katex::{
    KatexContext, Settings, parse as katex_parse,
    parser::parse_node::AnyParseNode,
    render_to_string,
    types::{ParseError, StrictMode, StrictSetting},
};

fn context() -> &'static KatexContext {
    static CONTEXT: OnceLock<KatexContext> = OnceLock::new();
    CONTEXT.get_or_init(KatexContext::default)
}

/// The formula's parse tree with source mapping on.
///
/// Every node's location is a byte range of `source`. Errors throw rather
/// than render (an unknown command is an error, not red text), strict
/// checks are off, and the settings are fresh per call: `\gdef` writes
/// into them.
pub fn parse(source: &str, display: bool) -> Result<Vec<AnyParseNode>, ParseError> {
    katex_parse(context(), source, &settings(display))
}

/// Whether the formula renders with [`parse`]'s settings: some source
/// parses and only fails to build (`\over^]\over`).
pub fn renders(source: &str, display: bool) -> Result<(), ParseError> {
    render_to_string(context(), source, &settings(display)).map(drop)
}

fn settings(display: bool) -> Settings {
    Settings::builder()
        .display_mode(display)
        .throw_on_error(true)
        .strict(StrictSetting::Mode(StrictMode::Ignore))
        .source_map(true)
        .build()
}
