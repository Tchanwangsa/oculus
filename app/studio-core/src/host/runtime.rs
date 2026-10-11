//! The QuickJS sandbox: one runtime per renderer (256 MB heap, 1 MB stack,
//! the step budget) and a fresh context for every entry, with only the
//! intrinsics a pure component needs. No `Date`, `performance`, `WeakRef` or
//! `FinalizationRegistry` (clock- or GC-dependent), no module loader, no I/O.
//! The prelude is compiled to bytecode once per runtime and evaluated into
//! each new context.

use super::budget::Budget;
use crate::error::StudioError;
use rquickjs::context::intrinsic::{
    Eval, Json, MapSet, Promise, Proxy, RegExp, RegExpCompiler, TypedArrays,
};
use rquickjs::{Context, Ctx, Module, Runtime, WriteOptions};

const MEMORY_LIMIT: usize = 256 << 20;
const STACK_LIMIT: usize = 1 << 20;

const PRELUDE: &str = include_str!("prelude.js");
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
