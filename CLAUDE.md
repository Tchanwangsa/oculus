# Oculus — agent instructions

A Tauri 2 desktop app that turns UniMelb coursework (Canvas, Ed Discussion,
Echo360) into a searchable personal knowledge base. Three processes:

- **Frontend** — React 19 + Vite + Tailwind v4, in `app/src/`
- **Rust core** — Tauri backend, scrape engine, and the `oculus` CLI, in `app/src-tauri/`
- **Python sidecar** — PDF parsing + page-image embeddings, in `sidecar/`

Ingestion, retrieval, and the chat agent are built (BYOK — local Ollama,
OpenRouter, OpenCode Go, or any OpenAI-compatible endpoint; see
`docs/llm.md`). Automations and the Inbox were built and then removed — they
live on the `automations` branch; do not reintroduce pieces of them here
without being asked.

## Orient before you edit

`docs/` is the map of this repo: where each piece lives, how the three
processes connect, and which measured facts the design rests on. **Read the
relevant page before exploring source** — it is far cheaper than rediscovering
structure by searching.

| You are working on… | Read first |
| --- | --- |
| Anything, unsure where things are | `docs/index.md` |
| How the processes talk, the data dir, the database | `docs/architecture.md` |
| Scraping Canvas / Ed / Echo360 | `docs/sync.md` |
| Sign-in, session cookies, keep-alive | `docs/auth.md` |
| PDF parsing, the Python sidecar | `docs/sidecar.md` |
| Embeddings, search, the `pages` table | `docs/retrieval.md` |
| LLM providers, keys, the chat agent | `docs/llm.md` |
| Class times, due dates, the calendar | `docs/calendar.md` |
| React pages, stores, hooks, UI system | `docs/frontend.md` |
| The `oculus` command line | `docs/cli.md` |
| Building, running, toolchains | `docs/development.md` |

The `read-docs` skill routes you there; `write-docs` covers updating them; and
`check-doc-drift` audits them against the code when you want a sweep.

## Keep the docs true

Docs and code ship in the same change. When you add, move, rename, or delete a
feature, update the matching page in `docs/` in that same commit. Keep the
pages high-level — where things live, how they connect, and why a shape is the
way it is — not line-by-line detail.

If a doc contradicts the code, trust the code, then fix the doc.

Pages cite source paths in backticks, **repo-relative** (e.g.
`app/src-tauri/src/sync.rs`). Those citations are load-bearing —
`check-doc-drift` resolves them against the filesystem to find stale pages, so
keep them exact.

Do not create per-directory `CLAUDE.md` files. This file holds conventions;
`docs/` holds structure.

---

# Toolchain

- **bun, never npm/yarn/pnpm.** `app/bun.lock` is the only lockfile; other
  lockfiles are gitignored. Tauri itself shells out to `bun run`, so an
  npm-installed `node_modules` is not what ships. One-off CLIs run with
  `bunx`, not `npx`.
- Rust builds with plain cargo (via `bun run tauri dev/build`, or
  `bun run cli` for the `oculus` binary).
- The sidecar is managed by **uv**: `cd sidecar && uv sync` creates the
  `.venv` the Rust supervisor looks for. Dependency pins in
  `sidecar/pyproject.toml` are deliberate — several are workarounds
  (`docs/sidecar.md` lists them). Do not "clean up" pins without reading it.

# Hard-won constraints (do not relearn these)

- **No work in hidden WebViews.** macOS suspends an off-screen WKWebView's
  content process, which silently freezes anything running there. All
  scraping lives in Rust (`app/src-tauri/src/sync.rs`) for exactly this
  reason. Never move background work back into a WebView.
- **Canvas API tokens are blocked by the university** — the admin has
  disabled self-service access tokens, so the session cookie is the only auth
  path. Don't re-propose `Authorization: Bearer`. See `docs/auth.md`.
- **A silent sidecar is not a hung sidecar.** Python block-buffers stdout on
  a pipe; `/parse-status` on the sidecar HTTP port is authoritative, stdout
  is not.
- **Retrieval embeds page images, not extracted text** — measured, not
  aesthetic. Image embeddings roughly double recall on formula/diagram pages.
  Don't switch to text embeddings or average the two; see `docs/retrieval.md`.

# UI conventions

The design direction is Linear-style: white/muted-grey palette, muted indigo
`#5e6ad2` accent, Inter Variable, Notion-style layout (sidebar subjects →
per-subject underline tabs, peek panel for files, top tab strip).

- Components are **shadcn/ui** — source in `app/src/components/ui`, config in
  `app/components.json`, primitives from the unified `radix-ui` package. Add
  with `bunx shadcn@latest add <name>`, then swap the generated `lucide-react`
  imports for `@phosphor-icons/react`.
- All colors go through the semantic tokens in `app/src/index.css` (light +
  `.dark` class). Trap: in shadcn's vocabulary `accent` is the quiet hover
  surface, **not** the brand colour — the brand indigo is `primary`.
- Dark mode is a `.dark` class on `<html>` driven by `app/src/lib/theme.ts`;
  `index.css` declares `@custom-variant dark` so `dark:` follows the class,
  not the OS.
- **Zoom is the webview's page zoom** (`setZoom` in
  `app/src/layouts/AppLayout.tsx`), never a CSS `zoom` on a container: inside
  a CSS-zoomed subtree WebKit reports pointer coordinates in visual pixels but
  element rects in layout pixels, which quietly breaks every popup's collision
  maths and any drag that mixes the two. Popups portal normally. Chrome that
  must match native furniture (the traffic-light gap) divides by
  `--app-zoom`.
- **No toasts, no bottom progress bars** — background jobs surface in the
  sidebar only. **No placeholder UI** for unbuilt features: only ship
  wired-up controls.
- No icons in section headers, no stat cards, no filler copy. Humanize
  kebab-case slugs for display (`humanizeSlug` in `app/src/lib/format.ts`);
  show Canvas codes via `displayCode` ("MULT20015", not "MULT20015_2026_SM2").
- After UI changes, screenshot the running dev app to verify
  (`screencapture -x -o -l<windowid>`; the window owner is "app" in dev).

# Git

- Single branch: `master`. Commit messages follow the existing
  `feat:`/`fix:`/`refactor(scope):` style — read `git log --oneline` and match.
- `data/`, `*.db`, `sidecar/.venv/`, and `app/src-tauri/binaries/` are
  gitignored user-state or fetched artifacts — never commit them.
