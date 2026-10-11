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
    /// The pause is a lockout or a rejected password, which only the app may
    /// override by hand. A record that does not say counts as one.
    pub(super) credentials_paused: bool,
    /// Failed manual attempts since the last success or newly saved credentials.
    pub(super) manual_failures: u32,
    /// Newly saved credentials forgave the wait after `last`, so the next
    /// attempt need not sit it out (the spacing still applies).
    pub(super) forgiven: bool,
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
        let count = |key: &str, default: u64| match object.get(key) {
            None => Ok(default),
            Some(_) => number(key),
        };
        let small =
            |key: &str, n: u64| u32::try_from(n).map_err(|_| format!("`{key}` is too large"));
        let flag = |key: &str, default: bool| match object.get(key) {
            None => Ok(default),
            Some(serde_json::Value::Bool(b)) => Ok(*b),
            Some(_) => Err(format!("`{key}` is not true or false")),
        };
        let paused = match object.get("paused") {
            None | Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::String(why)) => Some(why.clone()),
            Some(_) => return Err("`paused` is not text".to_string()),
        };
        let credentials_paused = flag("credentials_paused", paused.is_some())?;
        Ok(AttemptRecord {
            last: number("last")?,
            failures: small("failures", number("failures")?)?,
            paused,
            credentials_paused,
            manual_failures: small("manual_failures", count("manual_failures", 0)?)?,
            forgiven: flag("forgiven", false)?,
            damaged: None,
        })
    }

    pub(super) fn to_json(&self) -> String {
        format!(
            r#"{{"last":{},"failures":{},"paused":{},"credentials_paused":{},"manual_failures":{},"forgiven":{}}}"#,
            self.last,
            self.failures,
            serde_json::to_string(&self.paused).unwrap_or_else(|_| "null".to_string()),
            self.credentials_paused,
            self.manual_failures,
            self.forgiven
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
            credentials_paused: true,
            manual_failures: 2,
            forgiven: true,
            damaged: None,
        };
        let back = AttemptRecord::from_json(&r.to_json()).unwrap();
        assert_eq!((back.last, back.failures), (77, 3));
        assert_eq!(back.paused.as_deref(), Some("locked \"out\""));
        assert_eq!((back.credentials_paused, back.manual_failures), (true, 2));
        assert!(back.forgiven);

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
            (
                r#"{"last":1,"failures":1,"credentials_paused":"yes"}"#,
                "`credentials_paused`",
            ),
            (
                r#"{"last":1,"failures":1,"manual_failures":"2"}"#,
                "`manual_failures`",
            ),
            (r#"{"last":1,"failures":1,"forgiven":1}"#, "`forgiven`"),
        ] {
            let Err(why) = AttemptRecord::from_json(text) else {
                panic!("{text:?} read as a record");
            };
            assert!(why.contains(fault), "{text:?}: {why}");
        }
    }

    #[test]
    fn a_record_from_before_the_manual_rules_reads_with_their_defaults() {
        let old = AttemptRecord::from_json(r#"{"last":5,"failures":2,"paused":null}"#).unwrap();
        assert_eq!((old.manual_failures, old.credentials_paused), (0, false));
        assert!(!old.forgiven);
        // An old pause does not say what caused it, so it counts as the hard kind.
        let old = AttemptRecord::from_json(r#"{"last":5,"failures":2,"paused":"x"}"#).unwrap();
        assert!(old.credentials_paused);
    }

    #[test]
    fn the_fields_an_older_reader_needs_are_still_written() {
        let json = AttemptRecord {
            last: 1,
            failures: 2,
            ..AttemptRecord::default()
        }
        .to_json();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["last"], 1);
        assert_eq!(value["failures"], 2);
        assert!(value["paused"].is_null());
    }
}
