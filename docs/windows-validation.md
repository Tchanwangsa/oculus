# Windows validation

Evidence paths under `artifacts/` refer to local, ignored validation output.
Those files, installers and library backups are not included in this Git
repository. Commands in the development and Windows guides reproduce the
automated checks; live account checks are described separately below.

## macOS master update — 2026-09-24

Imported 66 macOS commits through
`24d7511a742214f2aa0eb03b25f50aa870d990aa`, including split panes, library
text search, the PDF.js reader, browser history and find, unified tasks,
editable calendar events, chat attachments and skills, and the opencode and
Antigravity providers. Windows keeps its native shortcuts, physical SQLite
path handling and Claude WSL2 sandbox.

The parser now uses Rust with either an external local MinerU server or
MinerU Cloud; Python is no longer bundled or launched. Windows upgrades
without an explicit cloud preference retain local parsing. Old Qwen vectors
require rebuilding with Voyage for semantic search; existing downloads and
page text are retained. See [Windows setup](windows.md) for the new services.

| Check | Result |
| --- | --- |
| Rust release suite | 474 passed, 3 ignored: 457 library, 14 CLI, one isolated Windows credential test and two migration integration tests. `artifacts/sept24-rust-tests.log`. |
| PDF rasterization | 16 focused tests passed with PDFium actually rendering synthetic PDFs. Only the optional real-library fixture was omitted. `artifacts/sept24-raster-tests.log`. |
| WSL2 supervisor | All 19 tests passed, including sandbox containment, native command brokering, generated skills, and the sign-in subprocess lifecycle. `artifacts/sept24-wsl-tests.log`. |
| Frontend | 38 tests / 180 assertions passed; TypeScript and the production Vite build passed. Regression coverage includes FTS5 ranking, provider recovery, split-browser navigation and native input focus. |
| Database upgrades | Migrations 1–33 remain unchanged. Synthetic upgrades through migration 37 retain tasks, subtasks, project links, chats, page text, vectors and model settings; foreign-key and integrity checks pass. |
| Publication scan | Candidate files and inherited Git history checked; no credential-pattern findings or accidental runtime files. `artifacts/sept24-publish-audit.json`. |

Native checks use a separate synthetic library upgraded from schema 33 to 37.
Calendar displays its retained task and subtask, and the unified Tasks view
shows both records. Closing and reopening a split restores its two app views.
The PDF.js reader renders a two-page PDF with spaces and Unicode in its Windows
path, including continuous view, two-page spread and fit-to-page. Native
browser history, successful and unsuccessful text find, tab shortcuts and
F11 fullscreen were exercised against a loopback fixture server. Ctrl+L and
Ctrl+F transfer focus from the native page to the app's fields; Ctrl+K accepts
typed text and finds the expected document page with a highlighted excerpt.
The fixture passes SQLite integrity and foreign-key checks after the upgrade.

The checks caught and fixed an inherited FTS5 aggregate-ranking error, failed
provider catalogue recovery, and native browser focus and split-adoption races.
A closed backup of the user's original library passed integrity checks with
all 560 file records before testing. Its 33 recorded SQLx migration checksums
match the unchanged migration definitions in this update.

The final release build and NSIS packaging passed. The installer at
`app/src-tauri/target/release/bundle/nsis/Oculus_0.1.0_x64-setup.exe` is
42,424,879 bytes, with SHA-256
`e2dbfcba78c2e2a447cbad705142407d364cdd90f64b0838c46199386b80913f`.
The build log is `artifacts/sept24-release-final.log`. Installation over the
existing app completed with exit code 0; the installed desktop app, CLI,
ffmpeg and PDFium are present, and `oculus --version` succeeds.

The installed app then upgraded the real library to schema 37. SQLite
integrity and foreign-key checks pass, all 560 file records remain, and the
Subjects view displays the four current subjects and 18 past subjects in
English. Temporary Unicode test files were removed from the real app-data
directory. The existing Canvas session was rejected and needs sign-in again;
local parsing reports unavailable until an external MinerU server is running.

This update's tests do not send coursework to MinerU or Voyage, invoke billed
models, or exercise fresh external-provider sign-in. Earlier live checks below
describe their dated builds and do not establish live-service compatibility
for this update.

## Publication review — 2026-09-19

The pre-publication review fixed three additional failure cases: Windows
`auth login` confusing the case-insensitive CLI name with the desktop binary;
committed checkbox edits hiding later selections from another Sync tab; and
upload deletion becoming impossible to retry after the database-row deletion
failed. File-count badges now refresh from committed metadata events too.

Focused validation passed: seven CLI tests (including the Windows filename
collision), ten upload tests (including repeated deletion and missing-path
junction checks), and all 20 frontend tests with 100 assertions. TypeScript
and Vite also passed. These extend the full-suite results below; unchanged
Python and WSL checks were not rerun for the publication-only fixes.

The final `bun run tauri build` passed, including the release CLI, TypeScript,
Vite, native desktop binary and NSIS packaging. The resulting installer at
`app/src-tauri/target/release/bundle/nsis/Oculus_0.1.0_x64-setup.exe` is
48,029,304 bytes, with SHA-256
`ab61536b2bcaaeaf11f0d11780414a690f5903f90432b98fc22f4203361d4702`.
The build log is `artifacts/publish-release-build.log`. This publication build
was not reinstalled; the installed-app checks below describe the preceding
validated build.

The Windows README now documents the actual build, installer, WSL2 setup,
data directory and provider network behavior. The source and inherited Git
history were checked for common credential patterns and accidental runtime
artifacts before publication; local databases, recovery backups, installers,
dependencies and model caches remain ignored.

## Andre update — 2026-09-19

Imported the changes from macOS branch `andre` through
`e63012830712d83af054f3d4536adec7315c0d20`: personal subject uploads,
Office documents in the PDF pipeline, and academic term ordering. Windows
path handling, native shortcuts, packaged Python and WSL2 Claude remain in
place. The macOS working tree remains unchanged.

Additional fixes cover the Sync picker's lexical sorting, stale Summer-term
defaults, mounted subject refresh, serialized database writes and parse
events, and failures that previously appeared as successful syncs. SQLite
connections now use one resolved physical filename; the bundled engine is
3.53.4. Runtime tests check its exact version and source ID.

| Check | Result |
| --- | --- |
| Rust library | 184 passed; one opt-in real Office test excluded from this suite. `artifacts/andre-rust-library-tests.log`. |
| CLI and Windows credentials | 11 CLI tests and one credential test passed, including actual SQLite failure cases. `artifacts/andre-rust-cli-tests.log`. |
| Frontend | 15 tests / 79 assertions passed; TypeScript and Vite build passed. `artifacts/andre-frontend-tests.log`. |
| Python | All 38 tests passed, including oversized spreadsheet render limits and an isolated HTTP-parent import check. `artifacts/andre-sidecar-tests.log`. |
| Real Office imports | Installed LibreOffice 26.8.0.3 converted DOCX/PPTX/XLSX through the actual import function. Original bytes and repeated import names/mtime were preserved. Unicode and spaces were present in the paths. `artifacts/andre-office-import-smoke.log`. |
| Office parse output | Word produced two pages; PowerPoint and Excel one each. Expected text markers and Excel's calculated 36 survived conversion. Each passed isolated fast parsing and 1-based page JSON/markdown checks. `artifacts/andre-office-pdf-parse-validation.log`. |

The user's downloaded coursework survived a corrupt SQLite file table. A
closed backup was recovered with the checksum-verified official SQLite tool,
preserving every row in all 23 other tables, including migrations, settings,
subjects, lectures and history. All 192 directly recovered file rows matched
their on-disk files and sizes; integrity and foreign-key checks passed before
replacement. The original database and journals remain under
`artifacts/sync-recovery-20260918/`. No auth credentials or coursework were
deleted. The repair addresses the file index; parsing and embeddings remain
rebuildable derived data.

The rebuilt CLI then completed a real four-subject metadata sync in 187.4
seconds, reusing the downloads and restoring 560 unique file rows from 568
artifact events: MAST30034 135, INFO30006 162, MAST30027 175, MAST30001 88.
It also restored 152 calendar events. Both sync records remained completed,
with no database error; post-sync integrity and foreign-key checks passed.
Legacy journals in both the MSIX package layer and ordinary Roaming layer
were retained in the recovery backup before reopening the database.

The release build and per-user NSIS installation passed. That validated installer
was `app/src-tauri/target/release/bundle/nsis/Oculus_0.1.0_x64-setup.exe`,
48,028,399 bytes, SHA-256
`16be153a3985779181a639de320c9232aae9a564900aa83f1304787b76eb8f8a`.
The installed `artifacts/Oculus/app.exe` opened the repaired library with its
managed sidecar healthy. Native UI checks confirmed the four Semester 2
subjects selected, Semester 1 above Summer Term, restored subject overview
and download rows, and a rendered 27-page lecture PDF. The Uploads page and
native multi-file picker opened successfully; cancelling returned cleanly to
the subject without adding test files to the user's library.

## Initial Windows and WSL2 validation — 2026-09-18

The initial Windows checkout imported macOS commit
`af492ed410ace30ddd43e4657109f3c45388a6ec`; the current imported revision is in
[`upstream.json`](../upstream.json). Windows changes are confined to
`oculus_window`; the original `oculus` checkout is unchanged and `MRI` was
neither read nor modified.

Validation used Windows x64, an NVIDIA RTX 4070 with 12 GB VRAM, driver
591.86, Python 3.12.14, and the locked torch 2.13.0+cu130 /
torchvision 0.28.0+cu130 pair. Results below describe this machine and these
fixtures, rather than every supported device or university account.

## Completed

| Check | Evidence / outcome |
| --- | --- |
| Rust regressions | 171 tests passed: 161 library, 9 CLI, and 1 isolated Windows credential test. [Log](../artifacts/claude-wsl-rust-tests.log). |
| WSL2 sandbox and broker | 17 tests passed inside the real WSL2 environment, including the installed inner Claude sandbox, concurrent FIFO requests, Unicode batches, interrupted-turn isolation, and descendant cleanup. [Log](../artifacts/wsl-supervisor-tests.log). |
| Frontend provider readiness | 5 tests passed (24 assertions), plus TypeScript and Vite production builds. |
| Python regressions | 34 tests passed, including actual descendant cleanup, nested Job Objects, forced worker exit/restart, Unicode pipes and PDF paths, parser routing, and memory enforcement. [Log](../artifacts/sidecar-tests.log). |
| Locked runtime | `uv sync --locked --no-dev --python 3.12` passed against the committed Windows dependency lock. |
| Python bootstrap | Fresh setup, repeated setup, and recovery from the managed-Python junction failure were verified. The bootstrap uses the full patch-version interpreter and preserves runtime state while reconciling packaged sources. |
| Codex integration | Live response streaming passed. An allowed `agents/` write succeeded and an outside synthetic sentinel write received Windows Access Denied. [Boundary trace](../artifacts/codex-physical-validation-1789734235175.jsonl). The rebuilt CLI also passed an allowed write using the normal, logical `APPDATA`, automatically resolving the physical library path. [Rebuilt CLI trace](../artifacts/codex-canonical-validation-allowed-1789734457776.jsonl). |
| Real local parsing and retrieval | A synthetic two-page PDF with a Unicode/spaced filename passed fast extraction and real MinerU quality parsing, produced two Qwen page-image embeddings, and ranked the correct page first for both text queries. No cloud parsing was used. |

Codex Desktop's MSIX package virtualized the initial AppData location. Passing
that logical path to the sandbox caused Windows error 267; resolving the
existing library and `agents/` directories to their physical paths fixed shell
startup without expanding the writable area. The rebuilt CLI used normal
`APPDATA` with no test override. Its allowed-write marker was checked by the
host; the outside sentinel remained unchanged. A repeat negative prompt was
refused by the agent's instructions before shell execution, so the OS-denial
evidence is the separate boundary trace above. All probes used synthetic
files, and their production-library markers were removed afterward.

The real-model smoke completed in **19.3 seconds** with cached weights. Stored
vectors retained the existing **512-dimensional normalized float16** contract.
The unchanged default **8192 MiB** governor budget reported a **7083 MiB** peak,
complete process-tree measurement and **zero kills**. Query scores were
0.601 versus 0.186 for the vector page and 0.643 versus 0.182 for the calculus
page. This is a small-fixture functional check, not a large-library benchmark.

Evidence: [smoke log](../artifacts/smoke-20260918-221745.log),
[result JSON](../artifacts/oculus-smoke-q_ldaukf/result.json),
[isolated SQLite fixture](../artifacts/oculus-smoke-q_ldaukf/retrieval.db),
and [memory profile](../artifacts/memory-20260918-221745.jsonl).
These generated artifacts are local validation output, not required runtime
resources or replacements for the user's application database.

The **final installed app-owned sidecar** also passed the full HTTP smoke:
fast extraction, real local MinerU quality parsing, two page-image embeddings
and both correct query rankings completed in **49.6 seconds**. Its unchanged
8192 MiB cap reported a **4909 MiB** resident peak, complete accounting and
**zero kills**. The source venv ran only the HTTP test driver; model inference
ran in the installed app's managed runtime.
[Installed HTTP log](../artifacts/installed-sidecar-smoke.log) and
[result JSON](../artifacts/oculus-smoke-bskocc8y/result.json).

The compiled Rust `retrieval_smoke.exe` then used this isolated SQLite fixture
through the same installed sidecar. Forced ingestion stored both vectors and
both page markdown records; a repeated ingestion correctly skipped embedding.
Statistics reported one file and two pages, and both searches returned their
expected page first in approximately 37 ms each.
[Rust integration log](../artifacts/installed-rust-retrieval.log).

Windows measures resident host RAM using `WorkingSetSize`, reported as
`working_set`; private commit is a separate diagnostic quantity. Profiling
showed 11,131 MiB committed while only 1,152 MiB was resident during CUDA
initialisation. A native committed-but-untouched/touched allocation test
verifies the distinction, and the balloon test still verifies termination
when actual resident memory exceeds its test cap. This metric matches the
POSIX RSS fallback but is not identical to macOS physical footprint. It does
not fully account for GPU-managed memory; CUDA OOM recovery remains active.
This historical sidecar was removed in the September 24 update; see the
current [parsing architecture](parsing.md).

## Initial installed-build checks

The normal `bun run tauri build` completed, including CLI documentation
generation and the NSIS installer. Silent installation returned exit code 0.
The installed desktop executable is `artifacts/Oculus/app.exe`; the adjacent
`oculus.exe` is the command-line tool.

The initial installer was
`app/src-tauri/target/release/bundle/nsis/Oculus_0.1.0_x64-setup.exe`
(44,288,927 bytes), with SHA-256
`03ccc6d5f8d5aa7a947019cdd9c25d84590586cd6fb3fa72d5f351ca9f4b9f3c`.
The installed desktop binary differs from the build output only in Tauri's
three-byte NSIS bundle-type marker (`NSS` versus `UNK`).

Native checks against the installed build passed:

- Settings displayed a healthy local service, the default 8 GB memory cap,
  and detected Codex. Claude's original availability limitation was subsequently
  replaced by the WSL2 integration described below.
- Ctrl+K opened the command palette and Escape closed it. Ctrl+T created a
  Subjects tab, and Ctrl+W closed that tab and returned to Settings.
- F11 entered fullscreen, and Escape restored the native title bar.
- Launching a second instance restored the existing minimized window without
  creating a second app or sidecar process.
- Alt+F4 closed the app, all six observed Python worker descendants and the
  sidecar, and released port 9547. A preceding build also passed forced app
  termination cleanup. The final native shortcut checks covered the main
  webview; shortcuts focused inside a remote browser were not separately
  exercised.

The frontend type check and production bundle passed. Five frontend health-store
tests cover readiness, stale errors, and shared refresh behavior. The macOS
checkout remained clean, and the Windows diff passed `git diff --check`.
Across the Windows port, the automated suites cover 227 passing tests: 171
Rust, 34 sidecar Python, 17 WSL supervisor, and 5 frontend tests.

## Claude through WSL2

Claude Code runs in a dedicated Ubuntu 24.04 WSL2 distribution named `Oculus`,
as the ordinary `oculus` user. The native Windows app owns its lifecycle and
brokers the supported native Oculus commands. Linux Claude sign-in was completed
by the user. Setup leaves the existing default WSL distribution unchanged.
See [setup and maintenance](claude-wsl-setup.md) and
[the harness boundary](harness.md).

The outer sandbox exposes the Windows library read-only and its `agents/`
folder read/write. Windows drive mounts and executable interop are hidden.
The inner Claude Bash sandbox keeps Unix sockets blocked; four precreated FIFO
pairs carry bounded requests to the native CLI broker. Commands remain literal
argument arrays and never become Windows shell text.

A real Claude conversation against an isolated synthetic library passed streamed
text, a Unicode lecture read, an `agents/` write, and `oculus --version` through
the native Windows broker. The same provider conversation resumed with its
previous context, produced rewind anchors, accepted rewind, interrupted a tool,
and completed the next turn. The command result itself was checked; a model's
claim of success was not used as evidence.
[Final-build live integration trace](../artifacts/claude-wsl-live-20260918-e.log).

The no-model tests also check that malformed or partially written FIFO frames
cannot disable a command slot, stale responses cannot reach a new client, and
commands queued before interruption cannot run in a later turn. Both outer
filesystem restrictions and the actual inner sandbox are exercised. The fixture
has no `lectures/` folder, covering fresh-library initialization.

The rebuilt NSIS installer completed with exit code 0. In the installed app,
Settings → AI completed its live health probe and displayed **Claude Code via
WSL2**, the `Oculus` distribution, and Claude version `2.1.267`, alongside Codex.
The app's local service also started and listened on port 9547. Further model
picker clicks were stopped when the user began interacting with the window;
the live integration tests above use the same compiled harness against isolated
fixtures. The installed app was left running for the user.

## Scope of the initial validation

The initial September 18 checks did not verify live Canvas, Ed or Echo
authentication or coursework synchronization against the user's accounts.
The September 19 Andre validation above subsequently exercised a real
four-subject metadata sync with existing credentials and downloads. Fresh
authentication and every provider-specific workflow remain outside that check.
