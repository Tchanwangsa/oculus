# Viewers: markdown, PDFs, lectures and web pages

How a pane shows content: library markdown (with maths, diagrams and a
lightbox) and PDFs, a lecture recording, and a web page.

## Where

| Piece | Location |
| --- | --- |
| Markdown, maths, mermaid, lightbox, PDF | `app/src/components/markdown/`, `app/src/components/ui/Lightbox.tsx`, `app/src/components/files/PDFViewer.tsx` |
| Lecture player | `app/src/components/lectures/`, `app/src/lib/lecturePlayback.ts`, `app/src/stores/playerPrefsStore.ts`, `app/src/hooks/useTranscriptDock.ts` |
| Transcript search and source-index mapping | `app/src/hooks/useTranscriptSearch.ts` |
| In-app browser | `app/src/pages/BrowserPage.tsx`, `app/src/hooks/useBrowserTabs.ts`, `app/src/lib/browserHistory.ts`, `app/src-tauri/src/browser.rs` |

## One markdown renderer serves every surface

`app/src/lib/mathMarkdown.ts` owns delimiter detection and source normalization.
Library file rendering always applies KaTeX so math fences and HTML math
classes work even when there are no dollar delimiters.

- **`app/src/components/markdown/MdComponents.tsx`** renders Canvas bodies,
  parsed PDFs, Ed threads and replies with KaTeX. `normalizeMath` rewrites
  `\(…\)` / `\[…\]` to `$…$` / `$$…$$` because CommonMark eats the backslash
  first. An inline `<code>` holding only a citation (`app/src/lib/citations.ts`)
  renders as `FileChip`. `![alt](path)` naming a library picture or HTML
  page renders it in place (`OutputEmbed.tsx`).
- **A reply goes through `CompactMd`**, whose `.md-compact` rules in `index.css`
  are deliberately *unlayered* — in `@layer base` they would lose to the
  utilities they override. **`InlineMd` flattens blocks** because chapter
  summaries sit inside buttons, and a `<p>` in a `<button>` closes it early.
- **`FileViewer` resolves in-file links locally** — relative links and Canvas
  `/files/<id>` or `/pages/<slug>` URLs, by `canvas_id`, path or `source_url`
  (`app/src/lib/libraryLinks.ts`, shared with the note editor). A selection
  in its rendered markdown copies or drags out as markdown, maths as TeX
  (`app/src/lib/selectionMarkdown.ts`, shared with the chat timeline).
- **A ```mermaid fence is caught at `pre`** (`Mermaid.tsx`), and the original
  `<pre>` shows until it renders or if it never parses. Config and drawing are
  `mermaidRender.ts`:
  - `htmlLabels: false` must sit at the config's **top level** (mermaid 12
    ignores it under `flowchart`); HTML labels wrap by an exact float compare
    that page zoom breaks, clipping every long label.
  - `layout: "dagre"` must ship with the spacing keys (`FLOWCHART_LAYOUT`), or
    `rankSpacing`/`nodeSpacing` are discarded.
  - The label is bounded, not the box: `--diagram-min`/`--diagram-max` keep
    labels at or above `MIN_LABEL_PX`, walking tall diagrams toward
    `TARGET_HEIGHT_PX`; below the floor it scrolls rather than shrinks.
  - Colours come from `--diagram-*` in `index.css`, which exist because
    Tailwind v4 emits a theme variable only if something references it.
  - The lightbox copy gets rewritten ids (`rescope`), since mermaid scopes styles
    and arrowheads by id.
- **`Lightbox.tsx` pans with its container's scroll and zooms with a
  `transform`**; every input moves a target the painted scale eases toward, so
  bursts compose.
- **`PDFViewer` mounts pdf.js's own viewer**, loaded by `app/src/lib/pdfjs.ts`
  through awaited dynamic imports because `pdf_viewer.mjs` reads
  `globalThis.pdfjsLib` at evaluation. `index.css` pins `color-scheme` to
  `.dark`, overriding the `:root` rule pdf.js's stylesheet adds. A pinch arrives
  as both `ctrlKey` wheel and `gesture*` events; only the gesture zooms.
  - **Selection and cursor are pdf.js's own**: a drag selects text and the
    trackpad scrolls. `index.css` only recolours `::selection` to the accent.
  - **A selection copies as the parse's markdown** when the file has one
    (`app/src/lib/pdfSelectionMarkdown.ts`). Text layer and `.pages.json`
    meet in `normalizeText` form and are aligned patience-diff style; the
    selection's ends land in the pages joined as one document, because
    MinerU files a paragraph that runs onto the next page under the page it
    starts on. The slice grows to keep maths, figures, links, code and HTML
    tables whole (tables come out as pipe tables through `selectionMarkdown`'s
    walkers), figure links match the Markdown view's copy, and an end on an
    unparsed or unalignable page copies that page's text. It is bound in the
    **capture** phase: pdf.js's text layer writes its own copy and stops it.
  - **Find is pdf.js's `PDFFindController` behind `ui/FindBar.tsx`**; each
    viewer is a find target, and which one ⌘F reaches is
    [find routing](./shell.md#f-reaches-one-registered-find).
  - **The page box names the pages on screen** — a spread, or in continuous
    scroll every page filling a fifth of the viewport or showing half of
    itself (`shownPages`) — and takes a page number to jump to.
  - **`pdfjs-dist` is patched** (`app/patches/`): its text layer multiplies
    every font size by a 1px probe's measured height, which page zoom 1.15
    reads as 0.87, so the selectable text ran 13% short of the glyphs. The
    probe is clamped to at least 1.

Chat's composer (`MentionInput.tsx` sends chips as backticked library paths),
picker and timeline are [harness.md](./harness.md).

## The lecture player's elements outlive the page

`LecturePlayer.tsx` streams from Rust's media server via `mediaSrc()`
(`app/src/lib/media.ts`), not `convertFileSrc` ([architecture.md](./architecture.md)).

- **`app/src/lib/lecturePlayback.ts` owns the `<video>` elements**, so playback
  survives tab switches. Between players they park in a 480×270 off-screen
  host — `display: none` lets WebKit stop playback, and a tiny host decodes
  tiny. Progress saves every 5s and on pause, seek, end and `pagehide`.
- **One player owns the elements** (`app/src/lib/playbackOwner.ts`). A tab's
  main page and its side panel can both be lecture pages; a player never takes
  the elements on mount from an owner in its own tab — only a player in the tab
  now in front, or any player once nobody owns them (a stop, or a paused owner
  unmounting). A user action claims them (`claimPlayback`): Play here, Space,
  or a seek from the dock; the claimed pane adopts once it shows that lecture.
  A player without them draws a still over its frames — title, "Playing/Paused
  in the other pane", a "Play here" pill — and never touches the elements; the
  frames stay mounted under it so the elements are never detached.
- **Only the focused pane's player hears keys.** Owning, every key works; not
  owning, Space takes the video over and Escape leaves fullscreen.
- **Leaving a playing lecture asks at the door** — the strip's × and
  `navigateActive` / `goInActiveTab` call `confirmLeavingLecture` — never
  through a router blocker, which strands the pane when nobody answers.
- **Two streams, one clock.** The main-frame source leads (audio, clock,
  progress); followers are rate-trimmed past `MAX_DRIFT` and seeked only past
  `SEEK_DRIFT`. `syncLectureSources` takes the whole plan, so only it can tell
  a fresh lecture (resume) from a source switch (keep the second). Speed and
  volume re-apply per leader element, since both reset on load.
- **Fullscreen is the Tauri window plus a `fixed inset-0` overlay**: element
  fullscreen shows only its subtree, and the player's Radix popups portal
  outside it. A lecture in the side panel is the full lecture page, dock and
  fullscreen included.
- **Preferences are global** (`playerPrefsStore`); only the position is per
  lecture. The clock counts against the leader's duration, not the catalogue's.
- **The dock is Chapters, Transcript and Chat** in a reorderable `ViewTabs`
  strip, always mounted because Chat needs no file. `useTranscriptDock` raises
  the floor for Chat and caps the drawn size by the container (`KEEP_W` /
  `KEEP_H`), dropping a side dock to the bottom when it can't fit — without
  rewriting the preference.
- **The transcript is virtualised** (~2500 cues a lecture) through `FollowList`,
  shared by both registers. Following is tracked by pointer *intent*, since the
  follow-scroll fires `scroll` too, and runs off the active-cue index rather
  than `timeupdate`. Don't call `measure()` on a search: heights are cached by
  cue key and survive it. Key callbacks stay stable between searches so a
  playback highlight does not rebuild every row's offsets. Chapters and the
  reading copy are [chapters.md](./chapters.md).

Playback's ordered cue, chapter and reading starts use an upper-bound lookup,
including the last entry when timestamps coincide. A player subscribes only to
its lecture's download progress; stable dock callbacks keep playback ticks
outside the transcript's memo boundary. The catalogue's `LectureRow`
(`app/src/components/lectures/LectureRow.tsx`) reads only its own
source-qualified download progress and active state.

## The in-app browser is a native page per tab, owned by Rust

A browser tab is `/browse/<id>`, naming a WKWebView that
`app/src-tauri/src/browser.rs` parks over an empty slot in `BrowserPage` (Canvas
refuses iframes).

- **Rust owns the tab list** and pushes `browser-state` on every change;
  `useBrowserTabs` reconciles it per *pane*. The mirror preserves unchanged
  page objects so another tab's snapshot does not rerender every pane.
  Page navigations change the URL in
  Rust only, never the route. What a page knows about itself (history, find,
  zoom) is pushed as events, because `with_webview` returns nothing.
- **A native page can't interleave with the DOM.** `BrowserPage` hides it while
  a portal overlaps the slot, while its tab is backgrounded (`useTabActive`),
  and while the address list is open — showing a PNG still
  (`browser_snapshot`) so the card doesn't blank.
- **History is one row per URL, ranked by frecency**, recorded on load
  *finish* (one row per redirect chain). `historyUrl` drops the fragment, and
  the whole query if any part looks like a credential.
- **A page must claim to be Safari** (`PAGE_USER_AGENT`) or UA-sniffing sites
  serve fallbacks, and an `initialization_script` reports a real
  `outerWidth`/`outerHeight` (a child view's are 0, which drops canvas renderers
  to minimum scale).
- **Pages get every plugin's init script.** `tauri-plugin-opener` is built
  with `open_js_links_on_click(false)`: its click handler would cancel a page's
  `target=_blank` links and call IPC pages cannot reach, so only ⌘-clicks got
  through. A `_blank` link reaches `on_new_window` and opens as a tab; the
  app's own links go through `AppLayout`'s capture-phase handler.
