//! What a backend says about itself, and the space handshake.

use serde::{Deserialize, Serialize};

use super::{EmbedError, Embedder, EMBED_DIM, EMBED_MODEL};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Health {
    pub backend: String,
    pub model: String,
    pub dim: usize,
    pub ready: bool,
}

impl Health {
    /// The space handshake: a backend in a different space is refused before a
    /// single vector is written. Two spaces in one `pages` table produce no
    /// error at all — every dot product returns a number and the ranking is
    /// noise — so this is stricter than `parse::Health::check`.
    pub fn check(&self) -> Result<(), EmbedError> {
        if self.model != EMBED_MODEL || self.dim != EMBED_DIM {
            return Err(EmbedError::ModelMismatch {
                app_model: EMBED_MODEL.to_string(),
                app_dim: EMBED_DIM,
                backend_model: self.model.clone(),
                backend_dim: self.dim,
            });
        }
        if !self.ready {
            return Err(EmbedError::NotReady {
                backend: self.backend.clone(),
            });
        }
        Ok(())
    }
}

/// Ask a backend whether it can be used, refusing one in another space. Every
/// call site about to embed goes through here.
pub fn preflight(embedder: &dyn Embedder) -> Result<Health, EmbedError> {
    let health = embedder.health();
    health.check()?;
    Ok(health)
}
