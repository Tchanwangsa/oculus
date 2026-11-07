use super::{check, Finding};

/// A titled widget around `body`, which goes inside the component.
fn widget(imports: &str, body: &str) -> String {
    format!("// title: Test\n{imports}\nexport default function W() {{\n{body}\nreturn null;\n}}\n")
}

fn rules(source: &str) -> Vec<&'static str> {
    check(source).into_iter().map(|f| f.rule).collect()
}

#[test]
fn a_clean_widget_has_no_findings() {
    let source = widget(
        "import { useEffect, useState } from \"react\";",
        "const [n, setN] = useState(0);\n\
         useEffect(() => { const id = setInterval(() => setN((v) => v + 1), 100); return () => clearInterval(id); }, []);\n\
         const s = { color: \"var(--color-brand)\", gap: 4 };",
    );
    assert_eq!(check(&source), Vec::<Finding>::new());
}

#[test]
fn a_transpile_error_is_the_only_finding_with_its_place() {
    let found = check("// title: T\nconst a = ;\nexport default () => null;");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].rule, "transpile");
    assert_eq!((found[0].line, found[0].col), (Some(2), Some(11)));

    let found = check("// title: T\nimport x from \"mafs\";\nexport default () => x;");
    assert_eq!(found[0].rule, "transpile");
    assert_eq!(found[0].line, None);
    assert!(found[0].message.contains("not allowed"), "{found:?}");
}

#[test]
fn the_first_line_names_the_widget() {
    assert_eq!(rules("export default () => null;"), ["title"]);
    assert_eq!(rules("// title:\nexport default () => null;"), ["title"]);
    assert!(rules("\n  // Title: Spaced\nexport default () => null;").is_empty());
}

#[test]
fn fixed_colours_are_found_wherever_they_are_written() {
    for body in [
        "const a = \"#ff0000\";",
        "const a = { background: \"rgb(0, 0, 0)\" };",
        "const a = `hsl(${1}, 50%, 50%)`;",
        "const a = 0xff8800;",
        "const a = <div style={{ color: \"#abc\" }} />;",
        "const a = <svg><rect fill=\"#123456\" /></svg>;",
    ] {
        let source = widget("", body);
        assert_eq!(rules(&source), ["colour"], "{body}");
    }
    for body in [
        "const a = 0xff;",
        "const a = \"Step #12\";",
        "const a = \"var(--color-chart-1)\";",
    ] {
        assert!(rules(&widget("", body)).is_empty(), "{body}");
    }
}

#[test]
fn font_families_are_found_as_keys_attributes_and_css() {
    for body in [
        "const a = { fontFamily: \"serif\" };",
        "const a = { \"font-family\": \"serif\" };",
        "const a = <text fontFamily=\"serif\" />;",
        "const a = `p { font-family: serif }`;",
    ] {
        assert_eq!(rules(&widget("", body)), ["font"], "{body}");
    }
}

#[test]
fn a_doubled_backslash_in_a_jsx_attribute_string_is_found() {
    let source = widget(
        "import { Tex } from \"oculus/ui\";",
        "const a = <Tex tex=\"\\\\theta\" />;",
    );
    let found = check(&source);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].rule, "tex");
    assert!(
        found[0].message.contains("tex={\"\\\\theta\"}"),
        "{found:?}"
    );
    let fine = widget(
        "import { Tex } from \"oculus/ui\";",
        "const a = <Tex tex={\"\\\\theta\"} />; const b = <Tex tex=\"\\theta\" />;",
    );
    assert!(check(&fine).is_empty());
}

#[test]
fn the_network_storage_and_eval_are_refused_unless_the_name_is_local() {
    for body in [
        "fetch(\"/x\");",
        "window.fetch(\"/x\");",
        "new XMLHttpRequest();",
        "new WebSocket(\"ws://x\");",
        "localStorage.setItem(\"a\", \"b\");",
        "globalThis.sessionStorage.clear();",
        "indexedDB.open(\"x\");",
        "const c = document.cookie;",
        "const c = window.document.cookie;",
        "eval(\"1\");",
        "new Function(\"return 1\");",
    ] {
        assert_eq!(rules(&widget("", body)), ["sandbox"], "{body}");
    }
    for body in [
        "const fetch = (x: string) => x; fetch(\"a\");",
        "const storage = { localStorage: 1 }; storage.localStorage;",
        "const d = { cookie: 1 }; d.cookie;",
    ] {
        assert!(rules(&widget("", body)).is_empty(), "{body}");
    }
}

#[test]
fn timers_and_listeners_need_an_effect_that_cancels_them() {
    let imports = "import { useEffect, useLayoutEffect } from \"react\";";
    for body in [
        "setTimeout(() => {}, 10);",
        "const onClick = () => setTimeout(() => {}, 10);",
        "window.addEventListener(\"keydown\", () => {});",
        "useEffect(() => { setInterval(() => {}, 10); }, []);",
        "useEffect(() => { const id = setInterval(() => {}, 10); return () => clearTimeout(id); }, []);",
        "useEffect(() => { requestAnimationFrame(() => {}); return () => {}; }, []);",
        "useEffect(() => { const id = setInterval(() => {}, 10); return () => { const f = () => {}; f(); }; }, []);",
    ] {
        assert_eq!(rules(&widget(imports, body)), ["cleanup"], "{body}");
    }
    for body in [
        "useEffect(() => { const id = setTimeout(() => {}, 10); return () => clearTimeout(id); }, []);",
        "useLayoutEffect(function () { const on = () => {}; window.addEventListener(\"resize\", on); return function () { window.removeEventListener(\"resize\", on); }; }, []);",
        "useEffect(() => { if (!n) return; let f = 0; const tick = () => { f = requestAnimationFrame(tick); }; f = requestAnimationFrame(tick); return () => cancelAnimationFrame(f); }, [n]);",
        "React.useEffect(() => { const id = window.setInterval(() => {}, 10); return () => window.clearInterval(id); }, []);",
        "const setTimeout = (f: () => void) => f(); setTimeout(() => {});",
    ] {
        let source = format!(
            "{}const n = 1;\nconst React = {{ useEffect }};",
            widget(imports, body)
        );
        assert!(rules(&source).is_empty(), "{body}: {:?}", check(&source));
    }
}

#[test]
fn findings_carry_their_place_in_source_order() {
    let source = "// title: T\nexport default function W() {\n  const a = \"#fff\";\n  fetch(\"/\");\n  return null;\n}\n";
    let found = check(source);
    let places: Vec<_> = found.iter().map(|f| (f.rule, f.line, f.col)).collect();
    assert_eq!(
        places,
        [("colour", Some(3), Some(13)), ("sandbox", Some(4), Some(3))]
    );
}
