//! TSX -> JS with oxc: TypeScript stripped, JSX to classic factory calls,
//! and `__source` on every element so tree errors point at TSX lines.
//! Keeps a source map for errors thrown at run time.
//!
//! The factory is `__studio_h`, not `h`: a component's own local named `h`
//! (a height, say) would otherwise shadow it and break every element in
//! that scope.

use super::sourcemap::LineMap;
use crate::error::{StudioError, TsxLoc};
use oxc::allocator::Allocator;
use oxc::ast::ast::Statement;
use oxc::codegen::{Codegen, CodegenOptions};
use oxc::parser::Parser;
use oxc::semantic::SemanticBuilder;
use oxc::span::{GetSpan, SourceType, Span};
use oxc::transformer::{JsxRuntime, TransformOptions, Transformer};
use std::path::{Path, PathBuf};

pub struct Transpiled {
    pub js: String,
    pub map: LineMap,
}

pub fn transpile(component: &str, tsx: &str) -> Result<Transpiled, StudioError> {
    let file = format!("{component}.tsx");
    let path = Path::new(&file);
    let fail = |message: String, span: Option<Span>| StudioError::Transpile {
        component: component.to_owned(),
        message,
        loc: span.map(|s| loc_at(tsx, s.start)),
    };
    let alloc = Allocator::default();
    let source_type = SourceType::tsx();
    let parsed = Parser::new(&alloc, tsx, source_type).parse();
    if let Some(d) = parsed.diagnostics.first() {
        let span = d
            .labels
            .first()
            .map(|l| Span::new(l.offset(), l.offset() + l.len()));
        return Err(fail(d.message.to_string(), span));
    }
    let mut program = parsed.program;
    for stmt in &program.body {
        if matches!(
            stmt,
            Statement::ImportDeclaration(_)
                | Statement::ExportAllDeclaration(_)
                | Statement::ExportFromDeclaration(_)
        ) {
            return Err(fail(
                "imports are not supported yet; Box, Text, Image, Stack, Row, the prop builders and the motion helpers are globals".to_owned(),
                Some(stmt.span()),
            ));
        }
    }
    let scoping = SemanticBuilder::new()
        .build(&program)
        .semantic
        .into_scoping();
    let mut opts = TransformOptions::default();
    opts.jsx.runtime = JsxRuntime::Classic;
    opts.jsx.pragma = Some("__studio_h".into());
    opts.jsx.pragma_frag = Some("__studio_Fragment".into());
    opts.jsx.jsx_source_plugin = true;
    opts.jsx.jsx_self_plugin = false;
    opts.jsx.display_name_plugin = false;
    opts.jsx.pure = false;
    let transformed =
        Transformer::new(&alloc, path, &opts).build_with_scoping(scoping, &mut program);
    if let Some(d) = transformed.diagnostics.first() {
        return Err(fail(d.message.to_string(), None));
    }
    let out = Codegen::new()
        .with_options(CodegenOptions {
            source_map_path: Some(PathBuf::from(&file)),
            ..CodegenOptions::default()
        })
        .build(&program);
    let map = out
        .map
        .as_ref()
        .map(|m| {
            LineMap::new(m.get_tokens().map(|t| {
                (
                    t.get_dst_line(),
                    t.get_dst_col(),
                    t.get_src_line(),
                    t.get_src_col(),
                )
            }))
        })
        .unwrap_or_default();
    Ok(Transpiled { js: out.code, map })
}

/// 1-based line and column of a byte offset, with the line's text.
pub fn loc_at(src: &str, offset: u32) -> TsxLoc {
    let offset = (offset as usize).min(src.len());
    let before = src.get(..offset).unwrap_or("");
    let line = before.matches('\n').count() as u32 + 1;
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    let col = before[line_start..].chars().count() as u32 + 1;
    TsxLoc {
        line,
        col,
        snippet: line_text(src, line),
    }
}

/// The text of a 1-based line, or "" past the end.
pub fn line_text(src: &str, line: u32) -> String {
    src.lines()
        .nth(line.saturating_sub(1) as usize)
        .unwrap_or("")
        .to_owned()
}
