# Canvases and notebooks live in Drive

Drive holds the student's own notes and drawings among their other files. A drawing is one of
two kinds on the same engine: a **canvas**, an infinite whiteboard, or a
**notebook**, paper pages to write on. Either opens in a tab or beside another
page.

## Where

| Piece | Location |
| --- | --- |
| Drive's folder browser, its Type filter and New menu | `app/src/components/drive/` ([frontend.md](./frontend.md#drive-is-one-folder-browser-over-the-disk)) |
| File route and editor choice (passes the file's kind to `CanvasEditor`) | `app/src/pages/subject/files/FilePage.tsx` |
| The two kinds: extension, category, path and row helpers, reconcile plan | `app/src/lib/canvas/drawingKinds.ts`, `DrawingKind` in `app/src-tauri/src/library/files/canvases.rs` |
| Extension → category for a path (`.notebook` is `notebook`) | `app/src/lib/files/libraryLayout.ts`, `app/src-tauri/src/library/layout/category.rs` |
| Drawing file rows, reconciles and native command wrappers | `app/src/lib/canvas/files.ts` |
| Shared ordered saves and the shared-canvas adapter | `app/src/lib/canvas/sessions.ts`, `app/src/lib/canvas/sessionRuntime.ts`, `app/src/lib/canvas/authority.ts` |
| Desktop recovery drafts | `app/src/lib/canvas/drafts.ts`, `app/src-tauri/src/library/files/canvases/drafts.rs` |
| Authority command wrappers and event listeners | `app/src/lib/canvas/files.ts` (`canvasAuthority`) |
| Title, save state, recovery, conflict and pause controls, the Share button and editor host | `app/src/components/canvas/CanvasEditor.tsx` |
| Drawing surface, label text overlay, tool bar and style bar | `app/src/components/canvas/CanvasSurface.tsx`, `app/src/components/canvas/CanvasToolbar.tsx`, `app/src/components/canvas/CanvasStyleBar.tsx` |
| Pointer gestures, history, keyboard commands and the notebook's page rules | `app/src/hooks/canvas/useCanvasInteraction.ts` |
| Two-finger pan and pinch | `app/src/lib/canvas/touchViewport.ts` |
| Pan momentum: release speed and glide | `app/src/lib/canvas/momentum.ts` |
| What a reload resumes on this device: tool, drawing style, each drawing's camera | `app/src/lib/canvas/viewState.ts` |
| Zoom ranges | `app/src/lib/canvas/zoom.ts` |
| Content transactions (undo steps and shared edits) and rebasing | `app/src/lib/canvas/collaboration.ts`, `app/src/lib/canvas/clientReconciliation.ts` |
| Device-local drawing preferences (the eraser's settings) | `app/src/lib/canvas/toolPreferences.ts` |
| Partial erasing: cutting strokes along the eraser's sweep | `app/src/lib/canvas/erase.ts` |
| What a selection box selects (release and live preview) | `app/src/lib/canvas/boxSelect.ts` |
| Native selection, callout and touch defaults kept off the canvas | `app/src/lib/canvas/browserSelectionGuard.ts` |
| Canvas2D drawing and image export | `app/src/lib/canvas/render.ts`, `app/src/lib/canvas/renderSvg.ts` |
| Text fonts, measuring, wrapping and label layout | `app/src/lib/canvas/renderText.ts` |
| Versioned document model and coordinate helpers | `app/src/lib/canvas/model.ts`, `app/src/lib/canvas/geometry.ts` |
| Pages: the `paper` field, `notebookPaper`, layout, templates, locked backdrops, the page camera | `app/src/lib/canvas/paper.ts` |
| Drawing pages and templates, the element → page owner, export bounds | `app/src/lib/canvas/renderPaper.ts`, `renderPagePng` and `paintCanvas` in `app/src/lib/canvas/render.ts`, `paperSvg` in `renderSvg.ts` |
| What the pointer reaches on a page | `app/src/lib/canvas/pageHits.ts` |
| Page operations: add, insert, duplicate, move, remove, template, size, PDF pages | `app/src/lib/canvas/pages.ts` |
| Page bar, page sidebar, thumbnails, per-page menu and drag to reorder | `app/src/components/canvas/pages/` |
| What a page's picture depends on | `app/src/lib/canvas/pageFingerprints.ts` |
| New notebook: blank paper or a copy of a library PDF | `app/src/components/canvas/NotebookDialog.tsx`, `app/src/lib/canvas/notebook.ts` |
| A PDF's Draw button: a notebook or a canvas copy of its pages | `app/src/components/canvas/DrawOnCopy.tsx`, `app/src/lib/canvas/pdfCopy.ts` |
| Page pictures for chat agents: change detection, writer, hook, Rust commands and manifest | `app/src/lib/canvas/snapshots.ts`, `app/src/hooks/canvas/useNotebookSnapshots.ts`, `app/src-tauri/src/library/files/canvases/snapshots.rs`, `app/src-tauri/src/library/paths/notebook.rs` |
| Which notebooks are open, for the chat message | `app/src/lib/canvas/openNotebooks.ts` (read by `harnessSend` in `app/src/lib/harness/commands.ts`) |
| `oculus read` on a notebook | `app/src-tauri/src/bin/oculus/commands/query/notebook.rs` |
| PDF export | `app/src-tauri/src/library/files/canvases/export_pdf.rs`, `exportPdf` in `app/src/lib/canvas/platform/` |
| Style fields, which element types take them, inspector sections | `app/src/lib/canvas/style.ts` |
| The style bar's controls, its More styles and selection popovers, palettes and popover guards | `app/src/components/canvas/inspector/` |
| Colour palette popover | `app/src/components/ui/color/ColorPalette.tsx` |
| Shape labels: commit, grow rule, restyling | `app/src/lib/canvas/labels.ts` |
| Bindable outlines | `app/src/lib/canvas/outline.ts` |
| Arrow binding, end placement, following | `app/src/lib/canvas/bindings.ts` |
| Arrow path (the one rendered path), curve, point edits, handles, retyping | `app/src/lib/canvas/arrow/` |
| Elbow routing: headings, ends, grid, A*, route, fixed segments and segment drags | `app/src/lib/canvas/elbow/` |
| Scaling a selection | `app/src/lib/canvas/resize.ts` |
| Image import modal (and a notebook's Insert PDF pages), library PDF search and selectable pages | `app/src/components/canvas/CanvasImageDialog.tsx`, `app/src/components/canvas/CanvasLibraryPdfs.tsx`, `app/src/components/canvas/CanvasPdfPages.tsx` |
| Image decoding and batch placement; native PDF import sessions | `app/src/lib/canvas/importMedia.ts`, `app/src/lib/canvas/mediaPlacement.ts`; `app/src/lib/canvas/importPdf.ts` |
| Image drops, placement previews and raster paste | `app/src/hooks/canvas/useCanvasMediaDrop.ts`, `app/src/lib/ui/nativeFileDrop.ts`, `app/src-tauri/src/library/files/canvases/images.rs` |
| What the surface takes from its host (export saving, library PDF search and opening): the browser default and the desktop's | `app/src/lib/canvas/platform/` |
| Area captures, crop geometry and clipped PNG rendering | `app/src/lib/canvas/region.ts`, `app/src/components/canvas/CanvasRegionActions.tsx`, `app/src/components/canvas/CanvasCaptureHold.tsx` |
| Embedded raster validation and Canvas2D image cache | `app/src/lib/canvas/imageSource.ts`, `app/src/lib/canvas/renderImages.ts` |
| Scoped library PDF sessions and page rasterisation | `app/src-tauri/src/library/files/canvases/imports.rs` |
| Element edits as replaceable maps, and additions stacked above an element | `app/src/lib/canvas/changes.ts` |
| Layer order moves | `app/src/lib/canvas/layers.ts` |
| Clipboard envelope and clone with remapped bindings | `app/src/lib/canvas/renderClipboard.ts` |
| Retained Rust scene and WebAssembly bridge | `app/canvas-core/src/`, `app/src/lib/canvas/engine.ts` |
| Native file commands, both kinds, and scoped paths | `app/src-tauri/src/library/files/canvases.rs` |
| Shared-canvas authority: validator, transactions, sessions, receipts, commands | `app/src-tauri/src/library/files/canvases/authority/` |
| `files` row updates from Rust | `app/src-tauri/src/db/store/files.rs` (`touch_file_row`, `rename_file_row`, `delete_file_row`) |
| Format and transaction parity fixtures (run by the TypeScript and Rust tests) | `app/tests/fixtures/canvas-model/`, `app/tests/fixtures/canvas-collab/`, `app/tests/lib/canvas/collabFixtures.test.ts` |
| WebAssembly build | `app/scripts/build-canvas.mjs` |
| Format, geometry, labels, style, bindings, layer order, pages, WASM parity and save-session tests | `app/tests/lib/canvas/` |
| Pen, touch, undo, notebook and outside-document tests that drive the hook's handlers | `app/tests/hooks/canvas/` |

## The kind is the file's extension

| Kind | File | `files` row |
| --- | --- | --- |
| Canvas | `courses/<code>/…/<title>.canvas` | `category = canvas`, `file_type = canvas` |
| Notebook | `courses/<code>/…/<title>.notebook` | `category = notebook`, `file_type = notebook` |

Either sits anywhere among the student's own files; a new one lands at the
subject's root. `list_canvases` walks the own files at any depth
(`layout::own_files`), so a drawing moved into a folder keeps its row.

Both are the same JSON. The kind comes from the path (`DrawingKind::of`, the
own-file guard, and `drawingKindOfPath`) or the row's category (`drawingKindOf`), never from what
the document holds. `FilePage` passes it to `CanvasEditor`, the companion page
reads it from the snapshot's `x-oculus-kind` header
([device-sharing.md](./device-sharing.md)), and below the editor it is one
boolean, `notebook`.

- **Commands.** `create_canvas` and `list_canvases` take an optional
  `kind` (`"canvas"` by default; `createDrawing`, `createCanvas`,
  `createNotebook` in `files.ts`). Read, write, rename and delete take a path
  and accept either kind's; a rename keeps the file's own folder and
  extension. A notebook's content must have valid `paper`
  (`validate` in `canvases.rs`), so a write that drops it is refused.
- **Reconciles.** Each kind has its own (`reconcileCanvases`,
  `reconcileNotebooks`, both `reconcileDrawings`), planned by
  `planDrawingRows`: it adds rows for its files on disk and removes rows of
  its own category whose file is gone, and never touches the other kind's
  rows or folder. A removed row's page pictures, if any, go with it.
- **Drive** lists notes, canvases and notebooks in the student's folders
  beside their other files; its Type filter narrows to one kind. New offers
  Note, Canvas and Notebook… (a dialog, below), made in the folder on screen.
  A card's picture of a drawing is drawn from its file by this renderer at
  card size, or is a notebook's first page picture
  ([frontend.md](./frontend.md#drive-is-one-folder-browser-over-the-disk)).
  Notes are ordinary Markdown documents ([editor.md](./editor.md)). A title
  is its filename; a rename keeps the database ID, and open tabs on the file
  follow it ([frontend.md](./frontend.md#path-keyed-state-follows-drive)).

## The format is one open envelope

The envelope is `type: oculus-canvas`, `version: 1`, with elements, a viewport
and background. Element coordinates are world coordinates; viewport x/y
translate in screen units before applying zoom. Polyline points are local to
their element's x/y. IDs are unique within one document.

The format describes geometry and appearance, independently of the editor
implementation. Sketch and clean are per-element appearance choices. Extra
JSON fields survive loading, edits and saving, so metadata can accompany the
scene. Unsupported versions and element kinds fail validation instead of
being dropped. Introducing a new geometry kind requires coordinated model,
renderer, Rust geometry and format validation changes.

Appearance and label fields are optional and validated when present; the
version stays 1, and an absent field means its default (`CANVAS_DEFAULTS` in
`model.ts`). A writer omits a field that equals the default.

| Field | Values (default) | Applies to |
| --- | --- | --- |
| `opacity` | 0..1 (1) | every element |
| `strokeStyle` | `solid` `dashed` `dotted` (solid) | shapes, line, arrow |
| `fontFamily` | `sans` `serif` `mono` (sans) | text, labelled shapes |
| `textAlign` | `left` `center` `right` (left for text, center for labels) | text, labelled shapes |
| `verticalAlign` | `top` `middle` `bottom` (middle) | labelled shapes |
| `startArrowhead`, `endArrowhead` | `none` `arrow` `triangle` `dot` `bar` `diamond` (none, arrow) | arrow |
| `roundness` | `sharp` `round` (round for a rectangle, sharp for a diamond) | rectangle, diamond |
| `arrowType` | `sharp` `curved` `elbow` (sharp) | arrow |
| `startBinding`, `endBinding` | `{elementId, mode: 'center'}` or `{elementId, mode: 'edge', anchor}` with `anchor` x/y in 0..1 of the target's bounds | arrow |
| `fixedSegments` | an array in path order of `{x}` (a vertical segment on that line) or `{y}` (a horizontal one), local to the element's x/y (none) | elbow arrow |
| `text`, `fontSize` on rectangle, ellipse, diamond | the shape's label | shapes |
| `locked` | boolean (false) | every element |

A binding's target ID is not checked at load, so a dangling binding never
blocks opening a file; its end stays where it is.

`locked: true` marks a backdrop, such as a page image taken from an
assignment, on either kind. It is drawn like any element, but the pointer
never picks it: no click, box or select-all selects it, nothing moves, resizes,
crops or erases it, and layer moves treat locked elements as a fixed bottom
layer, so ink is never sent beneath a page image. `selectableElements` hands
the hit test the unlocked elements, returning the same array when none is
locked so a whiteboard's cached geometry is reused.

## Pages exist only in a notebook

A notebook has a top-level `paper`: fixed-size pages stacked down the board.

```jsonc
"paper": { "gap": 24, "pages": [ { "id": "p1", "width": 595, "height": 842, "template": "grid" } ] }
```

- **Layout.** A page's rectangle is derived from its place in `pages`, never
  stored: world units are points (A4 is 595 × 842), pages stack top to bottom
  separated by `gap` (24), each centred on x = 0. `pageRects` in `paper.ts` is
  the one layout. An element belongs to the page holding the centre of its
  drawn bounds (`elementPage` in `renderPaper.ts`, cached per paper and
  element); clipping, page operations and pictures all use that owner.
- **Fields.** `pages` has 1 to 500 entries with unique non-empty `id`s and a
  `width` and `height` above 0 and up to 14400. `template` is `blank` (the
  default), `grid`, `ruled` or `dotted`; `gap` is 0 to 1000. Other fields are
  kept. `paperProblem` and `validate_paper` in `authority/model.rs` apply the
  same rule, and the model fixtures hold them together.
- **A canvas ignores `paper`.** A `.canvas` that holds one keeps it: the model
  validates and saves it back as written, and the file is never rewritten for
  it. Nothing else reads it: no sheets, page camera, clipping, page tools,
  pictures, chat context or PDF export, so it opens and exports exactly like
  the same canvas without it. Every page behaviour asks `notebookPaper(document,
  notebook)` rather than reading `paper`.
- **Sharing.** A change to the pages travels in a transaction as
  `paper: { before, after }`, both whole papers, and merges like the
  background: it applies when the current paper equals either side, and
  otherwise fails with `conflict-background`. Two devices editing pages at once
  therefore conflict, and the draft is kept. A document with no paper never
  gains or loses one through a transaction, and a canvas never sends one.

### The pages bound the camera and the pen

- **Drawing.** `paintCanvas` paints each page as a white sheet with a soft
  shadow on the desk (the document's `background`, `NOTEBOOK_DESK` grey for a
  new notebook), then its template, then the elements, each clipped to its own
  page; a run of elements on one page shares one clip. The whiteboard's dot
  grid and its toggle are off. Templates are a line or dot every 14 points
  (ruled: 23, below a 60-point top margin); `templateMarks` is the one geometry
  for the screen, the SVG export (`paperSvg`) and the thumbnails, and a pattern
  finer than 4 screen pixels a step is skipped.
- **Camera.** A notebook opens fitted to its page width (up to 140%) once,
  while its camera is still the untouched default; Fit (1) and the page
  controls use `pageView`. Every camera write goes through `setView`, which
  keeps it on the pages (`clampCamera`): a page column narrower than the view is
  centred, a wider one pans until its edge is 24 px inside the view, and at
  most 40% of the view's height may show desk above the first page or below
  the last. Zoom runs from 60% of the fitted zoom to 800% (`zoomLimits`); wheel,
  pinch and buttons share it, and a whiteboard keeps 10% to 800%
  (`CANVAS_ZOOM_LIMITS`). A resize or new paper from elsewhere re-clamps.
  A touch or pen pan, one finger or two, released while moving glides on and
  decelerates like iOS scrolling (`momentum.ts`); it stops at the end of the
  pages, and the next touch, wheel or camera button stops it where it is (a
  finger that stops it holds the page rather than drawing). The camera is saved
  once, when it comes to rest. Mouse drags stop dead, and the trackpad's wheel
  brings its own momentum. Only a real flick glides: the release speed is read
  from the events' own timestamps over at least 30 ms (moves queued behind a
  slow frame arrive in a burst), the pan must have travelled 24 px, and a
  cancelled contact, often a palm the system rejected, never glides. Two
  fingers move the camera once a frame from both latest positions (iPadOS
  sends each finger in its own event, and a per-event update made the zoom
  wobble), their speed is read from the fingers' midpoint on screen, and two
  fingers that zoomed more than 10% over the gesture don't glide.
- **The pen.** A stroke, shape or text that starts on the desk is refused, and
  pen recovery does not restart it as the pen crosses onto a page. A move or
  nudge is shortened so the selection's centre stays on a page. A duplicate
  whose centre would land on the desk moves to the nearest page edge, and such
  a paste to the middle of the page in view. Images go onto the page under the drop, else the page in view, no
  larger than that page, flowing onto the next page when one fills
  (`placeNotebookImages`); a drop previews where they will land.
- **Hit testing** sees only what the page shows: the pointer reaches an
  element only on its own page (`elementsAtPage`), and a box selection needs
  the shown part of an element inside it (`shownBounds`).
- **Export.** PNG and SVG cover the pages alone, each element clipped to its
  page, and an area capture clips the same way. PDF (desktop only,
  `CanvasPlatform.exportPdf`) draws each page on white at 2.5 pixels a point
  and `canvas_export_pdf` writes one PDF page per notebook page at its size:
  the picture is stored as PNG's own compressed rows (a Flate stream with a
  PNG predictor), so nothing is recompressed.

### Page tools edit the paper as one undo step

The bar at the bottom shows the page in view with steps either side and
toggles the **Pages** sidebar: a column of thumbnails at each page's
proportions, the page in view marked. A click goes to a page. **Add page** at
the end of the list (`AddPageMenu`) offers a new page, at the last page's
template and size, or pages from a library PDF (`CanvasImageDialog` with
`pdfPages`); the companion has no library, so there it adds the page straight
away. Each thumbnail's ⋯ menu adds a page before or after, duplicates,
changes the template or size (A4 and Letter, portrait or landscape) and
deletes. Dragging a thumbnail reorders the pages on pointer
events, so a mouse lifts after a few pixels and a finger or Pencil after a
short hold, and the list scrolls near its edges (`usePageReorder`). On a
narrow canvas the sidebar lies over the drawing like a drawer, behind it a dim
scrim that closes it when pressed.

- Every operation in `pages.ts` moves elements with their page, keeping each
  one's offset from its page's top-centre. Removing a page takes everything on
  it and moves the pages below up; a notebook keeps at least one page.
  Duplicating clones the page's elements under new IDs. A smaller page size
  moves an element only as far as keeps its centre on the page. Inserted PDF
  pages are new pages at the PDF page's size with the picture as a locked
  backdrop.
- Each is one undo step through `editPages` in the hook, the same transaction
  path as a stroke, so a page added on the iPad appears on the Mac. A gesture
  or pinch in progress blocks it; a capture or crop in progress ends.
- Thumbnails are drawn with `renderPagePng` one page at a time once the
  document rests, only for pages whose `pageFingerprints` key changed, nearest
  the page in view first (`usePageThumbnails`).

### Creating a notebook

New file → Notebook… opens a dialog with two starts. **Blank paper** is one A4
page, portrait or landscape, in a template. **From a PDF** lists the subject's
PDFs and copies their pages: each is rendered as for any PDF import
(`canvas_open_library_pdf`, `canvas_render_pdf_page`) and laid on its own page
as a `locked` image the size of the PDF page, so the original is only read and
the notebook outlives it. A PDF over 100 pages, or whose pages come to over
64 MB, is refused with a sentence that says so (`readPdfPages`).

A PDF's own page has a **Draw** button (`DrawOnCopy`) with two choices, each
a copy named after the PDF and opened beside it. **Notebook** is the copy
above. **Canvas** is a `.canvas` with no paper holding the PDF pages as
ordinary, unlocked images at their size, stacked top to bottom with a gap like
an image import, to sketch around (`boardFromPages`).

### What a chat agent sees

An agent reads neither file: it is JSON holding every stroke and the
assignment's pictures. While a notebook is open, `useNotebookSnapshots` redraws
the pages that changed 1.5 seconds after the content stops changing and sends
only those to `canvas_write_page_snapshots`. A canvas gets none of this.

- **Change detection** is by page, from the elements that touch it
  (`pageFingerprints`), so a stroke on page 2 redraws page 2, and a pan, a
  zoom or a stroke elsewhere redraws nothing. Each picture shows its page as
  the screen does, clipped, without ink from the next page. Writes run one at a
  time and the last scheduled document wins; a failed write is shown in the
  editor and tried again on the next change.
- **Storage** is `canvas-pages/<file id>/` under the library: `page-NNN.png`
  per page (2 pixels a point, about 1190 × 1680 for A4) and `pages.json`, the
  notebook's path, page count and, for each page, its id, picture and the Unix
  time it was last redrawn (`library::paths::notebook`). It is keyed by the
  `files` row, so a rename or a Drive move only rewrites the manifest's path
  (`snapshots::retarget`), and it is removed when the notebook is deleted
  (also by Drive's delete) or reconciled away. The command refuses a path that
  is not a notebook's, a page the notebook does not have, anything but a PNG,
  and a symlinked folder.
- **`oculus read <notebook>`** prints each page's picture path and how long
  ago it was redrawn; `--pages` picks pages. A notebook never opened since it
  was made has no pictures and the command says so; a canvas is not text and
  says so. The agent opens the PNG with its own image tool, and a chat message
  names the open notebooks and the page in view
  ([harness.md](./harness.md#a-notebook-the-student-has-open-rides-the-prompt)).

## The pointer stays local and Rust answers geometry queries

The WebView owns input and Canvas2D drawing. Gesture previews live in refs
and an overrides map; a requestAnimationFrame coalesces repaint requests.
The committed elements array changes at a gesture boundary. A cached Path2D
per immutable element avoids rebuilding its path during panning.

The Rust core is compiled to WebAssembly and loaded with the editor. A
retained BVH indexes the scene for hit testing and viewport culling. Geometry
is packed into numeric buffers once per committed scene, with scalar
coordinates for subsequent queries. There is no Tauri round trip for a pointer event. Rust scene storage
is capped at eight scene arrays independently of undo history.

Each shape is a record of 14 floats (`engine.ts`, documented on
`CanvasScene::new` in `app/canvas-core/src/lib.rs`): kind, x, y, width, height,
strokeWidth, filled, point offset, point count, `heads`, startAngle, startSize,
endAngle, endSize. `heads` is a bitfield: 1 for a head at the first point, 2 at
the last, so Rust tests a head only at the ends that draw one. An arrow’s
points are its flattened rendered path, and each head’s angle and size are
computed in JavaScript (the end tangent of a curve, one size from the whole
path), so Rust has no curve code. `filled` is true for a hollow shape that has a label,
which makes its inside selectable. Changing the record layout needs both
sides and the parity test.

The JavaScript geometry functions are the tested reference and serve while
the WASM module loads. A load failure shows an error and leaves the reference
implementation available. Tests compare queries against the actual compiled
WASM, including negative coordinates, transparent shapes, labelled shapes and
start and end arrowheads.

`bun canvas-core/bench.ts` measures geometry queries separately from rendering.
Its synthetic 10,000-shape grid gives approximately 6 ms for the first scene
transfer/index build, 0.45 ms for 2,000 retained hit queries and 0.86 ms for
500 viewport queries on the development machine. These are geometry timings,
not a guarantee of frame rate or pointer latency.

## One save session owns every pane editing a file

Sessions are keyed by database file ID. Every mounted pane receives the same
snapshot, and one queue orders writes, rename and delete. Updates during an
in-flight write are drained before Saved is published, and an update that lands
as a save finishes is saved by that same flush. The debounce batches completed
changes rather than sending every pointer point to disk.

A recovery record under `recovery/canvases/<file-id>.json` in the library
records the document and its base disk text. Native commands write it atomically
without the browser storage quota. Existing localStorage drafts migrate only
after a successful disk write. Recovery operations are ordered separately from
saves; a late recovery write cannot recreate a draft cleared after saving.
It is validated before restore. When the disk has changed independently,
the editor offers Use recovery copy or Keep disk version; recovery never
silently overwrites a newer file. Invalid recovery stays recoverable until
explicitly discarded. Save errors keep the draft and show Retry save. A
recovery-storage failure is a separate notice over the canvas: it neither
marks a successful document save as failed nor resizes the drawing area.

Rename flushes pending edits before moving the file. Unmount and pagehide
flush the session; a clean session is released after its final pane unmounts,
so reopening reads disk again. Delete runs through the same queue and marks
the session deleted, preventing a late autosave from recreating the file.

Native writes stage a sibling temporary file and rename it into place after
fsync. File commands accept only a four-part subject canvas or notebook path
and reject symlinks in its components. Canvas paths use their own guard; note mutation
commands remain scoped to notes.

## A shared canvas has one writer

While a canvas is shared, the canvas authority
(`library/files/canvases/authority/`) is the only writer of its file. Desktop
panes and paired devices send content transactions (`collaboration.ts`:
per-element `before`/`after`, an optional layer order, background and a notebook's paper); the
viewport never travels in one. An added element (`before: null`) goes on top
unless the edit names `afterId`, the element just below it in the sender's
scene (`null` for the bottom); it is then inserted above that element, or on
top if that element is gone. `diffCanvasContent` sets `afterId` only on an
addition with an older element above it, such as an undone delete, so
additions on top from two devices both land, in arrival order. `afterId` on an
edit that is not an addition, or naming the edit's own ID, is invalid. A session is keyed by the file's database ID
and starts with `canvas_authority_open`, given the pane's flushed text, which
must equal the file on disk. The native registry fences ordinary `write_canvas`,
`rename_canvas` and `delete_canvas` commands against a session owning the same
path, including a paused session. Opening and whole-file mutations take the
same registry lock, so a delayed desktop autosave cannot overwrite shared
changes between the ownership check and the write. Drive's move, rename and
delete take the same lock for their disk step (`while_unshared`) and refuse a
folder holding a shared canvas. One path cannot have two
sessions under different file IDs. Outside a session, `write_canvas` saves as
above.

- One lock per session. A commit validates the transaction, applies it to the
  canonical document, writes the file, bumps the revision and then broadcasts
  `{fileId, epoch, revision, clientId, transaction}`: the Tauri event
  `canvas-authority-change` to the main webview, and the in-process
  `Subscribers` list a network transport reads. A conflict, invalid input or
  failed write changes nothing.
- Preconditions compare only the touched objects, by value: numbers as numbers
  (`1` equals `1.0`), object keys in any order, arrays in order. An edit already
  in place is skipped. Unknown fields at every level survive. The broadcast
  transaction leaves out the parts that were already in place and keeps each
  applied addition's `afterId`.
- Each op carries a `clientId`, an increasing `seq` and an `opId`. The
  authority keeps each client's last 64 receipts: a retried op gets its
  original revision back without being applied again (a replayed ack has no
  transaction), and an older `seq` is `stale-op`, so the client resyncs and
  keeps its draft. The epoch is a random ID per session; an op naming another
  epoch is `session-ended`.
- Before every overwrite the authority compares the file's SHA-256 with the
  bytes it last read or wrote. On a mismatch it writes nothing: the session
  pauses (`external-edit`, and a `canvas-authority-state` event), keeping the
  outside version on disk and its own document in memory until
  `canvas_authority_resolve_external` adopts the disk version (broadcast as a
  change with no client) or keeps its own. Another process can still write
  between the check and the rename: this guards cooperating editors, it is not
  a cross-process compare-and-swap.
- Rename moves the file within the session. Delete removes the file and ends
  the session; a later commit is `session-ended` and never recreates the file.
  A file that vanishes underneath ends it too. Close and app exit end the epoch.
- A desktop commit can carry a viewport hint. It is saved with the next write
  or on close, never bumps the revision and is never broadcast.
- After each write a background thread updates the `files` row (size and
  modified/seen times, the path, or the row on delete), as `touchFileRow`,
  `renameFileRow` and `deleteFileRow` do. A failed row update is only logged:
  the file is the truth.

Errors are `{code, message}` with `code` one of `conflict-shape`,
`conflict-order`, `conflict-background`, `invalid`, `stale-op`,
`session-ended`, `too-large` (over 256 MiB serialized), `external-edit` or
`io`.

The Rust validator and transaction code port `model.ts`, `imageSource.ts` and
`collaboration.ts`. `app/tests/fixtures/canvas-model/` and `canvas-collab/`
hold the cases both test suites run, so the two cannot drift. The authority
writes the bytes `serializeCanvasDocument` writes: serde_json keeps key order
(`preserve_order`) and parses floats exactly (`float_roundtrip`), and
`authority/format.rs` prints numbers by JavaScript's rules. serde_json refuses
two things `JSON.parse` reads, a lone surrogate escape and nesting deeper than
128 levels; such a canvas cannot be shared.

## Sharing turns the session's saves into commits

Loading a desktop canvas first probes `canvas_authority_snapshot`, with its
event listener and whole-file fence already installed. If a native session
survived a webview reload, the desktop joins its canonical snapshot and replays
changes buffered during the probe. This happens without opening Share, and
does not write the disk file. Only `session-ended` means no authority owns the
canvas and ordinary file loading and saves may resume; other probe failures
surface as load errors. Recovery drafts still require the usual explicit
choice. Refreshing an existing share also rejoins a detached desktop session.

`attachAuthority()` runs on the session's queue: it finishes the whole-file
flush, then calls `canvas_authority_open` with that flushed text and switches
the session to commit mode. From the moment the open is sent until the file is
reread after sharing, the session is fenced: its whole-file write throws, a
save queued in that window writes nothing, and the step holding the fence saves
when it finishes. Edits made
while the open is awaited stay in the snapshot and are committed after it.
`detachAuthority()` commits what it can, closes the authority and rereads the
file; `snapshot().authority` (`off`, `attaching`, `live`, `paused`, `ended`)
says where sharing stands. An attached session stays registered after its last
pane unmounts, until it is detached.

In commit mode a save diffs the session's canonical document (the authority's
document at its last known revision) against the panes' document
(`diffCanvasContent`) and commits that transaction under the session's
`clientId`, an increasing `seq` and a fresh `opId`; a viewport that differs from
the last one sent travels with it, or alone with no edits. One op is in flight.
An ack whose revision follows the canonical one applies its transaction; one
already covered changes nothing; any other (a replay, or a gap) refetches
`canvas_authority_snapshot`. A failure with no code may not have reached the
authority, so the same op is resent unchanged on retry and a replay answers it.
A refused op frees its `seq`.

`canvas-authority-change` events, our own included, advance the canonical
document in revision order. Each advance redraws the panes as the canonical
document with this desktop's unsent edits rebased on top
(`reconcileClientDocument`) and this desktop's viewport, so a merge never
resets a gesture. The sent operation and edits made after it are separate layers.
Confirming our own operation removes only that first layer: a quick undo or a
second move of the same shape stays on top and becomes the next commit. Events
identify the sending client, and the operation stays tracked until its reply and
revision are reconciled. Someone else's change past a gap refetches the snapshot,
which is rebased under the unsent edits the same way.

- **Conflict.** A `conflict-*` refusal, or a remote change that touches a shape
  with an unsent edit, keeps the draft (and its recovery copy), stops commits
  and makes the surface inert. The editor offers Use shared canvas (drop the
  unsent change and show the canonical document) and Save draft as copy (a new
  canvas in the same subject holding the draft).
- **Paused.** `external-edit` pauses commits; edits keep collecting. Use file
  on disk and Keep my version call `canvas_authority_resolve_external`
  (`adopt-disk`, `keep-ours`) and then commit the unsent edits.
- **Stale op.** The session takes a new `clientId`, refetches the snapshot and
  commits again.
- **Ended.** An `ended` state, or `session-ended`, puts the session back on the
  whole-file path after rereading the file. Unsent edits become the Use
  recovery copy / Keep disk version choice; a recovery copy made while shared
  never matches the disk text, so after a restart it is offered, never applied.
- **Rename and delete** go through `canvas_authority_rename` and
  `canvas_authority_delete`. The authority updates the `files` row on a
  background thread; the wrappers in `files.ts` update it too and announce
  `CANVASES_CHANGED_EVENT`, so Drive's reload never reads the old row.

The status line reads Sharing, Sharing paused or Sharing ended before the save
state.

## Tools and exports share the same geometry

The editor supports selection, marquee, move and resize; rectangle, ellipse,
diamond, line, arrow, freehand and text tools; an eraser; undo/redo;
duplicate and copy/paste; layer order; grid, pan, zoom and fit. A single
selected line or arrow edits its points instead of resizing
([below](#arrows-are-sharp-curved-or-elbow-paths)), so it has no dashed
selection frame or corner handles, only its point handles (`selectionFrame`
in `render.ts`); any other selection, including several lines, has the frame.

A selection box includes an element only when its entire bounds fit inside,
in either drag direction. While it is dragged, each element its release would
add is outlined in a lighter selection colour; the preview and the release ask
the same `boxSelection` (`boxSelect.ts`), over bounds cached per element, so
they cannot disagree. Dragging on an unselected image or PDF page starts
a selection box for its annotations. Clicking the page selects it; dragging
an already selected page moves it. Shift adds enclosed elements to the current
selection. Dragging anywhere inside the current dashed selection frame moves
the selected elements together, including the empty gaps between strokes and
areas over an unselected page. A tap inside keeps the selection; Shift keeps
its element-toggle and add-to-selection behaviour.

Corner handles scale a selection proportionally, including every image in a
group. The four edge handles stretch only width or height; Shift on an edge
scales proportionally around the opposite edge's midpoint. Single lines and
arrows keep their point-editing handles.

The canvas root scopes shortcuts and participates in native Edit menu
routing. A visible note editor must not claim undo while a canvas owns focus.
Text inputs keep their own native editing behavior. Pressing the drawing takes
focus and clears retained DOM selections, so WebKit cannot send clipboard
commands to a chat's old contenteditable caret. Drawing gestures capture
the active pointer and suppress WebKit text selection: `installBrowserSelectionGuard`
on the canvas root cancels `selectstart` and `contextmenu` outside text fields and
native touch defaults on the drawing element. In the companion these touch
listeners attach to the page body, with a target check for this surface's canvas,
so touch tracking covers a stable ancestor across canvas repaints. Toolbar taps
and other surfaces keep their defaults. The desktop listeners stay on the editor
root. The companion opts into
`suppressNativeGestures` on `CanvasSurface`: Safari's `gesturestart`,
`gesturechange` and `gestureend` defaults are cancelled there so native
pinch/rotation does not compete with the Pointer Events camera. The desktop
leaves those events alone for WebKit trackpad zoom. This suppresses
browser tap gestures while Pointer Events keep driving drawing,
pan and pinch. Toolbar taps and text fields keep their native defaults. The
canvas and editor also declare `touch-action: none`; the canvas is not draggable.

PNG and SVG export cover the whole drawing with padding, excluding editor
chrome, selection, binding hints and grid. The surface saves an export and
opens a PDF for import through `useCanvasPlatform()`
(`lib/canvas/platform/`), so its own imports never reach Tauri. The default is
a plain browser's: an export downloads, and Choose PDF says to open Oculus
desktop while image files still import. The desktop root (`main.tsx`) provides
`tauriCanvasPlatform`, whose export command opens a native Save dialog and
writes only the path selected there. Its library search uses
`searchLibraryFiles` with a PDF type filter; its PDF opener is `importPdf.ts`. SVG text and attribute
values are escaped. PNG export caps its dimensions to bound bitmap allocation.
Canvas2D, SVG and hit testing all take dash lengths (`strokeDash`) and
arrowhead outlines (`elementArrowheads`) from `geometry.ts`.

## Pen, touch and mouse share one gesture

One gesture runs at a time, owned by the pointer that started it; other
pointers' moves, ups and cancels are ignored, so a palm resting on the screen
neither cancels nor redirects a Pencil stroke.

- A pen contact ends any finger gesture and claims the stroke. After the first
  pen contact, one finger pans instead of drawing (Safari cannot say whether a
  Pencil is paired); before it, a finger draws.
- A second finger turns the first finger's gesture into navigation without
  committing its preview: two fingers pan and pinch (`TouchViewportGesture`)
  until every finger has lifted, whatever the tool. The Text tool opens its
  overlay on a finger's release, not its press, so the second finger can still
  arrive.
- Selection drags wait for 4 screen pixels of mouse movement or 10 with a pen or finger. Small Pencil movements select the image beneath them without moving it or starting a box selection; ink keeps its own sampling.
- The release is the stroke's last sample, so a down and up with no move
  between still draws, and a freedraw tap commits a dot. Coalesced pen samples
  stay in the committed stroke. Committing keeps the preview’s points unchanged,
  so lifting the pointer preserves the curve already drawn.
- A pen move with its contact button down and no gesture starts the stroke
  that WebKit's lost pointerdown should have; hover never inks. A fresh pen down
  while a pen stroke is open commits that stroke's ink as its own stroke rather
  than dropping it or joining the two.
- Pointer capture can throw when the pointer is already gone; the gesture
  continues without it.

## The eraser takes whole elements or cuts strokes

With the eraser chosen, the style bar holds its settings, kept for this device
only in localStorage (`toolPreferences.ts`; a missing or unreadable field falls
back to its default):

- **Mode.** Whole stroke (the default) removes the topmost element within 10
  screen pixels of each pointer sample. Partial cuts freedraw strokes: every
  part within the eraser's radius plus half the stroke's width of the segment
  between consecutive samples, coalesced ones included, goes, so a fast sweep
  skips nothing. A stroke is subdivided to a quarter of the radius first, so a
  cut lands that close to the eraser's edge (`erase.ts`). Shapes, lines, arrows
  and text the sweep touches go whole.
- **Size**, shown only in Partial: a radius of 6, 12 (the default) or 24 screen
  pixels. A thin ring of that size follows the pointer while erasing and while
  a mouse or Pencil hovers.
- **Return after erasing**, off by default, switches back to the tool chosen
  before the eraser when an erase gesture ends; a cancelled erase or a pinch
  that takes over does not.

A cut stroke keeps its ID for its first remaining piece; each further piece is
a new freedraw element directly above it in layer order (`applyElementChanges`
with stacked additions, sent with `afterId`). A piece is normalised as a new
stroke is, x/y at its first point and `width`/`height` to its last, and holds
only the stroke's own points plus the two at each cut. A piece shorter than 2
screen pixels or half the stroke's width is dropped, and a stroke with nothing
left is deleted. Images and locked elements are never erased.

Nothing disappears mid-gesture. What the erase will remove is painted as a
ghost at a quarter of its opacity (`ghosts` in `paintCanvas`), a cut stroke's
kept pieces as they will be, and hit testing passes over what is ghosted. The
release commits the whole gesture as one undo step through `commitChanges`, so
it reaches a shared canvas like any edit; Escape, a cancelled pointer or a
second finger restores everything.

## Undo is per pane and touches only its own edits

Each committed edit records the content transaction from the scene before it
to the scene after it, and its inverse (`diffCanvasContent`). Undo and redo
apply that transaction to the current scene (`applyCanvasTransaction`), so a
shape someone else added since stays. If they changed a shape the step would
restore, the step is refused and dropped, and the surface shows why inline;
it never overwrites or removes their work. Undo leaves the camera alone, waits
while a gesture or a pinch is in progress, and keeps 100 steps. A step that re-adds deleted elements puts each back above the
element it followed: the transaction carries `afterId`, and the step also
remembers the layer order.

## A document from outside the pane is a new base, not a reset

The hook ignores a document it handed to `onChange` itself, including an echo
with a fresh identity but the same content. Any other document (another pane on
the same session, a collaborator's merged edit, a recovery copy) replaces the
scene but keeps this pane's camera, the gesture in progress and its preview,
the undo history, the text being written and the selection of shapes that still
exist. A preview of a shape someone removed is dropped, and a point or segment
drag ends when its line is removed.

The camera belongs to the pane. Without `onViewportChange`, pan and zoom are
saved in the document (a pan when it ends, wheel and buttons after 180 ms) as
the file's initial view. With it, they go only to that callback and never
through `onChange`, for a host that keeps its camera local. The desktop editor
and the companion both pass one, so scrolling never marks a canvas unsaved; the
camera reaches the file only inside the next real edit's document.

A reload or reopen resumes on the same device (`viewState.ts`, localStorage):
the last tool (Crop comes back as Select) and drawing style, and the camera of
each of the 60 most recently viewed drawings, keyed by `cameraKey` — the
file's id on the desktop, the share link on the companion. The camera is
written 400 ms after it stops moving and on `pagehide`, and comes back at the
first real size in place of a notebook's first fit, then clamped like any
other camera. Damaged fields fall back to the defaults one by one.

## Images and PDF pages are saved inside the canvas

Add image opens a modal for local PNG, JPEG, WebP or GIF images, or PDFs
already in the Oculus library. Choose PDF opens an in-app search by filename
and subject code. The empty search lists library PDFs with current subjects
and recently opened files first. The database filters to PDFs before limiting
results; parsing is not required. Selecting a result opens its page thumbnails
with a selection that survives moving between page groups. Import adds the selected pages in document order as
independent image elements. A batch is one undo step, stacked vertically with the first page centred in
the current view, and selected for moving or resizing.

Dropping images onto the drawing centres the first image at the pointer's
world position and stacks a batch below it as one undo step. Finder drags use
the desktop platform's webview listener and bounded raster reader; decoded
images preview their exact placement before release. Browser file drags show
a placement marker until file bytes become available on drop. Embedded or
local-asset images dragged from chat use the same decoder. Pasted raster images
are inserted at the view centre; canvas clipboard envelopes keep their cloning
and binding rules. Text fields keep their own paste behavior. Async reads are
cancelled on replacement or unmount, and errors leave the drawing intact.

An image element stores a base64 raster `dataUrl`, its source name and natural
dimensions alongside its world bounds. The `.canvas` stays self-contained:
reopening, copying and exporting do not depend on the original source file.
Remote URLs and SVG input are rejected. Images use their rectangular bounds
for selection and culling in both the JavaScript reference and Rust core.
The style bar offers opacity, layer controls and Crop image for a single image. The eraser skips
images, including imported PDF pages, and erases shapes and ink through them.
To delete an image, select it and use the style bar's trash button or Delete;
that deletion is one undo step.

### Capture duplicates an area; cropping trims an image

**Capture area** (S) drags a rectangle anywhere on the canvas. The resulting
PNG includes the background, a notebook's pages, images and annotations inside that area,
clipped at its edges, without tool overlays or the whiteboard grid. The area
stays selected with a dashed border and eight handles; the rest of the viewport
dims while the captured pixels stay clear. Drag its edges or
corners to adjust it, or drag inside to move the selection. Clicking outside
starts a new selection; an outside tap or Escape dismisses it. **Copy**, a
small icon action on an opaque, blurred card beside the area, or Cmd/Ctrl+C
writes a PNG to the system
clipboard where the browser supports image writes. Paste inserts that image
through the ordinary image-paste path. Clipboard encoding starts as a promise during the button click so Safari keeps user
activation while images decode. LAN HTTP pages cannot use the system image
clipboard; their error points to Duplicate for copying within the canvas.

**Duplicate** beside Copy adds the captured pixels as a selected image,
offset 24 units and ready to move. It works on a LAN HTTP page without system
clipboard access. A pointer held inside the selection for 1.2 seconds shows a
filling progress ring, then lifts a shadowed ghost with a Drag to place copy
indicator before any movement. Mouse, Pencil and touch all use this behavior.
Dragging the lifted ghost and releasing places one image; releasing without
dragging cancels the lift. Moving before the hold completes moves the capture
zone and cancels the timer, even if the pointer pauses later. Handles resize
without arming a hold; short selections keep a smaller edge hit band so their
interior remains movable. Escape, pointer cancellation and two-finger
navigation discard a pending or lifted copy.

**Crop image** opens from the style bar's crop button, on iPad too, or by
double-clicking an image on desktop. The full image
starts framed by eight visible handles: corners crop both axes freely, edges
crop one, and dragging inside repositions the crop rectangle. The discarded
area dims and the kept area shows thirds. Handles stay within the original
image and have larger hit targets for Pencil and touch. Enter applies the
crop; an outside click or tap also confirms it, and Escape discards its preview.
Dragging outside pans instead of confirming, and a second finger starts
navigation without applying the crop. A Pencil or touch handle release applies
the crop immediately; Undo restores the original image. Applying replaces only
the image's raster and bounds, preserving its ID and opacity; annotations stay
separate. There is no crop action panel. A locked backdrop cannot be
cropped.

Captures render at two pixels per world unit; crops use the source's pixel
density and preserve transparency. Both are capped at 8192 pixels a side and
32 million pixels. Rendering waits for decoded images and fonts. Each dropped
capture copy or applied crop is one undo step and uses the same shared content
transactions as other edits. Cancel and Escape discard the preview; a late
encoding result cannot commit after cancellation or a changed rectangle. A
crop also refuses to overwrite an image changed or removed by another device.

PDF imports use the native viewer's renderer without running the document
through the parse pipeline. A scoped library path opens a session owned by
the calling webview; subsequent render requests use that session ID rather
than accepting arbitrary filesystem paths. The
native commands take `tauri::Webview`, since browser child webviews
share the desktop window. Closing or changing the modal source releases the
session. PDF raster dimensions are bounded and share the viewer's render
memory budget. Only selected pages are rendered at
import resolution; previews render a bounded page group at thumbnail size.

Decoded raster images share a cache across element edits and copies. Decode
completion requests a repaint, and PNG export waits for all images to load.
SVG export embeds the same raster data in image elements.

## A shape holds its label as text

Rectangle, ellipse and diamond carry a label in `text`. It is written from
four places, all opening the same overlay textarea on the label box:
double-click a shape (including the inside of a hollow one), click a shape
with the Text tool, Enter with one shape selected, and Enter or double-click
on standalone text. The shape stays selected while its label is edited so the
inspector shows its text options. Escape, Cmd/Ctrl+Enter or leaving the text
and inspector commits. Inspector fields and portaled pickers keep the draft
open, including empty text, so formatting can be chosen before typing.
An empty label removes `text` and keeps the shape; empty standalone
text is removed.

The label box is the shape’s bounds minus 8 units of padding, inset to the
inscribed box for an ellipse (0.707 of each side) and a diamond (0.5). Words
wrap greedily and an over-long word breaks between characters
(`renderText.ts`). The grow rule: when the wrapped label is taller than its
box, the shape grows downward on commit and when the font size or family
changes; it never shrinks to fit. Resizing a labelled shape rewraps the label
and keeps its font size (standalone text scales its font).

Fonts are the `CANVAS_FONT_STACKS` in `renderText.ts`. Sans and mono repeat
`--font-sans` and `--font-mono` from `app/src/styles/theme.css` because
Canvas2D and SVG cannot read CSS variables; edit both together. The overlay
textarea, Canvas2D, SVG and the width measurer use the same stacks.

## The inspector shows what applies to the selection

`style.ts` is the single table of which style field applies to which element
type, and `inspectorSections` derives the style bar from it: the union of
sections that apply to any selected element (or to the active tool’s next
element when nothing is selected, or to the element being written in).
Sections are Color or Stroke, Fill, Font and Align, Stroke width, Stroke
style, Sketch, Edges, Arrow type, Arrowheads and Opacity, in a fixed order,
then Layer whenever something is selected. A shape’s text
options appear only once it has a label or one is being written. Each control
reads its value from the first element that shows it.

A style patch writes only the fields that apply to each selected element and
change what it draws; a patch that changes nothing commits nothing and makes
no undo step (`patchElementStyle`, `stylePatchChanges`). A font size or family
change re-measures standalone text and re-runs the grow rule on labelled
shapes, and an arrow type change reshapes the arrow (`restyleElement`,
`retypeArrow`). A patch also becomes the default for the next
element of those kinds, and selecting an element adopts its look as the
default. Opacity previews live while dragging and commits one undo step on
release.

The inspector is `CanvasStyleBar.tsx`, a slim bar centred under the tool bar
that shows only when sections apply or the eraser is chosen, so nothing covers
the drawing at the side. With the eraser it holds the eraser's settings
(`inspector/EraserControls.tsx`, [above](#the-eraser-takes-whole-elements-or-cuts-strokes)). Left to right, divided into groups: the stroke colour, Fill as one
bucket-and-swatch button, the three stroke widths, and the font size (S, M, L,
XL and an exact-size field). The other sections (font family and alignment,
stroke style, sketch, edges, arrow type, arrowheads, opacity) sit in one
column behind the More styles button (`inspector/MoreStyles.tsx`). With a
selection the bar ends with its actions (`inspector/SelectionActions.tsx`):
Crop image for a lone image, Layer, which opens the layer moves, and Delete.
Every popover opens below the bar. `canvas.css` places the bar by
`--canvas-top-edge`, the same edge as the tool bar, which the companion
lowers below the iPad's status bar; on panes 760px wide or less it drops the
exact-size field, and at 470px or less the five colour picks, keeping the
current-colour button. A pane still too narrow scrolls the bar sideways with
faded edges. Error notices sit below both bars.

The bar and the other floating bars (tools, zoom, history, pages, utilities)
share `.canvas-chrome` in `canvas.css`: a translucent, blurred card that turns
nearly opaque under the pointer or keyboard focus. One file per control lives
in `app/src/components/canvas/inspector/`. The colour shows five top picks and
a button for the current colour (`ColorRow`); that button and Fill open
`ColorPalette` (`app/src/components/ui/color/`) on the canvas palette in
`inspector/palette.ts`: a grid of hues, the shades of the hue the colour
belongs to (`colorShades`), the hex code and the full picker behind Custom.
Strokes pick a hue's base and fills its palest shade. Edges is a Sharp/Round
pair and Arrow type a Sharp/Curved/Elbow trio. Each arrow end is a button
showing its head that opens a grid of the six types. These popovers focus
themselves rather than their first option, leave focus in an open text
overlay, and keep their keys from the canvas's shortcuts
(`inspector/popoverGuards.ts`). Arrowhead glyphs are drawn by
`arrowheadShape`, so the picker previews exactly what the arrow gets. A press
on the bar or its popovers outside a field (`keepTextFocus`) keeps focus in an
open text overlay, so a style change restyles the text instead of committing
it.

## Layer order is array order

Elements paint in `elements` order, last on top, and hit testing picks the
topmost, so reordering the array is the whole model change. `layers.ts` moves
the selection: to the front or back of the stack, or forward or backward past
the nearest unselected element that overlaps it (bounds intersect), stepping
one place when nothing overlaps. A multiple selection keeps its order among
itself, so an element stops beneath (or above) the next selected one. A move
that changes nothing returns the input array; `canReorder` disables those
buttons.

The style bar's Layer popover and the shortcuts run the same `reorder` in the
hook, which publishes the reordered document as one undo step. It cannot go
through `commitChanges`, because `applyElementChanges` keeps order. ⌘] and ⌘[
step forward and backward, ⌘⇧] and ⌘⇧[ go to the front and back. The View
menu's Back and Forward own ⌘[ and ⌘], so the canvas claims them through
`onBackForward` (`app/src/lib/menu/backForwardRouting.ts`) while it has focus
and a selection; with nothing selected they step the pane's history as
elsewhere. The ⇧ variants reach the canvas's own keydown.

## Arrow ends bind to shapes and follow them

Rectangle, ellipse, diamond and text can be bound to (`outline.ts`; text binds
by its bounds rectangle). An end binds while an arrow is drawn and while an
endpoint handle is dragged (`bindings.ts`):

- The topmost bindable shape whose bounds, expanded by 16 screen pixels,
  contain the pointer is the target; the arrow itself is never one.
- In the central zone (the larger of 10 screen pixels and 20% of the smaller
  side) the end binds to the centre. Elsewhere it binds to the nearest
  outline point, stored as a 0..1 anchor in the target’s bounds so it
  survives moves and resizes, and snaps to a top, right, bottom or left
  midpoint within 12 screen pixels.
- An end may not bind by its centre to the shape the other end already binds
  to by its centre.
- Holding Cmd/Ctrl places the end freely and drops its binding.
- While an arrow tool hovers or drags, the target’s outline, its snap points
  and the active one are painted (`bindingHint`). It is never exported.

`resolveArrow` places a bound end: an edge binding at its anchor’s outline
point, a centre binding where the line to the other end crosses the outline,
each pulled back toward the other end by `4 + strokeWidth / 2`. Both raw
targets are found first so two bound ends aim at each other. A centre-bound end
whose other end is inside the shape stays at the centre. A sharp or curved arrow
keeps its interior points and aims each end at its neighbouring point; an elbow
arrow is routed afresh (below). `bindArrowEnd` re-routes an elbow arrow whether
or not the end bound, because `followBindings` only re-places bound arrows. An
elbow end pressed well inside a shape (beyond the 16-pixel edge range) binds by
centre, so it leaves by the side facing the other end rather than the nearest edge.

Binding an elbow end to a different shape drops the arrow’s fixed segments
([below](#elbow-segments-can-be-dragged-and-stay-where-they-are-put)).

`followBindings(elements, changes)` returns the changes plus every bound arrow
they affect, re-placed. A change that removes a target drops the binding on
that end and leaves the arrow where it is. Moving an arrow (drag or nudge)
without its target in the selection drops that end’s binding (`moveElements`).
Duplicate and paste remap bindings to elements cloned in the same batch and
drop the rest (`cloneCanvasElements`).

Every element edit goes through the hook’s two funnels: `previewChanges` for
gesture previews (into the overrides map) and `commitChanges` for committed
edits, both of which run `followBindings`. This covers move, resize, nudge,
label grow, font change, delete and erase.

## Arrows are sharp, curved or elbow paths

An arrow holds any number of `points`; `arrowType` decides how they are drawn.
`linePath` (`arrow/path.ts`) is the one rendered path, cached per element:
`points`, the `curves` through them, and a `polyline` flattened to 0.25 units.
Canvas2D and SVG draw the curves; bounds, hit testing, culling and the WASM
scene use the polyline. A curved arrow of three or more points is a uniform
Catmull-Rom spline (tension 1/6, as roughjs draws; `arrow/curve.ts`), which is
affine in its points, so resizing scales the points and the curve follows
(`resize.ts`). Two points are always straight.

A single selected line or arrow shows point handles (`arrow/handles.ts`): both
ends always; a sharp or curved arrow also every interior point and a midpoint on
each segment, an elbow arrow a segment handle on each segment, in both cases
only on segments at least 28 screen pixels long. Dragging a midpoint inserts a
point, double-clicking an interior point removes it, and Shift snaps a segment
to 45 degrees. All edits go through `arrow/points.ts`, which keeps x/y at the
first point so bindings place ends the same way. Elbow segment handles are
[below](#elbow-segments-can-be-dragged-and-stay-where-they-are-put).

Changing the type reshapes the arrow (`arrow/retype.ts`): becoming elbow routes
it; leaving elbow keeps only its two ends, since its bends were routed, not
placed; sharp and curved share their points. Any type change drops fixed
segments.

## Elbow arrows route around their targets

`routeElbow` (`elbow/index.ts`) gives a right-angled route and returns the arrow
itself when nothing changes, so a re-route that moves nothing is no edit.

- `ends.ts` places each end. A bound end sits on a side of its target, pulled
  out along that side’s normal by `4 + strokeWidth / 2`: an edge binding on the
  side its anchor is on, a centre binding at the midpoint of the side facing the
  other end (the next side clockwise when the other end is an edge binding on
  the same shape). A free end keeps its point and heads along the dominant axis
  toward the other end.
- `grid.ts` pads each target box by 40 units, except that the sides facing each
  other across a gap reach halfway, so the padded boxes meet on the midline and
  never overlap. Each end’s dongle is where its stub leaves the padded box. The
  grid is sparse: box edges, dongles and the midline between the bodies.
- `astar.ts` searches the grid for the route with the fewest bends, then the
  shortest length, without entering a padded box, leaving and arriving along the
  end headings. `route.ts` simplifies it to corners.
- With no grid route, two bound ends (overlapping targets) are joined without
  avoiding the boxes; otherwise, or for a free end inside the other target, a
  plain L or Z through the midpoint.

## Elbow segments can be dragged and stay where they are put

Dragging an elbow arrow’s segment handle moves that segment perpendicular to
itself, live, as one undo step (the hook’s `segment` gesture,
`elbow/segments.ts`). Its neighbours stretch to meet it and nothing else
moves. The first or last segment cannot take a bound end with it: a stub of 40
units (half the segment when it is shorter than 45) keeps leaving or entering
the target along the segment’s direction, and the route turns onto the moved
segment after it, as Excalidraw does. A free end turns straight onto it.

A dragged segment is stored as a fixed segment, which pins the line it runs on,
not its ends (`elbow/lanes.ts` calls a fixed line in world coordinates a lane).
Its extent and position in the points come from its neighbours on every route,
so there is no index to keep in step with the points. Lanes are local to x/y:
moving, nudging, pasting and duplicating carry them along, a resize scales
them, and `throughPoints` rebases them when x/y moves under points that stay.

`routeElbow` keeps an arrow with fixed segments as it is while its points still
fit its ends (`elbow/fits.ts`): they start and end there, are orthogonal, leave
and enter a bound target along its heading, and run along every lane. Once an
end moves (a target moved, resized or rebound at another anchor, an endpoint
dragged) or a drag breaks a heading (a segment dragged past an end), it routes
afresh:

- The grid gains each lane’s line and a frame 40 units outside everything, and
  A* (`astar.ts`) must run along the lanes in order. A lane may be crossed
  instead at a cost of 2.5 bends, so when the ends come level across a lane the
  segment shrinks away rather than becoming a detour, and comes back when they
  separate.
- A padded box is drawn in to a lane between the body and its padding
  (`clipToLanes`), so the route can take a line the user placed close to a
  target. A stub that already runs along the first or last lane counts.
- A free end may turn at once, since its heading was only a guess.
- A centre-bound end tries all four sides and the cheapest route wins
  (`elbow/sides.ts`): bends, then crossed lanes and any target body crossed,
  then length; the facing side wins ties. A fixed line on the far side of a
  target would otherwise force a loop.
- With no route clear of the targets the lanes still hold; with no route at
  all, they are ignored for that route.

Fixed segments are dropped when the arrow type changes, when an end binds to a
different shape than before (unbinding keeps them), and one by one by
double-clicking a fixed segment’s handle, which routes the arrow afresh along
the rest (`releaseSegment`). Double-clicking a segment that is not fixed does
nothing.

## Corners are sharp or round, sized from the shape

`roundness` is a choice, not a stored radius: the radius follows the shape's
size, so a resize keeps its look and the panel needs two buttons rather than
a slider. A round rectangle's radius is `min(8, side / 8)` (`rectangleRadius`).
A round diamond cuts each corner a fifth of an edge back, at most 12 units,
and joins the cuts with a quadratic curve whose control is the vertex
(`diamondCorners`). Both live in `geometry.ts` so Canvas2D, SVG and the
sketch pass draw the same outline. Hit testing, binding outlines and the Rust
record keep the sharp shape; a round corner sits at most a few units inside
it.

## Arrowheads are one geometry for every renderer

An arrow has an independent head at each end: `none`, `arrow` (open chevron),
`triangle` and `diamond` (filled with the stroke colour), `dot` and `bar`.
`arrowheadAt` returns the outline, wing pair and size for a head, aimed along
the path’s end tangent (`pathEnd`; `arrowheadShape` does so for a bare polyline,
which the picker glyphs use). Size is `min(16, whole path length / 3)`, so a
short last segment of a curve or elbow does not shrink the head, and no head
leaves Rust’s 16-unit arrow culling pad. Heads draw solid even on a dashed line. Hit testing covers
the tip-to-wing segments of any head type, and a `none` end has no head to hit.

## Build and verification include the WASM asset

`bun run canvas` builds the standalone Rust crate and wasm-bindgen glue.
The installed CLI version must match the pinned crate, and the
`wasm32-unknown-unknown` target must be installed. Generated pkg output is
ignored. Predev, build and test run the builder; CI checks native Rust tests,
formatting and Clippy as well as frontend format and WASM parity tests.

## Gotchas

- Read pages through `notebookPaper(document, notebook)`, never `document.paper`; a `.canvas` may hold paper and must still behave as a whiteboard.
- Never decide the kind from the document: it is the path's (`DrawingKind::of`, `drawingKindOfPath`), or a canvas holding paper turns into a notebook.
- Replace elements and their array immutably; the retained spatial scene and Path2D cache use identity.
- Keep gesture previews separate from the committed scene or every pointer move rebuilds the BVH.
- Never write the scene during a gesture: previews live in overrides, so an outside document can replace the base underneath and the gesture commits onto it.
- Key the surface by file (`FilePage` keys `CanvasEditor` by file ID); a different file's document is otherwise taken as an outside update and keeps the old undo history.
- Route every element edit through `previewChanges` / `commitChanges`; writing `elements` directly leaves bound arrows detached from a moved target. Layer order is the one exception (`reorder`), since it moves no element.
- Canvas2D font names must match the loaded `"Inter Variable"` family (`CANVAS_FONT_STACKS`); a wrong name measures and wraps with a fallback font, and the label cache is dropped on `document.fonts` `loadingdone` for the same reason.
- Keep `CANVAS_FONT_STACKS` in step with `theme.css`; drift makes the canvas, the overlay textarea and SVG export disagree on wrapping.
- A hollow labelled shape counts as filled for hit testing in both the JS reference and the WASM record; change one and the parity test fails.
- Re-place a line or arrow’s points with `throughPoints`; setting x/y by hand shifts an elbow arrow’s fixed segments, which are local to x/y.
- A new arrow path kind must come out of `linePath`; computing a path elsewhere makes bounds, hit testing and the WASM scene disagree with what is drawn.
- Add a new optional field to `STYLE_KEYS` and `KEY_TYPES` in `style.ts` as well as to `validate` in `model.ts`, or the inspector cannot patch it.
- Unknown format versions must fail before writing; partial loading must never erase unfamiliar data.
- Rename and delete through the shared session so pending writes cannot target an obsolete path.
- Native menu shortcuts need menu routing (`editRouting.ts`, `backForwardRouting.ts`): macOS gives the menu its accelerators first, so the canvas's keydown never sees ⌘Z, ⌘[ or ⌘].
- Reset the pending frame sentinel when cancelling RAF during effect cleanup, or StrictMode replay cannot schedule a paint.
- Geometry benchmarks do not measure Canvas2D painting or end-to-end interaction latency.
