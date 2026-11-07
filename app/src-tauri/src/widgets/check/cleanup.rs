//! Timers and listeners that outlive the widget (`cleanup`).
//!
//! A scheduling call is a call to the global `setInterval`, `setTimeout` or
//! `requestAnimationFrame` (bare, or on `window`, `globalThis` or `self`) or
//! to any `.addEventListener(…)`. It passes only when it is written inside
//! the callback (the first argument, an arrow or a function expression) of a
//! `useEffect` or `useLayoutEffect` call — nested functions in that callback
//! count — and that callback returns a cleanup that cancels it: a `return`
//! in the callback's own body (or an arrow's expression body) whose value is
//! an arrow or a function expression calling `clearInterval`, `clearTimeout`,
//! `cancelAnimationFrame` or `.removeEventListener` respectively. With
//! effects nested, the innermost one is judged.

use super::sandbox::global_named;
use super::Finding;
use oxc::ast::ast::{
    ArrowFunctionExpression, CallExpression, Expression, Function, Program, ReturnStatement,
    Statement,
};
use oxc::ast_visit::{walk, Visit};
use oxc::semantic::{ScopeFlags, Scoping};

/// Each scheduler and the call that cancels it.
const SCHEDULERS: &[(&str, &str)] = &[
    ("setInterval", "clearInterval"),
    ("setTimeout", "clearTimeout"),
    ("requestAnimationFrame", "cancelAnimationFrame"),
    ("addEventListener", "removeEventListener"),
];
const GLOBAL_SCHEDULERS: &[&str] = &["setInterval", "setTimeout", "requestAnimationFrame"];
const EFFECT_HOOKS: &[&str] = &["useEffect", "useLayoutEffect"];

pub(super) fn check(program: &Program, scoping: &Scoping, src: &str) -> Vec<Finding> {
    let mut rule = Cleanup {
        scoping,
        src,
        effects: Vec::new(),
        out: Vec::new(),
    };
    rule.visit_program(program);
    rule.out
}

struct Cleanup<'s> {
    scoping: &'s Scoping,
    src: &'s str,
    /// For each effect callback being walked, the cancelling calls its
    /// returned cleanups make.
    effects: Vec<Vec<String>>,
    out: Vec<Finding>,
}

impl Cleanup<'_> {
    /// The scheduler `call` makes, if it is one.
    fn scheduler(&self, call: &CallExpression) -> Option<&'static str> {
        let name = match global_named(self.scoping, &call.callee, GLOBAL_SCHEDULERS) {
            Some(name) => name,
            None => {
                let member = call.callee.without_parentheses().as_member_expression()?;
                member
                    .static_property_name()
                    .filter(|n| *n == "addEventListener")?
            }
        };
        SCHEDULERS.iter().find(|(s, _)| *s == name).map(|(s, _)| *s)
    }

    fn judge(&mut self, scheduler: &'static str, offset: u32) {
        let cancel = SCHEDULERS
            .iter()
            .find(|(s, _)| *s == scheduler)
            .map_or("", |(_, c)| *c);
        let message = match self.effects.last() {
            None => format!(
                "`{scheduler}` outside an effect keeps running after the widget is gone: \
                 start it in `useEffect` and return `() => {cancel}(…)`"
            ),
            Some(cancels) if !cancels.iter().any(|c| c == cancel) => format!(
                "`{scheduler}` in an effect whose cleanup doesn't call `{cancel}`: \
                 return `() => {cancel}(…)` from the effect"
            ),
            Some(_) => return,
        };
        self.out
            .push(Finding::at(self.src, offset, "cleanup", message));
    }
}

impl<'a> Visit<'a> for Cleanup<'_> {
    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        if let Some(scheduler) = self.scheduler(it) {
            self.judge(scheduler, it.span.start);
        }
        let effect = it
            .callee_name()
            .is_some_and(|n| EFFECT_HOOKS.contains(&n))
            .then(|| it.arguments.first().and_then(|a| a.as_expression()))
            .flatten()
            .and_then(cancels_in_cleanups);
        match effect {
            Some(cancels) => {
                self.effects.push(cancels);
                walk::walk_call_expression(self, it);
                self.effects.pop();
            }
            None => walk::walk_call_expression(self, it),
        }
    }
}

/// For an effect callback, the names of the calls made inside the cleanups
/// it returns; `None` when `callback` isn't a function.
fn cancels_in_cleanups(callback: &Expression) -> Option<Vec<String>> {
    let mut returned = Returned::default();
    match callback.without_parentheses() {
        Expression::ArrowFunctionExpression(f) => match f.get_expression() {
            Some(e) => returned.cleanup(e),
            None => returned.statements(f.get_function_body().map(|b| &b.statements[..])),
        },
        Expression::FunctionExpression(f) => {
            returned.statements(f.body.as_ref().map(|b| &b.statements[..]))
        }
        _ => return None,
    }
    Some(returned.calls)
}

/// Walks one function body for its `return`s, never into nested functions,
/// collecting the call names inside each returned function.
#[derive(Default)]
struct Returned {
    calls: Vec<String>,
}

impl Returned {
    fn statements(&mut self, statements: Option<&[Statement]>) {
        for s in statements.unwrap_or_default() {
            self.visit_statement(s);
        }
    }

    fn cleanup(&mut self, value: &Expression) {
        let mut calls = Calls::default();
        match value.without_parentheses() {
            Expression::ArrowFunctionExpression(f) => calls.visit_arrow_function_expression(f),
            Expression::FunctionExpression(f) => calls.visit_function(f, ScopeFlags::Function),
            _ => return,
        }
        self.calls.extend(calls.names);
    }
}

impl<'a> Visit<'a> for Returned {
    fn visit_return_statement(&mut self, it: &ReturnStatement<'a>) {
        if let Some(value) = &it.argument {
            self.cleanup(value);
        }
    }

    fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {}

    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
}

/// Every call's callee name inside one function, nested ones included.
#[derive(Default)]
struct Calls {
    names: Vec<String>,
}

impl<'a> Visit<'a> for Calls {
    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        if let Some(name) = it.callee_name() {
            self.names.push(name.to_string());
        }
        walk::walk_call_expression(self, it);
    }
}
