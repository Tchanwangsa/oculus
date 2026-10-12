//! `studio.d.ts` types the prelude for agents (the app ships it in the
//! `oculus-studio` skill), so its declared globals must be exactly the ones
//! the prelude defines: neither a name it lacks nor one it leaves out.

use crate::host::budget::Budget;
use crate::host::runtime;
use rquickjs::Ctx;
use std::collections::BTreeSet;

const DTS: &str = include_str!("../../../studio.d.ts");

/// The host's own entry points and the JSX factory, which no component names.
const INTERNAL_PREFIX: &str = "__studio";

fn global_names(c: &Ctx<'_>) -> BTreeSet<String> {
    let names: Vec<String> = c
        .eval("Object.getOwnPropertyNames(globalThis)")
        .expect("listing globalThis");
    names.into_iter().collect()
}

/// What the prelude adds to a fresh sandbox context, as QuickJS sees it.
fn prelude_globals() -> BTreeSet<String> {
    let budget = Budget::default();
    let rt = runtime::runtime(&budget).unwrap();
    let prelude = runtime::compile_prelude(&rt).unwrap();
    let ctx = runtime::context(&rt).unwrap();
    ctx.with(|c| {
        let before = global_names(&c);
        runtime::install(&c, &prelude).unwrap();
        runtime::install_natives(&c).unwrap();
        global_names(&c)
            .difference(&before)
            .filter(|n| !n.starts_with(INTERNAL_PREFIX))
            .cloned()
            .collect()
    })
}

/// Every top-level `declare const|let|var|function|class <name>`; a `.d.ts`
/// cannot declare a global value any other way. Overloads repeat a name.
fn declared(dts: &str) -> BTreeSet<String> {
    dts.lines()
        .filter_map(|line| line.strip_prefix("declare "))
        .filter_map(|rest| {
            let (keyword, rest) = rest.split_once(' ')?;
            matches!(keyword, "const" | "let" | "var" | "function" | "class").then_some(rest)
        })
        .map(|rest| {
            rest.chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '$')
                .collect()
        })
        .collect()
}

#[test]
fn the_d_ts_declares_exactly_the_prelude_globals() {
    let prelude = prelude_globals();
    let dts = declared(DTS);
    assert!(
        prelude.contains("word") && prelude.contains("Box"),
        "{prelude:?}"
    );
    let missing: Vec<_> = prelude.difference(&dts).collect();
    let extra: Vec<_> = dts.difference(&prelude).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "studio.d.ts is out of step with the prelude: not declared {missing:?}; declared but not defined {extra:?}"
    );
}

#[test]
fn declarations_are_read_by_name() {
    let names = declared(
        "declare const Box: X;\ndeclare function word(p: string): number;\ndeclare namespace JSX {}\n  declare const inner: Y;\ninterface Theme {}\n",
    );
    assert_eq!(names, BTreeSet::from(["Box".to_owned(), "word".to_owned()]));
}
