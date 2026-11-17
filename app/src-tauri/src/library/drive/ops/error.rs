//! The typed refusal every folder command returns, JSON-serialised into the
//! command's error string (`parseDriveError` in `app/src/lib/drive/commands.ts`).

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// The path is synced (Course) or app state: the student may not change it.
    ReadOnly,
    /// Something already has the name.
    Exists,
    NotFound,
    InvalidName,
    /// Anything else: the disk, the database, or a file busy being parsed.
    Io,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DriveError {
    pub kind: Kind,
    pub message: String,
}

pub type DriveResult<T> = Result<T, DriveError>;

impl DriveError {
    pub fn new(kind: Kind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn read_only(rel: &str) -> Self {
        Self::new(Kind::ReadOnly, format!("{} is read-only", shown(rel)))
    }

    pub fn exists(rel: &str) -> Self {
        Self::new(Kind::Exists, format!("{} already exists there", shown(rel)))
    }

    pub fn not_found(rel: &str) -> Self {
        Self::new(Kind::NotFound, format!("{} is not there", shown(rel)))
    }

    pub fn invalid_name(message: impl Into<String>) -> Self {
        Self::new(Kind::InvalidName, message)
    }

    pub fn io(message: impl Into<String>) -> Self {
        Self::new(Kind::Io, message)
    }
}

/// The last segment of a path: what the student knows an entry by.
pub fn shown(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel)
}

impl From<DriveError> for String {
    fn from(error: DriveError) -> String {
        serde_json::to_string(&error).unwrap_or(error.message)
    }
}

impl From<std::io::Error> for DriveError {
    fn from(error: std::io::Error) -> Self {
        Self::io(error.to_string())
    }
}
