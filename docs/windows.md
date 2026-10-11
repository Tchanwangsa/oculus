# Windows

Oculus targets Windows 10/11 x64 with WebView2. The Windows configuration,
credential broker, agent processes and Store packaging have explicit platform
boundaries; native Windows acceptance testing is required before release.

## Where

| Piece | Location |
| --- | --- |
| Windows window and NSIS settings | `app/src-tauri/tauri.windows.conf.json` |
| Native executable manifest | `app/src-tauri/windows-app-manifest.xml`, `app/src-tauri/build.rs` |
| Keyboard labels and caption controls | `app/src/lib/platform.ts`, `app/src/components/tabs/WindowControls.tsx` |
| WebView2 keyboard dispatch | `app/src-tauri/src/shell/menu/windows.rs` |
| Shared data directory and broker endpoint | `app/keyd/core/src/paths.rs` |
| Named pipes, DPAPI, file ACLs and broker lifetime | `app/keyd/core/src/platform/windows/` |
| GUI broker startup | `app/src-tauri/src/auth/keyd/install.rs` |
| WebView2 cookie sequencing | `app/src-tauri/src/shell/browser/windows.rs` |
| Native command and npm shim resolution | `app/src-tauri/src/runtime/platform/mod.rs` |
| Agent process containment | `app/src-tauri/src/runtime/platform/job.rs` |
| Windows CI and artifacts | `.github/workflows/ci.yml`, `.github/workflows/release.yml` |

## The app, CLI and broker share the user's profile folder

`keyd_core::paths::data_dir()` returns `%USERPROFILE%\Oculus` on Windows.
The GUI, `oculus.exe` and keyd use it directly; it does not depend on MSIX's
AppData virtualization or a roaming profile's AppData directory. Browser data
uses WebView2's default environment, shared by login and browser tabs.

Uninstalling the app leaves the profile folder, including the library and
vault. Removing it deletes the user's saved coursework, notes and credentials.
Reinstalling under the same Windows account can reopen the retained vault.
DPAPI binds the master key to that account; copying the folder to another
account or computer does not create a portable credential backup.

Relative library paths stored in SQLite and sent over IPC use `/`. Native
absolute paths are joined in Rust and normalized with `dunce` when compared.
Filenames avoid Windows device names and trailing dots. The native manifest
sets `longPathAware`; filesystem and machine policy still determine limits.

## MSIX protects the broker's executable boundary

The GUI executable is `oculus-app.exe`, the CLI is `oculus.exe`, and the
broker is `oculus-keyd.exe`. Separate names are required on a case-insensitive
filesystem and let the broker assign the app and CLI distinct roles.

Only the GUI starts the resident broker, detached and outside its Job Object.
The CLI connects to an existing broker and tells the user to open Oculus when
activation is needed. There is no logon startup task; open the GUI once after
Windows starts. The broker has no idle exit on Windows. Its registration
verifies the running broker before replacing or stopping it. An unchanged
source and path reuse the daemon; changes restart it. `oculus keyd uninstall`
stops the verified daemon before removing its registration.

The endpoint is `\\.\pipe\com.tchan.oculus.keyd.<user SID>`. Its DACL admits
only the current user and refuses remote clients. Overlapped I/O lets accept
checks and client read deadlines complete without blocking the event loop.
An accept whose client has already disconnected is rearmed for the next client;
a failed completion is retired before its error is returned, so it cannot keep
waking the accept loop and filling the broker log. Synchronous connects and
pending I/O completions have separate states.
Release clients verify the pipe server’s process, user and current MSIX
package before sending credentials; an identity mismatch is a connection
error rather than a missing broker. The broker checks the client process
identity before reading a request, pins
its process object and confirms the PID and creation time. Release callers
must run as the same user from the broker's installed MSIX package directory.
The role gate still applies to each operation: only the app can receive a
browser session or resume an Okta sign-in.

The random vault master key is protected by user-scope DPAPI and held in an
owner-only file. The vault uses authenticated encryption, file locking and
atomic replacement. **DPAPI does not isolate secrets from other processes
running as the same Windows user.** Such a process can unwrap the master key;
Windows has no per-binary keychain gate equivalent to the macOS adapter.
The pipe and package checks therefore restrict broker operations without
claiming protection against a compromised Windows user account.

Unsigned NSIS installations have user-writable executable files and no MSIX
package identity. Release keyd refuses that installation with a descriptive
error. The NSIS artifact supports installation and shell testing; Canvas and
headless Okta need the MSIX installation. The development preflight builds keyd with `--features dev`, enabling its
development broker policy. Cargo's debug profile alone does not select that
policy. Development brokers must not be distributed as release packages.

## Windows uses native providers and Groq transcription

Codex and opencode run natively. Discovery prefers `.exe`, recognizes PATHEXT,
and searches common npm, Bun and user installation directories. Known npm
batch shims are resolved to their package's script and passed directly to
`node.exe`; agent arguments never pass through `cmd.exe`. Native Claude is
disabled pending WSL2 support. Antigravity can be discovered as a native
executable but has no in-app Windows installer or updater.

Agent children start suspended, enter a kill-on-close Job Object, then run.
Stopping a thread or dropping its job terminates descendants. Native helper
commands hide console windows. Agent sandbox access to the `oculus` CLI and
named pipe must be checked with the vendor's actual Windows sandbox.

Windows offers Groq transcription only. Saved local engine preferences are
filtered against the available backend engine list. Settings hide Apple
speech and local whisper controls and do not invoke either helper on Windows.

## WebView2 cookies are queued and verified off the UI thread

Login and browser pages use the same WebView2 environment. Cookie reads run
on worker threads because reading on WebView2's event loop can deadlock.
Canvas and Okta cookies are seeded from keyd before navigation; the worker
reads them back until queued changes are visible. Sign-out deletes the
cookies for both hosts and verifies their absence before completing.
Failures surface instead of reporting a successful sign-out with live cookies.
See [auth.md](./auth.md) for cookie preservation and renewal rules.

Windows browser pages keep the native user agent. History, find and reload
use their JavaScript implementations; page snapshots and native rounded page
corners are unavailable. The suggestion list hides a page without a snapshot.
The tab bar supplies caption buttons and Ctrl labels; its custom caption has
no native Windows 11 snap-layout flyout.
Window shortcuts use WebView2's controller callback and the shared menu
handler; browser pages keep editing and find shortcuts. See
[shell.md](./shell.md#the-shell-owns-tabs-side-panels-and-every-window-shortcut)
for keyboard ownership.

## Release validation includes a packaged installation

Cross-check the code from macOS with LLVM tools on PATH:

```sh
cd app/src-tauri
cargo xwin check --target x86_64-pc-windows-msvc --all-targets --locked
```

A check compiles platform branches but does not prove Windows linking,
installation, WebView2 behavior, broker identity or Store certification.
The `render_memory` example reports peak working-set bytes with
`GetProcessMemoryInfo` on Windows and peak RSS with `getrusage` on Unix.
Windows CI runs real builds and tests; [development.md](./development.md)
explains artifact packaging and Partner Center configuration.

On a fresh Windows account, verify MSIX installation and `oculus` execution
alias, Okta sign-in and course sync, persisted credentials after restart,
PDF/video/document editing, Voyage search and Groq transcription. Run Codex
and opencode threads that invoke `oculus search`, then check descendant
teardown when the app quits. Verify an executable copied outside the package
is refused by keyd. Repeat sign-in, transcription and restart checks on a
real x64 Windows laptop on the university network before Store submission.

## Gotchas

- `%APPDATA%` is not the library root; all processes call the shared path helper.
- A release broker needs MSIX package identity; an unsigned NSIS tree fails closed.
- DPAPI user scope does not isolate secrets from another process running as that user.
- Cookie readback on WebView2's event-loop thread can deadlock.
- Do not execute batch shims with agent-provided argv; resolve the package script.
- The CLI must never start keyd inside an agent sandbox.
- A compile check and browser fixture screenshots do not replace Windows acceptance tests.
