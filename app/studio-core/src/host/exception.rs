//! A failed JS entry -> `StudioError`. The thrown value's stack gives the
//! first frame inside the component's module; the source map turns that JS
//! position into a TSX line. Errors raised while the prelude resolves the
//! tree have no such frame, and carry the element's TSX position instead.

use super::budget::Budget;
use super::sourcemap::LineMap;
use super::transpile::line_text;
use crate::error::{BudgetKind, StudioError, TsxLoc};
use rquickjs::{Ctx, Value};

/// What is known about the module that was running.
pub struct Source<'a> {
    pub component: &'a str,
    /// Module name as it appears in stack frames.
    pub module: &'a str,
    pub tsx: &'a str,
    pub map: &'a LineMap,
}

pub fn convert(
    ctx: &Ctx<'_>,
    err: rquickjs::Error,
    budget: &Budget,
    src: &Source<'_>,
) -> StudioError {
    let thrown = matches!(err, rquickjs::Error::Exception).then(|| ctx.catch());
    if budget.exceeded() {
        return StudioError::Budget {
            component: src.component.to_owned(),
            kind: BudgetKind::Steps,
        };
    }
    let Some(thrown) = thrown else {
        if matches!(err, rquickjs::Error::Allocation) {
            return memory(src);
        }
        return StudioError::Component {
            component: src.component.to_owned(),
            message: err.to_string(),
            loc: None,
        };
    };
    let (message, stack, tagged) = describe(&thrown);
    if message.contains("out of memory") {
        return memory(src);
    }
    let loc = stack.as_deref().and_then(|s| locate(s, src)).or_else(|| {
        tagged.map(|(line, col)| TsxLoc {
            line,
            col,
            snippet: line_text(src.tsx, line),
        })
    });
    StudioError::Component {
        component: src.component.to_owned(),
        message,
        loc,
    }
}

fn memory(src: &Source<'_>) -> StudioError {
    StudioError::Budget {
        component: src.component.to_owned(),
        kind: BudgetKind::Memory,
    }
}

/// The thrown value's message ("TypeError: x is undefined"), its stack, and
/// the TSX position the resolver tagged it with (`__studioLoc`).
fn describe(v: &Value<'_>) -> (String, Option<String>, Option<(u32, u32)>) {
    if let Some(obj) = v.as_object() {
        let get = |k: &str| obj.get::<_, Option<String>>(k).ok().flatten();
        let message = get("message");
        let name = get("name");
        if let Some(message) = message {
            let full = match name {
                Some(n) if !n.is_empty() && n != "Error" => format!("{n}: {message}"),
                _ => message,
            };
            let tagged = obj
                .get::<_, Option<Vec<u32>>>("__studioLoc")
                .ok()
                .flatten()
                .and_then(|l| Some((*l.first()?, l.get(1).copied().unwrap_or(1))));
            return (full, get("stack"), tagged);
        }
    }
    let text = v
        .as_string()
        .and_then(|s| s.to_string().ok())
        .map(|s| format!("threw {s:?}"))
        .unwrap_or_else(|| format!("threw a non-Error value ({})", v.type_name()));
    (text, None, None)
}

/// The first stack frame in the component's module, mapped to TSX.
fn locate(stack: &str, src: &Source<'_>) -> Option<TsxLoc> {
    let needle = format!("{}:", src.module);
    stack.lines().find_map(|frame| {
        let at = frame.find(&needle)? + needle.len();
        let rest = &frame[at..];
        let mut parts = rest
            .split(|c: char| !c.is_ascii_digit())
            .filter(|p| !p.is_empty());
        let line: u32 = parts.next()?.parse().ok()?;
        let col: u32 = parts.next().and_then(|c| c.parse().ok()).unwrap_or(1);
        let (tl, tc) = src.map.lookup(line, col)?;
        Some(TsxLoc {
            line: tl,
            col: tc,
            snippet: line_text(src.tsx, tl),
        })
    })
}
