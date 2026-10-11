//! A transition frame: two component calls (A at its last moment, B at
//! `t`), both isolated, composed onto one canvas or morphed into one tree.

use super::{Engine, SceneCall};
use crate::error::Result;
use crate::layout::LNode;
use crate::morph::morph;
use crate::render::Frame;
use crate::transition::{Look, Transition, layers};
use std::collections::HashSet;

impl Engine {
    pub(crate) fn render_transition(
        &mut self,
        a: SceneCall<'_>,
        b: SceneCall<'_>,
        transition: &Transition,
        t: f64,
    ) -> Result<Frame> {
        let pa = self.prepare(a)?;
        let pb = match self.prepare(b) {
            Ok(pb) => pb,
            Err(e) => {
                self.typeset.reclaim(pa.tree);
                return Err(e);
            }
        };
        let mut logs: Vec<String> = pa
            .logs
            .into_iter()
            .map(|l| format!("scene {}: {l}", a.index))
            .collect();
        logs.extend(pb.logs);
        let p = transition.progress(t);
        let look = if transition.auto_magic && shares_a_key(&pa.tree, &pb.tree) {
            Look::Magic
        } else {
            transition.look
        };
        let size = (f64::from(self.size.width), f64::from(self.size.height));
        let painted = if look == Look::Magic {
            let ink = self.ink(transition.morph_color);
            let tree = morph(pa.tree, pb.tree, p, &mut self.typeset, ink)
                .map_err(|e| self.morph_error(a.component, b.component, e))?;
            vec![tree]
        } else {
            let (painted, unused) = layers(
                look,
                transition.direction,
                p,
                pa.tree,
                pb.tree,
                size,
                self.theme.background,
            );
            if let Some(n) = unused {
                self.typeset.reclaim(n);
            }
            painted
        };
        self.compose(painted, logs, b.component)
    }
}

fn shares_a_key(a: &LNode, b: &LNode) -> bool {
    fn keys<'a>(n: &'a LNode, out: &mut HashSet<&'a str>) {
        out.extend(n.key.as_deref());
        for c in &n.children {
            keys(c, out);
        }
    }
    let (mut ka, mut kb) = (HashSet::new(), HashSet::new());
    keys(a, &mut ka);
    keys(b, &mut kb);
    !ka.is_disjoint(&kb)
}
