//! How a skill the student picked in the composer is named to Claude Code.

use std::path::Path;

/// Claude Code's line for a picked skill, put before the prompt. The fallback
/// sentence until a recorded turn shows a native form this bridge acts on;
/// that form replaces this body (docs/harness.md, "`/` finds commands and
/// skills").
pub fn invoke_skill(name: &str, path: &Path) -> String {
    crate::harness::app::skills::fallback_sentence(name, path)
}
