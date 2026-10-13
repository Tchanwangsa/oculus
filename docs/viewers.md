# Viewers: markdown, PDFs, code, lectures and web pages

How a pane shows content: library markdown (with maths, diagrams and a
lightbox), PDFs and source files, an Ed lesson's slides and a challenge's
workspace, a lecture or a library video in the media player, a video's
transcript, and a web page.

## Where

| Piece | Location |
| --- | --- |
| Markdown, maths, mermaid, lightbox, PDF | `app/src/components/markdown/`, `app/src/components/ui/lightbox/Lightbox.tsx`, `app/src/components/files/pdf/`, `app/src/lib/pdf/pdfView.ts`, `app/src/lib/pdf/pdfFind.ts` |
| Inline file, web and planning references; hover metadata | `app/src/components/markdown/FileChip.tsx`, `app/src/components/markdown/ReferenceLink.tsx`, `app/src/components/markdown/ReferencePreview.tsx`, `app/src/components/markdown/referenceDetails.ts`, `app/src/lib/files/referenceAppearance.ts` |
| The maths engine: loading, rendering, `rehypeMaths`, the visual field's edit model (`field.ts`) and the geometry of source-mapped maths (`geometry/`: where each caret stop is drawn, which stop a press lands on, a selection's bands); KaTeX's CSS and fonts | `app/src/lib/maths/`, `app/src/styles/katex/` |
| A parsed PDF's faces: PDF, Markdown | `app/src/components/files/FileViewer.tsx`, `app/src/components/files/FileMarkdown.tsx`, `app/src/components/files/pdf/pdfMdLink.ts`, `app/src/lib/pdf/pdfBlocks.ts` |
| An Ed lesson's view: the slide sidebar and pane, the index reader, statuses Ed answered this session | `app/src/components/lessons/`, `app/src/hooks/data/useLessonIndex.ts`, `app/src/lib/ed/lessons.ts`, `app/src/lib/ed/slideStatus.ts`, `app/src/lib/ed/views.ts` |
| A challenge's workspace: the pane, its commands and events, the save loop, the shared session, the brief's Files | `app/src/components/lessons/workspace/`, `app/src/lib/ed/briefFiles.ts`, `app/src/lib/ed/workspace.ts`, `app/src/lib/ed/workspaceSaver.ts`, `app/src/lib/ed/workspaceHandles.ts`, `app/src/lib/ed/workspaceStatus.ts` |
| A workspace file in conflict with Ed: the bar, Compare, Keep mine and Take Ed's | `app/src/components/lessons/workspace/conflict/`, `app/src/lib/ed/conflicts.ts`, `app/src/lib/ed/lineDiff.ts` |
| The workspace's terminal drawer, Run and its files, Mark and Feedback | `app/src/components/lessons/workspace/terminal/`, `app/src/components/lessons/workspace/run/`, `app/src/components/lessons/workspace/mark/`, `app/src/components/lessons/workspace/feedback/`, `app/src/lib/ed/termOutput.ts`, `app/src/lib/ed/runFiles.ts`, `app/src/lib/ed/flushForEd.ts`, `app/src/lib/ed/markPrompts.ts`, `app/src/lib/ed/feedback.ts` |
| Reset to scaffold and Restore: the header menu and the replace flow | `app/src/components/lessons/workspace/reset/` |
| The code editor and a library source file's read-only view | `app/src/components/code/`, `app/src/components/files/CodeFileViewer.tsx`, `fileLanguage` in `app/src/components/documents/editor/syntax/codeLanguages.ts`, `CODE_EXTS` in `app/src/lib/files/fileTypes.ts` |
| Media player: clock, controls, keys, fullscreen, captions, dock frame, cue list | `app/src/components/media/`, `app/src/lib/lectures/media/`, `app/src/stores/lectures/playerPrefsStore.ts`, `app/src/hooks/lectures/useTranscriptDock.ts` |
| Lecture player: two sources, playback owner, chapters, chat | `app/src/components/lectures/`, `app/src/lib/lectures/playback/`, `app/src/lib/lectures/playbackOwner.ts` |
| Done and Up Next: the lecture's end, the card, its thumbnail | `app/src/lib/lectures/end.ts`, `app/src/components/lectures/UpNext.tsx`, `lecture_thumbnail` in `app/src-tauri/src/lectures/chapters/app.rs` |
| Library videos | `app/src/components/files/VideoFileViewer.tsx`, `app/src/lib/files/openFile.ts` |
| A page's video cards: Echo360, Canvas, YouTube/Vimeo; their shared frame, the subject, the stores | `app/src/components/markdown/embed/EmbedVideoCard.tsx`, `app/src/components/markdown/embed/CanvasVideoCard.tsx`, `app/src/components/markdown/embed/WebVideoCard.tsx`, `app/src/components/markdown/embed/card/`, `app/src/components/markdown/embed/EmbedSubject.tsx`, `app/src/stores/lectures/embedDownloadStore.ts`, `app/src/stores/lectures/videoDownloadStore.ts`, `app/src/stores/lectures/webVideoStore.ts` |
| YouTube and Vimeo metadata (`web_video_info`) | `app/src-tauri/src/sources/oembed.rs` |
| Transcribe from a player | `app/src/hooks/lectures/useTranscription.ts`, `app/src/components/media/TranscribeEmpty.tsx` |
| Transcript search and source-index mapping | `app/src/hooks/lectures/useTranscriptSearch.ts` |
| Transcription of videos without captions | `app/src-tauri/src/transcribe/`, `app/src-tauri/src/providers/groq.rs`, `app/src/lib/lectures/transcribe/` |
| Settings → Transcription: the language, the engine list and each engine's dialog | `app/src/components/settings/transcription/TranscriptionSection.tsx`, `app/src/components/settings/transcription/GroqDialog.tsx`, `app/src/components/settings/transcription/OnDeviceSpeech.tsx`, `app/src/components/settings/transcription/LocalWhisper.tsx` |
| The on-device speech helper | `app/src-tauri/speech/main.swift`, `app/scripts/build-speech.mjs`, `app/src-tauri/src/transcribe/engines/apple.rs` |
| Local Whisper: the engine, its model files | `app/src-tauri/src/transcribe/engines/whisper.rs`, `app/src-tauri/src/transcribe/engines/whisper_models/`, `app/scripts/build-whisper.mjs` |
| In-app browser | `app/src/pages/tools/BrowserPage.tsx`, `app/src/hooks/shell/useBrowserTabs.ts`, `app/src/lib/browser/history/`, `app/src-tauri/src/shell/browser/` |

## One markdown renderer serves every surface

`app/src/lib/markdown/math.ts` owns delimiter detection and source normalization.
Library file rendering always runs `rehypeMaths` so math fences and HTML math
classes work even when there are no dollar delimiters.

- **`MD_COMPONENTS`** (`app/src/components/markdown/MdComponents.tsx`) is
  the component map for every surface. A library file — a Canvas page,
  announcement or assignment, a parsed PDF, an Ed thread — renders through
  `FileMarkdown` with it, extended by `FileViewer`'s
  `useLibraryMdComponents` to resolve in-file links, and without the front
  matter a scraped thread or lesson carries (`stripFrontmatter`); chat replies through
  `CompactMd`, with `rehypeMaths` only when `hasMath` finds a delimiter. `normalizeMath` rewrites
  `\(…\)` / `\[…\]` to `$…$` / `$$…$$` because CommonMark eats the backslash
  first. An inline `<code>` holding only a citation (`app/src/lib/citations/`)
  renders as `FileChip`. `![alt](path)` naming a library picture or HTML
  page renders it in place (`OutputEmbed.tsx`).
- **Inline references share a glyph and a hover card.** `FileChip` preserves
  citation paths for copying and the composer's atomic mentions; clickable
  references use an understated underline and content-specific Phosphor colours
  (`reference-*` tokens). Inline glyphs, favicons, spacing and baseline offsets
  use `em` units so references scale with headings; hover-card icons keep their
  own fixed size. In prose they have link semantics so they select with
  the sentence; only composer mentions are non-editable, and a selection drag
  never opens the reference. `ReferenceLink` handles web URLs and recognised task,
  project and lecture routes. Web links reuse the browser's cached favicon,
  falling back to a globe when absent or unreadable. Preview metadata loads
  only while the portaled card is open; a failed lookup leaves the path and
  type visible. Library-relative links use the same file reference after
  `libraryLinkTarget` resolves them, retaining their caller's opener.
- **A library file's raw HTML is sanitized** (`fileMarkdownPlugins` in
  `FileMarkdown.tsx`). When the text holds a `<`, `rehype-raw` parses its HTML
  and `rehype-sanitize` cleans the tree straight after, before
  `rehypeMaths` and the block anchors, so their own output (KaTeX markup,
  `data-page`) is never checked. The schema
  (`app/src/lib/markdown/sanitizeSchema.ts`) is GitHub's default plus
  `<echo-video link-id src title>`, `<canvas-video file-id title>`,
  `<web-video provider video-id hash title>` and remark-math's classes. Grow it from a tag
  scan of the library's `.md`, not from memory; across the library it drops
  only prose that parses as a tag
  (`<section1>`, `<the>`, kept as their text), `on*` handlers, comments and
  stray `<title>`/`<meta>`/`<form>`. MinerU's `<sub>`/`<sup>` render
  unchanged. Its `user-content-` id prefix stays: nothing in the app
  navigates by id, and it keeps a page's ids off `window`.
- **Canvas's `<details>` and video tags render as components** in
  `MD_COMPONENTS`: `details`/`summary` as a bordered dropdown whose caret
  turns by its own element's `[open]` (`group-open` would turn nested ones
  too), and the three video tags as cards. hast keeps the attribute names,
  so the props are `link-id`, `file-id`, `video-id`. The tags are written by
  the sync ([sync.md](./sync.md#canvas-html-becomes-markdown-with-a-few-kept-tags)).
- **A task list (`- [ ]`) keeps its indent and drops its bullets.** The
  `ul`/`ol`/`li` renderers merge remark-gfm's `contains-task-list` and
  `task-list-item` classes into their own rather than spreading over them;
  the disabled box hangs back into the list's indent (`input`), so wrapped
  lines align with the text, and sits on the first line.
- **Every video card shares one frame** (`app/src/components/markdown/embed/card/`):
  a 16:9 thumbnail (a camera glyph without a picture), the name and a meta
  line, the actions — one height in every state, the skeleton included. It is
  spans, since a tag can land in a paragraph or table cell, and carries the
  tag as `data-md` so a copied selection keeps it. Echo360 and Canvas cards
  need the subject, which `MD_COMPONENTS` doesn't have, so `FileViewer` (every
  library file) and a note's `VersionPreview` wrap the markdown in
  `EmbedSubjectProvider`, which looks the code up in the subjects list by id
  (a course folder is `safe_dir(code)`, not the code). For a subject absent from
  the list, those two are `VideoLinkOut`, a button named by the tag’s
  title that opens Canvas. Chat replies don't parse raw HTML, so a tag in one
  shows as its literal text.
- **`<echo-video>` is an Echo360 video**
  (`app/src/components/markdown/embed/EmbedVideoCard.tsx`). The tag's
  `title` (the Canvas iframe's) names the card until the info call answers,
  and stands in when Echo360's name is empty. Its states:
  - **resolving** — the skeleton, with the title when the tag has one;
  - **ready** — the poster from `posterRel`, Echo360's media name without its
    video extension, the length (`fmtDurationSecs`, as lecture rows show it)
    and a captions mark, then **Play** once `fileRel` is set (the file's row
    through `openFileSmart`, so the shared media player and its Transcript tab;
    ⌘-click opens a tab) or **Download**;
  - **downloading** — the percent, a spinner before the first byte,
    "Joining…" while ffmpeg muxes, and Cancel; it turns to Play once the
    file's row has landed;
  - **download off** (`restrictDownload`) — says so, with Open in Canvas;
  - **failed** (the info call or a download) — the message verbatim and Retry.
    A cancel goes back to Download silently.

  Every state but download-off keeps a small Open in Canvas icon (⌘-click for
  a new tab). The info call launches the link in Echo360, which
  records a view, so `embedDownloadStore` asks it once per link per app
  session and forgets only a rejection. Play that finds no row asks again;
  the backend clears a deleted file, so the card offers Download.
- **`<canvas-video>` is a video file uploaded to Canvas**
  (`app/src/components/markdown/embed/CanvasVideoCard.tsx`), named by its
  `title` without the extension, on a camera tile (Canvas gives no poster). A
  sync never fetches one — they run to hundreds of megabytes — so the card
  looks for a `files` row with this subject and `canvas_id = file-id`
  (`getFileByCanvasId`, again on every `FILE_SCRAPED_EVENT` for that file):
  found, it shows the size and **Play** (`openFileSmart`, ⌘-click a tab);
  otherwise **Download**, through `videoDownloadStore` and
  `canvas_download_video` exactly as a module video
  ([frontend.md](./frontend.md#uploads-and-documents-are-ordinary-library-files)),
  then the percent and Cancel, then Play once the row lands. A failure shows
  verbatim with Retry. Every state keeps the Open in Canvas icon, to
  `/courses/<subject id>/files/<id>` on the profile's Canvas, read at the
  click (a subject's id is its Canvas course id;
  `app/src/lib/files/canvasLinks.ts`).
- **`<web-video>` is a YouTube or Vimeo video**
  (`app/src/components/markdown/embed/WebVideoCard.tsx`), Play only. Its
  thumbnail, title, channel and (Vimeo only) length come from the provider's
  oEmbed endpoint through `web_video_info` (`app/src-tauri/src/sources/oembed.rs`,
  in Rust because the webview's CORS refuses it), which checks the provider
  and the id's charset before building a URL. `webVideoStore` asks once per
  video per app session and forgets a rejection. Until then, or without an
  answer, the card shows the tag's title (else "YouTube video"/"Vimeo video")
  and still plays. Play opens `youtube.com/watch?v=<id>` or
  `vimeo.com/<id>/<hash>` through `openLink`, so an in-app page beside it (⌘-click
  for a tab of its own); never an iframe in the app's webview, since
  YouTube refuses embeds from the app's origin. An unknown provider renders
  nothing.
- **A reply goes through `CompactMd`**, whose `.md-compact` rules in `index.css`
  are deliberately *unlayered* — in `@layer base` they would lose to the
  utilities they override. **`InlineMd` flattens blocks** because chapter
  summaries sit inside buttons, and a `<p>` in a `<button>` closes it early.
- **Maths is drawn by the maths engine**, a WebAssembly build of a Rust port
  of KaTeX 0.18.5 (`app/math-core/`; [development.md](./development.md#the-maths-wasm-is-built-not-committed))
  behind KaTeX's `renderToString` in `app/src/lib/maths/`, so the markup and
  every class our code reads (`.katex`, `.katex-display`, the MathML's TeX
  annotation that copy reads) are KaTeX's. `main.tsx` starts loading it at
  boot without waiting. Until it is ready `rehypeMaths` writes a formula's
  source as a placeholder (`.md-math-pending`), and each component that
  renders markdown calls `useMathsReady` to redraw then; a `memo` one
  (`Body` in `FileMarkdown.tsx`) must call it itself. A formula that traps
  the engine (a few hundred levels of nesting) is a render error, and a spare
  instance takes over (`engine.ts`). KaTeX's CSS and fonts are vendored in
  `app/src/styles/katex/`, imported unlayered.
- **A spreadsheet opens as its text** (`marks.xlsx.md`, a table per sheet),
  in-app like a PDF rather than in the system viewer (`lib/files/openFile.ts`), with no
  PDF ↔ Markdown toggle.
- **A source file opens read-only in `CodeEditor`** (`CodeFileViewer`), not
  as markdown: any extension in `CODE_EXTS`, each held by a test to a grammar
  `fileLanguage` knows. See [the code editor](#one-code-editor-for-source-files).
- **A file with a web `source_url` has an Open in Canvas / Open in Ed button**
  (`OpenOriginal` in `FilePage`'s header, ⌘-click for a new tab). A synced
  Canvas page or Ed thread is our markdown of it; the button opens the
  original, with its embeds and styling, beside it through `openLink`. Ed
  thread and slide files also carry an `[Open in Ed]` line in their text.
- **`FileViewer` resolves in-file links locally** — `../` links, links into a
  folder beside the file (an Ed lesson's `<id>-<slug>/…` assets) and Canvas
  `/files/<id>` or `/pages/<slug>` URLs, by `canvas_id`, path or `source_url`
  (`app/src/lib/files/libraryLinks.ts`, shared with the note editor). A selection
  in its rendered markdown copies or drags out as markdown, maths as TeX
  (`app/src/lib/markdown/selection.ts`, shared with the chat timeline).
  Rendered maths is one unit to a selection from the prose around it, as in
  the note editor: an end inside a formula copies the whole formula (inline
  as `$…$`, display as `$$` lines, `formulaMarkdown`), and
  `watchMathSelection` (`mathSelection/whole.ts`) paints bands over all its
  atoms, as a press-and-drag over all of it would, instead of KaTeX's
  per-glyph highlight; the display wrapper also suppresses native selection
  paint so WebKit leaves no full-width stripes. In a prose paragraph, selected text and single-line inline maths share one
  band per visual baseline (`mathSelection/prose.ts`): the taller ink sets the
  band’s height, so small maths fills the text height and a tall fraction
  gives the adjacent text the same height. The native range and clipboard
  stay unchanged. Multi-row arrays and note widgets keep structural bands;
  nothing boxes a whole display formula. A press on a formula
  selects by structure — a cell, a matrix, a run of atoms — and copies that
  part's LaTeX wrapped by its shape (`copied` in `mathSelection/clipboard.ts`,
  shared with the note's field): within one line as `$…$`, over more than
  one line (two of a display's rows, or an array whose rows it holds, which
  the model takes whole with its environment) or a whole display formula as
  `$$` lines, one row each, also as `BLOCK_MATH_TYPE`; the bare TeX goes as
  `application/x-latex`. A press outside it, Esc or focus moving ends it. It selects in place
  (`app/src/lib/markdown/mathSelection/`): a double-click selects the atom under
  the pointer; a triple-click selects its nearest enclosing structure (a
  fraction, radical or scripted base), or its row when there is none.
  `unitAt` in `lib/maths/geometry/units.ts` maps the glyphs to the model’s stops;
  multi-click dragging extends by whole units. A triple-click on prose takes
  the full DOM paragraph (`mathSelection/paragraph.ts`), including every inline
  formula and the text after it. `rehypeMaths` renders every
  formula with the source map, and the press runs the note field's edit model
  (`MathField`) read-only over the drawing: the geometry finds the stop under
  the pointer, a drag selects to it (the model widening over whole
  structures), and bands are drawn under the glyphs in a zero-size layer
  inside `.katex`, so no glyph moves. A band covers the selected atoms of
  one line (`bands` in `lib/maths/geometry`): a display's row, or a row of
  an array whose rows are all selected, its delimiter and the atoms beside
  it banded apart, so a whole `cases` reads as its rows, not one box. Padding
  checks every pair of bands that shares horizontal space so the tint does not
  double between rows. Bands and pointer geometry track the containing
  block and KaTeX’s inline blocks for wrapping and font changes
  (`mathSelection/resize.ts`); non-replaced inline spans do not send resize
  notifications. A band reaches past the formula's box, so closing clears the bands first and drops the layer two
  frames later: gone in the same paint as `.katex`'s stacking, WebKit
  repaints only that box and leaves the bands' edges on screen. The press's `mousedown` is
  cancelled (no text selection, focus is blurred), never its `pointerdown`. A single
  press with no drag selects nothing; ⌘A widens in the formula
  (`lib/menu/editRouting.ts`); copy is taken in the capture phase, after
  cancelling `beforecopy` so WebKit enables Copy. The source map's offsets
  index the TeX annotation's text, which is why `rehypeMaths` makes line ends
  `\n` first. Maths the model cannot open selects as text does, whole.
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
- **`PDFViewer` is ours, over Rust's `pdf_*` commands**
  (`app/src/components/files/pdf/`, invoke wrappers and caches in
  `app/src/lib/pdf/pdfView.ts`). It opens a file by its library-relative path;
  `pdf_open` gives the page sizes, `pdf_render` an RGBA raster at the device
  size the viewer asks for, `pdf_text` a page's lines with per-character
  stops, `pdf_links` its links — all in points of the rendered page. A
  document is shared by every viewer holding it and closed shortly after the
  last one lets go; a password-protected one says so in the viewer body.
  - **Layout is arithmetic, not measurement** (`layout.ts`): continuous
    scroll, single page, or a spread pairing 1|2, 3|4, chosen in the toolbar
    and remembered. Scroll offsets, the page range, the fit and the zoom
    anchor all come from the page sizes, since page zoom scales
    `getBoundingClientRect` ([ui.md](./ui.md#gotchas)).
  - **Zoom** is absolute (100% is printed size), 10%–1000%, by toolbar,
    ⌘-wheel or pinch about the pointer; a pinch arrives as both `ctrlKey`
    wheel and `gesture*` events, and only the gesture zooms. Until the reader
    zooms, the size fits the view and follows its resizes: a portrait page
    fills the width, a wider one fits whole, capped at 125%. The old rasters
    stretch at once and pages redraw 400 ms after the last change.
  - **Only pages near the view are drawn**: within a view's height of it,
    each gets a raster at its CSS size × `devicePixelRatio`, a text layer and
    links; the rest are empty boxes. The raster is capped at Rust's 16M-pixel
    limit and CSS-scaled past it, since one layered page that size can take
    ~1 GB to render. A
    render for a size since replaced is dropped, and queued renders run
    newest first.
  - **The text layer is one transparent span per line**, `<br>` between
    lines, so its text nodes read the page in order (`textLayer.ts`). Each
    span is placed in points × `--pdf-unit`, so a zoom never rebuilds it or
    drops a selection, and stretched to its box with `scaleX` from a canvas
    `measureText`. While a drag selects, an empty block under the spans
    keeps a pointer between lines from jumping the selection to the page's
    end. `index.css` recolours `::selection` to the accent.
  - **A selection copies as the parse's markdown** when the file has one
    (`app/src/lib/pdf/pdfSelectionMarkdown/`). Text layer and `.pages.json`
    meet in `normalizeText` form and are aligned patience-diff style; the
    selection's ends land in the pages joined as one document, because
    MinerU files a paragraph that runs onto the next page under the page it
    starts on. The slice grows to keep maths, figures, links, code and HTML
    tables whole (tables come out as pipe tables through `selectionMarkdown`'s
    walkers), figure links match the Markdown view's copy, and an end on an
    unparsed or unalignable page copies that page's text. It is bound in the
    **capture** phase on the scroller.
  - **A citation** jumps to its page. When the record has blocks, the cited
    lines' blocks (`citedPage` maps the line range into the page's markdown
    and onto the block spans) are boxed and the first is brought into view;
    otherwise the quote's spans are marked (`citation-hit`) each time that
    page's text layer is built. A citation jumps once: coming back from the
    Markdown face keeps the reader's place and the marks.
  - **Find is the viewer's own behind `ui/search/FindBar.tsx`** (`usePdfFind.ts`,
    `app/src/lib/pdf/pdfFind.ts`): opening it reads every page's text once, and a
    query matches case- and diacritic-blind over each page's lines joined by
    spaces, so a match runs across a line break. Every match is painted
    under the text layer, the current one stronger; a new query starts at the
    page in view. Each viewer is a find target, and which one ⌘F reaches is
    [find routing](./shell.md#f-reaches-one-registered-find).
  - **Links** are anchors over the page: a web link opens in-app like any
    other `<a href>`, an internal one scrolls to its page.
  - **The page box names the pages on screen** (`shownPages`: a spread, or in
    continuous scroll every page filling a fifth of the viewport or showing
    half of itself) beside a fixed "/ total". Focused, it holds only the first
    page: digits only, Enter jumps, a range is never typed.
  - **Rust's half is `app/src-tauri/src/library/pdf_view/`, on the pure-Rust
    renderer hayro**, opened and rendered through
    `app/src-tauri/src/library/pdf_render/`, which the embedder shares. A path
    must sit under `courses/`, `lectures/` or `agents/` in the data directory.
    The four most recent documents stay open, keyed by path and modification
    time. Renders and text extraction share half the cores, each on its own
    big-stack thread so a deeply nested page cannot overflow a stack and abort
    the app; a render also reserves from the process-wide render budget, ahead
    of the embedder's
    ([retrieval.md](./retrieval.md#rendering-has-two-guards-that-look-removable)).
    Each hayro call runs under `catch_unwind`, so a
    PDF that crashes hayro fails only that call. A raster is exactly the size
    asked for, opaque on white. Lines come in reading order from PdfCraft's
    layout: baseline segments, blocks, columns left to right, inferred word
    spaces. Links resolve URIs and in-document destinations, named ones
    included. Errors are codes: `not-found`, `encrypted`, `invalid`,
    `outside-library`, `page-out-of-range`, `invalid-size`, `too-large`,
    `render-failed`.

- **A parsed PDF has two faces, PDF | Markdown** (`PdfMdToggle` in the file
  page's header, state in `usePdfMd`). One shows at a time. A citation with
  a page turns Markdown to PDF.
  - **The Markdown face renders the `.pages.json` record**, not the `.md`
    (`buildBlockDoc` in `app/src/lib/pdf/pdfBlocks.ts`; [parsing.md](./parsing.md)
    has the record): pages joined by "\n\n" as the `.md` is, each cut at its
    blocks' spans with text outside every block kept as its page's own,
    `normalizeMath` per chunk. It is one `ReactMarkdown`, whose output
    matches the `.md`'s apart from the anchors. Two rehype passes
    (`blockAnchorPlugins`) give each top-level element, and each item of a
    top-level list, `data-page` and `data-block` from the chunk its source
    starts in; the second runs after `rehypeMaths`, which replaces a display
    formula's node with one that has no source position. Without a record
    it renders the flat `.md`; a record without blocks anchors pages only.
  - **The faces share one reading position** (`PdfMdLink`, fresh per file):
    each reports the block at its top a moment after scrolling stops and
    once more on unmount, and the face toggled to puts it back near its top. The PDF reads it by layout
    arithmetic over the page sizes and the boxes, the Markdown by offsets,
    never `getBoundingClientRect`. A record without blocks syncs by page. A
    citation not yet shown wins over the stored position. The Markdown face
    holds a restored block in place while images load or its width changes,
    until the reader scrolls.
  - **Boxes are drawn only for cited blocks**, in a layer of `PdfPage`
    under the text spans like find's, from the `bbox` fractions times the
    page size in points. A record without blocks boxes nothing; its
    citations mark spans.

Chat's composer (`harness/mentions/MentionInput.tsx` sends chips as backticked library paths),
picker and timeline are [harness.md](./harness.md).

## An Ed lesson opens as its slides

A lesson's index, `lessons/<id>-<slug>.md`, carries the lesson and an ordered
`slides` list in its front matter ([sync.md](./sync.md)); `parseLessonIndex`
(`app/src/lib/ed/lessons.ts`) reads it. When the file page opens an index
whose list has slides (`useLessonIndex`), its body is `LessonView`
(`app/src/components/lessons/`) instead of `FileViewer`, as in Ed:

- **The sidebar** (`LessonSidebar`, 240px) holds the fold toggle
  (`SideNavCollapseToggle`), the lesson's title, its due date and how many
  slides are completed, then a row per slide (`SlideRow`): a glyph for Ed's
  slide type (`slideIcon`), the title, a green tick once completed. A slide
  not yet opened in Ed reads a shade darker; the selected row takes the nav
  column's active fill. ↑/↓ step through the slides. Open in Ed
  (`OpenOriginal`, ⌘-click for a new tab) opens the shown slide's page in
  Ed, else the lesson's.
- **The sidebar folds to a 46px rail** of slide glyphs, its width easing as
  the nav columns' do and the fold remembered (`useResizablePanel`,
  `oculus-ed-lesson-sidebar`); the header keeps only the toggle. The labels
  and ticks fade out, a small tick (completed) or a brand dot (not yet
  opened in Ed) fades in on each glyph, and a glyph's tooltip is the slide's
  title. The list's scrollbar is hidden, as `SideNav`'s is, so glyphs and
  toggle share one centre line.
- **The pane shows the selected slide through `FileViewer`**: a PDF slide's
  PDF, any other slide's `.md`, both found by path beside the index
  (`slideFiles`). A slide whose rows a sync has not stored yet says so
  quietly. Links inside a slide open beside the page, as any file's do. A
  `code` slide with a `challenge_id` is its
  [workspace](#a-code-challenge-opens-as-its-workspace) instead; `jupyter`
  and other challenges keep the read-only view.
- **The slide rides in the URL** (`&slide=<Ed id>`, written with
  `replace`), so a restored tab or Back reopens it; without one the view
  opens on the first slide not completed (by the synced index, so a tick
  gained here never moves it). Showing a slide stamps its rows accessed, as
  opening a file does; the page stamps the index.
- **Showing a slide tells Ed** (`markSlideOpened`, once per app session,
  [sync.md](./sync.md) has the requests), and the status Ed then holds —
  `seen`, or `completed` for a reading slide — goes into
  `useSlideStatusStore` (`lib/ed/slideStatus.ts`). The sidebar, its count and
  the Lessons tab's rows read the index through it (`useLiveLessonIndex`),
  which only ever raises a status, so a tick shows at once rather than at
  the next sync.
- **A lesson file without front matter** (every slide in one file) renders
  as plain markdown, like any library file.

## A code challenge opens as its workspace

`WorkspacePane` (`app/src/components/lessons/workspace/`) is Ed's challenge
layout: the slide's `.md` (the brief and its Files list, `BriefMarkdown`) in
a left column, drag-resizable and remembered
(`useResizablePanel`), and the challenge's files on the right as `PillTabs`
over `CodeEditor`, the first file open. Run and Mark sit in the editor's
header and the terminal drawer under it, each only where the challenge's
`features` (from `ed_workspace_open`) allow. The library copy under the slide's
folder is the working copy; Rust owns the session
([sync.md](./sync.md) has the state file), the pane only calls the
commands typed in `app/src/lib/ed/workspace.ts`.

- **The brief leaves out the files' copies.** The sync writes each small
  workspace file into the slide's `.md` as a `**<name>**` paragraph over a
  fenced copy, after the Files list; `dropInlineFiles`
  (`lib/ed/briefFiles.ts`) cuts every such copy whose name the list holds
  before the Description renders (`FileMarkdown`'s `transform`), since the
  editor shows the files. The file on disk keeps them, for search, agents
  and the plain slide view. A link in the list (`workspaceFilePath`) opens
  that file's editor tab; a file the workspace does not list, or a ⌘-click,
  opens the library copy.
- **One session per challenge, shared by every pane on it**
  (`holdWorkspace` in `workspaceHandles.ts`). The first hold calls
  `ed_workspace_open`, which returns the file list, the challenge's settings
  and `connecting` at once; the last release writes what is unsaved, then
  calls `ed_workspace_close` a tick later, so a remount keeps the session. A
  new open waits for its challenge's close still in flight.
- **Files are read on first open** (`ed_workspace_read`) into the saver's
  buffers, which keep them across tab switches; each tab keeps its undo
  history and caret for as long as the pane is up.
- **Saving mirrors notes' loop** (`createWorkspaceSaver`): 600 ms after
  typing stops the file goes to `ed_workspace_save`, one write in flight
  across the workspace, looping until every buffer matches what was written.
  A tab switch, closing the pane and `pagehide` write at once. A rejected
  save is an inline `Alert` ("Not saved to Ed") and is retried by the next
  edit; nothing retries on a timer.
- **The status line is Ed's** (`workspaceStatusText`): "Saving…", "Not
  saved to Ed", "Offline — saved on this Mac" (a save that landed only on
  disk, or the socket down; "PC" on Windows, with Rust's reason under it in
  dim text), "Connecting…", else "All changes saved".
- **Offline, the editor keeps saving to the library copy.** Rust reconciles
  with Ed on every connect before it reports `online`, so `online` with
  nothing unsaved turns "saved on this Mac" back into "All changes saved"
  (`saver.synced`), and the open file reads again unless it has unsaved
  edits, the other clean buffers on next open.
- **`ed-workspace` events for this challenge**: `status` updates the line;
  `files` replaces the tabs (when the open file leaves the list, the first opens)
  and drops buffers absent from the new list unless unsaved; a `commit` with
  `ours: false` (Ed's web app saved) re-reads the open file unless it has
  unsaved edits, and forgets the other clean buffers so they read again;
  `conflict` marks a file in conflict; `error` shows as a warning `Alert`
  until the socket is back online.
- **A conflict** is a file both sides changed since the last sync: Rust
  keeps the library copy and writes Ed's beside it as
  `<name>.ed-server.<ext>` (`<name>.ed-server` without an extension).
  `splitListing` (`lib/ed/conflicts.ts`) reads conflicts off the file list,
  so they survive a reload, adds those `conflict` events named, and hides
  the server copies from the tabs. A file in conflict has a warning dot on
  its tab and a bar over its editor (`conflict/ConflictFrame.tsx`): "Ed has
  a different version of this file" with Compare, Keep mine and Take Ed's.
  Compare swaps the editor for a read-only line diff of the buffer against
  Ed's copy (`lineDiff`: LCS of lines, unchanged runs folded), yours red and
  Ed's green as in Feedback. Editing stays open and saves as usual. Keep
  mine writes pending saves and calls `ed_workspace_resolve` with
  `keep: "mine"`; Take Ed's asks first in an in-app dialog, then resolves
  with `keep: "ed"` and reads the replaced file into the buffer over
  anything typed (`saver.replace`). The editor is read-only while a resolve
  runs, both need Ed (disabled offline with the reason), and a failure is
  an inline `Alert`.
- **The terminal drawer** (`terminal/TerminalDrawer.tsx`) is Ed's bottom
  drawer: a Terminal tab when `features.terminal`, a Run tab when Run
  exists, each an xterm.js view (`XtermView`, loaded on demand) kept
  mounted behind the other. Its top edge drags its height
  (`useResizablePanel` with `side: "bottom"`), and it folds to its tab bar,
  folded until first opened; folding keeps the views laid out at full
  height, so it never resizes Ed's PTY. The views fit their box
  (`FitAddon` on a `ResizeObserver`) and report each new size to Rust; their
  colours and font are the theme's tokens and code font, read again when
  either changes. Output events carry base64, written to xterm as bytes;
  what lands before a view mounts or before an id is known is held and
  replayed (`createTermSink` in `lib/ed/termOutput.ts`).
- **The terminal opens on first show** (`useTerminalSession`): the first
  time its tab is showing, unfolded and online, `ed_terminal_open` starts
  Ed's bash, or re-attaches the one still running on Ed (`resumed`, noted
  in dim text). A dropped connection notes it and opens again on reconnect;
  a closed terminal offers "New terminal", a failed open "Try again".
  Nothing kills the terminal when the pane closes: it persists on Ed.
- **Run** shows when `features.run` and the challenge has a `runCommand`;
  without one the terminal is the way to run. A click writes pending saves
  (`flushForEd`, which stops on a save that failed), unfolds the drawer on
  Run, clears it and calls `ed_workspace_run` at its size; output streams
  in, typing goes to the program, and Stop replaces Run until `runExit`,
  which ends in dim text with Ed's message, if any, and "Program exited
  with code N". Offline or connecting, Run is disabled with the reason as
  its tooltip; a failed start is an inline `Alert`.
- **The files a run left** (`runFiles`, `run/RunFiles.tsx`) show under the
  Run tab's output until the next Run: pictures at the strip's width on
  white, as Ed draws them (a click opens `ImageLightbox`), text files as
  chips that open in a dialog, anything else as a chip that shows it in
  Finder or Explorer (`runFileView` in `lib/ed/runFiles.ts`).
- **Reset to scaffold** is the one item of the header's overflow menu
  (`reset/WorkspaceMenu.tsx`), disabled offline. It asks in Ed's words
  ("Reset challenge attempt?") in `ConfirmDialog`; confirming drops every
  buffer of the saver, unsaved text included, and waits out a write in
  flight (`saver.discard`, `reset/useReplaceFlow.ts`), so no save of the old
  text follows, then calls `ed_workspace_reset`. A failure is an inline
  `Alert`. Restore shares the flow.
- **A `reset` event** (a reset or a restore) drops every buffer again,
  forgets each tab's undo history and remounts the editor, which reads the
  files anew; `files` has already replaced the tabs, and the terminal's
  `termState exited` offers "New terminal".
- **Mark** shows when `features.mark` and only ever submits from its
  button (`mark/useMarkFlow.ts`): write pending saves, read the live
  standing (`ed_challenge_standing`), then ask in one in-app dialog what
  `markPrompts` (`lib/ed/markPrompts.ts`) says applies — past the due date
  ("Submit late?"), the last attempt (`attemptsRemaining` 1), or
  `features.confirmSubmit` — and submit (`ed_workspace_mark`) on Submit. No
  prompt applies, no dialog. It is disabled offline and while a mark is in
  flight, and a second click is ignored; a failed save, standing read or
  mark is an inline `Alert` and is never retried by itself.
- **Feedback** is a tab beside Description in the left column when
  `features.feedback` or `features.mark` (`feedback/FeedbackPanel.tsx`). It
  loads `ed_workspace_submissions` (Ed's list of the student's
  submissions, newest first) on first open, and a finished Mark joins the
  list, is shown and switches the column to it. The shown submission has
  Feedback and Code views and a Restore button. A submission reads as Ed's panel:
  "N / M passed, P points" (summed testcase scores, `lib/ed/feedback.ts`),
  Ed's feedback, a failed build's output, then each testcase with ✓ or ✗,
  its feedback, its output and its `check_diff` lines as word runs (your
  output red, expected green, a missing newline marked). Hidden and private
  testcases show their name only. Code (`feedback/SubmissionCode.tsx`)
  reads `ed_submission_files` and shows the files as it went in, in tabs
  over a read-only `CodeEditor`. Restore asks "Restore this submission?"
  with the submission's date, then replaces the workspace with its code
  (`ed_workspace_restore`) through the same flow as Reset; disabled while a
  status event says Ed is unreachable. Every submission is listed below as
  Ed's table — its number, when it went in, testcases passed and ✓ or ✗ —
  to pick from.

## One code editor for source files

`CodeEditor` (`app/src/components/code/CodeEditor.tsx`, loaded on demand
through `LazyCodeEditor`) is a CodeMirror view that fills its box and
scrolls itself, with a fixed gutter on the card's fill. It borrows the note
editor's Raw look and line numbers (`rawCodeChrome` in
`components/documents/editor/chrome/rawMode.ts`) and its code colours
(`codeHighlight`), and takes the grammar from the file name
(`fileLanguage`: language-data's extensions, `.m` as MATLAB), loaded on
first use. The indent unit is the one the file uses (`detectIndent`: tabs,
or the commonest step of 2–8 spaces, else 4); Tab indents. A new `text`
prop it does not hold is applied as one minimal change outside the undo
history (`textChange`), so another copy's edit keeps the caret. `readOnly`
turns editing off; the Edit menu's Undo and Redo reach it while focused.

## One media player plays lectures and library videos

`useMediaPlayer` and `MediaPlayer` (`app/src/components/media/MediaPlayer.tsx`)
are the player: clock, control bar, scrub preview, speed, volume, keys,
fullscreen, captions and the dock's frame (`MediaDock`). The caller owns the
element and the dock's tabs. It hands its element over with `attach(v)` from
its own effect and calls the returned detach on cleanup; it passes slots for
what only it has — frames, an overlay, bar buttons, the line above the scrub
bar. Every video streams from Rust's media server via `mediaSrc()`
(`app/src/lib/lectures/media/src.ts`, absolute or library-relative), not `convertFileSrc`
([architecture.md](./architecture.md)).

- **`LecturePlayer` adds the lecture's layers**: the shared elements and their
  owner (below), two sources with PiP and stack, chapters, chat, downloads and
  progress.
- **A video file in the library opens in the file page**
  (`openFileSmart` and `filePageHref` route `isVideoFile` there, not to the
  system viewer), where `VideoFileViewer` renders its own `<video>`. Its
  captions are the sibling `<video>.vtt`, and one that exists but won't read
  is an error, never a Transcribe offer. The element pauses when its tab goes
  to the back, since nothing parks it, and keeps no position.
- **A Studio render uses it two ways** (`VideoPlayer`, see
  [studio-page.md](./studio-page.md)): on a video's detail page with a dock
  (Script and Scenes in a `MediaDock`, sharing the dock size and open state
  with lectures but keeping its own side, passed as `dockSide`; `dockKeepWidth` asks the picture to keep more than
  300px beside a side dock), and on its thread page without one. `dock` and
  `dockLabel` are optional, and with no dock there is no dock button, no
  resize handle and T does nothing; `captions={false}` leaves out the captions
  button. `useLibraryPlayback` is what `VideoFileViewer` and Studio's element
  share: pause with the tab, and one sound across lectures and library videos.
- **Files ▸ Media lists the videos Oculus fetched** — module videos and
  Canvas-page embeds, the `file` rows that are videos
  (`app/src/pages/subject/files/MediaPage.tsx`) — with a captions mark when the
  sibling `.vtt` has content. A video not yet downloaded shows only on its
  module.
- **One sound at a time**: a library video playing pauses the lecture
  (`pauseLecturePlayback`) and other library videos; a lecture's leader
  playing pauses every registered library video (`registerLibraryVideo` in
  `app/src/lib/lectures/media/library.ts`).
- **A video without a transcript offers Transcribe** in its Transcript tab
  (`TranscribeEmpty`). Runs live in `useTranscription`, outside React, so a
  player opened mid-run shows the run; a finished run fires
  `TRANSCRIBED_EVENT` and stays `done` (a spinner, no second Transcribe)
  until the player has read the cues and calls `settleTranscription`. A lecture transcribes
  its `source1.mp4` and records the VTT in `lectures.transcript_path` in the
  run's `after` step, which runs even if the player has gone. Echo360's own
  transcript is still fetched by T and the dock button.

## The lecture player's elements outlive the page

- **`app/src/lib/lectures/playback/` owns the `<video>` elements**, so playback
  survives tab switches. Between players they park in a 480×270 off-screen
  host — `display: none` lets WebKit stop playback, and a tiny host decodes
  tiny. Progress saves every 5s and on pause, seek, end and `pagehide`,
  chained so writes land in order.
- **Done is the lecture's end, not the file's.** A progress write within 10 s
  of the found content end ([chapters.md](./chapters.md#where-the-lecture-ends))
  marks the lecture complete; without one, within 30 s of the file's end.
  `lectureEnd` in `app/src/lib/lectures/end.ts` is the one place that end is
  chosen, from the latest state of the end job rather than the player's row,
  and Up Next reads it too. So does `progressLabel`, the lists' "x left",
  counted to that end.
- **Done is also a toggle.** The status circle at the left of a `LectureRow`
  marks the lecture Done or not (`setLectureDone`). Marking keeps the
  position, since it may not have been watched; unmarking one watched to its
  end starts it over, or the next progress save would mark it Done again.
- **One player owns the elements** (`app/src/lib/lectures/playbackOwner.ts`). A tab's
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
  strip, always mounted because Chat needs no file. Transcript is dropped only
  while a transcript on disk has no cues loaded; a lecture without one keeps it
  for Transcribe. `useTranscriptDock` raises
  the floor for Chat and caps the drawn size by the container (`KEEP_W` /
  `KEEP_H`), dropping a side dock to the bottom when it can't fit — without
  rewriting the preference.
- **The transcript is virtualised** (~2500 cues a lecture) through `FollowList`.
  Following is tracked by pointer *intent*, since the follow-scroll fires
  `scroll` too, and runs off the active-cue index rather than `timeupdate`.
  Don't call `measure()` on a search: heights are cached by cue key and survive
  it. Key callbacks stay stable between searches so a playback highlight does
  not rebuild every row's offsets. Chapters are [chapters.md](./chapters.md).

Playback's ordered cue and chapter starts use an upper-bound lookup,
including the last entry when timestamps coincide. A player subscribes only to
its lecture's download progress; stable dock callbacks keep playback ticks
outside the transcript's memo boundary. The catalogue's `LectureRow`
(`app/src/components/lectures/LectureRow.tsx`) reads only its own
source-qualified download progress and active state.

## Up Next offers the subject's next lecture

`UpNextCard` (`app/src/components/lectures/UpNext.tsx`) is the card the
lecture player shows bottom-right in the frame once the lecture is over.

- **Next** is `getNextLecture`: the same subject's earliest strictly later
  `date`, ties by title then id, Done or not. None, no card.
- **It shows** while the playhead is at or past `upNextFrom` — the found end,
  else 20 s before the file's — and the player holds the element. Seeking back
  hides it; × hides it until the lecture is opened again (player state).
- **It goes through `MediaPlayer`'s `overlay` slot**, so it is inside the video
  area and therefore inside the fullscreen overlay. `z-35` puts it over the
  control bar's scrim (`z-30`) and under the other pane's still (`z-40`); while
  the bar shows it is lifted by a fixed translate, never a measurement, so page
  zoom holds. It has no keys of its own; Space still plays the lecture.
- **No countdown while the file plays** — the Q&A may be wanted. `useEnded`
  watches the element's `ended`; then the Play pill fills over 8 s and plays.
  A play, seek, pause, new source or × cancels it. Under
  `prefers-reduced-motion` the fill is hidden and the pill counts "Play in 5".
- **Play** awaits `completeLecture` (pause, the last progress write, then
  `markLectureComplete`, on the same chain so no write lands after), sets
  `playOnAdopt` and replaces the pane's route with the next lecture page. Same
  route, so the page and player stay mounted (fullscreen too); the next
  `syncLectureSources` for that lecture plays it, from the start if it was
  already watched. The player tags its media URLs with their lecture, so the
  last lecture's file is never adopted under the new one's id.
- **The thumbnail** is `lecture_thumbnail`: one `grab_frame` (with its probes)
  a quarter of the way in, source 1 else source 2, 320 px wide, cached as
  `lectures/<id>/thumb.jpg` (written aside and renamed). An image, so the card
  loads it over the asset protocol with `convertFileSrc`, not the media
  server. Not downloaded, it returns nothing and the card shows the subject's
  icon and a Download pill (`lectureDownloadStore`) in place of Play; the
  download's event re-reads the next lecture.

## Videos without captions are transcribed

Echo360 publishes WebVTT captions; other videos in the library have none.
`transcribe_video` (`app/src-tauri/src/transcribe/app.rs`) takes one video —
absolute, or relative to the data directory — and writes its transcript beside
it as `<video>.vtt` (`Week 1.mp4` → `Week 1.mp4.vtt`), returning that path. It
knows nothing of lectures or the database: a caller that tracks transcripts
records the path itself. `transcribe` in `app/src/lib/lectures/transcribe/engines.ts` is the
frontend's call; `oculus transcribe` is the terminal's.

1. The path is canonicalised and must land inside the data directory, so
   neither `..` nor a symlink reaches a file outside the library.
2. ffmpeg decodes the first audio stream to 16 kHz mono Ogg Opus at 24 kbit/s
   (constrained VBR) — about 11 MB an hour, so most recordings are one upload.
   The audio lives in a private temp directory, never in the library.
3. Audio over the engine's upload cap is cut by time into the fewest equal
   spans (stream copy, fixed boundaries, no overlap); each span's segments are
   offset by its start.
4. The VTT is written atomically after the last span returns. A failed span
   fails the run, and a run that heard no words is an error, never an empty
   transcript.
5. Progress is `transcribe-progress`:
   `{path, phase, engine?, chunk, chunks, error?}`, keyed by the path as the
   caller passed it, with `phase` one of `extracting`, `transcribing`,
   `complete`, `error`. `engine` (`groq`, `apple` or `whisper`) rides on
   `transcribing` and `complete`, and the player names it ("Transcribing with
   local Whisper"); a fall-through restarts `chunk` at 1 under the next engine.

**Engines are tried in the order set in Settings → Transcription** — Groq,
local Whisper, then on-device speech until the user drags them into another.
Windows offers Groq only: local Whisper and on-device speech are hidden in
Settings and excluded from backend selection, even if their switches are stored.
A missing enabled Groq key fails the run with a Settings → Transcription hint.
An `Engine` turns one audio file into timed segments; the pipeline owns
everything else. `engines()` in `app/src-tauri/src/transcribe/settings/mod.rs` lists
the configured ones in that order before any decoding: Groq when it is on and
its key is saved (keyd's `has groq`, or the keychain when keyd is absent),
whisper.cpp when it is on, a model is downloaded
and `whisper-cli` is found, Apple's on-device recogniser on macOS when it is
on and its helper is found. With none, the run fails at once and says to set
one up in Settings → Transcription.

- **One settings row, `transcribe`**, holds the order, the language and each
  engine's switch:
  `{"order": ["groq", "whisper", "apple"], "language", "groq": {"enabled"},
  "whisper": {"enabled", "model"}, "apple": {"enabled"}}`, parsed once per
  run by `settings_from` in `settings/mod.rs` and mirrored by `parseTranscribeSettings`
  in `app/src/lib/lectures/transcribe/settings.ts`. A missing row, key or mistyped value reads
  as its default — the default order, every engine on, the default language,
  the model `pick` chooses. An order keeps known names once each and appends
  any missing in the default order. Unknown keys survive both Rust and
  Settings, which rewrites the whole row (`writeTranscribeSettings`).
- **One language for every engine.** `language` is `auto`, a locale such as
  `en_AU`, or a bare code; absent, it is the default, English in the Mac's
  region. A row without it reads the older `apple.locale`, then
  `whisper.language`; Settings writes only the top-level key and drops those.
  Each engine derives its own form (`Language` in `settings/mod.rs`): Whisper's `-l`
  and Groq's `language` take the language part (`en`), Groq omitting it for
  `auto`; on-device speech takes the whole locale, and for the default, `auto`
  or a bare `en` its helper's default locale, since Apple has no
  auto-detection. The default is English rather than `auto` because
  auto-detection judges from the first 30 s, which is easily noise.
- **Only a decline falls through.** `EngineError::NotConfigured` (Apple's
  helper on a Mac before 26) and `RateLimited` (Groq's free tier), at any
  span, hand the run to the next engine: the extracted audio is reused,
  re-planned for that engine's cap, and what the first engine heard is
  dropped. `Failed` — a refused key, an unreadable file, a crash — always
  surfaces. When every engine declines, the error carries each one's reason.
- **`oculus transcribe --engine groq|whisper|apple`** uses that engine alone,
  with no fallback, though still not while it is switched off. The app always
  runs the whole order.
- **No timeout sits above any engine**, for the reason parsing has none
  ([parsing.md](./parsing.md#a-parse-takes-minutes-and-only-the-engine-bounds-it)).
- **Cues carry no identifiers**: `parseVtt` (`app/src/lib/lectures/media/vtt.ts`) would
  join a bare number line into the cue's text.
- One run per video per process, so a double click cannot spend the allowance
  twice.
- **Settings → Transcription is the Language select and the engine list.**
  The select offers Auto-detect, then on-device speech's locales when it
  lists them — the one engine that needs a region — else bare codes, English
  first, with a region shown only where a language has several
  (`languageOptions`). The list is one bordered group in run order: a row
  drags to a new place (`useStripReorder`) or moves with the arrow keys on its
  handle, and carries a one-line status, its switch, and a button opening the
  engine's dialog — Groq's key, Whisper's models, on-device speech's
  availability and downloaded languages. Model downloads are held by the page
  (`useWhisperDownloads`), so a row's status follows one after its dialog
  closes.

### Groq: Whisper over HTTPS

`whisper-large-v3-turbo`, one multipart upload per span, naming the language
unless it is `auto` (`app/src-tauri/src/transcribe/engines/groq.rs`).

- **Groq's free tier refuses uploads over 25 MB**; the budget is 20 MB
  (`UPLOAD_BUDGET`) because a cut by time can give one span more than its
  share of bytes.
- **The free tier also caps seconds of audio per hour and per day.** A 429 is
  `RateLimited` and says when to retry: `retry-after`, else Groq's own "try
  again in", else the reset header.
- **Saving the key lists Groq's models, which is free** — Settings never
  transcribes ([harness.md](./harness.md#no-model-is-ever-probed)). The probe
  runs in the app with the key just typed; the key is then stored through
  `oculus-keyd` when it is installed, else in the keychain
  (`app/src-tauri/src/providers/groq.rs`, shaped like `voyage.rs`).
- **With keyd installed, the upload goes through it**
  (`app/src-tauri/src/transcribe/engines/groq.rs`): a `forward` of the multipart body
  to `/openai/v1/audio/transcriptions` with no timeout, keyd adding the key
  ([architecture.md](./architecture.md#oculus-keyd-is-the-only-process-meant-to-read-its-key)).
  Both routes hand back the same status, headers and body, so the 429, 401
  and 413 messages read one way; keyd's own refusals say oculus-keyd could
  not send the request.

### On-device speech: a Swift helper

macOS 26's `SpeechAnalyzer` has no Rust binding, so a single-file Swift
program, `apple-speech`, wraps it and the engine
(`app/src-tauri/src/transcribe/engines/apple.rs`) spawns it like ffmpeg. It is free,
offline after the language's model is on the Mac, needs no permission prompt,
and reads the pipeline's Ogg Opus in one call — an hour of audio in about a
minute — so its cap is `None` and a run never splits.

- **Two subcommands, JSON on stdout.** `apple-speech locales` answers
  `{available, reason, supported, installed, defaultLocale}`.
  `apple-speech transcribe [--locale <id>] <audio>` downloads the locale's
  model when it is missing, then answers
  `{segments: [{start, end, text, words: [{start, end, text}]}]}`. Exit 3 is
  "not on this Mac" (before macOS 26, or the recogniser unavailable), which
  the engine reads as `NotConfigured`; any other non-zero exit is `Failed`
  with the helper's last stderr line.
- **The default locale is English in the Mac's region, else `en_US`** — never
  the system language, since the Mac may be set to another language while the
  lectures are English. Matches are exact: `supportedLocale(equivalentTo:)`
  answers an unsupported region differently from run to run.
- **A result is an utterance, up to half a minute**, so the engine splits each
  into cues of at most 7 s on its word timings: after a sentence end that
  fits, else after a clause break in the window's second half, else where the
  pieces come out most even. No cue is under 1.5 s unless a single word
  forces it.
- **Settings reads `apple_speech_status`** for the language select and the
  engine's row and dialog. It runs `apple-speech locales`: it lists what the
  OS has, never downloads a model, and answers `available: false` with a
  reason rather than an error.
- **The helper launches on any macOS the app does** (built for 10.15, with a
  synchronous `main`), so an older Mac gets exit 3 rather than a dyld crash.
  It ships as an `externalBin` from `app/src-tauri/tauri.macos.conf.json`,
  found by `app/src-tauri/src/runtime/bundled.rs` beside the executable or in dev's
  `binaries/`
  ([development.md](./development.md#the-speech-helper-is-compiled-not-fetched)).

### Local Whisper: whisper.cpp with a downloaded model

`whisper-cli`, built from a pinned whisper.cpp release
([development.md](./development.md#whisper-cli-is-built-from-a-pinned-release)),
runs a ggml model on this Mac — on the GPU through Metal on Apple Silicon —
free and offline once the model is on disk
(`app/src-tauri/src/transcribe/engines/whisper.rs`). It windows the audio itself, so
its cap is `None` and a run never splits; there is no timeout, since a long
recording on a large model takes minutes.

- **whisper-cli cannot read Ogg Opus**, so each call decodes the extracted
  audio to 16 kHz mono 16-bit WAV beside it in the run's temp directory, and
  reads back `-oj`'s JSON. A zero exit with no JSON is still a failure, named
  by stderr's last `error` line. Marker-only segments (`[BLANK_AUDIO]`,
  `(music)`, `♪`) are dropped.
- **Silero VAD skips silence**, where Whisper otherwise invents "Thank you."
  loops. Its model (under 1 MB) downloads before the first Whisper model; a run
  without it goes on without `--vad`.
- **Models live in `~/Library/Application Support/com.tchan.oculus/models/whisper/`**
  (`whisper_models::dir`), outside `courses/` and `lectures/`, where folder
  scans and agents look. `MODELS` is a fixed catalogue of seven, worst to
  best; downloads come from Hugging Face without an account, stream to
  `<file>.part` and rename into place, with no deadline but a 60 s stall.
- **The model a run uses** is `whisper.model` when set — not downloaded, the
  engine is unconfigured rather than quietly using another — else the default,
  `large-v3-turbo-q5_0` (near large-v3's accuracy at a fifth of its size), if
  downloaded, else the largest downloaded (`pick`, mirrored by
  `pickWhisperModel` for the engine row and the dialog's Automatic choice,
  which clears `whisper.model`).
- **Fit is judged against total RAM**: a model needing more than half is
  marked too large (still downloadable), and exactly one is recommended — the
  default when it needs at most a quarter and a GPU runs it, else the largest
  needing at most a quarter, capped at `small` without a GPU (`fits`).
- **Settings only lists, downloads and deletes.** `whisper_models` reads the
  directory, the RAM size and whether `whisper-cli` is found, so it runs on
  every macOS visit; `whisper_download_model`
  resolves when the file is on disk (rejecting `cancelled` on a cancel) and
  reports through `whisper-model-progress`
  (`{id, phase, received, total, error?}`, every ~150 ms). A download in
  flight is flagged `downloading` in the listing, so a revisited page picks up
  its bar. The dialog lists the recommended model first and folds the rest
  under "Other models" unless one of them is downloaded or chosen. Deleting the chosen model clears `whisper.model` so `pick` chooses
  again.

## The in-app browser is a native page per tab, owned by Rust

A browser tab is `/browse/<id>`, naming a native child webview (WKWebView on
macOS, WebView2 on Windows) that
`app/src-tauri/src/shell/browser/` places at an empty slot in `BrowserPage`, seen
through a hole the app opens ([architecture.md](./architecture.md#native-pages-show-through-holes-the-app-opens)).
Canvas refuses iframes.

- **Rust owns the tab list** and pushes `browser-state` on every change;
  `useBrowserTabs` reconciles it per *pane*. The mirror preserves unchanged
  page objects so another tab's snapshot does not rerender every pane.
  Page navigations change the URL in
  Rust only, never the route. What a page knows about itself (history, find,
  zoom) is pushed as events, because `with_webview` returns nothing.
- **A page is hidden only while its tab is backgrounded** (`useTabActive`).
  Popovers and the address suggestions (`AddressSuggestions`, a portal) draw
  over the live page. Hiding throttles a page's timers, and `browser_place`
  focuses it, so nothing else hides or re-places it.
- **Windows page creation runs off the main event loop.** `browser_open_url`
  is an async command so its tab ID or creation error reaches the caller without
  blocking WebView2. Popup tabs and the continuation after cookie seeding stay
  on worker threads; moving either back to the main loop deadlocks child creation.
  macOS popup creation is queued onto the main thread after the WebKit callback.
- **Windows browser pages and the login window share WebView2's default jar.**
  Cookie reads and writes run on worker threads: calling Tauri's cookie APIs
  on the main event loop deadlocks. Seeding copies both Canvas and SSO headers
  from keyd and reads back the jar until queued writes match before navigating.
  Sign-out deletes both hosts' cookies from the same jar.
- **Windows uses JavaScript for browser history, reload and find.**
- **History is one row per URL, ranked by frecency**, recorded on load
  *finish* (one row per redirect chain). `historyUrl` drops the fragment, and
  the whole query if any part looks like a credential.
- **macOS pages claim to be Safari** (`PAGE_USER_AGENT`) so UA-sniffing sites
  serve their full pages; Windows keeps WebView2's default user agent.
  An `initialization_script` reports a real
  `outerWidth`/`outerHeight` (a child view's are 0, which drops canvas renderers
  to minimum scale).
- **Pages get every plugin's init script.** `tauri-plugin-opener` is built
  with `open_js_links_on_click(false)`: its click handler would cancel a page's
  `target=_blank` links and call IPC pages cannot reach, so only ⌘-clicks got
  through. A `_blank` link reaches `on_new_window` and opens as a tab; the
  app's own links go through `AppLayout`'s capture-phase handler.
