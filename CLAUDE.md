# Oculus — agent instructions

A Tauri 2 desktop app that turns UniMelb coursework (Canvas, Ed Discussion,
Echo360) into a searchable personal knowledge base. Three processes:

- **Frontend** — React 19 + Vite + Tailwind v4, in `app/src/`
- **Rust core** — Tauri backend, scrape engine, and the `oculus` CLI, in `app/src-tauri/`
- **Python sidecar** — PDF parsing + page-image embeddings, in `sidecar/`

Ingestion and retrieval are built. Chat is a **CLI agent** — Claude Code or
Codex, driven as a subprocess from the library's `agents/` folder
(`docs/harness.md`). The BYOK API layer it replaced is dormant, not deleted:
nothing routes to it, and it is the planned third bridge (`docs/llm.md`).
Automations and the Inbox were built and then removed — they live on the
`automations` branch; do not reintroduce pieces of them here without being
asked.

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
| Chat: the CLI-agent bridges, containment, the timeline | `docs/harness.md` |
| LLM providers, keys, the dormant API path | `docs/llm.md` |
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
- **A drag needs `dataTransfer.setData()` or WebKit cancels it.** A
  `dragstart` handler that sets no data aborts the drag silently — no
  `dragover`, no `drop`, every handler correctly attached and nothing moves.
  That is why `app/src/components/projects/ProjectBoard.tsx` and
  `app/src/components/llm/FallbackList.tsx` each set a `text/plain` payload
  nothing ever reads. It looks like dead code; deleting it breaks the feature
  without breaking a type or a test.
- **Retrieval embeds page images, not extracted text** — measured, not
  aesthetic. Image embeddings roughly double recall on formula/diagram pages.
  Don't switch to text embeddings or average the two; see `docs/retrieval.md`.

# UI conventions

The design direction is quiet and neutral: a dead-grey white palette (no warm
or blue cast in the greys — anything else fights the accent), muted indigo
`#5e6ad2` as the one colour, **Manrope for headings / Inter for everything
else**, and Notion-style layout (sidebar subjects → per-subject underline tabs,
docked side panel for files and lectures, top tab strip).

- **The shell frames a floating document.** The window ground is
  `background`; the sidebar and tab strip sit directly on it with no fill or
  divider of their own, and content is an inset rounded `card` with a hairline
  border (`app/src/layouts/AppLayout.tsx`). Tabs are pills on that ground, not
  browser tabs merging into the page. A sidebar divider or a rule under the
  tab strip breaks the effect — the card's border is the separation.
- **Buttons and chips are pills** (`rounded-full` in
  `app/src/components/ui/button.tsx`); rectangles are for segmented toolbars
  that override the radius at the call site.
- **Two primitives are deliberately a notch below stock shadcn**, whose sizes
  are drawn for a 16px-base web page while this app's body text is 14px and its
  furniture is h-6/h-8 throughout. `button.tsx` has `default` at `h-8`, not
  `h-9` — a 36px button was the tallest thing in most rows. `dialog.tsx` is
  `p-5`/`rounded-xl` on `border-border-subtle` with a 16px title and 13px
  description, instead of `p-6`/`rounded-lg` at 18/14. Every dialog in the app
  overrides only `max-w`, so the scale lives in the primitive; don't "restore"
  either to what `shadcn add` generates.
- **`text-base md:text-sm` on a field is a trap, and it is why `input.tsx` and
  `textarea.tsx` now carry one unconditional `text-[13px]`.** The pair is
  shadcn's iOS fix — mobile Safari zooms the page when a focused field is under
  16px — and this viewport is always past `md`, so the field was always 14px.
  Worse, Tailwind emits variant utilities *after* plain ones, so `md:text-sm`
  outranked every `text-xs`/`text-[13px]` a call site passed: the class sat in
  the DOM and did nothing. The three `text-[13px]!` bangs in the composers were
  written to beat it. Never reintroduce a `md:` size on a base field.
- **Monospace is for code, and nothing else.** Timestamps, durations, counts,
  IDs, keys and badges all take the body font — reach for `tabular-nums` when
  digits need to hold a column, which is what mono was standing in for. The
  only `font-mono` in the app is `app/src/components/markdown/MdComponents.tsx`
  (code blocks and inline code); keep it that way.
- Headings are Manrope via an `h1–h4` rule in `@layer base`, so most pick it
  up with no markup change; a title that isn't a heading element takes the
  `font-display` utility.
- Components are **shadcn/ui** — source in `app/src/components/ui`, config in
  `app/components.json`, primitives from the unified `radix-ui` package. Add
  with `bunx shadcn@latest add <name>`, then swap the generated `lucide-react`
  imports for `@phosphor-icons/react`.
- All colors go through the semantic tokens in `app/src/index.css` (light +
  `.dark` class). Two traps: in shadcn's vocabulary `accent` is the quiet
  hover surface, **not** the brand colour; and the indigo has two tokens —
  `primary` is the *fill* (buttons, active underline, today's date) while
  `brand` is the same colour as an *accent* (links, selection, in-flight
  progress, new-item chips). They are split so the accent can be retuned
  without restyling every button; use the one that matches the meaning.
- **Every base reset in `index.css` belongs inside `@layer base`.** Unlayered
  CSS outranks every layer, so a bare `*, ::before, ::after { border-color }`
  rule silently beat *all* `border-<colour>` utilities app-wide — active tab
  underlines, destructive button outlines and selected-row borders all
  painted plain grey with the class present in the DOM and dead in the
  cascade. In `@layer base` it stays the default and utilities win again.
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
- **No native date/time inputs.** `<input type="date">` and `datetime-local`
  are not styleable on macOS in any way that matters: WebKit draws them as
  separate editable segments that grey themselves when it thinks they are
  unfilled and light individually on hover, so one field reads as several
  controls at several weights. Worse, the picker they open is the *OS's* — in
  the OS's locale and calendar system, which on a machine set to Thailand
  renders Buddhist-era years. And they cannot be committed on `change`, because
  a half-typed field reports itself as empty. Use
  `app/src/components/projects/DateTimeField.tsx` — shadcn's `Calendar` in a
  popover — or build on it. A bare `type="time"` is tolerable: two segments and
  no calendar.
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
