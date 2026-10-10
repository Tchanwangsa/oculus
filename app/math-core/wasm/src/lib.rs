// The fork's `bon` macros use syn 3 and wasm-bindgen's syn 2: build-time only.
#![allow(clippy::multiple_crate_versions)]
//! The app's maths engine in WebAssembly: the katex fork behind KaTeX JS's
//! `renderToString` and `parseError` (`render`), and `MathField`, the edit
//! model (`oculus-math-edit`) of the visual maths field (`field`).
//!
//! The JS surface counts UTF-16 code units, the edit model bytes;
//! `boundary` converts between them and carries commands, steps and slots
//! as JSON. It is plain Rust, so native `cargo test` covers it; the
//! wasm32-only modules only move values across.

pub mod boundary;
#[cfg(target_arch = "wasm32")]
mod field;
#[cfg(target_arch = "wasm32")]
mod render;
