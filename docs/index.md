# Oculus docs

Oculus is a Tauri 2 desktop app that scrapes a student's UniMelb coursework —
Canvas pages and files, Ed Discussion threads, Echo360 lecture recordings —
into a local library, parses every PDF to markdown, embeds each page as an
image, and answers questions by retrieving the right pages. Chat is an agent
loop over those pages — it searches, reads files, and answers with citations
(see [llm.md](./llm.md)).

These pages are the map: where things live, how the pieces connect, and the
measured facts the design rests on. Conventions (toolchain, UI rules, git)
live in the root `CLAUDE.md`, not here.

## Reading order

| Page | What it covers |
| --- | --- |
| [architecture.md](./architecture.md) | The three processes, how they talk, the data directory, `oculus.db` |
| [sync.md](./sync.md) | The scrape engine: Canvas modules, Ed threads, Echo360 lectures, HTML→md |
| [auth.md](./auth.md) | Canvas session cookie, keep-alive, Ed `x-token`, Echo360 LTI |
| [sidecar.md](./sidecar.md) | The Python process: fast/quality PDF parsing, lifecycle, endpoints |
| [retrieval.md](./retrieval.md) | Page-image embeddings, the `pages` table, query flow |
| [llm.md](./llm.md) | Provider config, keychain keys, streaming, spend limits, the chat agent |
| [calendar.md](./calendar.md) | Class times, deadlines and recordings on one grid |
| [frontend.md](./frontend.md) | Routes, layouts, stores, hooks, the UI system |
| [cli.md](./cli.md) | The `oculus` binary — headless sync from a terminal |
| [development.md](./development.md) | Building and running each piece |

## Repo layout

| Path | What it is |
| --- | --- |
| `app/src/` | React 19 frontend (Vite, Tailwind v4, shadcn/ui) |
| `app/src-tauri/src/` | Rust: Tauri commands, scrape engine, retrieval, sidecar supervisor |
| `app/src-tauri/src/bin/oculus.rs` | The headless CLI over the same engine |
| `sidecar/` | Python (uv): PDF parsing (pymupdf4llm + MinerU) and Qwen3-VL embeddings |
| `docs/` | These pages |
| `.agents/skills/` | Shared skills: `read-docs`, `write-docs`, `check-doc-drift` |
| `.claude/skills/` | Symlink to `.agents/skills/` for Claude Code |

## Status honesty

Built: Canvas SSO + sync, Ed Discussion sync, Echo360 download + player, the
two-tier PDF pipeline, page-image retrieval, and the chat agent over it
(BYOK: local Ollama, OpenRouter, OpenCode Go, or any OpenAI-compatible
URL). When a doc or UI string implies more than this, the doc is wrong — fix
it.

Removed: **automations and the Inbox** — the trigger/condition/action canvas
that delivered sync digests. It worked, but it was a detour from the core, so
it was cut rather than carried. The last version that has it is the
`automations` branch (its docs page went with it); master keeps migrations 18,
20 and 21 so the tables still exist, unused, and reinstating needs no
migration. Scheduled sync went with it — sync is manual-only now.
