# Windows setup

Windows and macOS are maintained together in
`https://github.com/Tchanwangsa/oculus`. Use `andre` for testing before merging
to `master`; the separate `oculus_window` repository is retired. `MRI` is
outside the project and must never be read or changed. Existing installed
Windows apps and libraries keep the same paths and application identifier.

## Build from source

Install Bun, stable Rust for `x86_64-pc-windows-msvc`, Visual Studio 2022 C++
Build Tools and the Windows SDK, and sccache. Ensure they are on PATH in
a fresh terminal. WebView2 is the Windows web runtime; the installer can
install it if absent.

```powershell
cd app
bun install --frozen-lockfile
bun run tauri dev
```

`bun run tauri build` creates
`app/src-tauri/target/release/bundle/nsis/Oculus_0.1.0_x64-setup.exe`.
The per-user installer includes the app, `oculus.exe`, ffmpeg and `pdfium.dll`.
The dev preflight builds the debug CLI and stages its external-binary slot,
including on a clean checkout. `scripts/dev.mjs` starts Bun with the watcher
environment without Unix-only shell syntax. No Python runtime is shipped.
`app/src-tauri/tauri.windows.conf.json` owns the native Windows title bar,
NSIS install mode and resource mapping.

## First installed launch

The app opens without downloading a Python environment or loading local models.
Settings → Library selects MinerU cloud (requires a token) or a separately
started local MinerU server, and configures Voyage indexing. Read
[parsing.md](./parsing.md) and [retrieval.md](./retrieval.md) before enabling
remote processing. The app never falls back from one parsing engine to another.
Windows keeps Local as the default when no engine was selected, preserving
the earlier Windows app's local processing policy. An explicit legacy Cloud
or Auto choice carries over as Cloud. A local server that is stopped reports
unavailable instead of uploading the document elsewhere.

Upgrades keep existing coursework, chats, settings and parsed artifacts.
The former `python-runtime` folder, source `.venv` and model caches are left
in place; Oculus no longer starts them. Old Qwen embeddings do not match
Voyage's model, so semantic search requires a new index. Parsed text remains
available to lexical search. The app remains single-instance on Windows.

PDFium loads from beside the installed desktop/CLI executable or from the
development `binaries` directory; `OCULUS_PDFIUM_LIB` can override it. The
fetch script stages on the checkout's volume so a C: temporary directory and
an F: checkout do not cause a cross-volume rename failure.

## Platform behavior

- Library and database: `%APPDATA%\com.tchan.oculus`; schema and relative
  library paths remain compatible with the upstream app. All database
  connections resolve the same physical file before opening SQLite, including
  under MSIX AppData virtualization. Existing journals at a conflicting alias
  require recovery before the app opens them. See [architecture.md](./architecture.md).
- Credentials: Windows Credential Manager through keyring's native backend.
- Sign-in: shared WebView2 profile for Canvas login and remote browser tabs;
  persisted Canvas cookies are restored with WebView2's native cookie API.
- Keepalive: a per-user Windows Task Scheduler job runs `oculus auth tick`
  while the user is logged in, even when Oculus is closed. It does not store
  a Windows password or request elevated privileges.
- Shortcuts: Ctrl replaces Command; F11 toggles fullscreen. Escape leaves
  fullscreen when a dialog or other control has not consumed it.
- Codex: native executable and standard npm installations are supported;
  the installed Codex desktop CLI is discovered when absent from PATH.
- Claude: Claude Code runs in a WSL2 Linux distribution with its filesystem
  sandbox enabled. Settings → AI reports bridge readiness and setup errors;
  its Recheck updates all composers and job pickers. Native Windows Claude
  is not used. Existing threads and drafts remain, and Codex stays the
  Windows default.
- Office conversion: install LibreOffice, or set `OCULUS_SOFFICE` to its
  executable. Word, PowerPoint and Excel use derived PDF siblings for the
  viewer, parsing and search. Original Office files remain available without
  the converter. Uploads lets students add their own subject files through
  the native picker or drag/drop, with deletion limited to that upload folder.

The update includes a transcript reading copy, split panes, lexical search,
unified tasks, editable calendar events, image attachments and agent setup.
The old recap and dormant BYOK layer were removed. Automations, Inbox and
scheduled coursework sync are not reintroduced.

## Claude Code through WSL2

The Windows frontend remains native. Only Claude and its filesystem sandbox
run in Linux. `app/scripts/setup-claude-wsl.ps1` prepares the dedicated
`Oculus` WSL2 distribution and its non-root `oculus` user, installs the Linux
Claude CLI and sandbox dependencies, and leaves other distributions alone.
See [Claude WSL2 setup](./claude-wsl-setup.md) for prerequisites and the setup
command.

Discovery prefers `Oculus`, then the default eligible WSL2 distribution, then
the first eligible distribution. Docker Desktop's internal distributions and
WSL1 are excluded. Set `OCULUS_CLAUDE_WSL_DISTRO` before launching Oculus to
choose another user distribution. It must have a non-root default user, the
native Linux Claude executable, Python 3, bubblewrap, socat and libseccomp;
health verifies that a sandbox can actually start, not only that its tools
are installed.

Claude's Linux installation has its own sign-in. Complete it in an interactive
terminal:

```powershell
wsl.exe --distribution Oculus --user oculus --cd /home/oculus --exec /home/oculus/.local/bin/claude auth login
```

Then use **Settings → AI → Recheck**. A ready bridge enables **Claude Code
via WSL2** in the model picker for new chats and job settings, and restores
sending in existing Claude threads. Setup failures appear inline and do not
discard drafts, change saved providers, or fall back to unsandboxed native
Claude. Codex remains the initial selection for fresh Windows chats.

## Verification

```powershell
cd app
bun test tests
bun run build
cd src-tauri
cargo test --release
```

Rust tests include isolated Windows credential and scheduler round trips,
Unicode media/range requests, file boundaries, paths, provider discovery and
the upstream suites. Parser and embedding tests use local protocol fixtures;
they do not require paid service requests or the user's coursework.
Run the desktop as well: screenshots and UI interactions are necessary for
WebView2 layout, tabs, viewers, fullscreen and authentication windows. A real
Canvas sync additionally requires the user's university sign-in.

Release binaries are currently unsigned. Windows Smart App Control or an
organization's application-control policy may reject a newly compiled binary.
Distribution to machines enforcing that policy requires trusted code signing;
the build does not disable or modify Windows security settings.

## Shared development

Develop in this repository on either OS. Shared features reach both builds
in the same commit; there is no import marker or recurring mirror merge.
Keep OS-specific behaviour behind the platform seams and run the native
build on both systems. `.github/workflows/desktop.yml` checks and packages
Windows and macOS on pushes to `andre` and `master`, and on pull requests.
Its installers are test artifacts, not signed production releases.

Key port seams are `app/src-tauri/src/platform.rs`,
`app/src-tauri/src/database.rs`, `app/src-tauri/src/harness/wsl.rs`,
`app/src-tauri/src/harness/discover.rs`, `app/src-tauri/src/embed/raster.rs`, and
`app/src/lib/platform.ts`.

Both builds pin a vendored `libsqlite3-sys` 0.30.1 with the SQLite
3.53.4 engine so the app and CLI share the updated WAL implementation without
changing SQLx. Source hashes and update instructions are in
`app/src-tauri/vendor/libsqlite3-sys/OCULUS-PROVENANCE.md`.
