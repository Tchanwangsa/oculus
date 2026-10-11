//! The JS host: oxc transpiles a component's TSX, QuickJS runs it in a
//! sandbox, and the element tree comes back as JSON. One `Host` per
//! renderer; it is not `Send`, so each render thread owns its own.
//!
//! Every load and call runs in a fresh context on the host's runtime: the
//! prelude, the theme and the component module are evaluated anew, so no
//! call can see what another left behind (module-level `let`s, mutated
//! globals or prototypes). Both are kept as bytecode so neither is parsed
//! again; the context and its garbage are freed before the entry returns.

mod budget;
mod exception;
mod runtime;
mod sourcemap;
mod transpile;

use crate::error::{Result, StudioError};
use crate::schema::{PropsSchema, parse_schema};
use budget::Budget;
use exception::{Source, convert};
use rquickjs::{Ctx, Function, Module, Object, Runtime, WriteOptions};
use serde_json::{Map, Value};
use sourcemap::LineMap;
use std::collections::HashMap;

/// What a component exports besides its default function.
#[derive(Clone, Debug)]
pub struct ComponentInfo {
    pub props: PropsSchema,
    /// Named example inputs (`export const examples`), unvalidated.
    pub examples: Map<String, Value>,
}

struct Loaded {
    tsx: String,
    map: LineMap,
    info: ComponentInfo,
    bytecode: Vec<u8>,
}

/// One component call's inputs besides its props.
#[derive(Clone, Copy, Debug)]
pub struct CallCtx {
    pub t: f64,
    pub duration: f64,
    pub width: f64,
    pub height: f64,
    pub seed: u32,
}

pub struct Host {
    rt: Runtime,
    budget: Budget,
    prelude: Vec<u8>,
    theme: String,
    loaded: HashMap<String, Loaded>,
}

impl Host {
    pub fn new(theme: &Value) -> Result<Host> {
        let budget = Budget::default();
        let rt = runtime::runtime(&budget)?;
        let prelude = runtime::compile_prelude(&rt)?;
        let host = Host {
            rt,
            budget,
            prelude,
            theme: theme.to_string(),
            loaded: HashMap::new(),
        };
        // Surfaces a prelude or theme failure here rather than on first use.
        host.isolated(|_| Ok(()))?;
        Ok(host)
    }

    pub fn info(&self, name: &str) -> Option<&ComponentInfo> {
        self.loaded.get(name).map(|l| &l.info)
    }

    /// The TSX a loaded component came from.
    pub fn source(&self, name: &str) -> Option<&str> {
        self.loaded.get(name).map(|l| l.tsx.as_str())
    }

    /// Compiles and evaluates a component, replacing any earlier version.
    pub fn load(&mut self, name: &str, tsx: &str) -> Result<&ComponentInfo> {
        let out = transpile::transpile(name, tsx)?;
        let module = module_name(name);
        let src = Source {
            component: name,
            module: &module,
            tsx,
            map: &out.map,
        };
        let budget = &self.budget;
        let (json, bytecode) = self.isolated(|c| {
            let run = || -> rquickjs::Result<(String, Vec<u8>)> {
                let declared = Module::declare(c.clone(), module.as_str(), out.js.as_str())?;
                let bytecode = declared.write(WriteOptions::default())?;
                let (m, promise) = declared.eval()?;
                promise.finish::<()>()?;
                let ns: Object = m.namespace()?;
                let exports: Function = c.globals().get("__studioExports")?;
                Ok((exports.call((ns,))?, bytecode))
            };
            run().map_err(|e| convert(c, e, budget, &src))
        })?;
        let parsed: Value = serde_json::from_str(&json)
            .map_err(|e| StudioError::Host(format!("component exports: {e}")))?;
        let bad = |message: String| StudioError::Component {
            component: name.to_owned(),
            message,
            loc: None,
        };
        let props = parse_schema(&parsed["props"]).map_err(bad)?;
        let examples = match &parsed["examples"] {
            Value::Object(m) => m.clone(),
            _ => {
                return Err(bad(
                    "`export const examples` must be an object of named prop sets".to_owned(),
                ));
            }
        };
        let info = ComponentInfo { props, examples };
        self.loaded.insert(
            name.to_owned(),
            Loaded {
                tsx: tsx.to_owned(),
                map: out.map,
                info,
                bytecode,
            },
        );
        self.info(name)
            .ok_or_else(|| StudioError::Host("component vanished after loading".to_owned()))
    }

    /// Calls a loaded component; returns its element tree and console output
    /// (its module's top-level logs included, since the module runs again).
    pub fn call(
        &mut self,
        name: &str,
        props: &Map<String, Value>,
        call: CallCtx,
    ) -> Result<(Value, Vec<String>)> {
        let loaded = self
            .loaded
            .get(name)
            .ok_or_else(|| StudioError::ComponentNotFound {
                name: name.to_owned(),
                message: "not loaded".to_owned(),
            })?;
        let module = module_name(name);
        let src = Source {
            component: name,
            module: &module,
            tsx: &loaded.tsx,
            map: &loaded.map,
        };
        let props_json = Value::Object(props.clone()).to_string();
        let budget = &self.budget;
        let (json, logs) = self.isolated(|c| {
            let globals = c.globals();
            let run = || -> rquickjs::Result<String> {
                // SAFETY: written by `Module::write` in `load`, on this same
                // QuickJS build.
                let declared = unsafe { Module::load(c.clone(), &loaded.bytecode) }?;
                let (m, promise) = declared.eval()?;
                promise.finish::<()>()?;
                let ns: Object = m.namespace()?;
                // The call gets the whole budget, as `load` gave the module.
                budget.reset();
                let f: Function = globals.get("__studioCall")?;
                f.call((
                    ns,
                    props_json.as_str(),
                    call.t,
                    call.duration,
                    call.width,
                    call.height,
                    call.seed,
                ))
            };
            let json = run().map_err(|e| convert(c, e, budget, &src))?;
            let logs = globals
                .get::<_, Function>("__studioTakeLogs")
                .and_then(|f| f.call::<_, String>(()))
                .ok()
                .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
                .unwrap_or_default();
            Ok((json, logs))
        })?;
        let tree = serde_json::from_str(&json)
            .map_err(|e| StudioError::Host(format!("element tree JSON: {e}")))?;
        Ok((tree, logs))
    }

    /// Runs `f` in a new context holding the prelude and theme, with the
    /// step budget reset, then frees the context.
    fn isolated<R>(&self, f: impl FnOnce(&Ctx<'_>) -> Result<R>) -> Result<R> {
        let ctx = runtime::context(&self.rt)?;
        self.budget.reset();
        let out = ctx.with(|c| {
            runtime::install(&c, &self.prelude)?;
            let set: Function = c
                .globals()
                .get("__studioSetTheme")
                .map_err(|e| StudioError::Host(e.to_string()))?;
            set.call::<_, ()>((self.theme.as_str(),))
                .map_err(|e| StudioError::Host(format!("setting the theme: {e}")))?;
            f(&c)
        });
        drop(ctx);
        // Collect the context's cycles now, so every entry starts from the
        // same heap and hits the memory limit at the same point.
        self.rt.run_gc();
        out
    }

    /// Bytes QuickJS has allocated on this host's runtime.
    #[cfg(test)]
    fn heap_bytes(&self) -> i64 {
        self.rt.memory_usage().malloc_size
    }
}

/// A TSX position (1-based) with that line's text.
pub fn tsx_loc(tsx: &str, (line, col): (u32, u32)) -> crate::error::TsxLoc {
    crate::error::TsxLoc {
        line,
        col,
        snippet: transpile::line_text(tsx, line),
    }
}

fn module_name(component: &str) -> String {
    format!("{component}.tsx")
}

/// A stable seed for `Math.random` from the component and scene index
/// (FNV-1a, so it never changes between builds or machines).
pub fn seed(component: &str, scene: usize) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in component.bytes().chain(scene.to_le_bytes()) {
        h ^= u32::from(b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

#[cfg(test)]
mod tests;
