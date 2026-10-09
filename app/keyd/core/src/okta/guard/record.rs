//! The attempt record on disk: `canvas-session/sign-in.json`, read and
//! replaced under a lock of its own, so two processes cannot both decide to
//! sign in and a crash cannot leave half a record.

use std::path::Path;

use crate::paths;
use crate::platform::files;

#[derive(Default)]
pub(in crate::okta) struct AttemptRecord {
    /// Unix seconds when the last attempt started.
    pub(super) last: u64,
    /// Failed attempts since the last success; sets the wait.
    pub(super) failures: u32,
    /// Why automatic sign-in is paused, if it is.
    pub(super) paused: Option<String>,
    /// Set when the file exists but is not a record: what is wrong with it.
    /// Never saved, so only a successful repair (`guard::admit` for a manual
    /// attempt, `resume_automatic_sign_in`) replaces the file.
    pub(super) damaged: Option<String>,
}

impl AttemptRecord {
    /// `Err` says why `text` is not a record. Empty, truncated, mistyped or
    /// incomplete text is damage, not a blank record: a blank one would
    /// forget a pause.
    pub(super) fn from_json(text: &str) -> Result<AttemptRecord, String> {
        if text.trim().is_empty() {
            return Err("it is empty".to_string());
        }
        let value = serde_json::from_str::<serde_json::Value>(text)
            .map_err(|_| "it is not valid JSON".to_string())?;
        let object = value
            .as_object()
            .ok_or_else(|| "it is not a JSON object".to_string())?;
        let number = |key: &str| {
            object
                .get(key)
                .and_then(|v| v.as_u64())
                .ok_or_else(|| format!("`{key}` is missing or not a number"))
        };
        let paused = match object.get("paused") {
            None | Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::String(why)) => Some(why.clone()),
            Some(_) => return Err("`paused` is not text".to_string()),
        };
        Ok(AttemptRecord {
            last: number("last")?,
            failures: u32::try_from(number("failures")?)
                .map_err(|_| "`failures` is too large".to_string())?,
            paused,
            damaged: None,
        })
    }

    pub(super) fn to_json(&self) -> String {
        format!(
            r#"{{"last":{},"failures":{},"paused":{}}}"#,
            self.last,
            self.failures,
            serde_json::to_string(&self.paused).unwrap_or_else(|_| "null".to_string())
        )
    }
}

/// Runs `f` on the record under an exclusive lock and saves what `f` left.
/// `Err` says why the record could not be locked, read or saved; `f` has not
/// run for the first two. A missing file is a blank record (nothing has been
/// attempted); a file that is not a record reaches `f` as `damaged`, and is
/// left as it is unless `f` repairs it.
pub(in crate::okta) fn with_record<T>(
    data_dir: &Path,
    f: impl FnOnce(&mut AttemptRecord) -> T,
) -> Result<T, String> {
    let path = paths::sign_in_record(data_dir);
    let lock = paths::sign_in_lock(data_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let fail =
        |what: &str, p: &Path, e: &dyn std::fmt::Display| format!("{what} {}: {e}", p.display());
    let _lock = files::lock(&lock).map_err(|e| fail("locking", &lock, &e))?;

    let (text, parsed) = match std::fs::read_to_string(&path) {
        Ok(text) => {
            let parsed = AttemptRecord::from_json(&text);
            (Some(text), parsed)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (None, Ok(AttemptRecord::default())),
        Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
            (None, Err("it is not text".to_string()))
        }
        Err(e) => return Err(fail("reading", &path, &e)),
    };
    let mut record = parsed.unwrap_or_else(|why| AttemptRecord {
        damaged: Some(format!("{} is damaged ({why})", path.display())),
        ..AttemptRecord::default()
    });
    let out = f(&mut record);
    if record.damaged.is_none() {
        let body = record.to_json();
        if text.as_deref() != Some(body.as_str()) {
            save(&path, &body).map_err(|e| fail("saving", &path, &e))?;
        }
    }
    Ok(out)
}

/// Replaces the record with `body` in one rename, after the new bytes are on
/// disk, so the file is always the old record or the new one.
fn save(path: &Path, body: &str) -> Result<(), String> {
    files::replace_file(path, |tmp| {
        files::write_private(tmp, body.as_bytes())?;
        std::fs::File::open(tmp)?.sync_all()
    })?;
    if let Some(dir) = path.parent() {
        files::sync_dir(dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_record_reads_back_what_it_wrote() {
        let mut r = AttemptRecord {
            last: 77,
            failures: 3,
            paused: Some("locked \"out\"".into()),
            damaged: None,
        };
        let back = AttemptRecord::from_json(&r.to_json()).unwrap();
        assert_eq!((back.last, back.failures), (77, 3));
        assert_eq!(back.paused.as_deref(), Some("locked \"out\""));

        r.paused = None;
        assert!(AttemptRecord::from_json(&r.to_json())
            .unwrap()
            .paused
            .is_none());
    }

    #[test]
    fn text_that_is_not_a_record_is_an_error_naming_the_fault() {
        for (text, fault) in [
            ("", "empty"),
            ("{", "not valid JSON"),
            ("[]", "not a JSON object"),
            (r#"{"last":1}"#, "`failures`"),
            (r#"{"last":"x","failures":1}"#, "`last`"),
            (r#"{"last":1,"failures":9999999999}"#, "too large"),
            (r#"{"last":1,"failures":1,"paused":[]}"#, "`paused`"),
        ] {
            let Err(why) = AttemptRecord::from_json(text) else {
                panic!("{text:?} read as a record");
            };
            assert!(why.contains(fault), "{text:?}: {why}");
        }
    }
}
