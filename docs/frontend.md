# Frontend

React 19 + Vite + Tailwind v4, hash-routed, Notion-style layout. UI
conventions (palette, shadcn, icons, no-toasts) are in the root `CLAUDE.md` —
this page is the structure.

## Where

| Piece | Location |
| --- | --- |
| Router + event bridge | `app/src/App.tsx` |
| Shell: sidebar + top tab strip | `app/src/layouts/AppLayout.tsx`, `app/src/components/sidebar/`, `app/src/components/tabs/TopTabBar.tsx` |
| Per-subject layout (underline tabs) | `app/src/layouts/SubjectLayout.tsx` |
| Subject tab pages | `app/src/pages/subject/` |
| Chat (agent loop, streamed) | `app/src/pages/ChatPage.tsx`, `app/src/stores/chatStore.ts` |
| Calendar (month / week / upcoming) | `app/src/pages/CalendarPage.tsx`, `app/src/components/calendar/`, `app/src/lib/calendar.ts` |
| Provider/model pickers (settings + composer) | `app/src/components/llm/` |
| Sync page + runner | `app/src/pages/SyncPage.tsx`, `app/src/lib/syncRunner.ts` |
| Settings | `app/src/layouts/SettingsLayout.tsx`, `app/src/pages/settings/` |
| Parse backend, memory budget + sidecar health | `app/src/pages/settings/LibraryPage.tsx` |
| Peek panel (file/lecture preview) | `app/src/components/peek/` |
| Viewers | `app/src/components/files/PDFViewer.tsx`, `app/src/components/files/FileViewer.tsx`, `app/src/components/lectures/LecturePlayer.tsx` |
| shadcn components (source, editable) | `app/src/components/ui/` |
| Zustand stores | `app/src/stores/` |
| Hooks | `app/src/hooks/` |
| DB access (tauri-plugin-sql) | `app/src/lib/db.ts` |

## Routes

`createHashRouter` in `app/src/App.tsx`: `/chat`, `/calendar`, `/subjects`,
`/subjects/:subjectId` (SubjectLayout → overview / modules / downloads /
lectures / announcements / assignments / discussion), `/subjects/:subjectId/file`
and `/lecture` (peek promoted to a full Notion-style page, outside
SubjectLayout on purpose), `/sync` and `/settings/*`.
Legacy routes (`/lectures`, a subject's `files` tab) redirect.

## How it connects

- Settings → Library owns parse preferences (`parse` in the `settings`
  table). Saves update SQLite and `sidecar_set_limits` in order; no restart
  is needed. The memory input has a 5 GB floor and recommends 8 GB. Health
  polling displays whole-tree memory, peak and recoveries inline, with no
  toasts. Backend changes affect new parse requests.
- Choosing MinerU cloud or Automatic opts into uploading PDFs to MinerU's
  PRC-hosted service; Local only is the default. Token save/remove invokes
  Rust keychain commands, never DB writes. Automatic is cloud-first when a
  token is available, with local fallback; see [sidecar.md](./sidecar.md).
  Saving checks the token against MinerU first, so a bad or expired one is
  named at that moment; a token MinerU later refuses mid-parse shows as
  Expired, read from health rather than polled.
- **Backend events are the write path.** `app/src/hooks/useBackendEvents.ts`
  (mounted once in `App.tsx`) listens for scrape/parse/auth Tauri events,
  upserts SQLite through `app/src/lib/db.ts`, and updates the stores. Pages
  read from the DB and stores; they do not talk to the scraper directly.
- In the app it is the **frontend** that owns scrape-table writes (the Rust
  engine only emits events); the CLI writes the same rows itself via
  `app/src-tauri/src/store.rs`. Change a table's shape and both writers must
  move together, plus the migration in `app/src-tauri/src/lib.rs`.
- **`sync_runs` is the only sync clock.** A subject's `last_synced_at` is not
  stored — `getSubjects` in `app/src/lib/db.ts` (and the CLI's
  `store::subjects`) derives it from the latest *completed* run whose
  `subject_codes` include the subject, so the subject list can never disagree
  with the history table and interrupted runs never count as a sync.
- `SubjectLayout` resolves the subject once and hands it to tab pages via
  outlet context — tab pages must not re-fetch it.
- Files and lectures open in the **peek panel** (`peekStore` +
  `app/src/components/peek/PeekPanel.tsx`); "expand" navigates to the
  full-page route. The top tab strip is `tabStore` +
  `app/src/components/tabs/TopTabBar.tsx`, Notion-style. It replaces the
  native title bar, so the strip and the empty space after the last tab carry
  `data-tauri-drag-region` to keep the window movable — which only works
  because `app/src-tauri/capabilities/default.json` grants
  `core:window:allow-start-dragging`; `core:default` does not include it, and
  without it the attribute silently does nothing.
- The calendar reads its own tables and never the scraper: `loadCalendar` in
  `app/src/lib/calendar.ts` pulls `calendar_events` plus `lectures` in one go
  and the page filters in memory, so month and week paging is arithmetic rather
  than queries. See [calendar.md](./calendar.md) for where the rows come from.
- Background job progress surfaces **only** in the sidebar (driven by the
  stores fed from `useBackendEvents`) — no toasts, no bottom bars.
- **Zoom scales the window, not a div.** `app/src/layouts/AppLayout.tsx`
  drives the webview's own page zoom (⌘+/⌘−/⌘0, persisted in
  `localStorage`), so the whole document — tab strip included — is laid out
  at one scale and every measurement stays in a single coordinate space.
  A CSS `zoom` container was the earlier shape and had to go: inside one,
  WebKit reports pointer coordinates in visual pixels but element rects in
  layout pixels, so Radix popups near a window edge misjudged the room below
  them and drag maths drifted by the zoom factor. The `--app-zoom` CSS var
  survives only for chrome measured in device pixels — the tab strip's
  traffic-light gap divides by it.
- `app/src/hooks/useQualitySweep.ts` periodically queries the DB for files in
  selected subjects whose `parse_status` is not yet `quality` and re-requests
  the pipeline for a few at a time (the sidecar skips whatever already
  exists). This is the recovery path for files that missed their quality
  pass — app closed mid-queue, sidecar down during a sync, parse died.
- `app/src/components/lectures/LecturePlayer.tsx` streams video from the
  Rust media HTTP server via `mediaSrc()` in `app/src/lib/media.ts` — not
  `convertFileSrc`, which WebKit's media stack rejects (see
  [architecture.md](./architecture.md)). Its scrub bar is a local `SeekBar`
  rather than the shadcn Slider, kept because `offsetX / offsetWidth` needs
  one measurement where Radix mixes `clientX` with `getBoundingClientRect`.
- Markdown rendering (Canvas bodies, parsed PDFs, Ed threads) goes through
  `app/src/components/markdown/` with KaTeX for math; PDF markdown quality
  therefore shows up directly in Chat results and file views.
- **In-markdown links resolve locally when they can.** `FileViewer`
  (`app/src/components/files/FileViewer.tsx`) matches `../`-relative links
  and raw Canvas `/courses/…/files/<id>` / `/pages/<slug>` URLs against the
  subject's `files` rows (by `canvas_id`, path, or `source_url` — the scrape
  records the slug a page was fetched under, since Canvas keeps serving a
  renamed page's old URL) and opens the local copy in the same peek; only
  unresolvable links open externally, marked with an arrow-square-out icon.
