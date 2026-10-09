//! Reaping `opencode serve` processes a signalled app left behind.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use super::server::SERVE_ARGS;

/// How long a server gets to close its listener before it is killed outright.
const STRAY_GRACE: Duration = Duration::from_secs(2);

/// Kill the `opencode serve` processes a signalled app left behind (a
/// `tauri dev` relaunch, a force-quit or crash runs neither `Drop` nor
/// `RunEvent::Exit`), and answer with the pids. Called once at startup.
///
/// A stray is **our argv** ([`super::server::SERVE_ARGS`]) **and `ppid == 1`** (adopted by
/// launchd): a living app's server, such as a worktree build running beside
/// this one, is parented to that app and never touched. Same uid, too.
pub fn sweep() -> Vec<u32> {
    let uid = unsafe { libc::getuid() };
    let found = strays(&ps_listing(), uid);
    if found.is_empty() {
        return found;
    }
    send_signal(&found, libc::SIGTERM);
    // SIGKILL survivors, re-listing first in case a pid was reused.
    let sent = found.clone();
    std::thread::spawn(move || {
        std::thread::sleep(STRAY_GRACE);
        let left: Vec<u32> = strays(&ps_listing(), uid)
            .into_iter()
            .filter(|p| sent.contains(p))
            .collect();
        send_signal(&left, libc::SIGKILL);
    });
    found
}

fn ps_listing() -> String {
    Command::new("/bin/ps")
        .args(["-axww", "-o", "pid=,ppid=,uid=,command="])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

fn send_signal(pids: &[u32], sig: i32) {
    for pid in pids {
        unsafe { libc::kill(*pid as libc::pid_t, sig) };
    }
}

/// The stray pids in a `ps -axww -o pid=,ppid=,uid=,command=` listing. The
/// command is matched from its end so a binary path with a space resolves.
pub(super) fn strays(listing: &str, uid: u32) -> Vec<u32> {
    let tail = SERVE_ARGS.join(" ");
    listing
        .lines()
        .filter_map(|line| {
            let mut rest = line;
            let pid: u32 = ps_field(&mut rest)?.parse().ok()?;
            let ppid: u32 = ps_field(&mut rest)?.parse().ok()?;
            let owner: u32 = ps_field(&mut rest)?.parse().ok()?;
            if ppid != 1 || owner != uid || pid == std::process::id() {
                return None;
            }
            let bin = rest.trim().strip_suffix(&tail)?.trim_end();
            (Path::new(bin).file_name()? == "opencode").then_some(pid)
        })
        .collect()
}

/// One space-delimited field off the front, advancing `rest` past it.
fn ps_field<'a>(rest: &mut &'a str) -> Option<&'a str> {
    let trimmed = rest.trim_start();
    let end = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
    let (head, tail) = trimmed.split_at(end);
    *rest = tail;
    (!head.is_empty()).then_some(head)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stray_is_our_own_argv_that_launchd_has_adopted() {
        let ours = format!("serve {}", SERVE_ARGS[1..].join(" "));
        let listing = format!(
            "\
  4011     1   501 /Users/s/.opencode/bin/opencode {ours}
  4012 54983   501 /Users/s/.opencode/bin/opencode {ours}
  4013     1   501 /Users/s/.opencode/bin/opencode serve
  4014     1   501 /Users/s/.opencode/bin/opencode serve --port 4096
  4015     1   501 /opt/homebrew/bin/opencode tui
  4016     1     0 /Users/root/.opencode/bin/opencode {ours}
  4017     1   501 /Users/some one/.opencode/bin/opencode {ours}
  4018     1   501 /Users/s/.bun/bin/opencodex {ours}
"
        );
        assert_eq!(strays(&listing, 501), vec![4011, 4017]);
    }
}
