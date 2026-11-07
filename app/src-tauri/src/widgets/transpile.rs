//! A widget's TSX to an ES module. TypeScript is stripped and JSX becomes
//! `react/jsx-runtime` calls; the `import` statements stay, restricted to
//! `ALLOWED_IMPORTS`, because the sandbox resolves them against the kit.
//!
//! The widget's default export is rebound to `const DEFAULT_BINDING` and the
//! module no longer has an `export default`: a module can't read its own
//! default export, so the frame appends a line that mounts that binding
//! (`srcdoc.ts` repeats the name). A module without a default export is an
//! error.

use oxc::allocator::Allocator;
use oxc::ast::ast::{
    ExportAllDeclaration, ExportDefaultDeclarationKind, ExportFromDeclaration, ExportSpecifier,
    ImportDeclaration, ImportExpression, ModuleDeclaration, TSExternalModuleReference,
};
use oxc::ast_visit::Visit;
use oxc::codegen::Codegen;
use oxc::parser::Parser;
use oxc::semantic::SemanticBuilder;
use oxc::span::{GetSpan, SourceType};
use oxc::transformer::{JsxRuntime, TransformOptions, Transformer};
use std::path::Path;

/// What a widget may import: the import map's names (`srcdoc.ts`).
/// `react/jsx-runtime` is here because the JSX transform emits it.
pub const ALLOWED_IMPORTS: &[&str] = &[
    "react",
    "react/jsx-runtime",
    "oculus/ui",
    "oculus/state",
    "oculus/2d",
    "oculus/math",
    "oculus/3d",
    "oculus/ask",
    "three",
    "@react-three/fiber",
];

/// The top-level binding that holds the widget's default export.
pub const DEFAULT_BINDING: &str = "__oculusWidget";

#[derive(serde::Serialize, Debug)]
pub struct Transpiled {
    pub js: String,
}

/// `Err` is `"<line>:<col> <message>"` (1-based) for a syntax error, the
/// disallowed-import message, or the missing-default-export message.
pub fn transpile(tsx: &str) -> Result<Transpiled, String> {
    let alloc = Allocator::default();
    let parsed = Parser::new(&alloc, tsx, SourceType::tsx()).parse();
    if let Some(d) = parsed.diagnostics.first() {
        let offset = d.labels.first().map_or(0, |l| l.offset());
        let (line, col) = line_col(tsx, offset as usize);
        return Err(format!("{line}:{col} {}", d.message));
    }
    let mut program = parsed.program;

    let mut imports = Imports::default();
    imports.visit_program(&program);
    if let Some(error) = imports.error {
        return Err(error);
    }

    let scoping = SemanticBuilder::new()
        .build(&program)
        .semantic
        .into_scoping();
    let mut opts = TransformOptions::default();
    opts.jsx.runtime = JsxRuntime::Automatic;
    opts.jsx.import_source = Some("react".into());
    opts.jsx.development = false;
    opts.jsx.pure = false;
    opts.jsx.refresh = None;
    let transformed = Transformer::new(&alloc, Path::new("widget.tsx"), &opts)
        .build_with_scoping(scoping, &mut program);
    if let Some(d) = transformed.diagnostics.first() {
        let offset = d.labels.first().map_or(0, |l| l.offset());
        let (line, col) = line_col(tsx, offset as usize);
        return Err(format!("{line}:{col} {}", d.message));
    }
    Ok(Transpiled {
        js: bind_default(&Codegen::new().build(&program).code)?,
    })
}

/// Rebinds the default export of `js` (the transform's output) to
/// `DEFAULT_BINDING`. The output is parsed again and edited at the exact spans
/// of its top-level export statements, so a template string or comment that
/// looks like one is never touched.
fn bind_default(js: &str) -> Result<String, String> {
    let alloc = Allocator::default();
    let parsed = Parser::new(&alloc, js, SourceType::mjs()).parse();
    if let Some(d) = parsed.diagnostics.first() {
        return Err(d.message.to_string());
    }

    // `(start, end, replacement)` byte ranges of `js`; `alias` is a name the
    // binding is set to after the module, for a default that stays a declaration.
    let mut edits: Vec<(u32, u32, String)> = Vec::new();
    let mut alias: Option<String> = None;
    for stmt in &parsed.program.body {
        match stmt.as_module_declaration() {
            Some(ModuleDeclaration::ExportDefaultDeclaration(d)) => {
                let (start, end) = (d.span.start, d.declaration.span().start);
                let declared = match &d.declaration {
                    ExportDefaultDeclarationKind::FunctionDeclaration(f) => f.id.as_ref(),
                    ExportDefaultDeclarationKind::ClassDeclaration(c) => c.id.as_ref(),
                    _ => None,
                };
                if let Some(id) = declared {
                    // `export default function App() {}` stays a declaration.
                    edits.push((start, end, String::new()));
                    alias = Some(id.name.to_string());
                } else {
                    // The `;` keeps a following line that starts with `(` from calling it.
                    edits.push((start, end, format!("const {DEFAULT_BINDING} = ")));
                    edits.push((d.span.end, d.span.end, ";".into()));
                }
            }
            Some(ModuleDeclaration::ExportNamedDeclaration(n)) => {
                let is_default = |s: &ExportSpecifier| s.exported.name() == "default";
                let Some(default) = n.specifiers.iter().find(|s| is_default(s)) else {
                    continue;
                };
                alias = Some(default.local.name().to_string());
                let kept: Vec<&str> = n
                    .specifiers
                    .iter()
                    .filter(|s| !is_default(s))
                    .map(|s| &js[s.span.start as usize..s.span.end as usize])
                    .collect();
                let list = if kept.is_empty() {
                    String::new()
                } else {
                    format!("export {{ {} }};", kept.join(", "))
                };
                edits.push((n.span.start, n.span.end, list));
            }
            _ => {}
        }
    }
    if edits.is_empty() {
        return Err("a widget needs a default export (the component to draw)".into());
    }

    let mut out = js.to_string();
    edits.sort_by_key(|e| std::cmp::Reverse(e.0));
    for (start, end, replacement) in edits {
        out.replace_range(start as usize..end as usize, &replacement);
    }
    if let Some(name) = alias {
        out.push_str(&format!("\nconst {DEFAULT_BINDING} = {name};\n"));
    }
    Ok(out)
}

fn not_allowed(what: &str) -> String {
    format!(
        "{what} is not allowed; widgets may import: {}",
        ALLOWED_IMPORTS.join(", ")
    )
}

/// The first import, re-export or dynamic import the allow-list refuses.
#[derive(Default)]
struct Imports {
    error: Option<String>,
}

impl Imports {
    fn check(&mut self, specifier: &str) {
        if self.error.is_none() && !ALLOWED_IMPORTS.contains(&specifier) {
            self.error = Some(not_allowed(&format!("import \"{specifier}\"")));
        }
    }

    fn refuse(&mut self, message: String) {
        self.error.get_or_insert(message);
    }
}

impl<'a> Visit<'a> for Imports {
    fn visit_import_declaration(&mut self, it: &ImportDeclaration<'a>) {
        self.check(it.source.value.as_str());
    }

    fn visit_export_all_declaration(&mut self, it: &ExportAllDeclaration<'a>) {
        self.refuse(not_allowed(&format!(
            "export ... from \"{}\"",
            it.source.value
        )));
    }

    fn visit_export_from_declaration(&mut self, it: &ExportFromDeclaration<'a>) {
        self.refuse(not_allowed(&format!(
            "export ... from \"{}\"",
            it.source.value
        )));
    }

    fn visit_import_expression(&mut self, _: &ImportExpression<'a>) {
        self.refuse(not_allowed("dynamic import()"));
    }

    fn visit_ts_external_module_reference(&mut self, it: &TSExternalModuleReference<'a>) {
        self.check(it.expression.value.as_str());
    }
}

/// 1-based line and column (in characters) of a byte offset.
pub(crate) fn line_col(src: &str, offset: usize) -> (usize, usize) {
    let before = src.get(..offset.min(src.len())).unwrap_or("");
    let line = before.matches('\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    (line, before[line_start..].chars().count() + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALLOWED_MESSAGE: &str = "is not allowed; widgets may import: react, react/jsx-runtime, oculus/ui, oculus/state, oculus/2d, oculus/math, oculus/3d, oculus/ask, three, @react-three/fiber";

    #[test]
    fn tsx_with_types_jsx_and_state_becomes_js() {
        let out = transpile(
            r#"
import { useState } from "react";
import { Widget, Slider } from "oculus/ui";

interface Props { start: number }
type Mode = "a" | "b";

export default function Demo({ start }: Props) {
  const [n, setN] = useState<number>(start);
  const mode: Mode = "a";
  return <Widget title={mode}><Slider value={n} onChange={setN} /></Widget>;
}
"#,
        )
        .unwrap();
        assert!(out.js.contains("react/jsx-runtime"), "{}", out.js);
        assert!(out.js.contains("from \"react\""), "{}", out.js);
        assert!(out.js.contains("from \"oculus/ui\""), "{}", out.js);
        assert!(!out.js.contains("export default"), "{}", out.js);
        assert!(
            out.js.contains("const __oculusWidget = Demo;"),
            "{}",
            out.js
        );
        assert!(!out.js.contains("interface"), "{}", out.js);
        assert!(!out.js.contains(": number"), "{}", out.js);
    }

    #[test]
    fn local_exports_and_state_import_are_fine() {
        let out = transpile(
            "import { useWidgetState } from \"oculus/state\";\nexport const k = 1;\nexport default function W() { useWidgetState(\"a\", 1); return <div/>; }",
        )
        .unwrap();
        assert!(out.js.contains("oculus/state"));
    }

    #[test]
    fn plane_and_math_imports_are_fine() {
        let out = transpile(
            "import { Plane, Plot } from \"oculus/2d\";\nimport { formula } from \"oculus/math\";\nexport default function W() { const f = formula(\"x^2\"); return <Plane>{f.ok && <Plot y={(x: number) => f.fn(x)} />}</Plane>; }",
        )
        .unwrap();
        assert!(out.js.contains("from \"oculus/2d\""), "{}", out.js);
        assert!(out.js.contains("from \"oculus/math\""), "{}", out.js);
    }

    #[test]
    fn three_d_imports_are_fine() {
        let out = transpile(
            "import { Scene3D, Sphere } from \"oculus/3d\";\nimport { Vector3 } from \"three\";\nimport { useFrame } from \"@react-three/fiber\";\nexport default function W() { useFrame(() => {}); const v = new Vector3(0, 0, 1); return <Scene3D><Sphere center={[v.x, v.y, v.z]} /></Scene3D>; }",
        )
        .unwrap();
        assert!(out.js.contains("from \"oculus/3d\""), "{}", out.js);
        assert!(out.js.contains("from \"three\""), "{}", out.js);
        assert!(out.js.contains("from \"@react-three/fiber\""), "{}", out.js);
    }

    #[test]
    fn disallowed_imports_are_rejected() {
        for specifier in [
            "mafs",
            "mathjs",
            "three/addons/controls/OrbitControls.js",
            "three/examples/jsm/x.js",
            "@react-three/drei",
            "oculus/2d/Plane",
            "./x",
            "https://cdn.example/x.js",
            "../y",
            "/abs",
        ] {
            let err = transpile(&format!(
                "import x from \"{specifier}\";\nexport default () => <div>{{x}}</div>;"
            ))
            .unwrap_err();
            assert_eq!(err, format!("import \"{specifier}\" {ALLOWED_MESSAGE}"),);
        }
    }

    #[test]
    fn re_exports_and_dynamic_imports_are_rejected() {
        for source in [
            "export * from \"react\";",
            "export { useState } from \"react\";",
            "export default () => { import(\"react\"); return <div/>; };",
            "export default async () => { const m = await import(\"three\"); return m; };",
            "import r = require(\"fs\");",
        ] {
            let err = transpile(source).unwrap_err();
            assert!(err.contains(ALLOWED_MESSAGE), "{source}: {err}");
        }
    }

    #[test]
    fn syntax_errors_carry_line_and_column() {
        let err = transpile("const a = 1;\nconst b = ;\n").unwrap_err();
        assert!(err.starts_with("2:11 "), "{err}");
    }

    /// The module's code with every default binding form checked on it.
    fn bound(source: &str) -> String {
        let js = transpile(source).unwrap().js;
        assert!(!js.contains("export default"), "{js}");
        assert!(!js.contains("as default"), "{js}");
        assert_eq!(js.matches(DEFAULT_BINDING).count(), 1, "{js}");
        js
    }

    #[test]
    fn a_named_default_declaration_keeps_its_name_and_is_aliased() {
        let js = bound("export default function App() { return <div/>; }");
        assert!(js.contains("function App()"), "{js}");
        assert!(js.contains("const __oculusWidget = App;"), "{js}");
        let js = bound("export default class Board { render() { return null; } }");
        assert!(js.contains("class Board"), "{js}");
        assert!(js.contains("const __oculusWidget = Board;"), "{js}");
        let js = bound("export default async function* Gen() {}");
        assert!(js.contains("const __oculusWidget = Gen;"), "{js}");
    }

    #[test]
    fn an_anonymous_default_becomes_the_binding() {
        for source in [
            "export default function () { return <div/>; }",
            "export default class { render() { return null; } }",
            "export default () => <div/>;",
            "const A = () => <div/>;\nexport default A;",
            "export default memo(function () { return null; });",
            "export default (1 as number);",
        ] {
            let js = bound(source);
            assert!(js.contains("const __oculusWidget = "), "{source}: {js}");
        }
    }

    #[test]
    fn an_expression_default_does_not_swallow_the_next_line() {
        let js = bound("export default A\n;(window as any).x = 1;\nconst A = () => null;");
        assert!(js.contains("const __oculusWidget = A;"), "{js}");
        assert!(!js.contains("A(window"), "{js}");
    }

    #[test]
    fn an_export_list_default_is_taken_out_and_the_rest_kept() {
        let js =
            bound("function App() { return <div/>; }\nconst Y = 1;\nexport { App as default };");
        assert!(js.contains("const __oculusWidget = App;"), "{js}");
        assert!(!js.contains("export"), "{js}");
        let js = bound("function X() { return null; }\nconst Y = 1, Z = 2;\nexport { X as default, Y, Z as W };");
        assert!(js.contains("export { Y, Z as W };"), "{js}");
        assert!(js.contains("const __oculusWidget = X;"), "{js}");
        let js = bound(
            "import { Widget as Imported } from \"oculus/ui\";\nexport { Imported as default };",
        );
        assert!(js.contains("const __oculusWidget = Imported;"), "{js}");
    }

    #[test]
    fn text_that_looks_like_an_export_is_left_alone() {
        let js = transpile(
            "// export default fake\nexport default function W() {\n  const s = `a\nexport default nope\nexport { x as default }`;\n  return <pre>{s}</pre>;\n}",
        )
        .unwrap()
        .js;
        assert!(
            js.contains("\nexport default nope\nexport { x as default }`"),
            "{js}"
        );
        assert!(!js.contains("\nexport default function"), "{js}");
        assert!(js.contains("function W()"), "{js}");
        assert!(js.contains("const __oculusWidget = W;"), "{js}");
    }

    #[test]
    fn no_default_export_is_an_error() {
        for source in [
            "export const x = 1;",
            "const s = `\nexport default nope`;\nexport function W() { return <div/>; }",
            "function W() { return null; }",
            "export type T = number;",
        ] {
            assert_eq!(
                transpile(source).unwrap_err(),
                "a widget needs a default export (the component to draw)",
                "{source}"
            );
        }
    }

    #[test]
    fn two_defaults_are_a_syntax_error() {
        let err = transpile("export default 1;\nexport default 2;").unwrap_err();
        assert!(err.contains("multiple default exports"), "{err}");
    }
}
