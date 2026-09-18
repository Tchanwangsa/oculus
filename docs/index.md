# Oculus docs

Oculus is a Tauri 2 desktop app that scrapes a student's UniMelb coursework —
Canvas pages and files, Ed Discussion threads, Echo360 lecture recordings —
into a local library, parses every PDF to markdown, embeds each page as an
image, and answers questions by retrieving the right pages. Chat is a coding
agent the student already has — Claude Code, Codex or opencode — run from the library
with the `oculus` CLI as its tool surface (see [harness.md](./harness.md)).

These pages are the map: where things live, how the pieces connect, and the
measured facts the design rests on. Conventions (toolchain, UI rules, git)
live in the root `CLAUDE.md`, not here.

## Reading order

| Page | What it covers |
| --- | --- |
| [architecture.md](./architecture.md) | The two processes, how they talk, the data directory, `oculus.db` |
| [sync.md](./sync.md) | The scrape engine: Canvas modules, Ed threads, Echo360 lectures, HTML→md |
| [auth.md](./auth.md) | Canvas session cookie, keep-alive, Ed `x-token`, Echo360 LTI |
| [parsing.md](./parsing.md) | PDFs to markdown: the parser seam, the two MinerU engines, the on-disk contract, failures |
| [retrieval.md](./retrieval.md) | Page-image embeddings, the `pages` table, query flow |
| [harness.md](./harness.md) | Chat as a CLI agent: the Claude Code, Codex and opencode bridges, containment, the timeline |
| [calendar.md](./calendar.md) | Class times, deadlines and recordings on one grid |
| [projects.md](./projects.md) | Assignments broken into tasks: the Overview, the board, a page per task, and the CLI the agent plans through |
| [chapters.md](./chapters.md) | Splitting a lecture recording at its topic boundaries — the detector, the two agent jobs, and what is not built yet |
| [frontend.md](./frontend.md) | Routes, layouts, stores, hooks, the UI system |
| [cli.md](./cli.md) | The `oculus` binary — headless sync from a terminal |
| [cli-reference.md](./cli-reference.md) | Every command and flag — generated from the binary, not hand-kept |
| [development.md](./development.md) | Building and running each piece |

## Repo layout

| Path | What it is |
| --- | --- |
| `app/src/` | React 19 frontend (Vite, Tailwind v4, shadcn/ui) |
| `app/src-tauri/src/` | Rust: Tauri commands, scrape engine, parsing, embedding, retrieval |
| `app/src-tauri/src/bin/oculus.rs` | The headless CLI over the same engine |
| `docs/` | These pages |
| `.agents/skills/` | Shared skills: `read-docs`, `write-docs`, `check-doc-drift` |
| `.claude/skills/` | Symlink to `.agents/skills/` for Claude Code |

## Status honesty

Built: Canvas SSO + sync, Ed Discussion sync, Echo360 download + player, PDF
parsing (MinerU cloud, or a MinerU you run), page-image retrieval, chat as a
CLI agent over it (Claude Code, Codex or opencode, driven as a subprocess — see
[harness.md](./harness.md)), projects: assignments broken into tasks, each
project opening on an Overview (a brief, tags, a pinned calendar event, what is
next) with its tasks on a board, a table or a timeline behind a Tasks tab and a
page per task — plus, on the same section's second tab, a **universal view** at `/tasks` over
every project at once, filtered by status/project/subject/due, where a task that
belongs to no project at all lives until it is filed
— all of it writable by the agent through the CLI
([projects.md](./projects.md)), a
**home launcher** at `/` — the composer over today's agenda, what you were
last in, and your projects — a **new-tab landing screen** at `/new`, where +
and ⌘T go: two doors (a browser tab, a new conversation) over your recent
pages, and nothing else — and an in-app browser: external links open as
tabs in Oculus's own tab strip, signed in to Canvas, with the site's own
favicon on the tab, back/forward that grey out honestly, page zoom, find in
page, and an address bar that suggests from where you have been. When a doc or
UI string implies more than this, the doc is wrong — fix it.

Built: **lecture chapters**. Boundaries are detected, named by a CLI agent and
stored from the command line (`oculus lecture chapters`) or the app, then read
as a dock list with a progress line on the playing entry, and a current-chapter
name plus scrub-bar ticks in the player.

Built: **the reading copy**, replacing the lecture recap. The backend job
segments a recording at its slide changes, asks a CLI agent to rewrite each
ten-minute window of the transcript as one sentence per line — pinned to its
second, spoken maths set as maths — checks that every line covers a few cues
and no more, and stores each window as it lands (`oculus lecture reading`, or
the player's dock). Its model is configured in Settings → AI. In the player it
is the Transcript tab's **Enhanced** register rather than a tab of its own, one
pick from the standard cues — and that picker is where a lecture's copy is
asked for in the first place.
[chapters.md](./chapters.md#the-reading-copy) describes the shared pipeline;
the dock is in [Reading them in the player](./chapters.md#reading-them-in-the-player).

Removed: the **Python sidecar**, code and directory both. PDF parsing and page
embedding run in Rust now, against MinerU and Voyage
([parsing.md](./parsing.md), [retrieval.md](./retrieval.md)); the supervisor,
the loopback port and the whole-tree memory governor went with it. Parsing on
this machine came back, but as an *engine you select* rather than a tier that
catches anything: Settings → Library offers MinerU's cloud service or a MinerU
server you install and run yourself, and **there is no fallback between them**.
A PDF that fails on the selected engine simply has no markdown, and the UI says
so. **The last commit holding `sidecar/` is `f875bb1`** — 32 files, 6,652
lines, every pin and measurement intact — which is where the separate
local-server repo forks from. Same pattern as automations at `d64dc11`:
moved, not lost.

Removed: the **BYOK API layer** — provider config, keychain keys, an
OpenAI-compatible streaming client, spend limits and its own agent tool loop —
which the CLI-agent bridges replaced. Nothing had routed to it for a while, so
it was deleted rather than carried; its page went with it. Migrations 16 and 17
stay, so `llm_usage`, `chats` and `chat_messages` still exist and are read by
nothing. The one measured fact worth keeping — how to ask macOS how much memory
a local model can actually have — is in
[retrieval.md](./retrieval.md#how-much-memory-a-local-model-can-actually-have).

Removed: **automations and the Inbox** — the trigger/condition/action canvas
that delivered sync digests. It worked, but it was a detour from the core, so
it was cut rather than carried. The last version that has it is commit
`d64dc11` (its docs page went with it); master keeps migrations 18,
20 and 21 so the tables still exist, unused, and reinstating needs no
migration. Scheduled sync went with it — sync is manual-only now.
