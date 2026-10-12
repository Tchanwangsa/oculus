//! Native subprocesses hide console windows; agent children own a kill-on-close job.
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
mod job;
#[cfg(windows)]
pub use job::ProcessJob;
#[cfg(not(windows))]
pub struct ProcessJob;

pub fn command(program: impl AsRef<OsStr>) -> Command {
    let cmd = Command::new(program);
    #[cfg(windows)]
    let cmd = {
        use std::os::windows::process::CommandExt;
        let mut cmd = cmd;
        cmd.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
        cmd
    };
    cmd
}

pub fn canonicalize(path: impl AsRef<Path>) -> std::io::Result<PathBuf> {
    #[cfg(windows)]
    {
        dunce::canonicalize(path)
    }
    #[cfg(not(windows))]
    {
        std::fs::canonicalize(path)
    }
}

/// Windows batch shims never receive agent-controlled arguments. Resolve only
/// a package.json bin entry beside an npm shim, and pass it directly to node.
pub fn cli_command(bin: &Path) -> Result<Command, String> {
    #[cfg(windows)]
    if bin
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"))
    {
        let script = npm_script(bin)?;
        let node = bin
            .parent()
            .map(|p| p.join("node.exe"))
            .filter(|p| p.is_file())
            .or_else(|| crate::harness::cli::discover::tool("node.exe"))
            .ok_or("an npm shim needs node.exe, which was not found")?;
        let mut cmd = command(node);
        cmd.arg(script);
        return Ok(cmd);
    }
    Ok(command(bin))
}

#[cfg(any(windows, test))]
pub fn npm_script(bin: &Path) -> Result<PathBuf, String> {
    let name = bin
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("npm shim has no name")?;
    let package = match name.to_ascii_lowercase().as_str() {
        "codex" => "@openai/codex",
        "opencode" => "opencode-ai",
        "claude" => "@anthropic-ai/claude-code",
        "npm" => "npm",
        _ => {
            return Err(format!(
                "unsupported batch shim {}: use a native executable",
                bin.display()
            ))
        }
    };
    let root = bin
        .parent()
        .ok_or("npm shim has no parent")?
        .join("node_modules")
        .join(package);
    let manifest = std::fs::read(root.join("package.json"))
        .map_err(|e| format!("cannot resolve npm shim {}: {e}", bin.display()))?;
    let manifest: serde_json::Value =
        serde_json::from_slice(&manifest).map_err(|e| e.to_string())?;
    let entry = manifest
        .get("bin")
        .and_then(|v| v.as_str().or_else(|| v.get(name).and_then(|v| v.as_str())))
        .ok_or_else(|| format!("{package} has no bin entry for {name}"))?;
    let root = canonicalize(&root).map_err(|e| e.to_string())?;
    let script =
        canonicalize(root.join(entry)).map_err(|e| format!("npm bin entry is unavailable: {e}"))?;
    if !script.starts_with(&root) || !script.is_file() {
        return Err("npm bin entry escapes its package or is not a file".into());
    }
    Ok(script)
}

pub fn spawn_agent(cmd: &mut Command) -> Result<(std::process::Child, ProcessJob), String> {
    #[cfg(windows)]
    {
        job::spawn(cmd)
    }
    #[cfg(not(windows))]
    {
        cmd.spawn()
            .map(|c| (c, ProcessJob))
            .map_err(|e| e.to_string())
    }
}

#[cfg(not(windows))]
impl ProcessJob {
    pub fn terminate(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn npm_shims_resolve_only_a_known_package_bin() {
        let dir = crate::test_support::Scratch::new("npm-shim");
        let root = dir.join("node_modules/@openai/codex");
        std::fs::create_dir_all(root.join("bin")).unwrap();
        std::fs::write(root.join("bin/codex.js"), "run").unwrap();
        std::fs::write(
            root.join("package.json"),
            r#"{"bin":{"codex":"bin/codex.js"}}"#,
        )
        .unwrap();
        assert_eq!(
            npm_script(&dir.join("codex.cmd")).unwrap(),
            canonicalize(root.join("bin/codex.js")).unwrap()
        );
        std::fs::write(dir.join("outside.js"), "outside").unwrap();
        std::fs::write(
            root.join("package.json"),
            r#"{"bin":{"codex":"../../../outside.js"}}"#,
        )
        .unwrap();
        assert!(npm_script(&dir.join("codex.cmd")).is_err());
        assert!(npm_script(&dir.join("untrusted.cmd")).is_err());
    }
}
