//! What a backend says about itself, and the preflight every parse goes through.

use super::{ParseError, Parser, PARSER_VERSION};
use serde::{Deserialize, Serialize};

/// What a backend says about itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Health {
    pub backend: String,
    pub parser_version: u32,
    pub ready: bool,
}

impl Health {
    /// The version handshake: a backend stamping another `parser_version` is
    /// refused, not warned about. Both backends render through the same
    /// `render` today, so that arm guards a future backend; `NotReady` is the
    /// local server still loading, which retries rather than failing the PDF.
    pub fn check(&self) -> Result<(), ParseError> {
        if self.parser_version != PARSER_VERSION {
            return Err(ParseError::VersionMismatch {
                app: PARSER_VERSION,
                backend: self.parser_version,
            });
        }
        if !self.ready {
            return Err(ParseError::NotReady {
                backend: self.backend.clone(),
            });
        }
        Ok(())
    }
}

/// Ask a backend whether it can be used; every call site about to parse
/// goes through here.
pub fn preflight(parser: &dyn Parser) -> Result<Health, ParseError> {
    let health = parser.health();
    health.check()?;
    Ok(health)
}
