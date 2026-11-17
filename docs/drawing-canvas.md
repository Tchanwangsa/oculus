# Drawing canvases live in Drive

Files → Drive holds the student’s own notes and drawing canvases. New file
chooses the editor; a canvas opens as an infinite drawing surface in a tab or
beside another page.

## Where

| Piece | Location |
| --- | --- |
| Drive list, type filters and creation menu | `app/src/pages/subject/files/DrivePage.tsx` |
| File route and editor choice | `app/src/pages/subject/files/FilePage.tsx` |
| Canvas file rows and native command wrappers | `app/src/lib/canvas/files.ts` |
| Shared ordered saves and the shared-canvas adapter | `app/src/lib/canvas/sessions.ts`, `app/src/lib/canvas/sessionRuntime.ts`, `app/src/lib/canvas/authority.ts` |
| Desktop recovery drafts | `app/src/lib/canvas/drafts.ts`, `app/src-tauri/src/library/files/canvases/drafts.rs` |
| Authority command wrappers and event listeners | `app/src/lib/canvas/files.ts` (`canvasAuthority`) |
| Title, save state, recovery, conflict and pause controls, the Share button and editor host | `app/src/components/canvas/CanvasEditor.tsx` |
| Drawing surface and its layout, label text overlay, the tool rail and the top bar's style controls | `app/src/components/canvas/CanvasSurface.tsx`, `app/src/components/canvas/CanvasToolbar.tsx`, `app/src/components/canvas/CanvasStyleBar.tsx`, `app/src/components/canvas/canvas.css` |
| The rail's tool art, drawn in code (outline at rest, flat colour when selected) | `app/src/components/canvas/toolArt/` (`ToolArt.tsx` renders the part lists in `pointer.ts`, `ink.ts`, `insert.ts`, `shapes.ts` and `graphs.ts`) |
| The matrix and axes the rail inserts | `app/src/lib/canvas/templates.ts` |
| Pointer gestures, history and keyboard commands | `app/src/hooks/canvas/useCanvasInteraction.ts` |
| Two-finger pan and pinch | `app/src/lib/canvas/touchViewport.ts` |
| Content transactions (undo steps and shared edits) and rebasing | `app/src/lib/canvas/collaboration.ts`, `app/src/lib/canvas/clientReconciliation.ts` |
| Device-local drawing preferences | `app/src/lib/canvas/toolPreferences.ts` |
| Native selection, callout and touch defaults kept off the canvas | `app/src/lib/canvas/browserSelectionGuard.ts` |
| Canvas2D drawing and image export | `app/src/lib/canvas/render.ts`, `app/src/lib/canvas/renderSvg.ts` |
| The freehand stroke being drawn, on its own layer | `app/src/lib/canvas/liveStroke.ts` |
| Pens and markers: how each lays ink along a stroke | `app/src/lib/canvas/pens.ts` |
| The pens and highlighters a device keeps ready | `app/src/lib/canvas/inkPresets.ts`, `app/src/components/canvas/CanvasPresetBar.tsx` |
| Lasso selection and the partial eraser | `app/src/lib/canvas/lasso.ts`, `app/src/lib/canvas/eraser.ts`, `app/src/components/canvas/CanvasEraserBar.tsx` |
| Text fonts, measuring, wrapping and label layout | `app/src/lib/canvas/renderText.ts` |
| Versioned document model and coordinate helpers | `app/src/lib/canvas/model.ts`, `app/src/lib/canvas/geometry.ts` |
| Notebook pages: the `paper` field, page layout, templates, locked backdrops, the page camera | `app/src/lib/canvas/paper.ts` |
| Adding and removing a page, changing a page's template | `app/src/lib/canvas/pages.ts`, `app/src/components/canvas/CanvasPageBar.tsx` |
| A template's thumbnail and name | `app/src/components/canvas/PaperThumb.tsx` |
| Drawing pages and templates; page pictures and the notebook's export bounds | `app/src/lib/canvas/renderPaper.ts`, `renderPagePng` and `paintCanvas` in `app/src/lib/canvas/render.ts`, `paperSvg` in `renderSvg.ts` |
| New notebook: blank paper or a copy of a library PDF | `app/src/components/canvas/NotebookDialog.tsx`, `app/src/components/canvas/WriteOnCopy.tsx`, `app/src/lib/canvas/notebook.ts` |
| Page pictures for chat agents: change detection, writer, hook, Rust commands and manifest | `app/src/lib/canvas/snapshots.ts`, `app/src/hooks/canvas/useNotebookSnapshots.ts`, `app/src-tauri/src/library/files/canvases/snapshots.rs`, `app/src-tauri/src/library/paths/notebook.rs` |
| Which notebooks are open, for the chat message | `app/src/lib/canvas/openNotebooks.ts` (read by `harnessSend` in `app/src/lib/harness/commands.ts`) |
| `oculus read` on a notebook | `app/src-tauri/src/bin/oculus/commands/query/notebook.rs` |
| PDF export | `app/src-tauri/src/library/files/canvases/export_pdf.rs`, `exportPdf` in `app/src/lib/canvas/platform/` |
| Style fields, which element types take them, inspector sections | `app/src/lib/canvas/style.ts` |
| Contextual inspector sections, palettes and popover guards | `app/src/components/canvas/inspector/` |
| Colour palette popover | `app/src/components/ui/color/ColorPalette.tsx` |
| Shape labels: commit, grow rule, restyling | `app/src/lib/canvas/labels.ts` |
| Bindable outlines | `app/src/lib/canvas/outline.ts` |
| Arrow binding, end placement, following | `app/src/lib/canvas/bindings.ts` |
| Arrow path (the one rendered path), curve, point edits, handles, retyping | `app/src/lib/canvas/arrow/` |
| Elbow routing: headings, ends, grid, A*, route, fixed segments and segment drags | `app/src/lib/canvas/elbow/` |
| Scaling a selection | `app/src/lib/canvas/resize.ts` |
| Image import modal, library PDF search and selectable pages | `app/src/components/canvas/CanvasImageDialog.tsx`, `app/src/components/canvas/CanvasLibraryPdfs.tsx`, `app/src/components/canvas/CanvasPdfPages.tsx` |
| Image decoding and batch placement; native PDF import sessions | `app/src/lib/canvas/importMedia.ts`, `app/src/lib/canvas/mediaPlacement.ts`; `app/src/lib/canvas/importPdf.ts` |
| What the surface takes from its host (export saving, library PDF search and opening): the browser default and the desktop's | `app/src/lib/canvas/platform/` |
| Embedded raster validation and Canvas2D image cache | `app/src/lib/canvas/imageSource.ts`, `app/src/lib/canvas/renderImages.ts` |
| Scoped library PDF sessions and page rasterisation | `app/src-tauri/src/library/files/canvases/imports.rs` |
| Element edits as replaceable maps | `app/src/lib/canvas/changes.ts` |
| Layer order moves | `app/src/lib/canvas/layers.ts` |
| Clipboard envelope and clone with remapped bindings | `app/src/lib/canvas/renderClipboard.ts` |
| Retained Rust scene and WebAssembly bridge | `app/canvas-core/src/`, `app/src/lib/canvas/engine.ts` |
| Native file commands and scoped paths | `app/src-tauri/src/library/files/canvases.rs` |
| Shared-canvas authority: validator, transactions, sessions, receipts, commands | `app/src-tauri/src/library/files/canvases/authority/` |
| `files` row updates from Rust | `app/src-tauri/src/db/store/files.rs` (`touch_file_row`, `rename_file_row`, `delete_file_row`) |
| Format and transaction parity fixtures (run by the TypeScript and Rust tests) | `app/tests/fixtures/canvas-model/`, `app/tests/fixtures/canvas-collab/`, `app/tests/lib/canvas/collabFixtures.test.ts` |
| WebAssembly build | `app/scripts/build-canvas.mjs` |
| Format, geometry, labels, style, bindings, layer order, WASM parity and save-session tests | `app/tests/lib/canvas/` |
| Pen, touch, undo and outside-document tests that drive the hook's handlers | `app/tests/hooks/canvas/` |

## Drive groups authored files without changing their storage

Drive combines `document` rows (shown as Notes) and `canvas` rows, with
All, Notes and Canvases filters. New file chooses Note or Canvas. Each type
keeps its own editor and persistence rules; adding another editor must not
change the canvas scene format to represent an unrelated document type.

Notes are ordinary Markdown documents under the subject’s documents folder
([editor.md](./editor.md)). A drawing is
`courses/<code>/canvases/<title>.canvas`, with `category = canvas` and
`file_type = canvas`. Its title is its filename. Reconciliation brings files
created outside the app into Drive and removes rows whose file is gone.
A rename keeps the database ID and updates routes that hold that file.

The canvas JSON envelope is `type: oculus-canvas`, `version: 1`, with elements,
a viewport and background. Element coordinates are world coordinates;
viewport x/y translate in screen units before applying zoom. Polyline points
are local to their element’s x/y. IDs are unique within one document.

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
| `pen` | `ball` `fountain` `pencil` `brush` `sketch` `chinese` `dashed` `dotted` `roundMarker` `flatMarker` (ball) | freedraw |
| `startBinding`, `endBinding` | `{elementId, mode: 'center'}` or `{elementId, mode: 'edge', anchor}` with `anchor` x/y in 0..1 of the target’s bounds | arrow |
| `fixedSegments` | an array in path order of `{x}` (a vertical segment on that line) or `{y}` (a horizontal one), local to the element’s x/y (none) | elbow arrow |
| `text`, `fontSize` on rectangle, ellipse, diamond | the shape’s label | shapes |
| `locked` | boolean (false) | every element |

A binding’s target ID is not checked at load, so a dangling binding never
blocks opening a file; its end stays where it is.

## A notebook is a canvas with pages

A document with a top-level `paper` is a notebook: fixed-size pages stacked
down the canvas. One without it is a whiteboard, and nothing else about the
file differs.

```jsonc
"paper": { "gap": 24, "pages": [ { "id": "p1", "width": 595, "height": 842, "template": "grid" } ] }
```

- **Layout.** A page's rectangle is derived from its place in `pages`, never
  stored: world units are points (A4 is 595 × 842), pages stack top to bottom
  separated by `gap` (24), each centred on x = 0. `pageRects` in `paper.ts` is
  the one layout; `pageAt` and `pageOfElement` answer which page a point or an
  element belongs to (the page holding the centre of its bounds).
- **Fields.** `pages` has 1 to 500 entries with unique non-empty `id`s and a
  `width` and `height` above 0 and up to 14400. `template` is `blank` (the
  default), `grid`, `ruled`, `dotted`, `graph`, `cornell`, `music` or
  `isometric`; `gap` is 0 to 1000. Other fields are
  kept. `paperProblem` and `validate_paper` in `authority/model.rs` apply the
  same rule, and the model fixtures hold them together.
- **Locked elements.** `locked: true` marks a backdrop, such as a page image
  taken from an assignment. It is drawn like any element, but the pointer
  never picks it: no click, box or select-all selects it, nothing moves, resizes
  or erases it, and layer moves treat locked elements as a fixed bottom layer,
  so ink is never sent beneath a page image. `selectableElements` hands the
  hit test the unlocked elements, returning the same array when none is
  locked so a whiteboard's cached geometry is reused.
- **Sharing.** A change to the pages (one added or removed) travels in a
  transaction as `paper: { before, after }`, both whole papers, and merges like
  the background: it applies when the current paper equals either side, and
  otherwise fails with `conflict-background`. A document with no paper never
  gains or loses one through a transaction.

### Drawing, pages and the camera

- **Drawing.** `paintCanvas` paints each page on screen as a white sheet with a
  soft shadow on the desk (the document's `background`), then its template, then
  the elements; the whiteboard's dot grid and its toggle are off. Templates:

  | Template | Marks |
  | --- | --- |
  | `grid`, `dotted` | A line each way, or a dot, every 14 points |
  | `ruled` | A line every 23 points below a 60-point top margin |
  | `graph` | The grid, with every fifth line bold |
  | `cornell` | Ruled, with bold rules: a header on the first line, a cue column 28% across, and a summary over the last 18%, starting on a ruled line |
  | `music` | Staves of five lines 7 points apart, one every 72 points from 60 down, inside 36-point side margins |
  | `isometric` | Upright lines and two families at 30° to the horizontal, meeting at the corners of 14-point triangles |

  `templateMarks` is the one geometry for the screen, the SVG export
  (`paperSvg`) and the thumbnails; a pattern finer than 4 screen pixels a step
  is skipped.
- **Camera.** A notebook opens fitted to its page width (up to 140%) once, while
  its camera is still the untouched default; Fit and the page controls use
  `pageView`. The page in view is the one at the centre of the screen.
- **Adding and removing pages.** `addPage` appends a page like the last one.
  `removePage` deletes the page and every element whose centre is on it, and
  moves the pages below, with their elements, up into its place, so a
  notebook is never left with a gap. Both are one undo step, taken through the
  same transactions as a stroke, so a page added on the iPad appears on the Mac.
  A notebook keeps at least one page.
- **Changing a page's paper.** The page bar's Paper menu sets the template of
  the page in view or of every page (`setPageTemplate`). Nothing drawn moves,
  and it is one undo step that travels as a paper edit like a page added.
- **Export.** PNG and SVG cover every page and anything drawn past a page's edge.
  PDF (desktop only, `CanvasPlatform.exportPdf`) draws each page on white at
  2.5 pixels a point and `canvas_export_pdf` writes one PDF page per notebook
  page at its size: the picture is stored as PNG's own compressed rows (a Flate
  stream with a PNG predictor), so nothing is recompressed.

### Creating a notebook

Drive's New file menu has Notebook…, a dialog with two starts. **Blank paper**
is one A4 page, portrait or landscape, in a template. **From a PDF** lists the
subject's PDFs and copies their pages: each is rendered as for any PDF import
(`canvas_open_library_pdf`, `canvas_render_pdf_page`) and laid on its own page
as a `locked` image the size of the PDF page, so the original is only read and
the notebook outlives it. A PDF over 100 pages, or whose pages come to over
64 MB, is refused with a sentence that says so. The new file is
`courses/<code>/canvases/<name>.canvas` like any canvas, and Drive lists it
beside them. A PDF's own page has a **Write on a copy** button (`WriteOnCopy`)
that does the same for that file and opens the notebook beside it.

### What a chat agent sees

An agent reads no `.canvas` file: it is JSON holding every stroke and the
assignment's pictures. While a notebook is open, `useNotebookSnapshots` redraws
the pages that changed 1.5 seconds after the content stops changing and sends
only those to `canvas_write_page_snapshots`.

- **Change detection** is by page, from the elements that touch it
  (`pageFingerprints`), so a stroke on page 2 redraws page 2, and a pan, a
  zoom or a stroke elsewhere redraws nothing. Writes run one at a time and the
  last scheduled document wins; a failed write is shown in the editor and tried
  again on the next change.
- **Storage** is `canvas-pages/<file id>/` under the library: `page-NNN.png`
  per page (2 pixels a point, about 1190 × 1680 for A4) and `pages.json`, the
  notebook's path, page count and, for each page, its id, picture and the Unix
  time it was last redrawn (`library::paths::notebook`). It is keyed by the
  `files` row, so a rename only rewrites the manifest, and it is removed when
  the canvas is deleted or reconciled away. The command refuses a path that is
  not a canvas's, a page the notebook does not have, anything but a PNG, and a
  symlinked folder.
- **`oculus read <notebook>`** prints each page's picture path and how long
  ago it was redrawn; `--pages` picks pages. A whiteboard, or a notebook never
  opened since it was made, has no pictures and the command says so. The
  agent opens the PNG with its own image tool
  ([harness.md](./harness.md#a-notebook-the-student-has-open-rides-the-prompt)).

## The pointer stays local and Rust answers geometry queries

The WebView owns input and Canvas2D drawing. Gesture previews live in refs
and an overrides map; a requestAnimationFrame coalesces repaint requests.
The committed elements array changes at a gesture boundary. A cached Path2D
per immutable element avoids rebuilding its path during panning.

A freehand stroke is the exception: while it is drawn the scene is not
repainted at all. `CanvasSurface` stacks a live-stroke layer of two canvases
over the scene, and `LiveStroke` paints into them straight from the pointer
handler, without waiting for an animation frame:

- **Ink** gains only the curve pieces that later samples cannot change, so a
  sample costs the same on a full page as on an empty one and on a long
  stroke as on a short one.
- **Tail** holds the one straight piece from the last settled point to the
  pen. Its box is erased and it is drawn again on every sample.
- Together they trace the path `render.ts` builds for the committed element.
  Both paint at full alpha and the layer's CSS opacity is the stroke's, so
  overlapping pieces never darken a translucent stroke.
- Samples are pushed onto the stroke's one points array. `paintCanvas` leaves
  the stroke out (`inkId`), so a repaint for another reason (a collaborator's
  edit, an image that finished loading) does not draw it twice.
- On release the scene repaints with the committed stroke and the layer is
  cleared in that same frame.

That is the path for a pen that lays one solid round line. Any other pen
([pens](#a-freehand-stroke-is-drawn-with-a-pen)) is erased and painted whole
on the tail canvas each sample, which costs the stroke's own length but still
never the scene's.

Where the layer's canvases have no 2D context the scene paints the stroke on
each frame, as it does every other gesture preview.

The Rust core is compiled to WebAssembly and loaded with the editor. A
retained BVH indexes the scene for hit testing and viewport culling; Rust
also simplifies completed freehand strokes. Geometry is packed into numeric
buffers once per committed scene, with scalar coordinates for subsequent
queries. There is no Tauri round trip for a pointer event. Rust scene storage
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
fsync. File commands accept only the four-part subject canvas path and reject
symlinks in its components. Canvas paths use their own guard; note mutation
commands remain scoped to notes.

## A shared canvas has one writer

While a canvas is shared, the canvas authority
(`library/files/canvases/authority/`) is the only writer of its file. Desktop
panes and paired devices send content transactions (`collaboration.ts`:
per-element `before`/`after`, an optional layer order, background and paper); the
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
changes between the ownership check and the write. One path cannot have two
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

The editor supports selection, marquee, lasso, move and resize; rectangle,
ellipse, diamond, line, arrow, pen, highlighter and text tools; an eraser;
undo/redo;
duplicate and copy/paste; layer order; grid, pan, zoom and fit. A single
selected line or arrow edits its points instead of resizing
([below](#arrows-are-sharp-curved-or-elbow-paths)), so it has no dashed
selection frame or corner handles, only its point handles (`selectionFrame`
in `render.ts`); any other selection, including several lines, has the frame.

A selection box includes an element only when its entire bounds fit inside,
in either drag direction. Dragging on an unselected image or PDF page starts
a selection box for its annotations. Clicking the page selects it; dragging
an already selected page moves it. Shift adds enclosed elements to the current
selection.

The canvas root scopes shortcuts and participates in native Edit menu
routing. A visible note editor must not claim undo while a canvas owns focus.
Text inputs keep their own native editing behavior. Drawing gestures capture
the active pointer and suppress WebKit text selection: `installBrowserSelectionGuard`
on the canvas root cancels `selectstart` and `contextmenu` outside text fields and
the native touch defaults on the canvas element itself, which is not draggable.

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
- The release is the stroke's last sample, so a down and up with no move
  between still draws, and a freedraw tap commits a dot. Coalesced pen samples
  are kept before the stroke is simplified.
- A pen move with its contact button down and no gesture starts the stroke
  that WebKit's lost pointerdown should have; hover never inks. A fresh pen down
  while a pen stroke is open commits that stroke's ink as its own stroke rather
  than dropping it or joining the two.
- Pointer capture can throw when the pointer is already gone; the gesture
  continues without it.

Drawing preferences (the gear button beside the grid) hold settings for this
device only, in localStorage (`toolPreferences.ts`). Return after erasing, off
by default, switches back to the tool chosen before the eraser when an erase
gesture ends; a cancelled erase or a pinch that takes over does not. The
eraser's mode and size are kept there too.

## The lasso selects by loop and the eraser can rub out part of a stroke

- **Lasso** (`lasso.ts`). A loop takes every unlocked element with more than
  half of its points inside it (a stroke or line's own points; a shape, text or
  image's centre and corners), so writing circled roughly is taken and a
  neighbour the loop only clips is not. The loop closes itself and fills
  even-odd. Shift adds to the selection. With something selected the tool
  moves and resizes it as Select does: a press on a handle, on a selected
  element or inside the selection's bounds drags it, and a press anywhere
  else starts a new loop. The tool stays chosen.
- **Whole-stroke eraser.** The default: whatever the eraser touches is
  removed, except images and locked backdrops.
- **Partial eraser** (`eraser.ts`). Rubs out the freehand ink within the
  eraser's radius of its path, the segment from the last sample to this one,
  so a fast drag leaves no gaps. A stroke is resampled along its smoothed
  curve first, its points under the eraser go, and each run of two or more
  that is left becomes a stroke: the first keeps the element's ID and the
  rest get new ones on top of the layer order. Shapes, lines, text and images
  are left alone. The size is in screen pixels, so the eraser feels the same
  at any zoom; a ring shows it while it is down.
- Both erasers preview through the gesture's overrides and commit as one undo
  step on release. The top bar shows the mode and size while the eraser is
  the tool (`CanvasEraserBar.tsx`).

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
through `onChange`, for a host that keeps its camera local.

## Images and PDF pages are saved inside the canvas

Add image opens a modal for local PNG, JPEG, WebP or GIF images, or PDFs
already in the Oculus library. Choose PDF opens an in-app search by filename
and subject code. The empty search lists library PDFs with current subjects
and recently opened files first. The database filters to PDFs before limiting
results; parsing is not required. Selecting a result opens its page thumbnails
with a selection that survives moving between page groups. Import adds the selected pages in document order as
independent image elements. A batch is one undo step, stacked vertically with the first page centred in
the current view, and selected for moving or resizing. A single image keeps its
proportions while resizing; Shift allows stretching it.

An image element stores a base64 raster `dataUrl`, its source name and natural
dimensions alongside its world bounds. The `.canvas` stays self-contained:
reopening, copying and exporting do not depend on the original source file.
Remote URLs and SVG input are rejected. Images use their rectangular bounds
for selection and culling in both the JavaScript reference and Rust core.
The inspector offers opacity and layer controls for images. The eraser skips
images, including imported PDF pages, and erases shapes and ink through them.
To delete an image, select it and use the top bar's trash button or Delete;
that deletion is one undo step.

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

## A freehand stroke is drawn with a pen

The Pen tool (`freedraw`) and the Highlighter tool both make `freedraw`
elements; the element's `pen` says what lays the ink, and `pens.ts` is the
one place that turns a pen into marks, for the screen, the PNG and the SVG.

| Pen | Ink |
| --- | --- |
| `ball`, `roundMarker` | One solid line with round ends |
| `flatMarker` | One solid line with square ends and bevelled corners |
| `dashed`, `dotted` | One line in the dash a shape's stroke style uses |
| `pencil` | Two light passes a little apart, denser down the middle |
| `sketch` | A full pass and a thinner, fainter one beside it |
| `fountain` | A filled outline: broad across a nib held at 45°, a hairline along it |
| `brush` | A filled outline that swells from a thin start and tapers to its end |
| `chinese` | A filled outline pressed in at the start, thinning as it travels, lifted to a point |

- **A nib pen's outline** (`nibOutline`) is SVG path data: a disc at every
  sample of the smoothed stroke and a quad joining each pair, wound the same
  way so one nonzero fill is their union and a translucent stroke never
  darkens where pieces overlap. Width comes from the stroke's direction and
  the distance along it, never from pressure, which is not stored.
- **The highlighter is a tool, not only a pen.** It offers the two markers
  where the Pen tool offers the eight pens, and shows fluorescent colour
  picks. A marker stroke is an ordinary element in layer order; its
  translucency is its `opacity`.
- **Each ink tool draws with a preset** (`inkPresets.ts`): four pens and three
  highlighters, each remembering its pen, colour, width and opacity, with one
  in use per tool. The top bar shows them as dots; a tap switches all four
  fields together. Changing the look with an ink tool chosen and nothing
  selected edits the preset in use; restyling a selected stroke changes only
  that stroke, and selecting a stroke never changes a preset or the look of
  the next shape. Presets are saved on the device in localStorage and a
  damaged save falls back to the defaults.
- **Stroke width is a number** from 0.5 to 30 (60 for a marker) in steps of
  0.5, set by a slider that previews while dragged or typed exactly.
- **While it is drawn**, a `ball` or `roundMarker` stroke is built piece by
  piece on the live layer; every other pen is erased and painted whole there
  on each sample by the scene's own `drawElement`, so what is drawn is what
  commits ([the live layer](#the-pointer-stays-local-and-rust-answers-geometry-queries)).
- A new pen needs `CanvasPen` and `CHOICES` in `model.ts`, `CHOICES` in
  `authority/model.rs`, and its ink and label in `pens.ts`.

## The bars are docked beside the drawing

Nothing floats over the drawing but a notebook's page controls and the tool
hint. `CanvasSurface` is a row: the tool rail on the left, then a column of
the top bar and the drawing area.

- **The rail** (`CanvasToolbar`) is a 52px column in five groups: Select,
  Lasso, Grab; Pen, Highlighter, Eraser; Text, Image, Matrix; Rectangle, Oval,
  Diamond, Arrow, Line; X-Y axis, X-Y-Z axis. It scrolls when the pane is
  shorter than the column. Each key is its label's first letter (S L G, P H E,
  T I M, R O D A N, Y Z), shown in the tooltip and on the button; ⇧G toggles the
  grid. Grab is the `hand` tool. Image opens the picker, and Matrix and the
  two axes drop a figure at the centre of the view and return to Select:
  nine squares in the current shape style, or arrows in x red, y green and z
  blue, selected as one undo step (`templates.ts`). They are ordinary
  rectangles and arrows, with no element type of their own. A button's art is
  an outline in the muted colour at rest and flat colour with a shaded side on
  a light tile while its tool is chosen (`toolArt/`).
- **The top bar** has an ink tool's presets (`CanvasPresetBar.tsx`) and the
  style controls on the left
  ([the inspector](#the-inspector-shows-what-applies-to-the-selection)), or the
  eraser's mode and size while it is the tool, and,
  on the right, undo and redo, zoom, drawing preferences, the grid, export and
  the shortcuts list. Its popovers open downward.
- **The drawing area** holds the scene canvas, the live-stroke layer, the text
  overlay and the empty state. The hook sizes the canvases from the scene
  canvas's own box, so the bars never count as drawing space.
- **Narrow panes drop controls instead of wrapping**, by container width in
  `canvas.css`: the zoom percentage and fit below 1340px, fill below 1280px,
  the pen's name below 1250px, stroke width below 1180px, all but five colour
  picks and the hint below 1010px, every pick but the current colour's button
  below 860px, that button and the shortcuts below 680px.
  The Style popover always has every style control, and the bar scrolls
  sideways if it still overflows.

## The inspector shows what applies to the selection

`style.ts` is the single table of which style field applies to which element
type, and `inspectorSections` derives the controls from it: the union of
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
element of those kinds, and selecting an element adopts its look as the
default. Opacity previews live while dragging and commits one undo step on
release.

`CanvasStyleBar.tsx` places the controls in the top bar
([the layout](#the-bars-are-docked-beside-the-drawing)): colour, the pen,
fill and stroke width inline, then a Style button whose popover holds every section
that applies and scrolls inside instead of overflowing. One file per section lives in
`app/src/components/canvas/inspector/` and `InspectorControls.tsx` assembles
them. Colour and Fill show five top picks and a button for the current
colour, which opens `ColorPalette` (`app/src/components/ui/color/`) on the
canvas palette in `inspector/palette.ts`: a grid of hues, the shades of the
hue the colour belongs to (`colorShades`), the hex code and the full picker
behind Custom. Strokes pick a hue's base and fills its palest shade. Edges is a Sharp/Round pair and Arrow type a
Sharp/Curved/Elbow trio. Each arrow end is a button showing its head that opens
a grid of the six types. These popovers focus themselves rather than their
first option, leave focus in an open text overlay, and keep their keys from the
canvas's shortcuts (`inspector/popoverGuards.ts`). Arrowhead glyphs are drawn
by `arrowheadShape`, so the picker previews exactly what the arrow gets. The bar and its popover keep focus in an open text overlay so a
style change restyles the text instead of committing it.

## Layer order is array order

Elements paint in `elements` order, last on top, and hit testing picks the
topmost, so reordering the array is the whole model change. `layers.ts` moves
the selection: to the front or back of the stack, or forward or backward past
the nearest unselected element that overlaps it (bounds intersect), stepping
one place when nothing overlaps. A multiple selection keeps its order among
itself, so an element stops beneath (or above) the next selected one. A move
that changes nothing returns the input array; `canReorder` disables those
buttons.

The inspector's Layer section and the shortcuts run the same `reorder` in the
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
