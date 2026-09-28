# Claude on Windows with WSL2

The Windows application stays native. Claude Code runs as a non-root Linux
process in a dedicated WSL2 distribution named `Oculus`. The Windows bridge
in `app/src-tauri/src/harness/wsl.rs` owns the process, translates library
paths, and brokers supported Oculus CLI requests. See `docs/harness.md` for
the containment boundary and chat protocol.

## One-time setup

Use a current Microsoft WSL release that supports `--install --name` and
`--manage --set-default-user`. WSL2 must already be enabled; follow
[Microsoft's installation instructions](https://learn.microsoft.com/en-us/windows/wsl/install)
if it is not. A Windows restart may be required when enabling WSL2 for the
first time. The setup script does not change Windows security settings.

Close Oculus before running setup or rerunning it, because the script restarts
only the dedicated `Oculus` distribution to apply its configuration. From the
repository root, run:

```powershell
.\app\scripts\setup-claude-wsl.ps1
```

`app/scripts/setup-claude-wsl.ps1` installs Ubuntu 24.04 under the separate name
`Oculus`, creates the `oculus` Linux user, and sets that user as this
distribution's default. The account has no administrative groups and no
login password; Windows invokes it through WSL. The script leaves other
distributions and the machine's default WSL distribution unchanged. A
distribution already named `Oculus` is reused only if it carries the script's
`/etc/oculus/managed-wsl` marker.

The script installs `bubblewrap`, `socat`, `libseccomp2`, Python 3, `ripgrep`,
and download utilities from Ubuntu packages. It uses Anthropic's native
installer for the stable Linux Claude release, and Bun for
`@anthropic-ai/sandbox-runtime@0.0.76`. The sandbox runtime supplies the
optional Unix-socket seccomp helper; Oculus also requires its own outer
sandbox and seccomp checks to succeed. A failed sandbox probe is an error,
not permission to run Claude without isolation.

For this dedicated distribution, `/etc/wsl.conf` disables systemd, Windows
executable interop and Windows PATH import. This CLI-only environment has no
background services to start; avoiding systemd also avoids its observed
ten-second WSL cold-start delay. The bridge communicates with the Windows
CLI through its controlled broker. The setup does not disable AppArmor or
relax machine-wide Linux namespace restrictions. If `bwrap` cannot create
its namespace, setup stops; diagnose the failure against
[Anthropic's Linux and WSL2 instructions](https://code.claude.com/docs/en/sandboxing#set-up-linux-and-wsl2).

## Sign in

Linux Claude has its own authentication state. A native Windows Claude
installation or login does not automatically sign in the Linux copy. Once
WSL setup is complete, Settings → AI → Claude → Sign in runs this Linux
login from the app: follow the browser link and paste the code into its
dialog when requested. Closing the dialog cancels the Linux login process;
successful completion refreshes discovery. To sign in from a terminal, run:

```powershell
wsl.exe --distribution Oculus --user oculus --cd /home/oculus --exec /home/oculus/.local/bin/claude auth login
```

Complete the browser sign-in that Claude displays. Because Windows executable
interop is disabled, open the displayed URL manually if the browser does not
open automatically. Then return to Oculus Settings → AI and recheck the
provider. No credentials need to be copied into the app or repository.

To check the Linux login directly:

```powershell
wsl.exe --distribution Oculus --user oculus --cd /home/oculus --exec /home/oculus/.local/bin/claude auth status --json
```

## Validated environment

The initial setup on 18 September 2026 installed Ubuntu 24.04.5, WSL2,
Claude Code 2.1.267 (stable), Bun 1.4.2, bubblewrap 0.9.0, socat 1.8.0.0,
Python 3.12, and libseccomp 2.5.5. The ordinary user successfully launched
a real bubblewrap isolation probe and loaded `libseccomp.so.2`. The existing
`docker-desktop` distribution remained stopped and remained the default.

Important paths inside the distribution:

- Claude launcher: `/home/oculus/.local/bin/claude`
- Claude data and authentication: `/home/oculus/.claude`
- Python: `/usr/bin/python3`
- Bubblewrap and socat: `/usr/bin/bwrap`, `/usr/bin/socat`
- Additional x64 seccomp helper:
  `/home/oculus/.bun/install/global/node_modules/@anthropic-ai/sandbox-runtime/vendor/seccomp/x64/apply-seccomp`

The native installer uses Anthropic's stable channel. Subsequent versions may
differ from the validated version above. Existing Linux Claude installations
and authentication survive a setup rerun.

## Supervisor tests

`app/src-tauri/src/harness/test_wsl_bridge.py` exercises the real Linux
supervisor and sandbox without a model call. It substitutes a small Python
program for Claude and uses isolated fixture folders. Run it as the ordinary
Linux user, adapting the Windows checkout drive/path if necessary:

```powershell
wsl.exe --distribution Oculus --user oculus --cd / --exec /usr/bin/python3 /mnt/f/codex/oculus/app/src-tauri/src/harness/test_wsl_bridge.py
```

The tests cover Unicode streaming, interrupt forwarding, broker argument and
exit-status preservation, batch-file forwarding, concurrent FIFO requests,
large payloads, cancellation and malformed-frame recovery. A nested run of
the installed Anthropic sandbox runtime proves that the broker works with
Unix sockets blocked, including a library with no lectures directory.
Containment checks cover permitted agent writes, denied library/symlink
writes, hidden Windows drives, preserved DNS configuration, blocked Windows
process and privileged-socket escapes, invalid working directories, and
detached descendant cleanup after EOF or missing owner heartbeats. They also
verify that valid heartbeats keep a session alive. Live Claude authentication
and a real chat turn are separate integration checks.

The suite also checks sign-in code forwarding and cancellation on Windows
owner EOF, and that generated skills and their provider routes remain
read-only inside the otherwise writable `agents/` folder.

The model catalogue is another fixed supervisor operation: a free `initialize`
handshake with hooks, MCP and session persistence disabled. Tests verify its
model response, reject a user turn before spawning Claude, and prove the Linux
child is reaped on completion, owner EOF and its deadline. On 28 September 2026,
all 22 supervisor tests passed, and a separate read-only check against the
installed Linux Claude returned five catalogue rows without starting a turn.

## Opt-in live integration check

The `claude_wsl_smoke` Rust example uses the signed-in Claude subscription and a
new synthetic library. It checks streaming, a Unicode document read, an allowed
file write, the native Oculus CLI, conversation resume, rewind, interruption and
recovery. Build it alongside the CLI, then place both executables together to
reproduce the installed application's CLI discovery:

```powershell
# From app/src-tauri
cargo build --release --example claude_wsl_smoke --bin oculus
New-Item -ItemType Directory -Force ../../artifacts/wsl-smoke-tools | Out-Null
Copy-Item target/release/examples/claude_wsl_smoke.exe,target/release/oculus.exe ../../artifacts/wsl-smoke-tools -Force
../../artifacts/wsl-smoke-tools/claude_wsl_smoke.exe ../../artifacts/claude-wsl-live-new
```

Choose a new fixture directory for each run. The example does not operate on the
application's real library and is excluded from the automated test suite.
