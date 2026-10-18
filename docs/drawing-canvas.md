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
| Shared ordered saves and recovery | `app/src/lib/canvas/sessions.ts`, `app/src/lib/canvas/sessionRuntime.ts` |
| Title, save state, recovery controls and editor host | `app/src/components/canvas/CanvasEditor.tsx` |
| Drawing surface, label text overlay, tool bar and style panel shell | `app/src/components/canvas/CanvasSurface.tsx`, `app/src/components/canvas/CanvasToolbar.tsx`, `app/src/components/canvas/CanvasStylePanel.tsx` |
| Pointer gestures, history and keyboard commands | `app/src/hooks/canvas/useCanvasInteraction.ts` |
| Canvas2D drawing and image export | `app/src/lib/canvas/render.ts`, `app/src/lib/canvas/renderSvg.ts` |
| Text fonts, measuring, wrapping and label layout | `app/src/lib/canvas/renderText.ts` |
| Versioned document model and coordinate helpers | `app/src/lib/canvas/model.ts`, `app/src/lib/canvas/geometry.ts` |
| Style fields, which element types take them, inspector sections | `app/src/lib/canvas/style.ts` |
| Contextual inspector sections | `app/src/components/canvas/inspector/` |
| Shape labels: commit, grow rule, restyling | `app/src/lib/canvas/labels.ts` |
| Bindable outlines | `app/src/lib/canvas/outline.ts` |
| Arrow binding, end placement, following | `app/src/lib/canvas/bindings.ts` |
| Element edits as replaceable maps | `app/src/lib/canvas/changes.ts` |
| Layer order moves | `app/src/lib/canvas/layers.ts` |
| Clipboard envelope and clone with remapped bindings | `app/src/lib/canvas/renderClipboard.ts` |
| Retained Rust scene and WebAssembly bridge | `app/canvas-core/src/`, `app/src/lib/canvas/engine.ts` |
| Native file commands and scoped paths | `app/src-tauri/src/library/files/canvases.rs` |
| WebAssembly build | `app/scripts/build-canvas.mjs` |
| Format, geometry, labels, style, bindings, layer order, WASM parity and save-session tests | `app/tests/lib/canvas/` |

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
| `startBinding`, `endBinding` | `{elementId, mode: 'center'}` or `{elementId, mode: 'edge', anchor}` with `anchor` x/y in 0..1 of the target’s bounds | arrow |
| `text`, `fontSize` on rectangle, ellipse, diamond | the shape’s label | shapes |

A binding’s target ID is not checked at load, so a dangling binding never
blocks opening a file; its end stays where it is.

## The pointer stays local and Rust answers geometry queries

The WebView owns input and Canvas2D drawing. Gesture previews live in refs
and an overrides map; a requestAnimationFrame coalesces repaint requests.
The committed elements array changes at a gesture boundary. A cached Path2D
per immutable element avoids rebuilding its path during panning.

The Rust core is compiled to WebAssembly and loaded with the editor. A
retained BVH indexes the scene for hit testing and viewport culling; Rust
also simplifies completed freehand strokes. Geometry is packed into numeric
buffers once per committed scene, with scalar coordinates for subsequent
queries. There is no Tauri round trip for a pointer event. Rust scene storage
is capped at eight scene arrays independently of undo history.

Each shape is a record of 10 floats (`engine.ts`, documented on
`CanvasScene::new` in `app/canvas-core/src/lib.rs`): kind, x, y, width, height,
strokeWidth, filled, point offset, point count, `heads`. `heads` is a bitfield:
1 for a head at the first point, 2 at the last, so Rust tests a head only at
the ends that draw one. `filled` is true for a hollow shape that has a label,
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
in-flight write are drained before Saved is published. The debounce batches
completed changes rather than sending every pointer point to disk.

A recovery copy in localStorage records the document and its base disk text.
It is validated before restore. When the disk has changed independently,
the editor offers Use recovery copy or Keep disk version; recovery never
silently overwrites a newer file. Invalid recovery stays recoverable until
explicitly discarded. Save errors keep the draft and show Retry save.

Rename flushes pending edits before moving the file. Unmount and pagehide
flush the session; a clean session is released after its final pane unmounts,
so reopening reads disk again. Delete runs through the same queue and marks
the session deleted, preventing a late autosave from recreating the file.

Native writes stage a sibling temporary file and rename it into place after
fsync. File commands accept only the four-part subject canvas path and reject
symlinks in its components. Canvas paths use their own guard; note mutation
commands remain scoped to notes.

## Tools and exports share the same geometry

The editor supports selection, marquee, move and resize; rectangle, ellipse,
diamond, line, arrow, freehand and text tools; an eraser; undo/redo;
duplicate and copy/paste; layer order; grid, pan, zoom and fit. Line and arrow endpoint
handles edit their endpoints.

The canvas root scopes shortcuts and participates in native Edit menu
routing. A visible note editor must not claim undo while a canvas owns focus.
Text inputs keep their own native editing behavior. Drawing gestures capture
the active pointer and suppress WebKit text selection.

PNG and SVG export cover the whole drawing with padding, excluding editor
chrome, selection, binding hints and grid. The native export command opens a
Save dialog and writes only the path selected there. SVG text and attribute
values are escaped. PNG export caps its dimensions to bound bitmap allocation.
Canvas2D, SVG and hit testing all take dash lengths (`strokeDash`) and
arrowhead outlines (`arrowheadShape`) from `geometry.ts`.

## A shape holds its label as text

Rectangle, ellipse and diamond carry a label in `text`. It is written from
four places, all opening the same overlay textarea on the label box:
double-click a shape (including the inside of a hollow one), click a shape
with the Text tool, Enter with one shape selected, and Enter or double-click
on standalone text. The shape stays selected while its label is edited so the
inspector shows its text options. Escape cancels; Cmd/Ctrl+Enter or blur
commits. An empty label removes `text` and keeps the shape; empty standalone
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
type, and `inspectorSections` derives the panel from it: the union of
sections that apply to any selected element (or to the active tool’s next
element when nothing is selected, or to the element being written in).
Sections are Color or Stroke, Fill, Font and Align, Stroke width, Stroke
style, Sketch, Arrowheads and Opacity, in a fixed order, then Layer whenever
something is selected. A shape’s text
options appear only once it has a label or one is being written. Each control
reads its value from the first element that shows it.

A style patch writes only the fields that apply to each selected element and
change what it draws; a patch that changes nothing commits nothing and makes
no undo step (`patchElementStyle`, `stylePatchChanges`). A font size or family
change re-measures standalone text and re-runs the grow rule on labelled
shapes (`restyleElement`). A patch also becomes the default for the next
element of those kinds, and selecting an element adopts its look as the
default. Opacity previews live while dragging and commits one undo step on
release.

The panel is `CanvasStylePanel.tsx`, which only places it: a card on wide
panes and a popover behind a Style button on narrow ones, scrolling inside
instead of overflowing. One file per section lives in
`app/src/components/canvas/inspector/` and `InspectorControls.tsx` assembles
them. Arrowhead glyphs are drawn by `arrowheadShape`, so the picker previews
exactly what the arrow gets. The panel keeps focus in an open text overlay so a style change restyles the
text instead of committing it.

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
whose other end is inside the shape stays at the centre. Arrows stay two-point.

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

## Arrowheads are one geometry for every renderer

An arrow has an independent head at each end: `none`, `arrow` (open chevron),
`triangle` and `diamond` (filled with the stroke colour), `dot` and `bar`.
`arrowheadShape` returns the outline, wing pair and size for a head. Size is
`min(16, segment length / 3)`, which keeps every head inside Rust’s 16-unit
arrow culling pad. Heads draw solid even on a dashed line. Hit testing covers
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
- Route every element edit through `previewChanges` / `commitChanges`; writing `elements` directly leaves bound arrows detached from a moved target. Layer order is the one exception (`reorder`), since it moves no element.
- Canvas2D font names must match the loaded `"Inter Variable"` family (`CANVAS_FONT_STACKS`); a wrong name measures and wraps with a fallback font, and the label cache is dropped on `document.fonts` `loadingdone` for the same reason.
- Keep `CANVAS_FONT_STACKS` in step with `theme.css`; drift makes the canvas, the overlay textarea and SVG export disagree on wrapping.
- A hollow labelled shape counts as filled for hit testing in both the JS reference and the WASM record; change one and the parity test fails.
- Add a new optional field to `STYLE_KEYS` and `KEY_TYPES` in `style.ts` as well as to `validate` in `model.ts`, or the inspector cannot patch it.
- Unknown format versions must fail before writing; partial loading must never erase unfamiliar data.
- Rename and delete through the shared session so pending writes cannot target an obsolete path.
- Native menu shortcuts need menu routing (`editRouting.ts`, `backForwardRouting.ts`): macOS gives the menu its accelerators first, so the canvas's keydown never sees ⌘Z, ⌘[ or ⌘].
- Reset the pending frame sentinel when cancelling RAF during effect cleanup, or StrictMode replay cannot schedule a paint.
- Geometry benchmarks do not measure Canvas2D painting or end-to-end interaction latency.
