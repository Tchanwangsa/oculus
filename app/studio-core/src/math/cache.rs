//! Typeset formulas kept across frames, by source and display mode, since
//! a scene draws the same formulas frame after frame. Each renderer has its
//! own; past `CAP` formulas (a counter typeset as maths, say) it starts
//! over. Errors are not kept.

use super::{Formula, typeset};
use std::collections::HashMap;
use std::sync::Arc;

const CAP: usize = 512;

#[derive(Default)]
pub struct MathCache {
    formulas: HashMap<(String, bool), Arc<Formula>>,
}

impl MathCache {
    pub fn get(&mut self, tex: &str, display: bool) -> Result<Arc<Formula>, String> {
        if let Some(f) = self.formulas.get(&(tex.to_owned(), display)) {
            return Ok(Arc::clone(f));
        }
        let f = Arc::new(typeset(tex, display)?);
        if self.formulas.len() >= CAP {
            self.formulas.clear();
        }
        self.formulas
            .insert((tex.to_owned(), display), Arc::clone(&f));
        Ok(f)
    }
}
