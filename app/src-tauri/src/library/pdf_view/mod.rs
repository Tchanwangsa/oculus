//! The PDF viewer's backend: open a library PDF, render a page to exact-size
//! RGBA, and read a page's text lines and links, all with the pure-Rust
//! renderer hayro, for the viewer in `docs/viewers.md`.
//!
//! Opened documents sit in a small LRU keyed by path and modification stamp.
//! Renders and text extraction run on blocking threads, at most half the cores
//! at once, so a fast scroll queues rather than oversubscribes. Every hayro
//! call runs under `catch_unwind`: a malformed PDF fails its own request with
//! "render-failed" and nothing else.
//!
//! Text layout follows PdfCraft's reading-order extraction
//! (github.com/storytold/pdfcraft, MIT OR Apache-2.0); the adapted part carries
//! its licence notice below.

pub(crate) mod commands;
mod documents;
mod layout;
mod links;
mod render;
#[cfg(test)]
mod tests;
mod text;
mod wire;
