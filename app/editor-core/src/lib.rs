//! The editor's document model and (later) its markdown parser, ported from
//! CodeMirror 6 and Lezer and checked against them by the oracle in `oracle/`.
//! See `data/plans/rust-editor.md`. Each area is a top-level module.

#![forbid(unsafe_code)]

pub mod markdown;
pub mod text;
