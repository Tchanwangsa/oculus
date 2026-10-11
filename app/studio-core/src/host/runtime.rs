//! The QuickJS sandbox: one runtime per renderer (256 MB heap, 1 MB stack,
//! the step budget) and a fresh context for every entry, with only the
//! intrinsics a pure component needs. No `Date`, `performance`, `WeakRef` or
//! `FinalizationRegistry` (clock- or GC-dependent), no module loader, no I/O.
//! The prelude (`prelude/*.js`, one module scope, concatenated in order) is
//! compiled to bytecode once per runtime and evaluated into each new context.

use super::budget::Budget;
use crate::error::StudioError;
use rquickjs::context::intrinsic::{
    Eval, Json, MapSet, Promise, Proxy, RegExp, RegExpCompiler, TypedArrays,
};
use rquickjs::{Context, Ctx, Function, Module, Runtime, WriteOptions};
use vello_cpu::kurbo::BezPath;

const MEMORY_LIMIT: usize = 256 << 20;
const STACK_LIMIT: usize = 1 << 20;

const PRELUDE: &str = concat!(
    include_str!("prelude/elements.js"),
    include_str!("prelude/resolve.js"),
    include_str!("prelude/props.js"),
    include_str!("prelude/motion.js"),
    include_str!("prelude/time.js"),
    include_str!("prelude/presets.js"),
    include_str!("prelude/arrow.js"),
    include_str!("prelude/words.js"),
    include_str!("prelude/entry.js"),
);
const PRELUDE_NAME: &str = "studio:prelude";

fn host(e: rquickjs::Error) -> StudioError {
    StudioError::Host(e.to_string())
}

pub fn runtime(budget: &Budget) -> Result<Runtime, StudioError> {
    let rt = Runtime::new().map_err(host)?;
    rt.set_memory_limit(MEMORY_LIMIT);
    rt.set_max_stack_size(STACK_LIMIT);
    rt.set_interrupt_handler(Some(Box::new(budget.handler())));
    Ok(rt)
}

/// A context with no globals beyond the allowed intrinsics.
pub fn context(rt: &Runtime) -> Result<Context, StudioError> {
    // Promise is needed because module evaluation returns one.
    Context::custom::<(
        Eval,
        RegExpCompiler,
        RegExp,
        Json,
        Proxy,
        MapSet,
        TypedArrays,
        Promise,
    )>(rt)
    .map_err(host)
}

/// The prelude as module bytecode, for `install`.
pub fn compile_prelude(rt: &Runtime) -> Result<Vec<u8>, StudioError> {
    context(rt)?.with(|c| {
        Module::declare(c.clone(), PRELUDE_NAME, PRELUDE)
            .and_then(|m| m.write(WriteOptions::default()))
            .map_err(|e| {
                let caught = c.catch();
                StudioError::Host(format!("prelude failed to compile: {e} {caught:?}"))
            })
    })
}

/// Evaluates the prelude bytecode into a fresh context.
pub fn install(c: &Ctx<'_>, prelude: &[u8]) -> Result<(), StudioError> {
    let run = || -> rquickjs::Result<()> {
        // SAFETY: `prelude` is `compile_prelude`'s output, written by this
        // same QuickJS build.
        let module = unsafe { Module::load(c.clone(), prelude) }?;
        let (_, promise) = module.eval()?;
        promise.finish()
    };
    run().map_err(|e| {
        let caught = c.catch();
        StudioError::Host(format!("prelude failed: {e} {caught:?}"))
    })
}

/// The prelude's natives. `__studioClosest(name, candidates)` is
/// `suggest::closest` (`word()`'s "closest:" hint), or `undefined`.
/// `__studioPathEnd(d, fraction)` is where a `[0, fraction]` trim of `d` ends,
/// `[x, y, ux, uy, length]` with a unit tangent, measured as the trim
/// measures it; `undefined` for bad data or a path with no length.
pub fn install_natives(c: &Ctx<'_>) -> Result<(), StudioError> {
    let closest = |name: String, candidates: Vec<String>| {
        crate::suggest::closest(&name, candidates.iter().map(String::as_str)).map(str::to_owned)
    };
    let path_end = |d: String, fraction: f64| {
        let path = BezPath::from_svg(&d).ok()?;
        let e = crate::paint::end_of(&path, fraction)?;
        Some(vec![
            e.point.x,
            e.point.y,
            e.tangent.x,
            e.tangent.y,
            e.length,
        ])
    };
    Function::new(c.clone(), closest)
        .and_then(|f| c.globals().set("__studioClosest", f))
        .and_then(|()| Function::new(c.clone(), path_end))
        .and_then(|f| c.globals().set("__studioPathEnd", f))
        .map_err(host)
}
