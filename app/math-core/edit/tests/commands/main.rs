//! Behaviour tests for the editing commands, one table per documented
//! key. "doc N" is a line of docs/editor-maths.md.
#![allow(
    clippy::non_ascii_literal,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests: Thai in the tables, and a failing case panics"
)]
#![allow(
    clippy::literal_string_with_formatting_args,
    reason = "LaTeX's braces, not format arguments"
)]

mod deleting;
mod harness;
mod matrices;
mod modes;
mod moving;
mod rows;
mod shortcuts;
mod typing;
