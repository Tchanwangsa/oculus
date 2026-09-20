# Oculus — agent instructions

A Tauri 2 desktop app that turns UniMelb coursework (Canvas, Ed Discussion,
Echo360) into a searchable personal knowledge base. Two processes:

- **Frontend** — React 19 + Vite + Tailwind v4, in `app/src/`
- **Rust core** — Tauri backend, scrape engine, PDF parsing, page-image
  embedding, and the `oculus` CLI, in `app/src-tauri/`

PDF parsing (MinerU) and page embedding (Voyage) are HTTP calls made
in-process from Rust, behind seams in `app/src-tauri/src/parse/` and
`app/src-tauri/src/embed/`. Embedding is cloud-only. Parsing has **two
engines, chosen in Settings → Library**: MinerU's cloud service, or a MinerU
server the user installs and runs themselves, reached over loopback. There is
no *bundled* local parser and no local inference tier in this repo — the
second engine is somebody else's program at an address, which Oculus never
starts, supervises or ships. **The Python sidecar is gone** — code and
directory. The last commit holding `sidecar/` is `f875bb1`, which is where the
separate local-server repo forks from; Rust comments that cite `sidecar/*.py`
are provenance for ported behaviour and point there. Do not re-add a Python
process, a `uv` step or a local inference tier to this repo.

Ingestion and retrieval are built. Chat is a **CLI agent** — Claude Code,
Codex or opencode, driven as a subprocess from the library's `agents/` folder
(`docs/harness.md`). The BYOK API layer it replaced has been deleted rather
than woken up: opencode is the API path, reached as a third bridge instead of
as a parallel world. Its migrations stay, its code does not.
Automations and the Inbox were built and then removed — the last commit that
has them is `d64dc11`, reachable from master's history; do not reintroduce
pieces of them here without being asked.

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
| PDF parsing, the two MinerU engines, the parser seam | `docs/parsing.md` |
| Embeddings, search, the `pages` table | `docs/retrieval.md` |
| Chat: the CLI-agent bridges, containment, the timeline | `docs/harness.md` |
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
- **No Python toolchain.** There is no `uv` step, no `.venv`, and no
  `sidecar/` — the pins that used to live in `sidecar/pyproject.toml` belong to
  the separate repo now, and are at `f875bb1` if you need them. **The local
  parse engine does not breach this**, and will read as if it does: the rule is
  about *this repo and this app* — no bundled interpreter, no venv in the
  build, no supervised child process, no 1.2 GB payload to sign. A MinerU the
  user installed with their own `uv` and started themselves, spoken to over
  HTTP, is the same boundary as MinerU cloud with a different hostname. Its
  `uv tool install` line lives in `docs/parsing.md` as an instruction *to a
  user*, never as a build step here.
- **The `oculus` CLI is built by the dev preflight, not by `tauri dev`.**
  `tauri dev` issues a bare `cargo run`, which builds `app` and no other bin
  target, so `target/debug/oculus` used to be whatever a stray `cargo test`
  last left there — while being the sibling of the running app and therefore
  the first `oculus` on a coding agent's PATH. `app/scripts/predev.mjs` builds
  it at every dev start and `app/scripts/watch-cli.mjs` keeps it current
  through the session; `bun run cli:dev` is the same build by hand. Both delete
  the binary first, because cargo reports "Finished" while leaving a stale one
  in place. See `docs/development.md`.
- `libpdfium` and `ffmpeg` are fetched, not vendored — `bun run pdfium` and
  `bun run ffmpeg` into the gitignored `app/src-tauri/binaries/`. The pdfium
  release tag is pinned to the Chromium revision `pdfium-render` binds
  against; a mismatch fails at *bind* time, not compile time.

# Hard-won constraints (do not relearn these)

- **No work in hidden WebViews.** macOS suspends an off-screen WKWebView's
  content process, which silently freezes anything running there. All
  scraping lives in Rust (`app/src-tauri/src/sync.rs`) for exactly this
  reason. Never move background work back into a WebView.
- **Canvas API tokens are blocked by the university** — the admin has
  disabled self-service access tokens, so the session cookie is the only auth
  path. Don't re-propose `Authorization: Bearer`. See `docs/auth.md`.
- **A parse or an embed blocks for minutes, and that is not a hang.** Neither
  has a timeout imposed from above: the MinerU client owns a 60-minute poll
  deadline, a local MinerU parse is minutes of silence on this machine's own
  CPU with only a *connect* timeout above it, and the Voyage client paces
  itself against
  the rate-limit tier it detected, where a 429 is routine rather than a
  failure. A second deadline layered on top could only abandon work that was
  still progressing. On a Voyage account with no payment method the ceiling is
  10K tokens/minute — under three pages a minute — so a large deck genuinely
  takes an hour. Watch the page counter, not the clock.
- **Nothing catches a failed parse — an engine you choose is not a tier that
  saves you.** A missing token, a rejected token, a spent quota, no network, a
  server that is not running: each means that PDF has no markdown at all. The
  parser setting selects *which* MinerU runs; it does not stack them, and a
  parse that fails on the selected engine is never re-tried on the other one.
  That is why `ParseError` carries `kind`/`retryable`/`latching` and why the
  file row and the background sweep both read them. **Never add a fallback
  between engines, and never let a failure degrade quietly into something that
  looks like success** — a failed parse must surface. See `docs/parsing.md`.
- **Nothing in Settings may spend money, and a model list is not worth a
  request.** opencode's catalogue advertises models whose gateway refuses
  everything, and the way that was solved was a *probe*: one real turn per
  model, swept automatically when the provider manager opened. It cost **two**
  billed requests per model — the probe, plus the session title opencode
  generates for every session it creates — at up to 8.6K input tokens each,
  against a provider like OpenRouter's ~300 models. It is deleted. Almost
  everything it bought is free: `/config/providers` lists only providers that
  are actually usable, `capabilities.toolcall` says whether a model can read a
  course file at all (69 of OpenRouter's 369 cannot), and the free Zen models
  are a static rule on the provider id. `unusableReason`/`isZen`/`filterOffered`
  in `app/src/lib/opencodeCatalogue.ts` are the whole gate. What no free check
  can see is a stale key, or a model behind the provider's own verification
  (Meta's Muse Spark) — both are allowed to fail once in the timeline. **Never
  re-add a per-model probe, and never let a settings page make a billed call.**
- **A drag needs `dataTransfer.setData()` or WebKit cancels it — which is why
  almost nothing here is on HTML5 drag-and-drop.** A `dragstart` handler that
  sets no data aborts the drag silently: no `dragover`, no `drop`, every
  handler correctly attached and nothing moves. **One place still uses it**,
  and must keep its payload: `app/src/components/harness/Timeline.tsx`, which
  drags a selection out as markdown and so *replaces* what WebKit already put
  on the transfer (a `preventDefault()` on **that** `dragstart` cancels the
  drag outright). The dock's view-tab reorder
  (`app/src/components/ui/ViewTabs.tsx`) was the second and is now on pointer
  events with the rest: the native drag *worked* there, but the strip stood
  still under a translucent copy of the label WebKit drew for itself until the
  drop, which is not the gesture the window's own tab strip a few pixels above
  it uses.
  **Everything else drags on pointer events and never meets this**: the tab
  strip's reorder (`app/src/components/tabs/TopTabBar.tsx`), the dock's view
  tabs (`app/src/components/ui/ViewTabs.tsx` — the same gesture, sized off
  measured rects because a label's width is its own, and swapping on a
  **leading edge** rather than on the centre `TopTabBar` compares: the clamp
  stops a grabbed tab with its edge on the strip's, so a centre can only pass
  the end tab's midpoint when that tab is strictly wider, which makes the far
  slot unreachable for anything else — `TopTabBar`'s uniform widths land exactly
  on it, where `>` is false), the chat column's
  group reorder (`app/src/components/harness/ThreadList.tsx`), and the three
  card-and-row surfaces — `ProjectBoard.tsx`, `ProjectTable.tsx` and
  `TasksBoard.tsx` in `app/src/components/projects/` — which share the
  pointer-capture gesture in `app/src/hooks/useCardDrag.ts`. The cards had a
  second reason to leave HTML5 DnD: a card is full of `<span>`s that
  `index.css` hands `user-select: text` back to, and in WebKit a **text
  selection pre-empts the element drag**, so a press on a due chip started a
  selection, `dragstart` never fired, and the card lifted only when the grab
  landed on dead space. Reach for HTML5 DnD only for something that must leave
  the window; otherwise extend that hook.
- **A native file drop reports its position in *points*, and Tauri types it
  `PhysicalPosition`.** wry reads macOS's `draggingLocation` and subtracts it
  from the view's frame height without ever multiplying by the backing scale
  factor, so the number that arrives is the window's own logical coordinates.
  Doing what the type asks — dividing by `devicePixelRatio` — halves every
  point on a retina screen and folds the whole window into its top-left
  quarter: a drop on the composer at the bottom of the window reports as the
  middle of the thread, hits nothing, and is swallowed with every handler
  correctly attached and no error anywhere. `useFileDrop`
  (`app/src/hooks/useFileDrop.ts`) *measures* the scale instead — the
  viewport's width over the window's own logical width.

- **A drop here is a *webview* event, and a window listener for it is never
  called.** Tauri delivers a drop as a window event only when the runtime
  built that webview as `WebviewKind::WindowContent`; otherwise it emits to
  `EventTarget::Webview`, and `filter_target` has no arm matching a `Window`
  listener against a `Webview` emit. This app is never `WindowContent`:
  `app/src-tauri/Cargo.toml` turns on tauri's `unstable` feature because
  `Window::add_child` needs it for the in-app browser
  (`app/src-tauri/src/browser.rs`), and `tauri-runtime-wry` picks the main
  window's own webview kind under exactly that cfg — `unstable` on means
  `WindowChild`. So `getCurrentWebview()` is the one to listen on, whatever
  the window holds; the number of webviews never enters into it. A window
  listener subscribes without error and reports success, which is what makes
  this cost a day: there is nothing to see but a drag that does nothing.

- **In WebKit a `click` is raised from a `mousedown`/`mouseup` pair, so
  cancelling `pointerdown` kills the click.** The spec says cancelling the
  press suppresses the compatibility mouse events but leaves `click` alone;
  WebKit does not behave that way, and with no `mousedown` there is no pair to
  raise one from. `useCardDrag` opened with `e.preventDefault()` on
  `pointerdown` to stop the text selection above, and silently removed the only
  way into a task's page — every card, deterministically. Hold the selection
  off with CSS (`DRAG_SURFACE`), and cancel **`pointermove`** instead: that
  suppresses the compatibility `mousemove` a selection is *extended* by while
  leaving the click untouched.
- **SQLite's `DROP TABLE` deletes every row first, so a table rebuilt with a
  self-referencing foreign key must point that key at the *new* table.** With
  foreign keys on — sqlx turns them on for both pools — a drop performs an
  implicit `DELETE` of the table's rows, which fires any `ON DELETE CASCADE`
  aimed at it. In the 12-step rebuild recipe, a new `project_tasks_new` whose
  `parent_id` still referenced `project_tasks` therefore had the copy it had
  just made cascaded empty the moment the old table was dropped: every subtask
  gone, silently, with the migration reporting success. `PRAGMA
  defer_foreign_keys` does not save you — it defers the *check*, not the
  action. `UNFILED_TASKS_SQL` in `app/src-tauri/src/projects.rs` is the shape
  that works, held as a constant so the test runs the string the app runs.
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
- **Three primitives are deliberately a notch below stock shadcn**, whose sizes
  are drawn for a 16px-base web page while this app's body text is 14px and its
  furniture is h-6/h-8 throughout. `button.tsx` has `default` at `h-8`, not
  `h-9` — a 36px button was the tallest thing in most rows. `dialog.tsx` is
  `p-5`/`rounded-xl` on `border-border-subtle` with a 16px title and 13px
  description, instead of `p-6`/`rounded-lg` at 18/14. `tooltip.tsx` adds a
  `max-w-64` and `break-words` that stock has no opinion about: a tooltip
  stands in for a label that did not fit, and the labels this app hangs one off
  — a tab carrying a Canvas page title, a file's full name — draw a single line
  most of the window wide, which is harder to read than the truncation it was
  explaining. Every dialog in the app overrides only `max-w` and no call site
  sets a tooltip width, so the scale lives in the primitive; don't "restore"
  any of them to what `shadcn add` generates.
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
  cascade. In `@layer base` it stays the default and utilities win again. The
  same section bit twice: `body { user-select: none }` and the
  `p, span, li, td, th, input, textarea { user-select: text }` that hands it
  back were written *after* the layer closed, which killed every `select-none`
  and `select-text` utility that landed on one of those tags — including the
  project board's drag. They are in the layer now; keep them there.
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
- `data/`, `*.db`, and `app/src-tauri/binaries/` are
  gitignored user-state or fetched artifacts — never commit them.
