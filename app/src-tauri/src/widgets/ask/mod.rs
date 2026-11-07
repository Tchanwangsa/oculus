//! A widget's `ask` from `oculus/ask` (docs/widgets.md): one
//! tool-less turn on the "Widget questions" job's models, answering a JSON
//! value that matches the widget's schema. The schema is a subset of JSON
//! Schema ([`schema`]); a reply that doesn't match is asked again once. The
//! student's Allow, the cap and the one-at-a-time rule are the host's, in
//! the webview, before this is called.

pub mod command;
mod prompt;
mod reply;
pub mod schema;

pub use prompt::INSTRUCTIONS;
