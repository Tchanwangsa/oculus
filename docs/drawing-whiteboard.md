# Whiteboards and notebooks live in Drive

Drive holds the student's own notes and drawings among their other files. A drawing is one of
two kinds on the same engine: a **whiteboard**, an infinite board, or a
**notebook**, paper pages to write on. Either opens in a tab or beside another
page.

## Where

| Piece | Location |
| --- | --- |
| Drive's folder browser, its Type filter and New menu | `app/src/components/drive/` ([frontend.md](./frontend.md#drive-is-one-folder-browser-over-the-disk)) |
| File route and editor choice (passes the file's kind to `WhiteboardEditor`) | `app/src/pages/subject/files/FilePage.tsx` |
| The two kinds: extension, category, path and row helpers, reconcile plan | `app/src/lib/whiteboard/drawingKinds.ts`, `DrawingKind` in `app/src-tauri/src/library/files/whiteboards.rs` |
| Extension → category for a path (`.notebook` is `notebook`) | `app/src/lib/files/libraryLayout.ts`, `app/src-tauri/src/library/layout/category.rs` |
| Drawing file rows, reconciles and native command wrappers | `app/src/lib/whiteboard/files.ts` |
| Shared ordered saves and the shared-drawing adapter | `app/src/lib/whiteboard/sessions.ts`, `app/src/lib/whiteboard/sessionRuntime.ts`, `app/src/lib/whiteboard/authority.ts` |
| Desktop recovery drafts | `app/src/lib/whiteboard/drafts.ts`, `app/src-tauri/src/library/files/whiteboards/drafts.rs` |
| Authority command wrappers and event listeners | `app/src/lib/whiteboard/files.ts` (`whiteboardAuthority`) |
| Title, save state, recovery, conflict and pause controls, the Share button and editor host | `app/src/components/whiteboard/WhiteboardEditor.tsx` |
| Drawing surface and its layout, label text overlay, the tool rail and the top bar's style controls | `app/src/components/whiteboard/WhiteboardSurface.tsx`, `app/src/components/whiteboard/WhiteboardToolbar.tsx`, `app/src/components/whiteboard/WhiteboardStyleBar.tsx`, `app/src/components/whiteboard/whiteboard.css` |
| The rail's tool drawings (an outline at rest, flat colour when chosen) | `app/src/components/whiteboard/toolArt/` (`ToolArt.tsx` renders the parts in `pointer.ts`, `ink.ts`, `insert.ts`, `shapes.ts` and `graphs.ts`) |
| Pointer gestures, history, keyboard commands and the notebook's page rules | `app/src/hooks/whiteboard/useWhiteboardInteraction.ts` |
| Two-finger pan and pinch | `app/src/lib/whiteboard/touchViewport.ts` |
| Pan momentum: release speed and glide | `app/src/lib/whiteboard/momentum.ts` |
| What a reload resumes on this device: tool, drawing style, each drawing's camera | `app/src/lib/whiteboard/viewState.ts` |
| Zoom ranges | `app/src/lib/whiteboard/zoom.ts` |
| Content transactions (undo steps and shared edits) and rebasing | `app/src/lib/whiteboard/collaboration.ts`, `app/src/lib/whiteboard/clientReconciliation.ts` |
| Device-local drawing preferences (the eraser's settings) | `app/src/lib/whiteboard/toolPreferences.ts` |
| Eraser controls and cuts along the pointer sweep | `app/src/components/whiteboard/inspector/EraserControls.tsx`, `app/src/lib/whiteboard/erase.ts` |
| What a selection box selects (release and live preview) | `app/src/lib/whiteboard/boxSelect.ts` |
| Selection by a closed lasso loop | `app/src/lib/whiteboard/lasso.ts` |
| Freehand pen geometry and device-local ink presets | `app/src/lib/whiteboard/pens.ts`, `app/src/lib/whiteboard/inkPresets.ts`, `app/src/components/whiteboard/WhiteboardPresetBar.tsx` |
| The two `<canvas>` layers that paint a freehand stroke in progress | `app/src/lib/whiteboard/liveStroke.ts`, `app/src/components/whiteboard/WhiteboardSurface.tsx` |
| Builders for matrix and coordinate axes | `app/src/lib/whiteboard/templates.ts` |
| Native selection, callout and touch defaults kept off the drawing | `app/src/lib/whiteboard/browserSelectionGuard.ts` |
| Canvas2D drawing and image export | `app/src/lib/whiteboard/render.ts`, `app/src/lib/whiteboard/renderSvg.ts` |
| Text fonts, measuring, wrapping and label layout | `app/src/lib/whiteboard/renderText.ts` |
| Versioned document model and coordinate helpers | `app/src/lib/whiteboard/model.ts`, `app/src/lib/whiteboard/geometry.ts` |
| Pages: the `paper` field, `notebookPaper`, layout, templates, locked backdrops, the page camera | `app/src/lib/whiteboard/paper.ts` |
| Drawing pages and templates, the element → page owner, export bounds | `app/src/lib/whiteboard/renderPaper.ts`, `renderPagePng` and `paintWhiteboard` in `app/src/lib/whiteboard/render.ts`, `paperSvg` in `renderSvg.ts` |
| What the pointer reaches on a page | `app/src/lib/whiteboard/pageHits.ts` |
| Page operations: add, insert, duplicate, move, remove, template, size, PDF pages | `app/src/lib/whiteboard/pages.ts` |
| Page bar, page sidebar, thumbnails, per-page menu and drag to reorder | `app/src/components/whiteboard/pages/` |
| What a page's picture depends on | `app/src/lib/whiteboard/pageFingerprints.ts` |
| New notebook: blank paper or a copy of a library PDF | `app/src/components/whiteboard/NotebookDialog.tsx`, `app/src/lib/whiteboard/notebook.ts` |
| A PDF's Draw button: a notebook or a whiteboard copy of its pages | `app/src/components/whiteboard/DrawOnCopy.tsx`, `app/src/lib/whiteboard/pdfCopy.ts` |
| Page pictures for chat agents: change detection, writer, hook, Rust commands and manifest | `app/src/lib/whiteboard/snapshots.ts`, `app/src/hooks/whiteboard/useNotebookSnapshots.ts`, `app/src-tauri/src/library/files/whiteboards/snapshots.rs`, `app/src-tauri/src/library/paths/notebook.rs` |
| Which notebooks are open, for the chat message | `app/src/lib/whiteboard/openNotebooks.ts` (read by `harnessSend` in `app/src/lib/harness/commands.ts`) |
| `oculus read` on a notebook | `app/src-tauri/src/bin/oculus/commands/query/notebook.rs` |
| PDF export | `app/src-tauri/src/library/files/whiteboards/export_pdf.rs`, `exportPdf` in `app/src/lib/whiteboard/platform/` |
| Style fields, which element types take them, inspector sections | `app/src/lib/whiteboard/style.ts` |
| The style bar's controls, its More styles and selection popovers, palettes and popover guards | `app/src/components/whiteboard/inspector/` |
| Colour palette popover | `app/src/components/ui/color/ColorPalette.tsx` |
| Shape labels: commit, grow rule, restyling | `app/src/lib/whiteboard/labels.ts` |
| Bindable outlines | `app/src/lib/whiteboard/outline.ts` |
| Arrow binding, end placement, following | `app/src/lib/whiteboard/bindings.ts` |
| Arrow path (the one rendered path), curve, point edits, handles, retyping | `app/src/lib/whiteboard/arrow/` |
| Elbow routing: headings, ends, grid, A*, route, fixed segments and segment drags | `app/src/lib/whiteboard/elbow/` |
| Scaling a selection | `app/src/lib/whiteboard/resize.ts` |
| Image import modal (and a notebook's Insert PDF pages), library PDF search and selectable pages | `app/src/components/whiteboard/WhiteboardImageDialog.tsx`, `app/src/components/whiteboard/WhiteboardLibraryPdfs.tsx`, `app/src/components/whiteboard/WhiteboardPdfPages.tsx` |
| Image decoding and batch placement; native PDF import sessions | `app/src/lib/whiteboard/importMedia.ts`, `app/src/lib/whiteboard/mediaPlacement.ts`; `app/src/lib/whiteboard/importPdf.ts` |
| Image drops, placement previews and raster paste | `app/src/hooks/whiteboard/useWhiteboardMediaDrop.ts`, `app/src/lib/ui/nativeFileDrop.ts`, `app/src-tauri/src/library/files/whiteboards/images.rs` |
| What the surface takes from its host (export saving, library PDF search and opening): the browser default and the desktop's | `app/src/lib/whiteboard/platform/` |
| Area captures, crop geometry and clipped PNG rendering | `app/src/lib/whiteboard/region.ts`, `app/src/components/whiteboard/WhiteboardRegionActions.tsx`, `app/src/components/whiteboard/WhiteboardCaptureHold.tsx` |
| Embedded raster validation and Canvas2D image cache | `app/src/lib/whiteboard/imageSource.ts`, `app/src/lib/whiteboard/renderImages.ts` |
| Scoped library PDF sessions and page rasterisation | `app/src-tauri/src/library/files/whiteboards/imports.rs` |
| Element edits as replaceable maps, and additions stacked above an element | `app/src/lib/whiteboard/changes.ts` |
| Layer order moves | `app/src/lib/whiteboard/layers.ts` |
| Clipboard envelope and clone with remapped bindings | `app/src/lib/whiteboard/renderClipboard.ts` |
| Retained Rust scene and WebAssembly bridge | `app/whiteboard-core/src/`, `app/src/lib/whiteboard/engine.ts` |
| Native file commands, both kinds, and scoped paths | `app/src-tauri/src/library/files/whiteboards.rs` |
| Shared-drawing authority: validator, transactions, sessions, receipts, commands | `app/src-tauri/src/library/files/whiteboards/authority/` |
| `files` row updates from Rust | `app/src-tauri/src/db/store/files.rs` (`touch_file_row`, `rename_file_row`, `delete_file_row`) |
| Format and transaction parity fixtures (run by the TypeScript and Rust tests) | `app/tests/fixtures/whiteboard-model/`, `app/tests/fixtures/whiteboard-collab/`, `app/tests/lib/whiteboard/collabFixtures.test.ts` |
| WebAssembly build | `app/scripts/build-whiteboard.mjs` |
| Format, geometry, labels, style, bindings, layer order, pages, WASM parity and save-session tests | `app/tests/lib/whiteboard/` |
| Pen, touch, undo, notebook and outside-document tests that drive the hook's handlers | `app/tests/hooks/whiteboard/` |

## The kind is the file's extension

| Kind | File | `files` row |
| --- | --- | --- |
| Whiteboard | `courses/<code>/…/<title>.whiteboard` | `category = whiteboard`, `file_type = whiteboard` |
| Notebook | `courses/<code>/…/<title>.notebook` | `category = notebook`, `file_type = notebook` |

Either sits anywhere among the student's own files; a new one lands at the
subject's root. `list_whiteboards` walks the own files at any depth
(`layout::own_files`), so a drawing moved into a folder keeps its row.

Both are the same JSON. The kind comes from the path (`DrawingKind::of`, the
own-file guard, and `drawingKindOfPath`) or the row's category (`drawingKindOf`), never from what
the document holds. `FilePage` passes it to `WhiteboardEditor`, the companion page
reads it from the snapshot's `x-oculus-kind` header
([device-sharing.md](./device-sharing.md)), and below the editor it is one
boolean, `notebook`.

Text the student reads names the drawing by its kind: "notebook" on a
notebook, "whiteboard" on a whiteboard (`kindNoun` and `kindTitle` in
`app/src/lib/whiteboard/kindWords.ts`; save sessions take it from the file's
extension). Where no file is in view, as in Settings → Device sharing, it says
"a whiteboard or notebook".

- **Commands.** `create_whiteboard` and `list_whiteboards` take an optional
  `kind` (`"whiteboard"` by default; `createDrawing`, `createWhiteboard`,
  `createNotebook` in `files.ts`). Read, write, rename and delete take a path
  and accept either kind's; a rename keeps the file's own folder and
  extension. A notebook's content must have valid `paper`
  (`validate` in `whiteboards.rs`), so a write that drops it is refused.
- **Reconciles.** Each kind has its own (`reconcileWhiteboards`,
  `reconcileNotebooks`, both `reconcileDrawings`), planned by
  `planDrawingRows`: it adds rows for its files on disk and removes rows of
  its own category whose file is gone, and never touches the other kind's
  rows or folder. A removed row's page pictures, if any, go with it.
- **Drive** lists notes, whiteboards and notebooks in the student's folders
  beside their other files; its Type filter narrows to one kind. New offers
  Note, Whiteboard and Notebook… (a dialog, below), made in the folder on screen.
  A card's picture of a drawing is drawn from its file by this renderer at
  card size, or is a notebook's first page picture
  ([frontend.md](./frontend.md#drive-is-one-folder-browser-over-the-disk)).
  Notes are ordinary Markdown documents ([editor.md](./editor.md)). A title
  is its filename; a rename keeps the database ID, and open tabs on the file
  follow it ([frontend.md](./frontend.md#path-keyed-state-follows-drive)).

## The format is one open envelope

The envelope is `type: oculus-whiteboard`, `version: 1`, with elements, a viewport
and background. Element coordinates are world coordinates; viewport x/y
translate in screen units before applying zoom. Polyline points are local to
their element's x/y. IDs are unique within one document.

Both readers (`parseWhiteboardDocument`, `validate` in `authority/model.rs`)
also take `type: oculus-canvas` and write `oculus-whiteboard` on the next
save. A drawing file named `.canvas` is renamed to `.whiteboard` at launch or
as Drive finds it ([library-move.md](./library-move.md#drawings-named-canvas)).

The format describes geometry and appearance, independently of the editor
implementation. Sketch and clean are per-element appearance choices. Extra
JSON fields survive loading, edits and saving, so metadata can accompany the
scene. Unsupported versions and element kinds fail validation instead of
being dropped. Introducing a new geometry kind requires coordinated model,
renderer, Rust geometry and format validation changes.

Appearance and label fields are optional and validated when present; the
version stays 1, and an absent field means its default (`WHITEBOARD_DEFAULTS` in
`model.ts`). A writer omits a field that equals the default.

| Field | Values (default) | Applies to |
| --- | --- | --- |
| `opacity` | 0..1 (1) | every element |
| `pen` | `ball` `fountain` `pencil` `brush` `sketch` `chinese` `dashed` `dotted` `roundMarker` `flatMarker` (ball) | freehand strokes |
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

## Freehand ink shares its pen geometry across exports

Pen and Highlighter both create freehand elements. Canvas2D and SVG use
the same definitions in `pens.ts`. Ball and round marker draw a solid line
with round ends. Flat marker uses square ends and bevelled corners. Dashed
and dotted pens use the shape dash patterns, and pencil and sketch layer
offset passes. Fountain, brush and Chinese pens fill an outline whose width
varies with direction or distance along the path. A missing `pen` draws as
ball. The marker's opacity is an element style, so each mark follows the
drawing's ordinary layer order.

The top bar shows four Pen presets or three Highlighter presets as dots
while the tool is active and there is no selection. Each dot switches the
pen, colour, width and opacity together. Style changes in this state update
the active preset. Restyling a selected stroke leaves the presets intact.
Presets and the active slot for each tool live in localStorage
(`inkPresets.ts`), with defaults for invalid saved values.

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
  default), `grid`, `ruled`, `dotted`, `graph`, `cornell`, `music` or
  `isometric`. `gap` is 0 to 1000. Other fields are kept. `paperProblem` and `validate_paper` in `authority/model.rs` apply the
  same rule, and the model fixtures hold them together.
- **A whiteboard ignores `paper`.** A `.whiteboard` that holds one keeps it: the model
  validates and saves it back as written, and the file is never rewritten for
  it. Nothing else reads it: no sheets, page camera, clipping, page tools,
  pictures, chat context or PDF export, so it opens and exports exactly like
  the same whiteboard without it. Every page behaviour asks `notebookPaper(document,
  notebook)` rather than reading `paper`.
- **Sharing.** A change to the pages travels in a transaction as
  `paper: { before, after }`, both whole papers, and merges like the
  background: it applies when the current paper equals either side, and
  otherwise fails with `conflict-background`. Two devices editing pages at once
  therefore conflict, and the draft is kept. A document with no paper never
  gains or loses one through a transaction, and a whiteboard never sends one.

### The pages bound the camera and the pen

- **Drawing.** `paintWhiteboard` paints each page as a white sheet with a soft
  shadow on the desk (the document's `background`, `NOTEBOOK_DESK` grey for a
  new notebook), then its template, then the elements, each clipped to its own
  page; a run of elements on one page shares one clip. The whiteboard's dot
  grid and its toggle are off. `templateMarks` supplies the geometry for the
  screen, rendered page thumbnails and SVG export (`paperSvg`). Graph paper
  adds bold lines to a regular grid, Cornell adds cue and summary divisions
  to ruled paper, music uses five-line staves, and isometric uses upright and
  angled lines. Canvas2D skips patterns with less than 4 screen pixels between
  marks. The template pickers use coarser drawings in `PaperThumb.tsx`.
- **Camera.** A notebook opens fitted to its page width (up to 140%) once,
  while its camera is still the untouched default; Fit (1) and the page
  controls use `pageView`. Every camera write goes through `setView`, which
  keeps it on the pages (`clampCamera`): a page column narrower than the view is
  centred, a wider one pans until its edge is 24 px inside the view, and at
  most 40% of the view's height may show desk above the first page or below
  the last. Zoom runs from 60% of the fitted zoom to 800% (`zoomLimits`); wheel,
  pinch and buttons share it, and a whiteboard keeps 10% to 800%
  (`WHITEBOARD_ZOOM_LIMITS`). A resize or new paper from elsewhere re-clamps.
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
  `WhiteboardPlatform.exportPdf`) draws each page on white at 2.5 pixels a point
  and `whiteboard_export_pdf` writes one PDF page per notebook page at its size:
  the picture is stored as PNG's own compressed rows (a Flate stream with a
  PNG predictor), so nothing is recompressed.

### Page tools edit the paper as one undo step

The bar at the bottom shows the page in view with steps either side and
toggles the **Pages** sidebar: a column of thumbnails at each page's
proportions, the page in view marked. A click goes to a page. **Add page** at
the end of the list (`AddPageMenu`) offers a new page, at the last page's
template and size, or pages from a library PDF (`WhiteboardImageDialog` with
`pdfPages`); the companion has no library, so there it adds the page straight
away. Each thumbnail's ⋯ menu adds a page before or after, duplicates,
changes the template or size (A4 and Letter, portrait or landscape) and
deletes. **Apply to every page** in the **Template** submenu copies that
page's template to the whole notebook as one undo step. Dragging a
thumbnail reorders the pages on pointer events. A mouse lifts after a few
pixels and a finger or Pencil after a short hold. The list scrolls near its
edges (`usePageReorder`). On a
narrow pane the sidebar lies over the drawing like a drawer, behind it a dim
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
(`whiteboard_open_library_pdf`, `whiteboard_render_pdf_page`) and laid on its own page
as a `locked` image the size of the PDF page, so the original is only read and
the notebook outlives it. A PDF over 100 pages, or whose pages come to over
64 MB, is refused with a sentence that says so (`readPdfPages`).

A PDF's own page has a **Draw** button (`DrawOnCopy`) with two choices, each
a copy named after the PDF and opened beside it. **Notebook** is the copy
above. **Whiteboard** is a `.whiteboard` with no paper holding the PDF pages as
ordinary, unlocked images at their size, stacked top to bottom with a gap like
an image import, to sketch around (`boardFromPages`).

### What a chat agent sees

An agent reads neither file: it is JSON holding every stroke and the
assignment's pictures. While a notebook is open, `useNotebookSnapshots` redraws
the pages that changed 1.5 seconds after the content stops changing and sends
only those to `whiteboard_write_page_snapshots`. A whiteboard gets none of this.

- **Change detection** is by page, from the elements that touch it
  (`pageFingerprints`), so a stroke on page 2 redraws page 2, and a pan, a
  zoom or a stroke elsewhere redraws nothing. Each picture shows its page as
  the screen does, clipped, without ink from the next page. Writes run one at a
  time and the last scheduled document wins; a failed write is shown in the
  editor and tried again on the next change.
- **Storage** is `whiteboard-pages/<file id>/` under the library: `page-NNN.png`
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
  was made has no pictures and the command says so; a whiteboard is not text and
  says so. The agent opens the PNG with its own image tool, and a chat message
  names the open notebooks and the page in view
  ([harness.md](./harness.md#a-notebook-the-student-has-open-rides-the-prompt)).

## The pointer stays local and Rust answers geometry queries

The WebView owns input and Canvas2D drawing. The scene paints gesture
overrides on an animation frame. Freehand samples paint into the live layer
directly from the pointer handler, and the scene skips that stroke by ID
during a repaint. If either live canvas lacks a 2D context, the scene paints
the stroke with the other gesture overrides. On release the scene paints
the committed element and clears the live layer in the same frame. The
committed elements array changes at a gesture boundary. A cached Path2D per
immutable element avoids rebuilding its path during panning.

The live layer sits between the scene and the editor controls. For a ball
pen or round marker with an opaque hex colour, one canvas accumulates
settled quadratic segments and the other redraws the tip. The layer
applies the stroke's opacity once, so a marker can cross itself without
darkening. Other strokes use the scene's element renderer to repaint the
whole stroke on the tip canvas. A notebook clips both canvases to the same
page as the committed stroke, replaying the ink if its growing bounds
change which page owns it. All three backing stores use the scene canvas's measured
rectangle and device pixel ratio. A resize or camera move replays the ink
with the new transform.

The Rust core is compiled to WebAssembly and loaded with the editor. A
retained BVH indexes the scene for hit testing and viewport culling. Geometry
is packed into numeric buffers once per committed scene, with scalar
coordinates for subsequent queries. There is no Tauri round trip for a pointer event. Rust scene storage
is capped at eight scene arrays independently of undo history.

Each shape is a record of 14 floats (`engine.ts`, documented on
`WhiteboardScene::new` in `app/whiteboard-core/src/lib.rs`): kind, x, y, width, height,
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

`bun whiteboard-core/bench.ts` measures geometry queries separately from rendering.
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

A recovery record under `recovery/whiteboards/<file-id>.json` in the library
records the document and its base disk text. Native commands write it atomically
without the browser storage quota. Existing localStorage drafts migrate only
after a successful disk write. Recovery operations are ordered separately from
saves; a late recovery write cannot recreate a draft cleared after saving.
It is validated before restore. When the disk has changed independently,
the editor offers Use recovery copy or Keep disk version; recovery never
silently overwrites a newer file. Invalid recovery stays recoverable until
explicitly discarded. Save errors keep the draft and show Retry save. A
recovery-storage failure is a separate notice over the drawing: it neither
marks a successful document save as failed nor resizes the drawing area.

Rename flushes pending edits before moving the file. Unmount and pagehide
flush the session; a clean session is released after its final pane unmounts,
so reopening reads disk again. Delete runs through the same queue and marks
the session deleted, preventing a late autosave from recreating the file.

Native writes stage a sibling temporary file and rename it into place after
fsync. File commands accept only a four-part subject whiteboard or notebook path
and reject symlinks in its components. Drawing paths use their own guard; note mutation
commands remain scoped to notes.

## A shared drawing has one writer

While a drawing is shared, the whiteboard authority
(`library/files/whiteboards/authority/`) is the only writer of its file. Desktop
panes and paired devices send content transactions (`collaboration.ts`:
per-element `before`/`after`, an optional layer order, background and a notebook's paper); the
viewport never travels in one. An added element (`before: null`) goes on top
unless the edit names `afterId`, the element just below it in the sender's
scene (`null` for the bottom); it is then inserted above that element, or on
top if that element is gone. `diffWhiteboardContent` sets `afterId` only on an
addition with an older element above it, such as an undone delete, so
additions on top from two devices both land, in arrival order. `afterId` on an
edit that is not an addition, or naming the edit's own ID, is invalid. A session is keyed by the file's database ID
and starts with `whiteboard_authority_open`, given the pane's flushed text, which
must equal the file on disk. The native registry fences ordinary `write_whiteboard`,
`rename_whiteboard` and `delete_whiteboard` commands against a session owning the same
path, including a paused session. Opening and whole-file mutations take the
same registry lock, so a delayed desktop autosave cannot overwrite shared
changes between the ownership check and the write. Drive's move, rename and
delete take the same lock for their disk step (`while_unshared`) and refuse a
folder holding a shared drawing. One path cannot have two
sessions under different file IDs. Outside a session, `write_whiteboard` saves as
above.

- One lock per session. A commit validates the transaction, applies it to the
  canonical document, writes the file, bumps the revision and then broadcasts
  `{fileId, epoch, revision, clientId, transaction}`: the Tauri event
  `whiteboard-authority-change` to the main webview, and the in-process
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
  pauses (`external-edit`, and a `whiteboard-authority-state` event), keeping the
  outside version on disk and its own document in memory until
  `whiteboard_authority_resolve_external` adopts the disk version (broadcast as a
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
`collaboration.ts`. `app/tests/fixtures/whiteboard-model/` and `whiteboard-collab/`
hold the cases both test suites run, so the two cannot drift. The authority
writes the bytes `serializeWhiteboardDocument` writes: serde_json keeps key order
(`preserve_order`) and parses floats exactly (`float_roundtrip`), and
`authority/format.rs` prints numbers by JavaScript's rules. serde_json refuses
two things `JSON.parse` reads, a lone surrogate escape and nesting deeper than
128 levels; such a drawing cannot be shared.

## Sharing turns the session's saves into commits

Loading a desktop drawing first probes `whiteboard_authority_snapshot`, with its
event listener and whole-file fence already installed. If a native session
survived a webview reload, the desktop joins its canonical snapshot and replays
changes buffered during the probe. This happens without opening Share, and
does not write the disk file. Only `session-ended` means no authority owns the
file and ordinary file loading and saves may resume; other probe failures
surface as load errors. Recovery drafts still require the usual explicit
choice. Refreshing an existing share also rejoins a detached desktop session.

`attachAuthority()` runs on the session's queue: it finishes the whole-file
flush, then calls `whiteboard_authority_open` with that flushed text and switches
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
(`diffWhiteboardContent`) and commits that transaction under the session's
`clientId`, an increasing `seq` and a fresh `opId`; a viewport that differs from
the last one sent travels with it, or alone with no edits. One op is in flight.
An ack whose revision follows the canonical one applies its transaction; one
already covered changes nothing; any other (a replay, or a gap) refetches
`whiteboard_authority_snapshot`. A failure with no code may not have reached the
authority, so the same op is resent unchanged on retry and a replay answers it.
A refused op frees its `seq`.

`whiteboard-authority-change` events, our own included, advance the canonical
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
  and makes the surface inert. The editor offers Use shared whiteboard, or
  notebook (drop the unsent change and show the canonical document), and Save
  draft as copy (a new drawing in the same subject holding the draft).
- **Paused.** `external-edit` pauses commits; edits keep collecting. Use file
  on disk and Keep my version call `whiteboard_authority_resolve_external`
  (`adopt-disk`, `keep-ours`) and then commit the unsent edits.
- **Stale op.** The session takes a new `clientId`, refetches the snapshot and
  commits again.
- **Ended.** An `ended` state, or `session-ended`, puts the session back on the
  whole-file path after rereading the file. Unsent edits become the Use
  recovery copy / Keep disk version choice; a recovery copy made while shared
  never matches the disk text, so after a restart it is offered, never applied.
- **Rename and delete** go through `whiteboard_authority_rename` and
  `whiteboard_authority_delete`. The authority updates the `files` row on a
  background thread; the wrappers in `files.ts` update it too and announce
  `WHITEBOARDS_CHANGED_EVENT`, so Drive's reload never reads the old row.

The status line reads Sharing, Sharing paused or Sharing ended before the save
state.

## Tools and exports share the same geometry

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

The Lasso tool is Select with a loop in place of the box (`lasso.ts`). A loop
takes every unlocked element with more than half of its points inside it (a
stroke or line's own points; a shape, text or image's centre and corners), so
writing circled roughly is taken and a neighbour the loop only clips is not.
The loop closes itself and fills even-odd. Shift adds to the selection. A
press on a handle, on a selected element or inside the selection's frame
moves or resizes it as Select does; a press anywhere else, an unselected
element included, starts a new loop. The tool stays chosen, and choosing it
keeps the selection Select made.

Corner handles scale a selection proportionally, including every image in a
group. The four edge handles stretch only width or height; Shift on an edge
scales proportionally around the opposite edge's midpoint. A single line or
arrow has [point-editing handles](#arrows-are-sharp-curved-or-elbow-paths) in
place of the selection frame (`selectionFrame` in `render.ts`).

The editor root scopes shortcuts and participates in native Edit menu
routing. A visible note editor must not claim undo while a drawing owns focus.
Text inputs keep their own native editing behavior. Pressing the drawing takes
focus and clears retained DOM selections, so WebKit cannot send clipboard
commands to a chat's old contenteditable caret. Drawing gestures capture
the active pointer and suppress WebKit text selection: `installBrowserSelectionGuard`
on the editor root cancels `selectstart` and `contextmenu` outside text fields and
native touch defaults on the drawing element. In the companion these touch
listeners attach to the page body, with a target check for this surface's canvas,
so touch tracking covers a stable ancestor across canvas repaints. The
desktop listeners stay on the editor root. The companion opts into
`suppressNativeGestures` on `WhiteboardSurface`: Safari's `gesturestart`,
`gesturechange` and `gestureend` defaults are cancelled there so native
pinch/rotation does not compete with the Pointer Events camera. The desktop
leaves those events alone for WebKit trackpad zoom. This suppresses
browser tap gestures while Pointer Events keep driving drawing,
pan and pinch. Toolbar taps and text fields keep their native defaults. The
canvas and editor also declare `touch-action: none`; the canvas is not draggable.

PNG and SVG export cover the whole drawing with padding, excluding editor
chrome, selection, binding hints and grid. The surface saves an export and
opens a PDF for import through `useWhiteboardPlatform()`
(`lib/whiteboard/platform/`), so its own imports never reach Tauri. The default is
a plain browser's: an export downloads, and Choose PDF says to open Oculus
desktop while image files still import. The desktop root (`main.tsx`) provides
`tauriWhiteboardPlatform`, whose export command opens a native Save dialog and
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
- A pointer cancellation, tool switch or Undo during a freehand stroke
  discards the preview. The held Pencil can start drawing again on its next
  contact. Undo during the stroke keeps earlier committed edits.
- A pen move with its contact button down and no gesture starts the stroke
  that WebKit's lost pointerdown should have; hover never inks. A fresh pen down
  while a pen stroke is open commits that stroke's ink as its own stroke rather
  than dropping it or joining the two.
- Pointer capture can throw when the pointer is already gone; the gesture
  continues without it.

## The eraser takes whole elements or cuts strokes

With the eraser active, `EraserControls` displays its settings in the style
bar. `toolPreferences.ts` keeps them in localStorage and supplies defaults
for missing or unreadable values.

- **Mode.** Whole stroke (the default) removes the topmost element within 10
  screen pixels of each pointer sample. Partial cuts freedraw strokes: every
  part within the eraser's radius plus half the stroke's width of the segment
  between consecutive samples, coalesced ones included, goes, so a fast sweep
  skips nothing. A stroke is subdivided to a quarter of the radius first, so a
  cut lands that close to the eraser's edge (`erase.ts`). In a notebook each
  cut is clipped to the stroke's owning page, preserving samples hidden
  outside it. Shapes, lines, arrows and text the sweep touches go whole.
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
only the stroke's own points plus cut endpoints. A piece must reach the
larger of 2 screen pixels and half the stroke's width. If no piece survives,
the stroke is deleted. Images and locked elements are never erased.

During the gesture, the scene paints erased parts at a quarter of their
opacity (`ghosts` in `paintWhiteboard`) and the kept pieces at full style opacity.
The eraser passes over the ghosts. Release commits the whole gesture as one
undo step through `commitChanges`. Escape, a cancelled pointer or a second
finger discards the preview.

## Undo is per pane and touches only its own edits

Each committed edit records the content transaction from the scene before it
to the scene after it, and its inverse (`diffWhiteboardContent`). Undo and redo
apply that transaction to the current scene (`applyWhiteboardTransaction`), so a
shape someone else added since stays. If they changed a shape the step would
restore, the step is refused and dropped, and the surface shows why inline;
it never overwrites or removes their work. Undo leaves the camera alone, waits
while a gesture or a pinch is in progress, and keeps 100 steps. A step that re-adds deleted elements puts each back above the
element it followed: the transaction carries `afterId`, and the step also
remembers the layer order.

## A document from outside the pane is a new base, not a reset

The hook ignores a document it handed to `onChange` itself, including an echo
with a fresh identity but the same content. Any other document (another pane on
the same session, a collaborator's edit, a recovery copy) replaces the
scene but keeps this pane's camera, the gesture in progress and its preview,
the undo history, the text being written and the selection of shapes that still
exist. A preview of a shape someone removed is dropped, and a point or segment
drag ends when its line is removed.

The camera belongs to the pane. Without `onViewportChange`, pan and zoom are
saved in the document (a pan when it ends, wheel and buttons after 180 ms) as
the file's initial view. With it, they go only to that callback and never
through `onChange`, for a host that keeps its camera local. The desktop editor
and the companion both pass one, so scrolling never marks a drawing unsaved; the
camera reaches the file only inside the next real edit's document.

A reload or reopen resumes on the same device (`viewState.ts`, localStorage):
the last tool (Crop comes back as Select) and drawing style, and the camera of
each of the 60 most recently viewed drawings, keyed by `cameraKey` — the
file's id on the desktop, the share link on the companion. The camera is
written 400 ms after it stops moving and on `pagehide`, and comes back at the
first real size in place of a notebook's first fit, then clamped like any
other camera. Damaged fields fall back to the defaults one by one.

## Images and PDF pages are saved inside the drawing

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
are inserted at the view centre; drawing clipboard envelopes keep their cloning
and binding rules. Text fields keep their own paste behavior. Async reads are
cancelled on replacement or unmount, and errors leave the drawing intact.

An image element stores a base64 raster `dataUrl`, its source name and natural
dimensions alongside its world bounds. The `.whiteboard` stays self-contained:
reopening, copying and exporting do not depend on the original source file.
Remote URLs and SVG input are rejected. Images use their rectangular bounds
for selection and culling in both the JavaScript reference and Rust core.
The style bar offers opacity, layer controls and Crop image for a single image. The eraser skips
images, including imported PDF pages, and erases shapes and ink through them.
To delete an image, select it and use the style bar's trash button or Delete;
that deletion is one undo step.

### Capture duplicates an area; cropping trims an image

**Capture area** (C) drags a rectangle anywhere on the drawing. The resulting
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
clipboard; their error points to Duplicate for copying within the drawing.

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

Fonts are the `WHITEBOARD_FONT_STACKS` in `renderText.ts`. Sans and mono repeat
`--font-sans` and `--font-mono` from `app/src/styles/theme.css` because
Canvas2D and SVG cannot read CSS variables; edit both together. The overlay
textarea, Canvas2D, SVG and the width measurer use the same stacks.

## The bars are docked beside the drawing

`WhiteboardSurface` docks the tool rail at the left and places the top bar
above the drawing. The notebook's Pages sidebar opens between the rail and
the editor root. The sidebar and rail sit outside the root, which scopes
shortcuts and clipboard commands. Tool and figure buttons hand focus back
to the root.

- **The rail** (`WhiteboardToolbar`) is a 52px column (`--whiteboard-rail-width`) in
  five groups: Select, Lasso, Grab, Capture area; Pen, Highlighter, Eraser;
  Text, Image, Matrix; Rectangle, Oval, Diamond, Arrow, Line; X-Y axes, X-Y-Z
  axes. It scrolls when the pane is shorter than the column. Each entry's key
  is shown on it and in its tooltip: S L G C, P H E, T I M, R O D A N, Y Z;
  Shift+G toggles a whiteboard's grid. Grab is the `hand` tool. Image opens the
  image picker. Matrix inserts nine squares in the current shape style,
  and the axes insert arrows in x red, y green and z blue. They are ordinary
  rectangles and arrows placed at the centre of the view, shifted onto the
  page in view if the centre falls on the notebook desk. Each insertion
  selects the figure and returns to Select in one undo step (`templates.ts`).
  `1` fits, `0` sets zoom to 100% within the zoom limits, and `+` and `-`
  zoom. The arrows nudge a selection by one unit, or ten with Shift. ⌘[ and
  ⌘] step its layer, and adding Shift moves it to the back or front.
- **The rail's tool drawings** (`toolArt/`) show a muted outline at rest
  and flat colour with a shaded side on a tinted background while selected.
- **The top bar** holds the style controls on the left
  ([below](#the-inspector-shows-what-applies-to-the-selection)) and, on the
  right, undo and redo, zoom (out, the percentage, in, fit), the grid on a
  whiteboard, export and the shortcuts list.
- **The drawing area** holds the scene, live ink, overlays and the
  notebook's bottom page bar. The scene canvas's box sets the dimensions
  and coordinates for painting, pointer events and image drops.
- **Narrow panes give up parts of the top bar** by the editor root's width
  (`whiteboard.css`): the zoom percentage and fit at 1180px or less, the pen's
  name at 1100px, the sixth colour pick and the hint at 1000px, every pick but
  the current-colour button at 860px, the exact font-size field and the
  shortcuts at 760px. The style half then scrolls sideways with faded edges.
- **The companion** keeps 100px at the bar's right end for its expand and
  debug buttons; expanded, the rail and the bar start below the status bar
  and the page bar clears the home indicator (`--whiteboard-bottom-edge`,
  `app/src/companion/companion.css`).

## The inspector shows what applies to the selection

`style.ts` is the single table of which style field applies to which element
type, and `inspectorSections` derives the style bar from it: the union of
sections that apply to any selected element (or to the active tool’s next
element when nothing is selected, or to the element being written in).
Sections are Color or Stroke, Pen, Fill, Font and Align, Stroke width, Stroke
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
element of those kinds. Clicking a non-ink element adopts its look as the
default. Freehand ink uses the active tool preset. Opacity previews while
dragging and commits one undo step on release.

The inspector is `WhiteboardStyleBar.tsx`, the left half of the top bar
([above](#the-bars-are-docked-beside-the-drawing)), shown when sections apply,
an ink tool is chosen or the eraser is. With the eraser it holds the eraser's
settings (`inspector/EraserControls.tsx`, [above](#the-eraser-takes-whole-elements-or-cuts-strokes)).
The inline controls include the ink presets, stroke colour, pen menu, Fill,
stroke width and font size. The pen menu draws samples with the drawing's
pen geometry and offers eight writing pens or two markers according to the
current pen. The width slider previews while dragged. It and the numeric
field use 0.5-unit steps from 0.5 to 30, or up to 60 for a marker. Font
family and alignment, stroke style, sketch, edges, arrow type, arrowheads
and opacity sit behind **More styles** (`inspector/MoreStyles.tsx`). With a
selection the bar ends with its actions (`inspector/SelectionActions.tsx`).
**Crop image** appears for a lone image. **Layer** opens the layer moves,
and **Delete** removes the selection. Popovers open below the bar.

The page bar and the style popovers share `.whiteboard-chrome` in `whiteboard.css`: a
translucent, blurred card that turns nearly opaque under the pointer or
keyboard focus. One file per control lives in
`app/src/components/whiteboard/inspector/`. The colour shows five top picks (six
fluorescent ones for a marker) and a button for the current colour
(`ColorRow`); that button and Fill open
`ColorPalette` (`app/src/components/ui/color/`) on the drawing palette in
`inspector/palette.ts`: a grid of hues, the shades of the hue the colour
belongs to (`colorShades`), the hex code and the full picker behind Custom.
Strokes pick a hue's base and fills its palest shade. Edges is a Sharp/Round
pair and Arrow type a Sharp/Curved/Elbow trio. Each arrow end is a button
showing its head that opens a grid of the six types. These popovers focus
themselves rather than their first option, leave focus in an open text
overlay, and keep their keys from the drawing's shortcuts
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
menu's Back and Forward own ⌘[ and ⌘], so the drawing claims them through
`onBackForward` (`app/src/lib/menu/backForwardRouting.ts`) while it has focus
and a selection; with nothing selected they step the pane's history as
elsewhere. The ⇧ variants reach the drawing's own keydown.

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
drop the rest (`cloneWhiteboardElements`).

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

`bun run whiteboard` builds the standalone Rust crate and wasm-bindgen glue.
The installed CLI version must match the pinned crate, and the
`wasm32-unknown-unknown` target must be installed. Generated pkg output is
ignored. Predev, build and test run the builder; CI checks native Rust tests,
formatting and Clippy as well as frontend format and WASM parity tests.

## Gotchas

- Read pages through `notebookPaper(document, notebook)`, never `document.paper`; a `.whiteboard` may hold paper and must still behave as a whiteboard.
- Never decide the kind from the document: it is the path's (`DrawingKind::of`, `drawingKindOfPath`), or a whiteboard holding paper turns into a notebook.
- Replace elements and their array immutably; the retained spatial scene and Path2D cache use identity.
- Keep gesture previews separate from the committed scene or every pointer move rebuilds the BVH.
- Never write the scene during a gesture: previews live in overrides, so an outside document can replace the base underneath and the gesture commits onto it.
- Key the surface by file (`FilePage` keys `WhiteboardEditor` by file ID); a different file's document is otherwise taken as an outside update and keeps the old undo history.
- Route every element edit through `previewChanges` / `commitChanges`; writing `elements` directly leaves bound arrows detached from a moved target. Layer order is the one exception (`reorder`), since it moves no element.
- Canvas2D font names must match the loaded `"Inter Variable"` family (`WHITEBOARD_FONT_STACKS`); a wrong name measures and wraps with a fallback font, and the label cache is dropped on `document.fonts` `loadingdone` for the same reason.
- Keep `WHITEBOARD_FONT_STACKS` in step with `theme.css`; drift makes Canvas2D, the overlay textarea and SVG export disagree on wrapping.
- A hollow labelled shape counts as filled for hit testing in both the JS reference and the WASM record; change one and the parity test fails.
- Re-place a line or arrow’s points with `throughPoints`; setting x/y by hand shifts an elbow arrow’s fixed segments, which are local to x/y.
- A new arrow path kind must come out of `linePath`; computing a path elsewhere makes bounds, hit testing and the WASM scene disagree with what is drawn.
- Add a new optional field to `STYLE_KEYS` and `KEY_TYPES` in `style.ts` as well as to `validate` in `model.ts`, or the inspector cannot patch it.
- Unknown format versions must fail before writing; partial loading must never erase unfamiliar data.
- Rename and delete through the shared session so pending writes cannot target an obsolete path.
- Native menu shortcuts need menu routing (`editRouting.ts`, `backForwardRouting.ts`): macOS gives the menu its accelerators first, so the drawing's keydown never sees ⌘Z, ⌘[ or ⌘].
- Reset the pending frame sentinel when cancelling RAF during effect cleanup, or StrictMode replay cannot schedule a paint.
- Geometry benchmarks do not measure Canvas2D painting or end-to-end interaction latency.
