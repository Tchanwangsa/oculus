//! The step budget: QuickJS calls the interrupt handler every ~10k
//! bytecode branches; past `STEP_LIMIT` calls the handler aborts the run.
//! Per runtime and reset on every entry (a module load; a call, and again
//! before its default export runs), so one component's spend never counts
//! against another's, and the count is deterministic (no clock).

use std::cell::Cell;
use std::rc::Rc;

/// About a quarter of a second of tight JS; a frame normally needs well
/// under a millisecond.
const STEP_LIMIT: u64 = 20_000;

#[derive(Clone, Default)]
pub struct Budget {
    ticks: Rc<Cell<u64>>,
    exceeded: Rc<Cell<bool>>,
}

impl Budget {
    pub fn reset(&self) {
        self.ticks.set(0);
        self.exceeded.set(false);
    }

    pub fn exceeded(&self) -> bool {
        self.exceeded.get()
    }

    /// The handler QuickJS polls; `true` interrupts the running script.
    pub fn handler(&self) -> impl FnMut() -> bool + 'static {
        let budget = self.clone();
        move || {
            let n = budget.ticks.get() + 1;
            budget.ticks.set(n);
            if n > STEP_LIMIT {
                budget.exceeded.set(true);
            }
            budget.exceeded.get()
        }
    }
}
