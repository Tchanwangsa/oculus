//! Windows-to-WSL2 Claude transport. Native Windows Claude is never a fallback.
//! Arguments are passed directly to wsl.exe; prompts/settings travel over stdin
//! as JSON, never through a shell or the Windows command-line length limit.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{json, Value};

use super::claude::ClaudeSpawn;

const SUPERVISOR: &str = include_str!("wsl_bridge.py");
pub const DISTRO_ENV: &str = "OCULUS_CLAUDE_WSL_DISTRO";

#[derive(Clone, Debug, Deserialize)]
pub struct Bridge {
    #[serde(skip)]
    pub launcher: PathBuf,
    #[serde(skip)]
    pub distro: String,
    pub home: String,
    pub config: String,
    pub claude: String,
    pub version: String,
    pub logged_in: bool,
}

#[derive(Debug, PartialEq)]
struct Distribution {
    name: String,
    version: u8,
    default: bool,
}

fn decode_wsl(bytes: &[u8]) -> String {
    // WSL list output is UTF-16LE; output from Linux --exec is UTF-8.
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.iter().skip(1).step_by(2).take(12).any(|b| *b == 0) {
        let skip = usize::from(bytes.starts_with(&[0xff, 0xfe])) * 2;
        String::from_utf16_lossy(&bytes[skip..].chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).collect::<Vec<_>>())
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

fn parse_distributions(text: &str, names: &[String]) -> Vec<Distribution> {
    text.lines().filter_map(|line| {
        let line = line.trim().trim_start_matches('\u{feff}');
        let default = line.starts_with('*');
        let line = line.trim_start_matches('*').trim();
        // The status column is localized and can contain several words.
        // Match the unlocalized quiet-list name rather than interpreting it.
        let (before_version, version) = line.rsplit_once(char::is_whitespace)?;
        let version = version.trim().parse::<u8>().ok()?;
        let name = names.iter().filter(|name| before_version.strip_prefix(name.as_str())
            .is_some_and(|rest| rest.starts_with(char::is_whitespace)))
            .max_by_key(|name| name.len())?;
        Some(Distribution { name: name.clone(), version, default })
    }).collect()
}

fn choose_distribution(distributions: &[Distribution], requested: Option<&str>) -> Result<String, String> {
    let eligible = |d: &&Distribution| d.version == 2 && !d.name.to_ascii_lowercase().starts_with("docker-desktop");
    if let Some(requested) = requested {
        return distributions.iter().filter(eligible).find(|d| d.name.eq_ignore_ascii_case(requested))
            .map(|d| d.name.clone()).ok_or_else(|| format!("{DISTRO_ENV} names '{requested}', which is not an available user WSL2 distribution."));
    }
    let eligible: Vec<_> = distributions.iter().filter(eligible).collect();
    eligible.iter().find(|d| d.name.eq_ignore_ascii_case("Oculus"))
        .or_else(|| eligible.iter().find(|d| d.default))
        .or_else(|| eligible.first()).map(|d| d.name.clone())
        .ok_or_else(|| "Install a user WSL2 distribution with Linux Claude Code. Docker Desktop's internal distributions cannot run Oculus chat.".into())
}

fn output(mut command: Command, timeout: Duration) -> Result<std::process::Output, String> {
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|e| format!("Cannot start WSL2: {e}"))?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || { let mut b = Vec::new(); stdout.take(1024 * 1024).read_to_end(&mut b).map(|_| b) });
    let err = std::thread::spawn(move || { let mut b = Vec::new(); stderr.take(1024 * 1024).read_to_end(&mut b).map(|_| b) });
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("WSL2 did not answer in time. Start the distribution in a terminal and recheck Claude.".into());
            }
        }
    };
    Ok(std::process::Output { status, stdout: out.join().ok().and_then(Result::ok).unwrap_or_default(), stderr: err.join().ok().and_then(Result::ok).unwrap_or_default() })
}

fn launcher() -> Result<PathBuf, String> {
    let root = std::env::var_os("SystemRoot").ok_or("Windows SystemRoot is unavailable")?;
    let path = PathBuf::from(root).join("System32/wsl.exe");
    path.is_file().then_some(path).ok_or_else(|| "WSL2 is not installed. Install WSL2 and a Linux distribution before enabling Claude.".into())
}

fn probe() -> Result<Bridge, String> {
    let launcher = launcher()?;
    let mut list = crate::platform::command(&launcher);
    list.args(["--list", "--verbose"]);
    let listed = output(list, Duration::from_secs(25))?;
    if !listed.status.success() {
        return Err("WSL2 is unavailable. Install WSL2 and a user Linux distribution, then recheck Claude.".into());
    }
    let mut quiet = crate::platform::command(&launcher);
    quiet.args(["--list", "--quiet"]);
    let named = output(quiet, Duration::from_secs(25))?;
    if !named.status.success() { return Err("Cannot list available WSL2 distributions.".into()); }
    let names: Vec<String> = decode_wsl(&named.stdout).lines()
        .map(|line| line.trim().trim_start_matches('\u{feff}').to_string())
        .filter(|line| !line.is_empty()).collect();
    let distro = choose_distribution(&parse_distributions(&decode_wsl(&listed.stdout), &names), std::env::var(DISTRO_ENV).ok().as_deref())?;
    let mut cmd = crate::platform::command(&launcher);
    cmd.args(["--distribution", &distro, "--exec", "python3", "-u", "-c", SUPERVISOR, "probe"]);
    let result = output(cmd, Duration::from_secs(60))?;
    if !result.status.success() {
        let detail = decode_wsl(&result.stderr);
        return Err(format!("Claude in WSL2 '{distro}': {}", detail.trim().chars().take(1200).collect::<String>()));
    }
    let mut bridge: Bridge = serde_json::from_slice(&result.stdout).map_err(|_| format!("WSL2 '{distro}' returned an invalid Claude setup response."))?;
    bridge.launcher = launcher;
    bridge.distro = distro;
    Ok(bridge)
}

fn cache() -> &'static Mutex<Option<Result<Bridge, String>>> {
    static CACHE: OnceLock<Mutex<Option<Result<Bridge, String>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

pub fn bridge() -> Result<Bridge, String> {
    let mut cache = cache().lock().unwrap();
    cache.get_or_insert_with(probe).clone()
}

pub fn forget() { *cache().lock().unwrap() = None; }

impl Bridge {
    /// The free initialize handshake in the same Linux installation as chat.
    /// The supervisor accepts no user messages, disables hooks/MCP/session
    /// persistence and kills the process group on completion, EOF or timeout.
    pub fn models_command(&self, cwd: &Path) -> Result<Command, String> {
        let cwd = self.path(cwd)?;
        let mut command = crate::platform::command(&self.launcher);
        command.args(["--distribution", &self.distro, "--exec", "python3", "-u", "-c",
            SUPERVISOR, "models", &cwd]);
        command.env_remove("WSLENV").env_remove("ANTHROPIC_API_KEY")
            .env_remove("OPENAI_API_KEY").env_remove("CLAUDECODE")
            .env_remove("CLAUDE_CODE_ENTRYPOINT");
        Ok(command)
    }

    /// Authentication is a fixed CLI operation, not an agent turn. The
    /// supervisor gives it the Linux credential environment and kills it on
    /// stdin EOF, including when a Windows sign-in dialog is cancelled.
    pub fn auth_command(&self, login: bool) -> Command {
        let mut command = crate::platform::command(&self.launcher);
        command.args(["--distribution", &self.distro, "--exec", "python3", "-u", "-c",
            SUPERVISOR, if login { "auth-login" } else { "auth-status" }]);
        command.env_remove("WSLENV").env_remove("ANTHROPIC_API_KEY")
            .env_remove("OPENAI_API_KEY").env_remove("CLAUDECODE")
            .env_remove("CLAUDE_CODE_ENTRYPOINT");
        command
    }

    pub fn auth_error(&self) -> Option<String> {
        (!self.logged_in).then(|| format!("Sign in to Claude inside WSL2: wsl -d {} -- {} auth login. Then recheck the bridge.", self.distro, self.claude))
    }

    pub fn path(&self, windows: &Path) -> Result<String, String> {
        let windows = dunce::canonicalize(windows).map_err(|e| format!("Cannot resolve the library path: {e}"))?;
        let mut cmd = crate::platform::command(&self.launcher);
        cmd.args(["--distribution", &self.distro, "--exec", "wslpath", "-a", "-u"])
            .arg(windows.as_os_str());
        let out = output(cmd, Duration::from_secs(15))?;
        let path = String::from_utf8_lossy(&out.stdout).trim_end_matches(['\r', '\n']).to_string();
        if !out.status.success() || !path.starts_with('/') || path.contains('\0') || path.contains('\n') {
            return Err("The Windows library path cannot be opened from the selected WSL2 distribution.".into());
        }
        Ok(path)
    }

    pub fn transcript_root(&self) -> PathBuf {
        PathBuf::from(format!(r"\\wsl.localhost\{}{}", self.distro, self.config.replace('/', "\\")))
    }

    /// Cache identity for Claude, not for the shared Windows WSL launcher.
    pub fn executable_path(&self) -> PathBuf {
        PathBuf::from(format!(r"\\wsl.localhost\{}{}", self.distro, self.claude.replace('/', "\\")))
    }
}

pub struct Launch {
    pub command: Command,
    pub config: Value,
    pub transcript_root: PathBuf,
    pub windows_library: String,
    pub linux_library: String,
}

pub fn prepare(cfg: &ClaudeSpawn) -> Result<Launch, String> {
    let bridge = bridge()?;
    if let Some(error) = bridge.auth_error() { return Err(error); }
    let library = bridge.path(&cfg.library)?;
    let cwd = bridge.path(&cfg.cwd)?;
    let system_append = translate_text(&cfg.system_append, &cfg.library.display().to_string(), &library);
    let mut args = vec!["-p".into(), "--input-format".into(), "stream-json".into(), "--output-format".into(), "stream-json".into(),
        "--verbose".into(), "--include-partial-messages".into(), "--permission-mode".into(), cfg.permission_mode.clone(),
        "--permission-prompts".into(), "none".into()];
    for (flag, value) in [("--model", cfg.model.as_ref()), ("--effort", cfg.effort.as_ref()), ("--resume", cfg.resume.as_ref())] {
        if let Some(value) = value { args.extend([flag.into(), value.clone()]); }
    }
    if !system_append.trim().is_empty() { args.extend(["--append-system-prompt".into(), system_append]); }
    args.extend(["--add-dir".into(), library.clone(), "--settings".into(),
                 super::claude::wsl_settings_json(&library, &cwd, &bridge.home, &bridge.config)]);
    let mut command = crate::platform::command(&bridge.launcher);
    command.args(["--distribution", &bridge.distro, "--exec", "python3", "-u", "-c", SUPERVISOR, "session"])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    // Linux receives an explicit environment constructed by the supervisor.
    // Also prevent WSLENV from copying API credentials into the supervisor.
    command.env_remove("WSLENV").env_remove("ANTHROPIC_API_KEY").env_remove("OPENAI_API_KEY")
        .env_remove("CLAUDECODE").env_remove("CLAUDE_CODE_ENTRYPOINT");
    Ok(Launch { command, config: json!({"library": library, "cwd": cwd, "args": args}),
        transcript_root: bridge.transcript_root(), windows_library: cfg.library.display().to_string(), linux_library: library })
}

/// Only known library prefixes are translated; prose and unrelated paths are
/// preserved. Backslashes in the replaced path's suffix become Linux slashes.
pub fn translate_text(text: &str, windows_root: &str, linux_root: &str) -> String {
    let mut result = text.replace(windows_root, linux_root).replace(&windows_root.replace('\\', "/"), linux_root);
    // Paths quoted in prompts can include a Windows suffix after the root.
    let marker = format!("{linux_root}\\");
    while let Some(start) = result.find(&marker) {
        let end = result[start..].find(['`', '"', '\n', '\r']).map(|i| start + i).unwrap_or(result.len());
        let replacement = result[start..end].replace('\\', "/");
        result.replace_range(start..end, &replacement);
    }
    result
}

pub fn write_config(stdin: &mut std::process::ChildStdin, config: &Value) -> Result<(), String> {
    let mut bytes = serde_json::to_vec(config).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    stdin.write_all(&bytes).and_then(|_| stdin.flush()).map_err(|e| format!("WSL2 bridge startup: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distro_selection_excludes_internal_and_v1_and_handles_localized_status() {
        let names = ["docker-desktop", "Ubuntu", "学生 Linux", "Oculus"].map(str::to_string);
        let parsed = parse_distributions("  NAME STATE VERSION\n* docker-desktop Running 2\n  Ubuntu Stopped 1\n  学生 Linux Arrêté 2\n  Oculus Stopped 2\n", &names);
        assert_eq!(choose_distribution(&parsed, None).unwrap(), "Oculus");
        assert_eq!(choose_distribution(&parsed, Some("学生 Linux")).unwrap(), "学生 Linux");
        assert!(choose_distribution(&parsed, Some("docker-desktop")).is_err());
        assert!(choose_distribution(&parsed, Some("Ubuntu")).is_err());
    }

    #[test]
    fn distro_names_are_not_confused_with_localized_multiword_states() {
        let names = ["Ubuntu", "Ubuntu Dev", "学生  Linux"].map(str::to_string);
        let parsed = parse_distributions("  NOM  ÉTAT  VERSION\n* Ubuntu Dev   En cours d’exécution   2\n  Ubuntu   Arrêté   1\n  学生  Linux   En cours d’exécution   2\n", &names);
        assert_eq!(parsed[0], Distribution { name: "Ubuntu Dev".into(), version: 2, default: true });
        assert_eq!(parsed[2].name, "学生  Linux");
        assert_eq!(choose_distribution(&parsed, None).unwrap(), "Ubuntu Dev");
    }

    #[test]
    fn wsl_output_decodes_utf16_without_losing_non_ascii() {
        let text = "* 学生 Linux Stopped 2\r\n";
        let bytes: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
        assert_eq!(decode_wsl(&bytes), text);
    }

    #[test]
    fn library_paths_with_spaces_quotes_and_unicode_are_data() {
        let root = r"F:\student's 学生\library";
        let text = format!("Read `{root}\\agents\\frame.jpg`; don't change C:\\elsewhere.");
        assert_eq!(translate_text(&text, root, "/mnt/f/student's 学生/library"),
            "Read `/mnt/f/student's 学生/library/agents/frame.jpg`; don't change C:\\elsewhere.");
    }

    #[test]
    fn transcript_root_uses_selected_linux_distribution() {
        let bridge = Bridge { launcher: PathBuf::new(), distro: "Oculus".into(), home: "/home/学生".into(),
            config: "/home/学生/.claude".into(), claude: "/home/学生/.local/bin/claude".into(), version: String::new(), logged_in: false };
        assert_eq!(bridge.transcript_root(), PathBuf::from(r"\\wsl.localhost\Oculus\home\学生\.claude"));
        assert!(bridge.auth_error().unwrap().contains("claude auth login"));
    }
}
