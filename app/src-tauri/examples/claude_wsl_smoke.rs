//! Opt-in real Claude/WSL integration smoke, using only a new synthetic library.
//! Build this example and `oculus`, then copy both executables into the same
//! tool directory before running `claude_wsl_smoke.exe <new artifact directory>`.
//! This reproduces the installed app's adjacent CLI discovery and uses Claude
//! subscription quota. It is deliberately not part of the automated test suite.
use app_lib::harness::{Harness, HarnessEvent, Provider, SendOptions, Sink};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

struct Shutdown(Harness);
impl Drop for Shutdown { fn drop(&mut self) { self.0.shutdown(); } }

#[derive(Default)]
struct Turn {
    session: String,
    text: String,
    anchor: String,
    status: String,
    tool_count: usize,
    deltas: usize,
    cli_succeeded: bool,
}

struct Events { sink: Sink, receiver: mpsc::Receiver<HarnessEvent> }
fn events() -> Events {
    let (tx, rx) = mpsc::channel();
    let sink: Sink = Arc::new(move |event| { let _ = tx.send(event); });
    Events { sink, receiver: rx }
}

fn turn(harness: &Harness, events: &Events, resume: Option<&str>, prompt: &str, interrupt: bool) -> Result<Turn, String> {
    let opts = SendOptions { model: Some("sonnet".into()), reasoning_effort: Some("low".into()), ..Default::default() };
    harness.send(901, Provider::Claude, resume, &opts, prompt, events.sink.clone())?;
    let start = Instant::now();
    let mut result = Turn::default();
    let mut cli_tools = std::collections::HashSet::new();
    let mut interrupted = false;
    loop {
        let remaining = Duration::from_secs(240).checked_sub(start.elapsed()).ok_or("Claude smoke turn timed out")?;
        let event = events.receiver.recv_timeout(remaining).map_err(|e| format!("Claude smoke stream: {e}"))?;
        println!("{}", serde_json::to_string(&event).unwrap());
        match event {
            HarnessEvent::SessionStarted { provider_session_id, .. } => result.session = provider_session_id,
            HarnessEvent::AssistantDelta { .. } => result.deltas += 1,
            HarnessEvent::AssistantMessage { text } => { result.text.push_str(&text); result.text.push('\n'); }
            HarnessEvent::TurnAnchor { anchor } => result.anchor = anchor,
            HarnessEvent::ToolStarted { id, name, input, kind, .. } => {
                result.tool_count += 1;
                if kind == app_lib::harness::ToolKind::OculusCli { cli_tools.insert(id); }
                if interrupt && !interrupted && name == "Bash" && input.to_string().contains("sleep") {
                    std::thread::sleep(Duration::from_millis(800));
                    harness.interrupt(901)?;
                    interrupted = true;
                }
            }
            HarnessEvent::ToolFinished { id, ok, output, .. } => {
                if cli_tools.contains(&id) && ok && output.contains("oculus 0.1.0") { result.cli_succeeded = true; }
            }
            HarnessEvent::TurnFinished { status } => { result.status = status; break; }
            HarnessEvent::Error { message, .. } => return Err(message),
            HarnessEvent::Exited { code } => return Err(format!("Claude exited before turn completion: {code:?}")),
            _ => {}
        }
    }
    if interrupt && !interrupted { return Err("Claude never started the interrupt test command".into()); }
    Ok(result)
}

fn run(root: &Path) -> Result<(), String> {
    let library = root.join("com.tchan.oculus");
    if library.exists() { return Err("Use a fresh artifact directory for the smoke test.".into()); }
    std::fs::create_dir_all(library.join("agents")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(library.join("courses/SMOKE")).map_err(|e| e.to_string())?;
    let marker = "OCULUS_WSL_STUDY_7391";
    std::fs::write(library.join("courses/SMOKE/lecture 学生.txt"), format!("Synthetic lecture marker: {marker}\n")).map_err(|e| e.to_string())?;
    let harness = Shutdown(Harness::new(library.clone()));
    let first_events = events();
    let first = turn(&harness.0, &first_events, None,
        "This is an isolated integration test with synthetic files. Read ../courses/SMOKE/lecture 学生.txt, write its marker into ./verified.txt, and run `oculus --version` using Bash. Reply with the marker followed by BRIDGE_OK. Do not inspect any other library or system files.", false)?;
    if first.status != "completed" || !first.text.contains(marker) || !first.text.contains("BRIDGE_OK") || first.deltas == 0 || !first.cli_succeeded {
        return Err("Initial streamed Claude response did not match the fixture.".into());
    }
    if !std::fs::read_to_string(library.join("agents/verified.txt")).map_err(|e| e.to_string())?.contains(marker) {
        return Err("Claude's allowed agents/ write was missing.".into());
    }
    if first.session.is_empty() || first.anchor.is_empty() { return Err("Session id or transcript rewind anchor was missing.".into()); }
    harness.0.close(901);
    let resumed_events = events();
    let resumed = turn(&harness.0, &resumed_events, Some(&first.session),
        "Without using tools, repeat the synthetic lecture marker from our previous turn and append RESUME_OK.", false)?;
    if resumed.status != "completed" || resumed.session != first.session || !resumed.text.contains(marker) || !resumed.text.contains("RESUME_OK") {
        return Err("Claude session resume failed.".into());
    }
    if resumed.anchor.is_empty() { return Err("Resumed turn has no rewind anchor.".into()); }
    harness.0.rewind(901, Provider::Claude, Some(&first.session),
        &SendOptions { model: Some("sonnet".into()), reasoning_effort: Some("low".into()), ..Default::default() },
        &resumed.anchor, resumed_events.sink.clone())?;
    let interrupted = turn(&harness.0, &resumed_events, None,
        "For the cancellation test, use Bash to run exactly: python3 -c \"import time; time.sleep(20); open('should-not-exist.txt','w').write('late')\". Wait for the command before replying.", true)?;
    if interrupted.status != "interrupted" { return Err(format!("Expected interrupted turn, got {}", interrupted.status)); }
    let after = turn(&harness.0, &resumed_events, None, "Reply with exactly AFTER_INTERRUPT_OK and do not use tools.", false)?;
    if after.status != "completed" || !after.text.contains("AFTER_INTERRUPT_OK") { return Err("Session did not recover after interrupt.".into()); }
    if library.join("agents/should-not-exist.txt").exists() { return Err("Interrupted tool wrote its late marker.".into()); }
    harness.0.shutdown();
    println!("{}", serde_json::json!({"smoke":"passed", "library":library, "streamed_deltas":first.deltas,
        "tool_count":first.tool_count, "resume":true, "interrupt":true, "rewind":true, "transcript_anchor":true}));
    Ok(())
}

fn main() {
    let Some(root) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("Pass a new artifact directory."); std::process::exit(2);
    };
    if let Err(error) = run(&root) { eprintln!("Claude WSL smoke failed: {error}"); std::process::exit(1); }
}
