//! Every `Math` in a scene tree -> its Box of keyed glyphs, before `select`
//! splits Morphs and before layout. A Math's pieces are keyed under its
//! own `key` (`eq/x`), so two formulas given one key match term to term,
//! and two Math elements elsewhere never collide. A Math without a key
//! inside a `<Morph>` is keyed by its place instead (`~m.n`: the nth
//! Math, in tree order, of that Morph's child), so the two sides of a
//! Morph match with no key at all; anywhere else its pieces have no keys.

use super::build;
use super::cache::MathCache;
use crate::scene::{Color, Kind, Node, NodeError};
use serde_json::{Map, Value};

/// What a formula's colours resolve against: the theme's text colour (a
/// Math without `color`) and its colour tokens (`\color{accent}`).
pub struct MathEnv<'a> {
    pub text: Color,
    pub colours: Option<&'a Map<String, Value>>,
}

pub fn expand(root: &mut Node, env: &MathEnv, cache: &mut MathCache) -> Result<(), NodeError> {
    if !has_math(root) {
        return Ok(());
    }
    let path = root.kind.name().to_owned();
    Walker {
        env,
        cache,
        morphs: 0,
        scope: None,
    }
    .visit(root, &path)
}

fn has_math(n: &Node) -> bool {
    n.kind == Kind::Math || n.children.iter().any(has_math)
}

struct Walker<'a, 'b> {
    env: &'a MathEnv<'b>,
    cache: &'a mut MathCache,
    /// Morphs met so far, in tree order (as `select` numbers them).
    morphs: u32,
    /// The Morph being walked and the Math elements met in its child.
    scope: Option<(u32, u32)>,
}

impl Walker<'_, '_> {
    fn visit(&mut self, n: &mut Node, path: &str) -> Result<(), NodeError> {
        if n.kind == Kind::Math {
            return self.math(n, path);
        }
        let morph = (n.kind == Kind::Morph).then(|| {
            self.morphs += 1;
            self.morphs - 1
        });
        let outer = self.scope;
        for (i, c) in n.children.iter_mut().enumerate() {
            if let Some(id) = morph {
                self.scope = Some((id, 0));
            }
            let p = format!("{path} > {}[{i}]", c.kind.name());
            self.visit(c, &p)?;
        }
        self.scope = outer;
        Ok(())
    }

    fn math(&mut self, n: &mut Node, path: &str) -> Result<(), NodeError> {
        let loc = n.loc;
        let fail = |message: String, is_style: bool| NodeError {
            message,
            path: path.to_owned(),
            loc,
            is_style,
        };
        let Some(spec) = n.math.take() else {
            return Err(fail("<Math> has no tex".to_owned(), false));
        };
        let prefix = n.key.clone().or_else(|| {
            let (id, k) = self.scope.as_mut()?;
            *k += 1;
            Some(format!("~{id}.{}", *k - 1))
        });
        let formula = self
            .cache
            .get(&spec.tex, spec.display)
            .map_err(|m| fail(format!("<Math tex={}>: {m}", shown(&spec.tex)), false))?;
        let built = build::node(n, &formula, prefix.as_deref(), self.env)
            .map_err(|(message, is_style)| fail(message, is_style))?;
        *n = built;
        Ok(())
    }
}

/// The source as a JS string literal, cut short.
fn shown(tex: &str) -> String {
    let cut: String = tex.chars().take(60).collect();
    let more = if tex.chars().count() > 60 { "…" } else { "" };
    format!("{:?}", format!("{cut}{more}"))
}
