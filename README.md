# Oculus

A macOS desktop app that turns a UniMelb student's coursework into a
searchable knowledge base — and then lets a coding agent answer questions
against it.

Oculus signs in to Canvas the way you do, scrapes your subjects (pages, files,
assignments, modules), pulls Ed Discussion threads and Echo360 lecture
recordings, parses every PDF to markdown, embeds each page as an *image*, and
retrieves the right pages on demand. Chat is not a chatbot bolted on: it is
Claude Code, Codex, opencode or Antigravity, run as a subprocess inside your
library, with the `oculus` CLI as its tool surface.

There's no Oculus server and no Oculus account. Sync talks only to the
university's own systems, over the session cookie you're already signed in
with. But **PDF parsing and page-image embedding are cloud calls** — every
PDF goes to MinerU (their service, or a MinerU server you install and run
yourself) and every page image goes to Voyage for embedding, in-process from
Rust, no local model or fallback of either kind left in this repo. If that's
not the trade you want, run MinerU yourself and know the images still leave
the machine for Voyage — there's no local embedder here to switch to instead.

---

## What it does

**Sync.** Canvas is scraped from Rust over your session cookie — the
university blocks self-service API tokens, so the cookie is the only auth
path. Ed Discussion threads and Echo360 recordings come along with it, keyed
to the same subjects. Incremental: unchanged files cost one metadata call, not
a download. → [docs/sync.md](docs/sync.md)

**Sign-in that stays signed in.** Canvas SSO runs through Okta. With your
password and a TOTP seed in the keychain, Oculus rebuilds a dead session
headlessly, and a LaunchAgent keeps it warm every six hours while the app is
closed. → [docs/auth.md](docs/auth.md)

**PDF parsing, no fallback.** Every PDF is read by MinerU — its cloud service,
or a MinerU server you install and run yourself, reached over loopback —
chosen once in Settings → Library. Both are plain HTTP calls made in-process
from Rust; neither is a process Oculus starts, supervises or ships. There is
no fast tier and no second engine to catch a failure: a PDF either gets
markdown or it doesn't, and the UI says which. → [docs/parsing.md](docs/parsing.md)

**Retrieval over page images.** Each page is rasterised with pdfium and
embedded by Voyage's `voyage-multimodal-3.5` as a 512-dim vector — not
extracted text. Measured, not aesthetic: image embeddings roughly double
recall on formula and diagram pages, and averaging image with text scored
worse than image alone. A brute-force scan over a degree's worth of pages
costs single-digit megabytes and milliseconds — no vector index. Alongside it,
a local FTS5 index over the same parsed text answers a keystroke instantly and
for free, so ⌘K can search inside your documents without a cloud round trip.
→ [docs/retrieval.md](docs/retrieval.md)

**Chat as a CLI agent.** Claude Code, Codex, opencode or Antigravity, driven
as a subprocess from the library's `agents/` folder, contained to it, with its
work surfaced as a timeline rather than a spinner. Claude Code, Codex and
Antigravity ride your existing subscription — the whole reason to drive them
rather than an API; opencode is the odd one out and spends per token against
whatever provider key you've connected, earning its place by reaching every
provider at once instead of being free. → [docs/harness.md](docs/harness.md)

**Bring your own files.** A tutor's handout, an old exam, anything not on
Canvas — add it to any subject and it's parsed, embedded and searchable
exactly like a scraped file. It's the one folder a sync never touches, which
is what makes it the one folder you can delete from freely.

**Lectures.** Echo360 recordings download and play in-app. Chapter boundaries
are detected and named by a CLI agent, shown as a dock list, a
current-chapter strip and ticks on the scrub bar. A reading copy rewrites the
transcript into one sentence per moment — spoken maths set as maths — as an
alternate register of the same transcript tab.
→ [docs/chapters.md](docs/chapters.md)

**Projects.** An assignment broken into tasks: an Overview with a brief, tags
and a pinned deadline; the tasks on a board, a table or a timeline; a page per
task. A second tab holds a universal view across every project at once,
filtered by status, subject or due date, for the tasks that belong to no
project at all. All of it writable by the agent through the CLI.
→ [docs/projects.md](docs/projects.md)

**Calendar.** Class times, due dates and recordings on one grid.
→ [docs/calendar.md](docs/calendar.md)

**An in-app browser.** External links open as tabs in Oculus's own strip,
already signed in to Canvas, with the site's own favicon, honest back/forward,
page zoom and find in page.

**A headless CLI.** The same engine without the window: `oculus run` to
scrape, `oculus search` and `oculus grep` to find, `oculus auth` to sign in,
plus the `project`/`task` commands the agent plans through, and `oculus agent`
to drive any of the four bridges from a terminal.
→ [docs/cli.md](docs/cli.md), [docs/cli-reference.md](docs/cli-reference.md)

---

## Two processes

| Process | Lives in | Does |
| --- | --- | --- |
| **Frontend** | `app/src/` | React 19, Vite, Tailwind v4, shadcn/ui |
| **Rust core** | `app/src-tauri/` | Tauri commands, the scrape engine, PDF parsing and page embedding (both in-process, over HTTPS to MinerU and Voyage), retrieval, the CLI-agent bridges, the `oculus` CLI |

There used to be a third: a Python sidecar ran parsing and embedding locally.
It's gone — code and directory both, at `f875bb1` — and nothing in this repo
starts, supervises or ships a child process for either job any more.

All scraping lives in Rust for a specific reason: macOS suspends an off-screen
WKWebView's content process, so background work in a hidden WebView silently
freezes. → [docs/architecture.md](docs/architecture.md)

---

## Setup

### Prerequisites

- **[bun](https://bun.sh)** — never npm/yarn/pnpm. `app/bun.lock` is the only
  lockfile, and Tauri itself shells out to `bun run`.
- **Rust** (stable, via [rustup](https://rustup.rs))
- **[sccache](https://github.com/mozilla/sccache)** —
  `brew install sccache`. Not optional: `app/src-tauri/.cargo/config.toml` sets
  `rustc-wrapper = "sccache"`, and cargo fails outright if the wrapper is
  missing (`could not execute process sccache ... (never executed)`).
- **macOS.** The keep-alive LaunchAgent, the window chrome and the WebView
  behaviour notes are all macOS-specific.

There is no Python step, no `uv`, and no `sidecar/` directory. If you have an
orphaned `sidecar/.venv` from an older checkout, it's 1.2 GB of nothing —
delete it.

### First run

```sh
cd app
bun install
bun run predev      # ffmpeg, libpdfium and the debug `oculus` CLI, in one step
```

`predev` is the whole preflight, is idempotent, and is also what `bun run dev`
and `bun run tauri dev` run first on their own — you rarely need to call it by
hand. `OCULUS_SKIP_PREDEV=1` skips it when you only want vite.

### Running

```sh
cd app
bun run tauri dev       # the app
bun run tauri build     # release build → .app bundle
bun run cli             # the headless `oculus` binary, release profile
bun run cli:install     # + symlink into ~/.local/bin
```

The first Rust build is cold and slow — several hundred crates. After that
sccache makes rebuilds quick.

→ [docs/development.md](docs/development.md) for the full command list, the
dev-CLI staleness guard, environment overrides, and the test/benchmark
commands.

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
├── oculus.db                  # subjects, files, pages + embeddings, projects, chat threads
├── courses/<CODE>/            # files/, pages/, modules/, ed/, assignments/, images/, uploads/
├── agents/                    # the CLI agent's workspace
├── canvas-session.cookie      # replayed on every Canvas request
├── canvas-session/authenticated
├── mineru-usage.json          # daily parse allowance + quota latch
├── voyage-usage.json          # embedding allowance, rate-limit tier, the spend guard
└── file-manifest.json         # what's downloaded, for incremental syncs
```

Deleting that directory is a full reset, auth included. Canvas-derived content
re-syncs; a subject's own uploads, chats, projects and settings do not.
Credentials live in the macOS keychain (Canvas SSO, the MinerU token, the
Voyage API key) separately from all of the above — no model weights sit on
disk anywhere, since neither cloud client downloads one.

---

## Repo layout

| Path | What it is |
| --- | --- |
| `app/src/` | React frontend — routes, layouts, stores, the UI system |
| `app/src-tauri/src/` | Rust — commands, `sync.rs`, `okta.rs`, `retrieval.rs`, and the `parse/`/`embed/`/`harness/` seams |
| `app/src-tauri/src/bin/oculus.rs` | The headless CLI over the same engine |
| `docs/` | The map: where things live, how they connect, why |
| `CLAUDE.md` | Conventions — toolchain, UI rules, hard-won constraints |

Start at [docs/index.md](docs/index.md). The pages are written to be read
*before* exploring source, and they record measured facts and dead ends rather
than restating the code.

---

## Status

**Built:** Canvas SSO and sync, Ed Discussion, Echo360 download and playback,
PDF parsing (MinerU cloud or a MinerU you run), page-image retrieval plus a
local lexical index, chat as a CLI agent over four bridges (Claude Code,
Codex, opencode, Antigravity), your own files alongside the scraped ones,
lecture chapters and the transcript's reading copy, projects with a universal
tasks view, the calendar, a home launcher, and an in-app browser.

**Removed:** the **Python sidecar** — PDF parsing and page embedding run in
Rust now, against MinerU and Voyage; the supervisor, the loopback callback
server and the whole-tree memory governor went with it. The last commit
holding `sidecar/` is `f875bb1`, which is where a separate local-MinerU-server
repo forks from.

**Removed:** the **BYOK API layer** — provider config, keychain keys, an
OpenAI-compatible streaming client, spend limits. Deleted rather than woken
up once the CLI-agent bridges replaced it; opencode is the API path now,
reached as a bridge rather than a parallel world.

**Removed:** **automations and the Inbox**, and scheduled sync along with
them — sync is manual-only. The last commit that has them is `d64dc11`,
reachable from master's history.

---

## Not affiliated with the University of Melbourne

Oculus is a personal tool that signs in as you, to material you already have
access to, and keeps your Canvas session local to your own machine.
