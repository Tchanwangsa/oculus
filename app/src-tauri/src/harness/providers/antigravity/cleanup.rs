//! Taking back the permission rules earlier versions of Oculus wrote into
//! the student's own agy settings (`~/.gemini/antigravity-cli/settings.json`,
//! `permissions.allow` and `deny`). Those versions recorded exactly what they
//! wrote in `<library>/antigravity-rules.json`; only those entries go, every
//! other key, entry and its order stays, and the record is deleted once the
//! file no longer holds them. With no record nothing can be told apart as
//! Oculus's, so nothing is touched. Runs at each launch until the record is
//! gone; the student's approvals row (`antigravity_allowed_rules`) goes too.
//! docs/harness.md, "Antigravity asks over ACP".

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::json::{Node, Ordered};

/// The record earlier versions kept at the library root.
pub const RECORD: &str = "antigravity-rules.json";

/// The settings row that held the student's approvals.
pub const APPROVALS_KEY: &str = "antigravity_allowed_rules";

/// What the record says Oculus wrote.
#[derive(Debug, Default, Deserialize)]
struct Record {
    #[serde(default)]
    allow: Vec<String>,
    #[serde(default)]
    deny: Vec<String>,
}

/// What one run did.
#[derive(Debug, PartialEq, Eq)]
pub enum Cleaned {
    /// No record: nothing Oculus wrote is known, so nothing was touched.
    NoRecord,
    /// The record is gone, and this many entries with it.
    Removed(usize),
}

/// The agy settings file under the home folder `home`.
fn settings_path(home: &Path) -> PathBuf {
    home.join(".gemini/antigravity-cli/settings.json")
}

/// Take Oculus's rules out of the student's agy settings, for the library at
/// `library`.
pub fn run(library: &Path) -> Result<Cleaned, String> {
    let home = dirs::home_dir().ok_or("the home directory cannot be found")?;
    run_in(&home, library)
}

/// [`run`] for the home folder `home`: tests give it a scratch one.
pub(super) fn run_in(home: &Path, library: &Path) -> Result<Cleaned, String> {
    let settings = settings_path(home);
    let record = library.join(RECORD);
    let recorded = match std::fs::read_to_string(&record) {
        Ok(text) => serde_json::from_str::<Record>(&text)
            .map_err(|e| format!("{} does not read as a record: {e}", record.display()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Cleaned::NoRecord),
        Err(e) => return Err(format!("cannot read {}: {e}", record.display())),
    };
    let removed = remove(&settings, &recorded)?;
    std::fs::remove_file(&record)
        .map_err(|e| format!("cannot remove {}: {e}", record.display()))?;
    Ok(Cleaned::Removed(removed))
}

/// Drop the recorded entries from the settings file; how many went.
fn remove(path: &Path, recorded: &Record) -> Result<usize, String> {
    // Rename onto the file, not a dotfiles symlink.
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
    };
    if text.trim().is_empty() {
        return Ok(0);
    }
    // Invalid JSON is the student's to fix: it is never overwritten.
    let refuse = |why: String| format!("{} is not valid JSON ({why})", path.display());
    let Ordered(entries) = serde_json::from_str(&text).map_err(|e| refuse(e.to_string()))?;
    let mut top: Vec<(String, Node)> = entries
        .into_iter()
        .map(|(k, v)| (k, Node::Raw(v)))
        .collect();
    let Some(at) = top.iter().position(|(k, _)| k == "permissions") else {
        return Ok(0);
    };
    let Node::Raw(raw) = &top[at].1 else {
        return Ok(0);
    };
    let Ok(Ordered(entries)) = serde_json::from_str::<Ordered>(raw.get()) else {
        return Ok(0);
    };
    let mut perms: Vec<(String, Node)> = Vec::new();
    let mut removed = 0;
    for (key, value) in entries {
        let ours: HashSet<&str> = match key.as_str() {
            "allow" => recorded.allow.iter().map(String::as_str).collect(),
            "deny" => recorded.deny.iter().map(String::as_str).collect(),
            _ => HashSet::new(),
        };
        let list = (!ours.is_empty())
            .then(|| serde_json::from_str::<Vec<String>>(value.get()).ok())
            .flatten();
        let Some(list) = list else {
            perms.push((key, Node::Raw(value)));
            continue;
        };
        let kept: Vec<String> = list
            .iter()
            .filter(|e| !ours.contains(e.as_str()))
            .cloned()
            .collect();
        removed += list.len() - kept.len();
        // A list that held only Oculus's entries goes with them.
        match (kept.is_empty(), kept.len() == list.len()) {
            (_, true) => perms.push((key, Node::Raw(value))),
            (true, false) => {}
            (false, false) => perms.push((key, Node::Value(Value::from(kept)))),
        }
    }
    if removed == 0 {
        return Ok(0);
    }
    if perms.is_empty() {
        top.remove(at);
    } else {
        top[at].1 = Node::Map(perms);
    }
    let mut body = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(
        &mut body,
        serde_json::ser::PrettyFormatter::with_indent(b"  "),
    );
    Node::Map(top)
        .serialize(&mut ser)
        .map_err(|e| e.to_string())?;
    body.push(b'\n');
    write_atomic(&path, &body).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(removed)
}

/// Temp file beside the target, then a rename; permission bits carried over.
fn write_atomic(path: &Path, body: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".oculus-{}.tmp", std::process::id()));
    let tmp = PathBuf::from(tmp);
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(body)?;
        f.sync_all()?;
    }
    if let Ok(meta) = std::fs::metadata(path) {
        let _ = std::fs::set_permissions(&tmp, meta.permissions());
    }
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// Forget the student's approvals row.
pub async fn forget_approvals(pool: &sqlx::SqlitePool) -> Result<(), String> {
    sqlx::query("DELETE FROM settings WHERE key = ?1")
        .bind(APPROVALS_KEY)
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch home with an agy settings file holding `settings`, and a
    /// library whose record holds `record`. Never the student's own home.
    fn home(settings: Option<&str>, record: Option<&str>) -> (PathBuf, PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "oculus-agy-cleanup-{}-{}",
            std::process::id(),
            rand_suffix()
        ));
        let home = dir.join("home");
        let library = dir.join("library");
        std::fs::create_dir_all(home.join(".gemini/antigravity-cli")).unwrap();
        std::fs::create_dir_all(&library).unwrap();
        let file = settings_path(&home);
        assert!(
            file.starts_with(std::env::temp_dir()),
            "a scratch home only"
        );
        if let Some(s) = settings {
            std::fs::write(&file, s).unwrap();
        }
        if let Some(r) = record {
            std::fs::write(library.join(RECORD), r).unwrap();
        }
        (dir, home, library)
    }

    fn rand_suffix() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        N.fetch_add(1, Ordering::SeqCst)
    }

    const SETTINGS: &str = r#"{
  "colorScheme": "dark",
  "permissions": {
    "allow": ["command(git)", "command(oculus)", "write_file(/lib/studio)"],
    "ask": ["command(rm)"],
    "deny": ["write_file(/lib/courses)", "read_file(/secret)"]
  },
  "trustedWorkspaces": ["/code"]
}
"#;

    const RECORDED: &str = r#"{
  "allow": ["command(oculus)", "write_file(/lib/studio)"],
  "deny": ["write_file(/lib/courses)"],
  "approved": ["command(python3)"]
}"#;

    #[test]
    fn only_the_recorded_entries_go_and_everything_else_stays_in_its_order() {
        let (dir, home, library) = home(Some(SETTINGS), Some(RECORDED));
        assert_eq!(run_in(&home, &library), Ok(Cleaned::Removed(3)));
        let after = std::fs::read_to_string(settings_path(&home)).unwrap();
        assert_eq!(
            after,
            r#"{
  "colorScheme": "dark",
  "permissions": {
    "allow": [
      "command(git)"
    ],
    "ask": ["command(rm)"],
    "deny": [
      "read_file(/secret)"
    ]
  },
  "trustedWorkspaces": ["/code"]
}
"#,
            "the student's entries, keys and their order are kept; untouched values keep their spelling"
        );
        assert!(!library.join(RECORD).exists(), "the record goes");
        assert_eq!(run_in(&home, &library), Ok(Cleaned::NoRecord), "once");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn lists_that_held_only_oculuss_entries_go_and_so_does_an_emptied_permissions() {
        let only_ours = r#"{"theme": 1, "permissions": {"allow": ["command(oculus)"], "deny": ["write_file(/lib/courses)"]}}"#;
        let (dir, home, library) = home(Some(only_ours), Some(RECORDED));
        assert_eq!(run_in(&home, &library), Ok(Cleaned::Removed(2)));
        let after: Value =
            serde_json::from_str(&std::fs::read_to_string(settings_path(&home)).unwrap()).unwrap();
        assert_eq!(after, serde_json::json!({ "theme": 1 }));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn with_no_record_nothing_is_touched() {
        let (dir, home, library) = home(Some(SETTINGS), None);
        assert_eq!(run_in(&home, &library), Ok(Cleaned::NoRecord));
        assert_eq!(
            std::fs::read_to_string(settings_path(&home)).unwrap(),
            SETTINGS
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_file_with_none_of_the_entries_is_not_rewritten_and_the_record_goes() {
        let theirs = r#"{"permissions":{"allow":["command(git)"]}}"#;
        let (dir, home, library) = home(Some(theirs), Some(RECORDED));
        assert_eq!(run_in(&home, &library), Ok(Cleaned::Removed(0)));
        assert_eq!(
            std::fs::read_to_string(settings_path(&home)).unwrap(),
            theirs
        );
        assert!(!library.join(RECORD).exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn no_settings_file_needs_nothing_and_the_record_goes() {
        let (dir, home, library) = home(None, Some(RECORDED));
        assert_eq!(run_in(&home, &library), Ok(Cleaned::Removed(0)));
        assert!(!settings_path(&home).exists(), "nothing is created");
        assert!(!library.join(RECORD).exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn invalid_json_is_never_overwritten_and_the_record_stays_for_next_time() {
        let broken = "{ \"permissions\": ";
        let (dir, home, library) = home(Some(broken), Some(RECORDED));
        assert!(run_in(&home, &library).is_err());
        assert_eq!(
            std::fs::read_to_string(settings_path(&home)).unwrap(),
            broken
        );
        assert!(library.join(RECORD).exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_settings_file_is_rewritten_at_its_target_with_its_mode() {
        use std::os::unix::fs::PermissionsExt;
        let (dir, home, library) = home(None, Some(RECORDED));
        let target = dir.join("dotfiles-settings.json");
        std::fs::write(&target, SETTINGS).unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::os::unix::fs::symlink(&target, settings_path(&home)).unwrap();
        assert_eq!(run_in(&home, &library), Ok(Cleaned::Removed(3)));
        assert!(std::fs::symlink_metadata(settings_path(&home))
            .unwrap()
            .file_type()
            .is_symlink());
        let after = std::fs::read_to_string(&target).unwrap();
        assert!(after.contains("command(git)") && !after.contains("command(oculus)"));
        let mode = std::fs::metadata(&target).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[cfg(test)]
mod source {
    /// Every Rust file under `dir`, with its text.
    fn files(dir: &std::path::Path, out: &mut Vec<(std::path::PathBuf, String)>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                files(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push((path.clone(), std::fs::read_to_string(&path).unwrap()));
            }
        }
    }

    /// The student's agy settings file is named in one place, this module,
    /// which only takes Oculus's old entries out of it.
    #[test]
    fn nothing_else_names_the_students_agy_settings_file() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut all = Vec::new();
        files(&src, &mut all);
        let naming: Vec<_> = all
            .iter()
            .filter(|(_, text)| text.contains("antigravity-cli/settings"))
            .map(|(path, _)| path.strip_prefix(&src).unwrap().display().to_string())
            .collect();
        assert_eq!(naming, ["harness/providers/antigravity/cleanup.rs"]);
    }
}
