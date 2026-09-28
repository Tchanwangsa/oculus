//! The narrow Windows CLI door for Claude running in WSL.
//!
//! Requests arrive through the owned supervisor's pipes. No TCP listener or
//! Windows executable is exposed to the Linux sandbox, and argv is never shell
//! text. Library reads and planning writes use the same native CLI as the app;
//! authentication, syncing and recursive model jobs stay with the application.

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

const MAX_ARGUMENT_BYTES: usize = 64 * 1024;
const MAX_OUTPUT_BYTES: usize = 1024 * 1024;
const MAX_INPUT_BYTES: usize = 1024 * 1024;
static ACTIVE: AtomicUsize = AtomicUsize::new(0);

struct Permit;
impl Drop for Permit {
    fn drop(&mut self) { ACTIVE.fetch_sub(1, Ordering::SeqCst); }
}

fn validate_args(args: &[String]) -> Result<(), String> {
    if args.len() > 128 || args.iter().map(String::len).sum::<usize>() > MAX_ARGUMENT_BYTES {
        return Err("Oculus command is too large.".into());
    }
    if args.iter().any(|s| s.contains('\0')) {
        return Err("Oculus arguments cannot contain NUL.".into());
    }
    let mut words = args.iter().map(String::as_str).filter(|s| *s != "--json").peekable();
    let Some(command) = words.next() else { return Ok(()) };
    let route: Result<(), String> = match command {
        "--help" | "-h" | "--version" | "-V" if words.next().is_none() => Ok(()),
        "status" | "list" | "search" | "grep" | "read" | "files" | "calendar" => Ok(()),
        "project" => match words.next() {
            Some("list" | "show" | "create" | "update" | "--help" | "-h") => Ok(()),
            _ => Err("This project command is not available through the Claude bridge.".into()),
        },
        "task" => match words.next() {
            Some("list" | "add" | "update" | "move" | "refile" | "rm" | "--help" | "-h") => Ok(()),
            _ => Err("This task command is not available through the Claude bridge.".into()),
        },
        // Candidate detection is a bounded native media operation. The CLI
        // resolves the lecture id through the database, not an arbitrary path.
        "lecture" => match words.next() {
            Some("candidates" | "--help" | "-h") => Ok(()),
            _ => Err("Start lecture model jobs from Oculus, not from another agent turn.".into()),
        },
        _ => Err("The Claude bridge allows library queries and project/task commands. Use Oculus for sign-in, sync and model jobs.".into()),
    };
    route?;
    let flags: Vec<&str> = args.iter().map(String::as_str).take_while(|a| *a != "--").collect();
    if command == "list" && flags.contains(&"--refresh") {
        return Err("Refresh subjects from Oculus; agent queries use the local library.".into());
    }
    // Batch files are opened by the Linux shim inside its sandbox and passed
    // through stdin. Never give the native host CLI a model-supplied filename.
    for (index, flag) in flags.iter().enumerate() {
        if (*flag == "--batch" && flags.get(index + 1) != Some(&"-"))
            || flag.strip_prefix("--batch=").is_some_and(|value| value != "-") {
            return Err("Pass task batches through --batch - in the Claude bridge.".into());
        }
    }
    // Reject legacy process-wide memory overrides as well as current commands.
    if args.iter().take_while(|a| a.as_str() != "--")
        .any(|a| a == "--memory-cap" || a.starts_with("--memory-cap=")) {
        return Err("Change the memory budget in Oculus Settings.".into());
    }
    Ok(())
}

/// Called only by the bridge reader, on a worker thread. `alive` is cleared
/// before bridge shutdown so a pending command cannot outlive its session.
pub fn invoke(library: &Path, args: &[String], alive: &AtomicBool) -> Result<Value, String> {
    invoke_with_input(library, args, None, alive)
}

pub fn invoke_with_input(library: &Path, args: &[String], input: Option<&str>, alive: &AtomicBool) -> Result<Value, String> {
    validate_args(args)?;
    if input.is_some_and(|text| text.len() > MAX_INPUT_BYTES) {
        return Err("Oculus command input exceeds 1 MiB.".into());
    }
    if !alive.load(Ordering::SeqCst) { return Err("Claude session has stopped.".into()); }
    ACTIVE.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| (n < 4).then_some(n + 1))
        .map_err(|_| "Too many concurrent Oculus commands.".to_string())?;
    let _permit = Permit;
    let library = dunce::canonicalize(library).map_err(|e| format!("Cannot resolve library: {e}"))?;
    if library.file_name() != Some(std::ffi::OsStr::new(crate::paths::IDENTIFIER)) {
        return Err("Oculus CLI bridge requires the application's library directory.".into());
    }
    let parent = library.parent().ok_or("Library has no parent directory")?;
    let cwd = dunce::canonicalize(library.join("agents")).map_err(|e| format!("Cannot resolve agents: {e}"))?;
    if cwd.parent() != Some(library.as_path()) {
        return Err("Agent directory must be directly inside the library.".into());
    }
    let binary = super::discover::oculus_cli().ok_or("The bundled Oculus CLI is missing. Reinstall Oculus.")?;
    let mut cmd = crate::platform::command(binary);
    cmd.args(args)
        .current_dir(cwd)
        .env_clear()
        .envs(super::discover::child_env())
        .env("APPDATA", parent)
        .env("OCULUS_BROKER_QUERY_ONLY", "1")
        .env("NO_COLOR", "1");
    run_command(cmd, input, alive, Duration::from_secs(90))
}

fn read_bounded(mut stream: impl Read) -> String {
    let mut bytes = Vec::new();
    let mut block = [0_u8; 8192];
    let mut truncated = false;
    loop {
        match stream.read(&mut block) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let keep = n.min(MAX_OUTPUT_BYTES.saturating_sub(bytes.len()));
                bytes.extend_from_slice(&block[..keep]);
                truncated |= keep < n;
            }
        }
    }
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if truncated { text.push_str("\n[Oculus output truncated at 1 MiB]\n"); }
    text
}

fn run_command(mut cmd: Command, input: Option<&str>, alive: &AtomicBool, timeout: Duration) -> Result<Value, String> {
    let mut child = cmd.stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().map_err(|e| format!("Cannot start Oculus CLI: {e}"))?;
    let job = match crate::platform::ProcessJob::assign(&child) {
        Ok(job) => job,
        Err(e) => {
            let _ = child.kill(); let _ = child.wait();
            return Err(format!("Cannot supervise Oculus CLI: {e}"));
        }
    };
    let stdout = child.stdout.take().ok_or("Oculus CLI has no stdout")?;
    let stderr = child.stderr.take().ok_or("Oculus CLI has no stderr")?;
    let writer = child.stdin.take().map(|mut stdin| {
        let input = input.unwrap_or_default().as_bytes().to_vec();
        std::thread::spawn(move || { let _ = stdin.write_all(&input); })
    });
    let out = std::thread::spawn(move || read_bounded(stdout));
    let err = std::thread::spawn(move || read_bounded(stderr));
    let started = Instant::now();
    let (code, reason) = loop {
        match child.try_wait() {
            Ok(Some(status)) => break (status.code().unwrap_or(1), None),
            Err(e) => break (1, Some(format!("Cannot wait for Oculus CLI: {e}"))),
            Ok(None) => {}
        }
        if !alive.load(Ordering::SeqCst) {
            break (130, Some("Oculus command cancelled because its Claude session stopped.".to_string()));
        }
        if started.elapsed() >= timeout {
            break (124, Some("Oculus command exceeded its 90-second deadline.".to_string()));
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    // Closing the job also reaps descendants that retained either pipe.
    drop(job);
    let _ = child.kill();
    let _ = child.wait();
    if let Some(writer) = writer { let _ = writer.join(); }
    let stdout = out.join().unwrap_or_default();
    let mut stderr = err.join().unwrap_or_default();
    if let Some(reason) = reason {
        if !stderr.is_empty() { stderr.push('\n'); }
        stderr.push_str(&reason);
    }
    Ok(json!({"stdout": stdout, "stderr": stderr, "code": code}))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn argv(args: &[&str]) -> Vec<String> { args.iter().map(|s| (*s).to_owned()).collect() }

    #[test]
    fn broker_allows_queries_and_planning_but_not_host_administration() {
        for args in [vec![], vec!["--help"], vec!["--json", "files"],
            vec!["read", "lecture 学生.pdf"], vec!["project", "create", "Study"],
            vec!["task", "add", "Revise"], vec!["task", "refile", "12", "-p", "7"],
            vec!["task", "--json", "add", "--batch", "-"],
            vec!["lecture", "candidates", "abc", "--frames"]] {
            assert!(validate_args(&argv(&args)).is_ok(), "{args:?}");
        }
        for args in [vec!["auth", "setup"], vec!["run"], vec!["agent", "hello"],
            vec!["index"], vec!["docs"], vec!["lecture", "chapters", "abc"],
            vec!["--memory-cap", "5120", "status"], vec!["files", "--memory-cap=5120"],
            vec!["cmd.exe"], vec!["project", "unknown"], vec!["list", "--refresh"],
            vec!["task", "add", "--batch", "C:\\private.json"],
            vec!["task", "add", "--batch=/mnt/c/private.json"]] {
            assert!(validate_args(&argv(&args)).is_err(), "{args:?}");
        }
    }

    #[test]
    fn broker_treats_shell_syntax_as_literal_arguments_and_bounds_requests() {
        assert!(validate_args(&argv(&["grep", "$(whoami); & powershell 'x'"])).is_ok());
        assert!(validate_args(&argv(&["read", "a\0b"])).is_err());
        assert!(validate_args(&vec!["x".into(); 129]).is_err());
        assert!(validate_args(&["grep".into(), "x".repeat(MAX_ARGUMENT_BYTES)]).is_err());
    }

    #[test]
    fn broker_bounds_output_without_stopping_pipe_drain() {
        let data = vec![b'x'; MAX_OUTPUT_BYTES + 4096];
        let text = read_bounded(std::io::Cursor::new(data));
        assert!(text.starts_with(&"x".repeat(64)));
        assert!(text.ends_with("[Oculus output truncated at 1 MiB]\n"));
        assert!(text.len() < MAX_OUTPUT_BYTES + 100);
    }

    #[test]
    fn broker_stops_a_timed_out_native_child() {
        if std::env::var_os("OCULUS_BROKER_SLEEP_TEST").is_some() {
            std::thread::sleep(Duration::from_secs(15));
            return;
        }
        let mut cmd = crate::platform::command(std::env::current_exe().unwrap());
        cmd.args(["--exact", "harness::wsl_cli::tests::broker_stops_a_timed_out_native_child"])
            .env("OCULUS_BROKER_SLEEP_TEST", "1");
        let start = Instant::now();
        let response = run_command(cmd, None, &AtomicBool::new(true), Duration::from_millis(150)).unwrap();
        assert_eq!(response["code"], 124);
        assert!(start.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn broker_cancels_running_native_commands() {
        let mut cmd = crate::platform::command(std::env::current_exe().unwrap());
        cmd.args(["--exact", "harness::wsl_cli::tests::broker_stops_a_timed_out_native_child"])
            .env("OCULUS_BROKER_SLEEP_TEST", "1");
        let alive = std::sync::Arc::new(AtomicBool::new(true));
        let cancel = alive.clone();
        let trigger = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            cancel.store(false, Ordering::SeqCst);
        });
        let start = Instant::now();
        let response = run_command(cmd, None, &alive, Duration::from_secs(90)).unwrap();
        trigger.join().unwrap();
        assert_eq!(response["code"], 130);
        assert!(start.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn broker_streams_unicode_batch_input_larger_than_a_pipe_buffer() {
        if std::env::var_os("OCULUS_BROKER_INPUT_TEST").is_some() {
            let mut input = String::new();
            std::io::stdin().read_to_string(&mut input).unwrap();
            assert_eq!(input, "学生".repeat(20000));
            println!("OCULUS_BATCH_INPUT_OK");
            return;
        }
        let mut cmd = crate::platform::command(std::env::current_exe().unwrap());
        cmd.args(["--exact", "harness::wsl_cli::tests::broker_streams_unicode_batch_input_larger_than_a_pipe_buffer", "--nocapture"])
            .env("OCULUS_BROKER_INPUT_TEST", "1");
        let response = run_command(cmd, Some(&"学生".repeat(20000)), &AtomicBool::new(true), Duration::from_secs(5)).unwrap();
        assert_eq!(response["code"], 0);
        assert!(response["stdout"].as_str().unwrap().contains("OCULUS_BATCH_INPUT_OK"));
    }
}
