# Oculus

A macOS desktop app that turns a UniMelb student's coursework into a local,
searchable knowledge base — and then lets a coding agent answer questions
against it.

Oculus signs in to Canvas the way you do, scrapes your subjects (pages, files,
assignments, modules), pulls Ed Discussion threads and Echo360 lecture
recordings, parses every PDF to markdown, embeds each page as an *image*, and
retrieves the right pages on demand. Chat is not a chatbot bolted on: it is
Claude Code or Codex, run as a subprocess inside your library, with the
`oculus` CLI as its tool surface.

Everything stays on your machine. There is no server, no account, and no
telemetry — the only network traffic is to the university's own systems, plus
MinerU's parsing service if you explicitly opt in.

---

## What it does

**Sync.** Canvas is scraped from Rust over your session cookie — the university
blocks self-service API tokens, so the cookie is the only auth path. Ed
Discussion threads and Echo360 recordings come along with it, keyed to the same
subjects. Incremental: unchanged files cost one metadata call, not a download.
→ [docs/sync.md](docs/sync.md)

**Sign-in that stays signed in.** Canvas SSO runs through Okta. With your
password and a TOTP seed in the keychain, Oculus rebuilds a dead session
headlessly, and a LaunchAgent keeps it warm every six hours while the app is
closed. → [docs/auth.md](docs/auth.md)

**Two-tier PDF parsing.** A fast pass (pymupdf4llm, ~2s per deck) means a sync
is never blocked on quality; MinerU then re-parses properly in the background
with layout detection, formula recognition and OCR, locally on your GPU or via
MinerU's cloud if you opt in. → [docs/sidecar.md](docs/sidecar.md)

**Retrieval over page images.** Pages are embedded as rendered images with
Qwen3-VL, not as extracted text. This is measured, not aesthetic: image
embeddings roughly double recall on formula and diagram pages. About 1 KB per
page, so an entire degree's worth of slides indexes into a few megabytes.
→ [docs/retrieval.md](docs/retrieval.md)

**Chat as a CLI agent.** Claude Code or Codex, driven as a subprocess from the
library's `agents/` folder, contained to it, with its work surfaced as a
timeline rather than a spinner. → [docs/harness.md](docs/harness.md)

**Lectures.** Echo360 recordings download and play in-app, with chapter
boundaries detected and named by an agent, shown as a dock list, a
current-chapter strip and ticks on the scrub bar.
→ [docs/chapters.md](docs/chapters.md)

**Projects.** An assignment broken into tasks: an Overview with a brief, tags
and a pinned deadline; the tasks on a board, a table or a timeline; a page per
task. All of it writable by the agent through the CLI.
→ [docs/projects.md](docs/projects.md)

**Calendar.** Class times, due dates and recordings on one grid.
→ [docs/calendar.md](docs/calendar.md)

**A headless CLI.** The same engine without the window: `oculus run` to
scrape, `oculus search` and `oculus grep` to find, `oculus auth` to sign in,
plus the `project`/`task` commands the agent plans through.
→ [docs/cli.md](docs/cli.md), [docs/cli-reference.md](docs/cli-reference.md)

---

## Three processes

| Process | Lives in | Does |
| --- | --- | --- |
| **Frontend** | `app/src/` | React 19, Vite, Tailwind v4, shadcn/ui |
| **Rust core** | `app/src-tauri/` | Tauri commands, the scrape engine, retrieval, the sidecar supervisor, the `oculus` CLI |
| **Python sidecar** | `sidecar/` | PDF parsing (pymupdf4llm + MinerU) and Qwen3-VL page embeddings, over HTTP on port 9547 |

All scraping lives in Rust for a specific reason: macOS suspends an off-screen
WKWebView's content process, so background work in a hidden WebView silently
freezes. → [docs/architecture.md](docs/architecture.md)

---

## Setup

### Prerequisites

- **[bun](https://bun.sh)** — never npm/yarn/pnpm. `app/bun.lock` is the only
  lockfile, and Tauri itself shells out to `bun run`.
- **Rust** (stable, via [rustup](https://rustup.rs))
- **[uv](https://docs.astral.sh/uv/)** for the sidecar's Python environment
- **[sccache](https://github.com/mozilla/sccache)** —
  `brew install sccache`. Not optional: `app/src-tauri/.cargo/config.toml` sets
  `rustc-wrapper = "sccache"`, and cargo fails outright if the wrapper is
  missing (`could not execute process sccache ... (never executed)`).
- **macOS.** The keep-alive LaunchAgent, the window chrome and the WebView
  behaviour notes are all macOS-specific.

### First run

```sh
cd app
bun install
bun run ffmpeg          # ffmpeg binary → src-tauri/binaries/
bun run stage-cli       # builds the oculus CLI and stages it as a sidecar

cd ../sidecar
uv sync                 # creates .venv (~1.2 GB, mostly torch)
```

`bun run stage-cli` is easy to skip and shouldn't be. `tauri.conf.json`
declares `binaries/oculus` as an `externalBin`, and `tauri-build` checks that
every `externalBin` resolves — including under `tauri dev`, which does *not*
run the staging step itself. Without it the first `bun run tauri dev` dies with
`resource path binaries/oculus-<triple> doesn't exist`, which doesn't point at
its own cause.

### Running

```sh
cd app
bun run tauri dev       # the app (spawns the sidecar itself)
bun run tauri build     # release build → .app bundle
bun run cli             # just the headless `oculus` binary
bun run cli:install     # + symlink into ~/.local/bin
```

The first Rust build is cold and slow — several hundred crates. After that
sccache makes rebuilds quick.

→ [docs/development.md](docs/development.md) for the full command list, the
sidecar's environment overrides, and the test/benchmark commands.

### Installing it properly

`tauri dev` binaries live under `target/`, which build caches routinely clear.
Since the keep-alive LaunchAgent stores an absolute path to the CLI, a
`cargo clean` can silently disable it — launchd keeps running the job, finds
nothing, and exits 0. For daily use, `bun run tauri build`, copy the `.app` to
`/Applications`, and sign in once from there so the agent points inside the
bundle.

---

## Where your data lives

```
~/Library/Application Support/com.tchan.oculus/
├── oculus.db                  # subjects, files, pages + embeddings, chats, projects
├── courses/<CODE>/            # files/, pages/, modules/, ed/, assignments/, images/
├── agents/                    # the CLI agent's workspace
├── canvas-session.cookie      # replayed on every Canvas request
├── canvas-session/authenticated
└── file-manifest.json         # what's downloaded, for incremental syncs
```

Deleting that directory is a full reset, auth included. Canvas-derived content
re-syncs; chats, projects and settings do not. Credentials live in the macOS
keychain under `com.oculus.unimelb-sso`, separately from all of the above.

Model weights (~5 GB) sit in the shared `~/.cache/huggingface`, downloaded
once.

---

## Repo layout

| Path | What it is |
| --- | --- |
| `app/src/` | React frontend — routes, layouts, stores, the UI system |
| `app/src-tauri/src/` | Rust — commands, `sync.rs`, `okta.rs`, `retrieval.rs`, `sidecar.rs` |
| `app/src-tauri/src/bin/oculus.rs` | The headless CLI over the same engine |
| `sidecar/` | Python — parsing and embeddings |
| `docs/` | The map: where things live, how they connect, why |
| `CLAUDE.md` | Conventions — toolchain, UI rules, hard-won constraints |

Start at [docs/index.md](docs/index.md). The pages are written to be read
*before* exploring source, and they record measured facts and dead ends rather
than restating the code.

---

## Status

**Built:** Canvas SSO and sync, Ed Discussion, Echo360 download and playback,
the two-tier PDF pipeline, page-image retrieval, chat as a CLI agent, projects,
lecture chapters, the calendar, the home launcher, an in-app browser.

**Part-built:** lecture recap — the backend job segments a recording and stores
windowed notes; the player tab that reads them is the next stage.

**Dormant:** the BYOK API layer (provider config, keychain keys, streaming,
spend limits). Nothing routes to it; it is the planned third chat bridge.

**Removed:** automations and the Inbox, along with scheduled sync — sync is
manual only. They live on the `automations` branch.

---

## Not affiliated with the University of Melbourne

Oculus is a personal tool that signs in as you, to material you already have
access to, and keeps it on your own machine.
