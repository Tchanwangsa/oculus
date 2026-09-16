---
name: read-docs
description: Orient in the Oculus repo using the docs under docs/. Use at the start of any task touching app/ or sidecar/ — before exploring the codebase manually — and when asked where a feature lives, how the processes connect, or what conventions apply.
---

# Read the docs first

This repo keeps maintained, high-level documentation in `docs/` — plain
Markdown, read with Read/Grep, no build step. Read the relevant page
**before** spelunking through source: it tells you where each piece lives,
how the three processes (React frontend, Rust core, Python sidecar) connect,
and which measured facts the design rests on. That last part matters here —
several decisions (image embeddings, MinerU, the dependency pins) look
arbitrary from the code and are not.

## Doc map

| You are working on… | Read first |
|---|---|
| Any task, unsure where things are | `docs/index.md` |
| Process boundaries, IPC, data dir, `oculus.db` | `docs/architecture.md` |
| Scraping Canvas / Ed Discussion / Echo360 | `docs/sync.md` |
| Sign-in, session cookie, keep-alive, tokens | `docs/auth.md` |
| PDF parsing, the sidecar, python deps | `docs/sidecar.md` |
| Embeddings, search, the `pages` table | `docs/retrieval.md` |
| Chat: the CLI-agent bridges, containment, the timeline | `docs/harness.md` |
| Class times, due dates, the calendar | `docs/calendar.md` |
| React pages, stores, hooks, event bridge | `docs/frontend.md` |
| The `oculus` CLI | `docs/cli.md` |
| Building, running, checks | `docs/development.md` |

## Conventions

`CLAUDE.md` at the repo root is the binding convention set (bun not npm,
shadcn + Phosphor, semantic color tokens, no toasts, no placeholder UI, no
work in hidden WebViews). `AGENTS.md` points at it. Follow it for all new and
refactored code.

## Rules

1. Cite the doc you used when explaining structure decisions.
2. If a doc contradicts the code, trust the code — then fix the doc in the
   same change.
3. When you add, move, rename, or delete a feature, update the matching page.
   Use the `write-docs` skill. To audit the whole set for staleness, use
   `check-doc-drift`.
4. Do not create per-directory `CLAUDE.md` files. Conventions live in the
   root `CLAUDE.md`; structure lives in `docs/`.
