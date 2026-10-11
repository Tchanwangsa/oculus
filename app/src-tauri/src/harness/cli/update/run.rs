//! Checking each installed CLI against its registry and running its update.

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::Serialize;

use super::fetch::{arch, claude_channel, latest, latest_endpoint};
use super::source::{command, source_of, Source};
use super::version::is_newer;
use crate::harness::cli::discover;
use crate::harness::cli::install::{self, InstallLine};
use crate::harness::event::Provider;

/// One CLI's row in Settings: what is installed, what is published, and the
/// command that would close the gap.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub provider: Provider,
    pub installed: Option<String>,
    pub latest: Option<String>,
    /// Only when `latest` is strictly newer; never on a failed check.
    pub available: bool,
    pub source: Source,
    pub command: String,
    pub error: Option<String>,
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// The binary, its source and its update command; `None` when not installed.
fn plan(provider: Provider) -> Option<(PathBuf, Source, String)> {
    let bin = discover::binary(provider).ok()?;
    let real = bin.canonicalize().unwrap_or_else(|_| bin.clone());
    let source = source_of(&real, home().as_deref());
    let cmd = command(provider, source, &bin);
    Some((bin, source, cmd))
}

/// The update check for one provider, `None` when its CLI is missing.
pub fn check(provider: Provider) -> Option<UpdateInfo> {
    let (_, source, command) = plan(provider)?;
    let installed = discover::health(provider).version;
    let home = home();
    let (url, shape) = latest_endpoint(provider, source, claude_channel(home.as_deref()), arch());
    let (latest, error) = match latest(&url, shape) {
        Ok(v) => (Some(v), None),
        Err(e) => (None, Some(e)),
    };
    let available = matches!((&latest, &installed), (Some(l), Some(i)) if is_newer(l, i));
    Some(UpdateInfo {
        provider,
        installed,
        latest,
        available,
        source,
        command,
        error,
    })
}

/// Every installed provider's check, the four requests side by side.
pub fn check_all() -> Vec<UpdateInfo> {
    let handles: Vec<_> = discover::PROVIDERS
        .iter()
        .map(|&p| std::thread::spawn(move || check(p)))
        .collect();
    handles
        .into_iter()
        .filter_map(|h| h.join().ok().flatten())
        .collect()
}

/// Where an update's output reaches the webview.
pub const UPDATE_EVENT: &str = "harness-update";

/// The provider updating now; one at a time across all of them.
fn running() -> &'static Mutex<Option<Provider>> {
    static R: OnceLock<Mutex<Option<Provider>>> = OnceLock::new();
    R.get_or_init(Default::default)
}

/// Run the provider's update command, streaming its output to `emit` and
/// closing with one `done` line. The lock and the provider's cached health
/// are released before that line, so the next queued update can start and a
/// recheck reads the new version.
pub fn start<F>(provider: Provider, emit: F) -> Result<(), String>
where
    F: Fn(InstallLine) + Send + 'static,
{
    let (_, _, command) =
        plan(provider).ok_or_else(|| format!("{} is not installed", provider.label()))?;
    {
        let mut r = running().lock().unwrap();
        if let Some(busy) = *r {
            return Err(format!(
                "{} is updating; updates run one at a time",
                busy.label()
            ));
        }
        *r = Some(provider);
    }
    let result = install::run_command(&command, "updated", move |line| {
        if line.done {
            discover::forget_health(provider);
            *running().lock().unwrap() = None;
        }
        emit(InstallLine {
            provider,
            line: line.line,
            done: line.done,
            ok: line.ok,
            status: line.status,
        });
    });
    if result.is_err() {
        *running().lock().unwrap() = None;
    }
    result
}
