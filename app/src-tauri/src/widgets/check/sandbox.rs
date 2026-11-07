//! What the frame refuses or forgets (`sandbox`): a global `fetch`,
//! `XMLHttpRequest`, `WebSocket`, `EventSource`, `localStorage`,
//! `sessionStorage`, `indexedDB` or `eval`, `new Function`/`Function(…)`, and
//! `document.cookie` — each bare or through `window`, `globalThis` or
//! `self`. A local binding of the same name is the widget's own and passes.

use super::Finding;
use oxc::ast::ast::{
    CallExpression, Expression, IdentifierReference, MemberExpression, NewExpression, Program,
};
use oxc::ast_visit::{walk, Visit};
use oxc::semantic::Scoping;
use oxc::span::GetSpan;

const GLOBAL_OBJECTS: &[&str] = &["window", "globalThis", "self"];
const REFUSED: &[&str] = &[
    "fetch",
    "XMLHttpRequest",
    "WebSocket",
    "EventSource",
    "localStorage",
    "sessionStorage",
    "indexedDB",
    "eval",
];

pub(super) fn check(program: &Program, scoping: &Scoping, src: &str) -> Vec<Finding> {
    let mut rule = Sandbox {
        scoping,
        src,
        out: Vec::new(),
    };
    rule.visit_program(program);
    rule.out
}

/// Whether `ident` names a global: no binding in the module declares it.
pub(super) fn is_global(scoping: &Scoping, ident: &IdentifierReference) -> bool {
    ident
        .reference_id
        .get()
        .is_none_or(|id| scoping.get_reference(id).symbol_id().is_none())
}

/// `expr` is a global named one of `names`, bare or as a property of
/// `window`, `globalThis` or `self`; the name it was found under.
pub(super) fn global_named<'a>(
    scoping: &Scoping,
    expr: &Expression<'a>,
    names: &[&str],
) -> Option<&'a str> {
    match expr.without_parentheses() {
        Expression::Identifier(id)
            if names.contains(&id.name.as_str()) && is_global(scoping, id) =>
        {
            Some(id.name.as_str())
        }
        e => {
            let member = e.as_member_expression()?;
            let name = member.static_property_name()?;
            (names.contains(&name) && is_global_object(scoping, member.object())).then_some(name)
        }
    }
}

fn is_global_object(scoping: &Scoping, expr: &Expression) -> bool {
    matches!(expr.without_parentheses(), Expression::Identifier(id)
        if GLOBAL_OBJECTS.contains(&id.name.as_str()) && is_global(scoping, id))
}

fn message(name: &str) -> String {
    match name {
        "localStorage" | "sessionStorage" | "indexedDB" | "cookie" => format!(
            "`{name}` isn't kept in the sandbox: keep what must last with `useWidgetState` from `oculus/state`"
        ),
        "eval" | "Function" => format!(
            "`{name}` is refused by the frame's policy: evaluate maths with `oculus/math` (`evaluate`, `compile`, `formula`)"
        ),
        _ => format!(
            "`{name}` can't reach anything: the frame has no network. Put the data in the widget, or ask a model with `oculus/ask`"
        ),
    }
}

struct Sandbox<'s> {
    scoping: &'s Scoping,
    src: &'s str,
    out: Vec<Finding>,
}

impl Sandbox<'_> {
    fn push(&mut self, offset: u32, name: &str) {
        self.out
            .push(Finding::at(self.src, offset, "sandbox", message(name)));
    }

    /// `Function` only as something called: `new Function(…)` or `Function(…)`.
    fn constructs_function(&mut self, callee: &Expression, offset: u32) {
        if global_named(self.scoping, callee, &["Function"]).is_some() {
            self.push(offset, "Function");
        }
    }
}

impl<'a> Visit<'a> for Sandbox<'_> {
    fn visit_identifier_reference(&mut self, it: &IdentifierReference<'a>) {
        if REFUSED.contains(&it.name.as_str()) && is_global(self.scoping, it) {
            self.push(it.span.start, it.name.as_str());
        }
    }

    fn visit_member_expression(&mut self, it: &MemberExpression<'a>) {
        if let Some(name) = it.static_property_name() {
            let object = it.object();
            if REFUSED.contains(&name) && is_global_object(self.scoping, object) {
                self.push(it.span().start, name);
            }
            if name == "cookie" && global_named(self.scoping, object, &["document"]).is_some() {
                self.push(it.span().start, "cookie");
            }
        }
        walk::walk_member_expression(self, it);
    }

    fn visit_new_expression(&mut self, it: &NewExpression<'a>) {
        self.constructs_function(&it.callee, it.span.start);
        walk::walk_new_expression(self, it);
    }

    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        self.constructs_function(&it.callee, it.span.start);
        walk::walk_call_expression(self, it);
    }
}
