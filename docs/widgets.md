# Chat widgets

A ` ```widget ` fence in a reply is a small TSX program the agent wrote for one
question, run in a sandboxed frame inside the reply and built from the Oculus
kit so it looks like the app. ` ```openui ` explains inline with charts,
small layouts and bound controls, and shows live student data, proposals,
jobs and quizzes that feed reviews
([harness.md](./harness.md#openui-fences-draw-interactive-study-widgets));
a widget illustrates a picture with its own rules.
The agent learns to write one from the `oculus-widgets` skill and checks it
with `oculus widget check` ([below](#the-agent-learns-widgets-from-oculus-widgets)).

## Where

| Piece | Location |
| --- | --- |
| Fence routing and numbering | `app/src/components/markdown/MdComponents.tsx` (`pre`), `app/src/components/markdown/openui/remarkWidgets.ts` |
| The fence in a reply | `app/src/components/markdown/widget/` (`WidgetFence.tsx`, `LiveWidget.tsx`, `WidgetBar.tsx`, `WidgetExpand.tsx`) |
| A fence while its reply streams | `app/src/components/markdown/widget/stream/` (`repair.ts`, `preview.ts`, `schedule.ts`, `usePreview.ts`, `gate.ts`, `context.ts`), `app/widget-kit/runtime/` (`stream.tsx`, `preview.ts`, `mode.ts`, `Pending.tsx`), `app/src/components/harness/timeline/rows.ts` (`replyKey`, `streamingReply`) |
| The frame's document | `app/src/components/markdown/widget/srcdoc.ts` |
| Messages from a frame | `app/src/components/markdown/widget/messages.ts`, `app/src/components/markdown/widget/useWidgetFrame.ts`, `app/widget-kit/protocol.ts` |
| Telling a frame it is in view | `app/src/components/markdown/widget/useFrameInView.ts`, `app/widget-kit/runtime/visibility.ts` |
| Transpile and kit URL | `app/src/components/markdown/widget/transpile.ts`, `app/src/components/markdown/widget/kit.ts` |
| Saved state, and each widget's Allow and use count | `app/src/stores/chat/widgetStore.ts` |
| Save to Revision, and a saved widget on the Artifacts page | `app/src/components/revision/SaveWidget.tsx`, `app/src/lib/revision/artifacts.ts`, `app/src/components/revision/ArtifactBody.tsx`, `app/src-tauri/src/db/revision/mod.rs` |
| A widget's requests (`oculus/ask`): the gate, Allow and Stop, the call | `app/src/components/markdown/widget/ask/` (`gate.ts`, `limits.ts`, `useWidgetAsk.ts`, `AskLine.tsx`), `app/widget-kit/ask/`, `app/src-tauri/src/widgets/ask/` |
| The kit's source | `app/widget-kit/` (`ui/`, `state/`, `2d/`, `math/`, `3d/`, `ask/`, `runtime/`, `tex/`, `colors.ts`, `three.ts`, `fiber.ts`, `kit.css`) |
| The releasing canvas | `app/widget-kit/3d/canvas/` |
| The kit's contract | `app/widget-kit/widget.d.ts` |
| The kit's build | `app/vite.widget-kit.config.ts`, `app/scripts/build-widget-kit.mjs` |
| Transpiler | `app/src-tauri/src/widgets/transpile.rs` |
| `oculus widget check` and its lint | `app/src-tauri/src/widgets/check/` (`mod.rs`, `literals.rs`, `sandbox.rs`, `cleanup.rs`), `app/src-tauri/src/bin/oculus/commands/widget.rs`, `app/src-tauri/src/bin/oculus/args/widget.rs` |
| The agent's skill and gallery | `app/src-tauri/templates/skills/oculus-widgets/` (`SKILL.md`, four `.tsx` examples), registered in `app/src-tauri/src/agents/docs.rs` with `app/widget-kit/widget.d.ts` |
| The gallery's tests | `app/src-tauri/src/agents/tests.rs` (`widget_gallery`), `app/tests/widget-kit/gallery.test.tsx`, `app/tests/fixtures/widget-gallery/` |
| Kit route | `app/src-tauri/src/widgets/kit.rs`, `app/src-tauri/src/lectures/media.rs` |

## A widget fence is a module with a default export

The fence holds one TSX module. Its first line is `// title: …` (the card's
title; "Widget" without one), and its default export is a component taking no
props. It may import only `react`, `react/jsx-runtime`, the six kit
modules, `oculus/ui`, `oculus/state`, `oculus/2d`, `oculus/math`,
`oculus/3d` and `oculus/ask`, and `three` and `@react-three/fiber` as published (no
`three/addons/…`, no drei). `widget.d.ts` types the kit modules; three and
r3f keep their own types.

`remarkWidgets` numbers ` ```widget ` and ` ```openui ` fences with one
counter, so a reply's `widgetKey(thread, row, fence)` never collides between
the two kinds. `MD_COMPONENTS.pre` hands a widget fence to `WidgetFence`
from its opening line, so it never shows as code.

## OpenUI fences draw while their reply streams

OpenUI mounts its runtime from the opening fence and fills partial cards with
shimmer rows. Its store survives source deltas, and controls stay inactive
until the fence is closed and the reply commits. Final parse or render
failures show a one-line card with **Show source** and **Ask to fix**; the
request includes the original `openui` fence through `fixRequest` in
`app/src/components/markdown/widget/fixRequest.ts`. The lifecycle and
validation contract are in
[harness.md](./harness.md#openui-fences-draw-interactive-study-widgets).

## OpenUI reads local student data and illustrates short lessons

Use OpenUI for a small chart or seeded simulation beside the prose it explains,
a standalone `Question` that reveals its explanation after a choice, or the
student's live data. A widget illustrates something that needs its own rules
(geometry, physics, a game); it cannot read the app's database. OpenUI's
`Query` reads local rows through the same data layer as the Live components,
using the CLI's JSON field names, and refreshes after edits. It never makes a
network request or a write; changes still require proposal cards. Query
failures stay in the fence with Retry. Tool names, arguments and simulation
limits live in [harness.md](./harness.md#openui-queries-read-local-rows-and-refresh-after-edits).

## A widget draws while it streams

From its opening line a ` ```widget ` fence is its card: the `// title:`
shimmering, "Building…" beside it and a shimmering block where the widget
will be. In a chat reply that belongs to a run, the card then previews the
program as it grows, in the one frame the committed reply keeps. Markdown
outside a chat row (a library file) and a turn with no run (one the agent
started itself) show only the card, "Building <title>…".

- **One frame per fence, booted once.** `Timeline` draws a reply streaming
  a widget fence as the row it will become: the same `Reply` under the key
  its committed row takes (`replyKey(run, ordinal)` from `streamingReply`
  in `app/src/components/harness/timeline/rows.ts`), so at commit React keeps
  the row, the `LiveWidget` and its iframe
  ([harness.md](./harness.md#the-timeline-never-re-renders-per-token)).
  `WidgetStream` (thread and run) tells the fence it streams; `WidgetRow`
  takes over at commit. The document is built with no program
  (`buildSrcdoc` with `js: null`), and the runtime's `stream` waits for
  programs posted as `module` messages.
- **A preview is a repaired prefix.** `usePreview` tries at a pause in the
  stream (200 ms quiet, or a second after the last try if it never pauses),
  one transpile in flight (an IPC round trip of about 24 ms), never the same
  text twice (`stream/schedule.ts`). `previewSource` takes the complete
  lines once there is an `export default`; `closePrefix` (`stream/repair.ts`)
  appends what makes them parse: it ends a string, template or comment,
  finishes an open JSX tag, then closes every open bracket and JSX element in
  the order opened. It never edits the prefix or guesses past it, and refuses
  a cut inside a `for`, `while` or `do` body, which could leave a loop
  nothing can stop. Previews bypass the transpile cache.
- **Parts not written yet are placeholders.** `widget_transpile` returns
  `pending`, the components drawn as JSX but declared nowhere, and the
  preview declares each as the kit's shimmering block (`runtime/Pending.tsx`).
  A root written first therefore draws early, so `oculus-widgets` asks for
  the default export first and helpers after, and its gallery is in that
  order. A helper function not written yet throws, which keeps the last good
  preview.
- **A hot update, not a reload.** `moduleCode` (`stream/preview.ts`) makes
  each preview a module that hands its component to the runtime's `update`,
  run as an inline module script with the document's nonce, so the policy
  is unchanged. `update` renders it into the same React root under a fresh
  error boundary. `oculus/state` lives outside the tree, so values kept by
  name carry over; plain `useState` starts over. A preview that throws, at
  its top level, in render or in an effect, puts the newest good preview back
  (the runtime keeps three). While the frame streams, the guard keeps throws
  and rejections to itself (`window.__oculusQuiet`) but still reports a file
  that won't load or a CSP block.
- **A preview's work is capped.** Its module shadows `setTimeout`,
  `setInterval`, `requestAnimationFrame` and their cancels with timers scoped
  to it (`runtime/preview.ts`): the next program cancels whatever an older one
  left running, and a scope stops once its callbacks have run for 1 s in all.
  A 3D canvas an update unmounts drops its context at once.
- **Nothing acts while it streams.** The frame refuses `oculus/ask`'s `ask`
  and `send` before posting them and doesn't post its state; the parent
  refuses both too (`stream/gate.ts`), since the frame is untrusted. The
  frame sits in an `inert` box at 60% opacity, so nothing in it takes a click
  or focus, and Source, Expand, Save to Revision, the Allow line and errors
  are absent.
- **The commit is one more message.** The row keeps the `LiveWidget`, which
  transpiles the whole fence and posts it as the `final` module. Its first
  statement is `commit`: older previews' timers stop, the gates open, and
  from there it fails like any widget (the parent first forgets what the
  previews reported). A final program that doesn't compile shows its error
  and drops the frame. A later rebuild (a theme change) boots the program
  inline like any committed widget.

Measured on the student's widgets, a preview first draws at about half the
fence's lines, then nearly every line; the plan's spike has the numbers. A
reply that a later tool call folds into its run's work moves its fence to the
run's widget row ([harness.md](./harness.md#a-runs-work-folds-above-its-answer)),
which builds a frame of its own.

A committed fence goes through `widget_transpile` (cached by source in
`transpile.ts`, errors included), then `buildSrcdoc`, into an iframe with
`sandbox="allow-scripts"`. Never `allow-same-origin`: the widget would get the
app's origin and the Tauri IPC.

## The sandbox lets in the kit and nothing else

`buildSrcdoc` writes, in order: the CSP, `color-scheme`, the guard script, the
import map, `kit.css`, the theme's tokens as `--color-*` properties, the saved
state as `<script type="application/json" id="oculus-state">`, `#root`, and
the widget as one inline module script. The policy is the minimum measured to
run every part of the kit:

```
default-src 'none'; script-src <kit> 'nonce-<n>' 'wasm-unsafe-eval';
style-src <kit> 'unsafe-inline'; font-src <kit> data:; img-src data: blob:;
connect-src <kit>
```

`<kit>` is the kit's own path (`http://127.0.0.1:<port>/widget-kit/`), not the
media server's whole origin. Every inline script carries a fresh nonce.
`'wasm-unsafe-eval'` is for the maths engine and `connect-src` for fetching its
wasm; `style-src 'unsafe-inline'` because maths output sets `style=`. There is
no `'unsafe-eval'`, no network beyond the kit and no CDN: the app is offline.
A model call from a widget is relayed by the parent
([`oculus/ask`](#oculusask-asks-a-model-or-the-chat-once-the-student-allows-it)),
so the policy has no host for it.

A module can't read its own default export, so the transpiler rebinds it
(see [Rust side](#rust-side)): the code holds the component in the top-level
`__oculusWidget` and has no `export default`. `DEFAULT_BINDING` in
`transpile.rs` and `DEFAULT_BINDING` in `widget-kit/protocol.ts` name it, and
an appended line imports `mount` from the kit's `runtime.js` and passes it that
binding. `escapeScript` rewrites `</script` and `<!--` in the module; the JSON
blocks escape every `<`.

A theme or accent change rebuilds the document with the new tokens; the
state survives through `widgetStore`.

## A frame talks to its reply only through postMessage

The frame posts `{ type: "oculus-widget", kind, … }` to its parent:

| `kind` | Fields | Sent by |
| --- | --- | --- |
| `height` | `height` (CSS px, rounded up) | the runtime, whenever `#root` resizes |
| `state` | `state`, the whole object | the runtime, 200 ms after the last change |
| `error` | `phase` (`kit`, `csp`, `runtime`, `render`), `message` | the guard script, or the runtime's error boundary |
| `overflow` | `width`, `frameWidth` | the runtime, when content is wider than the frame |
| `ask` | `id`, `prompt`, `schema` (an object) | `oculus/ask`'s `ask` |
| `send` | `id`, `text` | `oculus/ask`'s `send` |

`readWidgetMessage` accepts a message only when its source is that frame's
own `contentWindow` and its shape is exactly one of these. Each frame element
is keyed by its document, so a replaced frame's late messages match nothing.
The parent posts back `{ type: "oculus-host", kind, … }`:

| `kind` | Fields | Sent |
| --- | --- | --- |
| `state` | `state` | when the expanded view closes; the runtime applies it without posting it back |
| `pause` | — | when the frame leaves the view or the expanded copy opens over it |
| `resume` | — | when it is in view again ([below](#a-3d-scene-releases-its-context-off-screen)) |
| `reply` | `id`, then `ok: true` and `value`, or `ok: false` and `error` | the answer to the frame's `ask` or `send` with that `id` |
| `module` | `id` (increasing per frame), `code`, `final` | a streaming frame's next program ([above](#a-widget-draws-while-it-streams)); nothing follows the `final` one |

The runtime takes a host message only from `parent` and only in exactly one
of these shapes (`runtime/host.ts`); `pause` and `resume` go to the
visibility store, which a frame without a 3D scene never reads, `reply`
to `oculus/ask`'s waiting requests, and `module` to a streaming frame's
`stream` (a frame booted with its program ignores it). A request `id` is 1–64 letters, digits,
`_` or `-`, made in the frame; the parent only echoes it.

The guard is a classic inline script that runs before any module. It reports
a script or stylesheet that failed to load, an uncaught throw (handlers,
timers), an unhandled rejection and a CSP violation, and on `load` reports a
kit that never evaluated (`runtime.js` sets `window.__oculusKit`). That is how
a missing kit is caught without a timeout. While a streaming frame shows
previews it reports only the failed loads and CSP blocks
([above](#a-widget-draws-while-it-streams)).

## The frame is exactly its content's height and never scrolls

`html` and `body` are `overflow: hidden`; `#root` is `display: flow-root` so
its box holds its children's margins, and its height is what the runtime
reports. The parent sets the iframe to that height, with no minimum and no
maximum — except past `WIDGET_HEIGHT_BUDGET` (560 CSS px), where the frame is
clipped at the budget and the widget shows an overflow error. Content that
needs more room goes on `Pages`. Before the first report the card shows a
spinner; the last height per source is kept so a remount starts at its size.

Expand opens a dialog with a second frame at the inline frame's width (so it
lays out the same) and its full reported height, scaled with a CSS transform
to fit the window. A transform, not CSS `zoom`, keeps pointer input inside the
frame exact.

## State is saved per fence and seeds the next document

`useWidgetState(key, initial)` reads and writes one flat JSON object. Every
change is posted whole, debounced, and `LiveWidget` saves it with
`widgetStore.save(widgetKey, source, state)` — the same store, keys and pruning
as OpenUI. A document is built from the last state the frame reported, else
from the saved entry when its `source` is this fence's source; changed source
starts fresh. The document reads the state when it is built and never
subscribes, so a save never reloads the frame.

## A running widget can be saved to Revision

Once a widget runs — its first height report, and no error — its title bar
shows **Save to Revision** on hover or keyboard focus, as an OpenUI fence
shows it under the widget. It asks for a title (the widget's to start) and
writes a `revision_artifacts` row with `format` `widget`, the fence's TSX as
`program`, and the widget's last state. Only the state: the entry's Allow and
use count stay with the fence (`entryState`), so a saved widget asks the
student again. While the reply streams there is nothing to save.

Rust's writer (`db/revision/`) compiles a widget program with `transpile`
before it writes, for the app and `oculus artifact save --format widget`
alike, so a widget that can't run is never saved. An update that changes
`program` or `format` is checked against the row it produces.

The Artifacts page draws a widget artifact through `ArtifactBody` with
`LiveWidget`, under the store key `artifact:<id>` (`artifactWidgetKey`):
`prune` never reaches it, and deleting the artifact drops it. The widget is
seeded once from the row's `state` (`savedState`) and hands each change to the
page (`onPersistState`), which writes it to the row 400 ms after the last one
and on unmount; the store keeps only its grant. The page has no thread, so
`send` is refused with the no-thread reason and there is no Ask to fix; `ask`
runs after Allow, as in a reply. The frame is its content's height there too:
the page scrolls, never the frame.

## Every failure is one line with Ask to fix

A transpile error (a missing default export included), an unreachable kit, a
runtime or render error, and vertical or horizontal overflow each show as one
line under the title in the destructive tone. **Ask to fix** sends
`The widget "<title>" failed: <error>. Fix it and resend the widget.` and the
fence to the thread through `useAssistantSend`; it is hidden while a turn runs
and outside a conversation. The TSX is behind the **Source** toggle, drawn as
the fence's code block.

## The kit is ES modules sharing one React

`vite.widget-kit.config.ts` builds one entry per import-map name — `react.js`,
`jsx-runtime.js`, `ui.js`, `state.js`, `2d.js`, `math.js`, `3d.js`,
`ask.js`, `three.js`, `fiber.js` — plus `runtime.js`, into `app/dist/widget-kit/`.
Entries keep fixed names (the import map names them) and share hashed
chunks, so the widget's `react` is the same module instance as the kit's and
`react-dom`'s, and its `three` the same as `oculus/3d`'s and r3f's. React is
CommonJS, so `react.ts` and `jsx-runtime.ts` re-export every name explicitly;
three is ES modules, so `three.ts` is `export *`.

| Module | What it holds |
| --- | --- |
| `oculus/ui` | `Widget`, `Row`, `Column`, `Pages`, `Tabs`, `Slider`, `Toggle`, `Select`, `Field`, `Pill`, `Value`, `Tex`, `Note` |
| `oculus/state` | `useWidgetState` |
| `oculus/2d` | `Plane`, `Point`, `MovablePoint`, `Vector`, `Line`, `Segment`, `Circle`, `Arc`, `Angle`, `Polygon`, `Plot`, `Label` |
| `oculus/math` | mathjs (`parse`, `compile`, `evaluate`, `derivative`, `simplify`, complex numbers, matrices, `eigs`, …), `math`, `formula` |
| `oculus/3d` | `Scene3D`, `Axes3D`, `Sphere`, `Arrow3D`, `Arc3D`, `Curve3D`, `Surface`, `Label3D` |
| `oculus/ask` | `ask`, `send`, `useAsk` |
| `three` | three.js 0.186, whole |
| `@react-three/fiber` | r3f 9.8, with the kit's releasing `Canvas` in place of its own |

- **Styled from injected tokens.** `kit.css` names no colour of its own; the
  parent writes the applied theme's `--color-*` values (`tokens.ts`) into the
  frame, dark included. Fonts are Inter and Manrope copied into `fonts/`.
- **`Pages` and `Tabs` stack every panel in one grid cell**, inactive ones
  `visibility: hidden` and `inert`, so the frame is the tallest panel's height
  from the start and switching never resizes it.
- **`Tex` loads the maths engine lazily**: its chunk fetches `math-core`'s wasm,
  compiles it and calls `initSync`, and links KaTeX's stylesheet from
  `katex/` beside `kit.css`. The build copies the stylesheet and its fonts there.
- **`Tex` and `Label` share one engine load** (`tex/load.ts`), so the wasm
  and stylesheet are fetched once per frame whichever typesets first.
- **`Field` never focuses itself**, so a widget mounting can't take the
  keyboard from the composer.
- `Select` is a native `<select>`; there are no date inputs.

`bun run widget-kit` builds the kit. `bun run build` runs it after `vite build`,
which empties `dist/`; the dev preflight runs it when its inputs changed
(`build-widget-kit.mjs`, never fatal). Rust needs no rebuild after a kit
rebuild in debug.

### `oculus/2d` is Mafs drawn in the app's tokens

`2d/` wraps Mafs (an npm dependency) where Mafs has the piece; `Arc`,
`Angle` and `Label` are the kit's own. `2d.js` is a separate entry, so a
widget that never imports it never fetches it.

- **Width is measured, height is fixed.** `Plane` is its column's width
  (`clientWidth`, through a `ResizeObserver`) and `height` CSS px (default
  320) from the first frame; Mafs gets a number, so it never measures itself.
  `PlaneView` is the same plane at a given width, which the tests render.
- **No zoom, no scroll hijack.** Mafs' `zoom` is always off, so no wheel or
  pinch handler is bound and a wheel over the plane scrolls the chat; `pan`
  is opt-in. `.MafsView` takes `touch-action: auto`; only a movable point
  takes the pointer.
- **Colours are token names.** Every colour prop is a `Colour` (`colors.ts`, shared with `oculus/3d`):
  `foreground`, `muted`, `brand`, `success`, `warning`, `danger`, `chart-1`…
  `chart-5`, drawn as `var(--color-…)`. Anything else throws, so a hex shows
  as a render error with Ask to fix, never on screen.
- **Mafs' stylesheets are not loaded.** `core.css` names its own colours and
  `font.css` is CMU Serif; `kit.css` redraws the rules Mafs needs with its
  `--mafs-*` variables mapped to the tokens (grid `border`, axes and ticks
  `muted-foreground`, background `card`), in the kit's fonts. Ticks use a true
  minus sign.
- **KaTeX is aliased away.** Mafs imports `katex` at its top level; the kit
  build resolves it to `2d/katex.ts`, whose `render` shows the TeX muted while
  the engine loads and draws the last TeX asked of the element once it is in.
  `Label` doesn't go through it: it typesets through `tex/load.ts` itself, in
  a fixed-position `foreignObject` (what WebKit needs to place HTML inside the
  SVG), centred on its point or beside it on the side `attach` names.

### `oculus/3d` is three.js through react-three-fiber, z up

`3d/` draws on r3f; `three.js` and `fiber.js` share its chunks, so a widget
mixing kit parts with raw `<mesh>`es and three's classes has one three.

- **Maths coordinates.** z is up (the camera's `up` is `[0, 0, 1]`) and a
  point is a `Vec3`, `[x, y, z]`.
- **`Scene3D` is sized like `Plane`**: its column's width (`clientWidth`,
  through a `ResizeObserver`) and `height` CSS px (default 320), its box at
  full size before anything draws. Inside, r3f measures with offset sizes,
  so page zoom can't scale the canvas past its box. The background is
  transparent, so the card shows; tone mapping is off, so a token's colour
  is drawn as it is; an ambient light and a key light that follows the
  camera light every face the student turns to.
- **No zoom, no scroll hijack.** OrbitControls (three's own, not drei)
  orbits about `target`; `zoom` is off unless asked, and OrbitControls
  calls `preventDefault` only on a wheel it zooms with, so a wheel over the
  scene scrolls the chat. `pan` is opt-in.
- **Colours are token names**, as in `oculus/2d`. `3d/colour.ts` reads the
  token's `--color-…` from the frame's root and normalises it through a 1×1
  canvas (a token may be in a syntax `THREE.Color` can't read), once per
  token: a theme change rebuilds the frame.
- **Lines are tubes.** WebGL line widths are 1px in WebKit, so curves, arcs,
  rings and axes are tube meshes (`geometry/tube.ts`), drawn unlit in the
  token's colour like the 2D strokes; spheres and surfaces are lit.
  `weight` multiplies a part's default thickness (a line is about 2 CSS px
  at the starting view), scaled by the starting camera's distance.
- **`Label3D` is DOM.** It registers with its canvas's label layer, a
  `pointer-events: none` overlay that typesets through `tex/load.ts` and
  moves each label every frame by projecting its point (the transform is
  written straight to the element, no React render), so a label always
  faces the student and hides behind the camera.
- The geometry (`geometry/`: arcs, curve pieces, surface grids, sphere
  rings, tubes, arrow heads) is plain arrays, tested without WebGL.

### A 3D scene releases its context off-screen

WebKit loses WebGL contexts once about sixteen are live, and the lost one is
not reliably the oldest; the timeline keeps off-screen rows' frames attached
(`useRowVisibility`), so nothing else frees them.

- **The parent observes each inline frame** (`useFrameInView`: an
  `IntersectionObserver` rooted at the chat's scroll parent, margin
  `200px 0px`, re-hung on the new element when a theme change rebuilds the
  document). A frame with zero height is out: it has nothing to draw, and
  frames stacked before their first real height all intersect. Leaving
  counts at once; entering counts only once the frame has stayed in for
  `IN_VIEW_SETTLE_MS` (150 ms), so a frame passed in a fast scroll never
  creates a context. An observer doesn't call back when a fully visible
  frame only grows, so a `ResizeObserver` re-observes the frame whenever
  its height changes, for an entry with its real geometry. The frame is live while it is in view and the
  expanded copy is closed — the observer can't see the dialog covering it,
  and the dialog's frame is a second context. The expanded frame is always live.
- **A handshake per document.** `useWidgetFrame` posts the flag once the
  document's first `height` report shows its runtime is listening (a message
  before `mount` is lost), then only when it changes (`visibilityToPost`).
  Every frame gets it; only a 3D scene reads it.
- **Nothing before `resume`.** The kit's `Canvas` mounts r3f's only while the
  store says live. On `pause` it renders once more, reads the canvas as a
  PNG in the same task (no `preserveDrawingBuffer`; PNG keeps the
  transparency), puts the still in the canvas's place, calls the renderer's
  `dispose()` and `forceContextLoss()` itself, and unmounts r3f's canvas.
  On `resume` it mounts it again and drops the still after the first frame.
  The box never changes size, so the frame's height doesn't either. An
  unmounted `Canvas` (a streaming preview replaced) drops its context at
  once and expects the loss; r3f's own drop, 500 ms later, would read as a
  lost context.
- **What survives.** React state above the canvas does; state inside its
  tree starts over, so it belongs in `useWidgetState`. `Scene3D` keeps the
  orbited camera and target outside the canvas and restores them; labels
  stay over the still at their last positions.
- **Every `Canvas` releases.** `fiber.js` exports the kit's `Canvas` in place
  of r3f's, so a widget writing raw r3f is covered too (it is 320 px tall
  unless its `style` sets a height); a renderer made by hand
  (`new WebGLRenderer()`) is not.
- **A context lost any other way surfaces.** It posts a `runtime` error
  (one line under the title), the box stays empty at its size with the labels
  where they were, and the scene is recreated on the next `resume`.

### `oculus/math` is a mathjs subset

`math/instance.ts` builds mathjs with `create()` from the factories it lists:
parsing and evaluation, `derivative`, `simplify`, complex numbers, matrices
and `eigs`, arithmetic, roots, exp/log, every trig and hyperbolic function
and its inverse (each one `derivative` can write into its result), and `pi`,
`e`, `tau`, `i`. A function outside the list adds its whole dependency
closure to `math.js`, so add one only when a widget needs it.

Nothing in mathjs evaluates a string as JavaScript: `parse`, `compile`,
`evaluate` (function definitions included), `derivative`, `simplify`, `eigs`
and `toTex` all run under the frame's CSP with no `'unsafe-eval'`. Only a
widget's own `eval` or `new Function` is refused.

`widget.d.ts` types a parsed `Node`'s `type`, `op`, `name`, `args` and
`content` (mathjs sets them), so a widget can read which rule an expression
calls for, as `derivative-steps.tsx` does.

`formula(text, variables)` turns what a student typed into
`{ ok: true, fn, tex }` or `{ ok: false, error }` and never throws. A name
that is neither a variable, a constant (`pi`, `e`, `tau`, `i`) nor one of the
maths functions in `FUNCTIONS` is an error, so the parser's own functions
can't be called from a formula. `fn` returns `NaN` for anything that isn't a
real number, which a plot skips, joining the points either side.

## `oculus/ask` asks a model or the chat once the student allows it

A widget that can't work without a model gets two calls, typed in
`widget.d.ts`:

- `ask<T>(prompt, schema)` — one tool-less turn on the **Widget questions**
  job (`widgetAsk`), answering a JSON value that matches `schema`.
- `send(text)` — `text` as the student's next message in the chat, through
  the thread's `useAssistantSend`, the path Ask to fix takes.
- `useAsk()` — both, with `busy` and the last `error`; its calls never
  reject (a failure resolves `undefined` and sets `error`).

Each call posts an `ask` or `send` with an id it made (`widget-kit/ask/`,
`pending.ts`) and waits for the `reply` with that id. `ask.js` and
`runtime.js` share one `pending` chunk, so the runtime's listener settles the
same map the widget waits on. Arguments that aren't a string, an object or
JSON reject in the frame, before anything is posted.

**Everything is decided in the parent, never in the frame.** `useWidgetAsk`
(one per `LiveWidget`, shared by its expanded copy, which posts with the same
store key) runs each request through `gate`, in this order:

1. One at a time: a request while another runs, or waits for Allow, is
   refused.
2. Sizes: a prompt of at most 20,000 characters, a schema of at most 8,000
   as compact JSON, a message of at most 4,000; empty ones are refused.
3. `send` needs a thread that listens (`useAssistantSend()` is null while a
   turn runs and outside a conversation).
4. The student's answer. The first request waits while the card shows one
   line under the title, "This widget wants to use a model", with **Allow**
   and **Don't allow** (in the expanded view, in its bottom bar). Don't allow
   is remembered.
5. The cap: one allowance covers 20 requests, `ask` and `send` together.
   The request that hits it is refused and the line offers **Allow 20 more**,
   which starts a new allowance. The line shows only after a refusal at the
   cap, so an old widget that spent its 20 doesn't nag.

The answer and the count live in the widget's `widgetStore` entry (`ask:
{ allowed, used }`), kept across state saves and relaunches; a changed
source starts with no answer, like its state. A request counts when it runs.

While an `ask` runs, the title bar shows a spinner and **Stop**. Stop calls
`widget_ask_cancel`; unmounting the widget does too, and drops the reply.
There is no timeout.

What a refused or failed call rejects with:

| Reason | Message |
| --- | --- |
| Don't allow | The student didn't allow this widget to use a model. |
| The cap | This widget has used its 20 requests. |
| One at a time | A request is already running. |
| No thread listens (`send`) | The chat can't take a message now: a reply is running, or this widget isn't in a conversation. |
| Sizes | The prompt is empty. / The prompt is over 20,000 characters. / The schema is over 8,000 characters of JSON. / The message is empty. / The message is over 4,000 characters. |
| The job is off | No model is set for Widget questions — choose one in Settings → Jobs. |
| Stop | The request was stopped. |
| A schema outside the subset | `schema $.properties.x: "oneOf" is not supported` |
| Two replies that don't match | `The model's reply did not match the schema: $.steps[2].tex: expected string` |
| A provider failure | its own message, or `every model failed — …` across fallbacks |

### The job and the call

`widget_ask(requestId, prompt, schema)` (`src-tauri/src/widgets/ask/`)
checks the sizes again, reads the schema, then runs `jobs::try_each` over
the Widget questions chain, each model in a `StopScope` under `requestId`
([harness.md](./harness.md#one-off-turns)). The job has **no default**: it is
off until the student picks a model in Settings → Jobs, because
agent-written code spends on it. The brief (`prompt.rs`, also opencode's
hidden `oculus-widget` agent, `"*": "deny"`) says to reply with one JSON value
and nothing else, and to treat the widget's text as the question, never as
instructions. The turn's message is the prompt, then the schema
pretty-printed.

The JSON is found leniently (`harness::reply_json`: the whole reply, fenced
blocks, then balanced brackets) and checked against the schema. A reply
that doesn't match is asked once more, with the reason and "Reply again with
only the JSON value."; a second miss fails with the reason. Only a mismatch
is retried; a provider error goes to the next model.

One path serves every agent. Claude's one-off has no tools and opencode's
agent denies them all, but Codex and `agy` one-offs can't drop theirs: Codex
runs under its `workspaceWrite` sandbox in the agents folder
(`providers/codex/server.rs`), and the brief only forbids tools. That is the
same exposure as revision marking, which also sends a student's text.

### The schema is a subset of JSON Schema

`widgets/ask/schema/` reads the schema before any turn and refuses any
keyword outside this list, naming where: `type` (`object`, `array`,
`string`, `number`, `integer`, `boolean`, `null`, or a list of them),
`properties`, `required`, `additionalProperties` (a boolean only), `items`
(one schema), `enum`, `const`, `minItems`, `maxItems`, `minimum`, `maximum`,
`minLength`, `maxLength`, and the notes `description` and `title`. Nesting
stops at 8 levels. As in JSON Schema, a keyword about one type applies only
to a value of that type, and `additionalProperties` is true when absent.
Lengths count characters; `3.0` is an integer. A mismatch names its path in
the value: `$.steps[2].tex: expected string`, `$["a b"]: expected number`.

## The agent learns widgets from `oculus-widgets`

The brief's "Showing a picture or a page" (`HARNESS.template.md`) routes by
what the answer needs: ` ```openui ` for inline explanations, recall, live data and
changes, ` ```widget ` for geometry, physics or algorithm illustrations, and
`outputs/` for a static picture or page. It names the skill and stops there,
since every CLI reads the brief into every thread. Its "Diagrams" section
sends "how does X work" about a mechanism, and a cycle, to a widget rather
than mermaid: the route is decided in the brief, and an agent that never
opens the skill never sees the skill's own routing table. `oculus-ui` and
`oculus-revision` point at the same split.

`oculus-widgets` is one skill for every CLI (the skills folder,
[harness.md](./harness.md#the-brief-is-appended-except-on-opencode)). Its
files are `SKILL.md` (routing on the question's verb, drawing the mechanism,
the fence's rules, a checklist whose loop is `oculus widget check`,
`oculus/ask`, Ask to fix, Save to Revision), `widget.d.ts` compiled in from
the kit by `include_str!` so it can't drift, and four unrelated examples:
`bloch-sphere.tsx` (`oculus/3d`), `derivative-steps.tsx` (`oculus/math`,
`Field`, `Pages`), `bubble-sort.tsx` (`oculus/ui`, `oculus/state`, plain
elements coloured with `var(--color-…)`) and `vector-field.tsx`
(`oculus/2d`, a field built from `Vector`s). Every file is listed in
`SKILLS`, since `write_skill` deletes anything else in the folder.

Each example is a test three ways: Rust runs `widgets::check` on it and
wants no findings; Rust compares its `transpile` output with
`app/tests/fixtures/widget-gallery/<name>.js` (`WIDGET_BLESS=1 cargo test
widget_gallery` rewrites them), from which the bun test builds a frame
document with `buildSrcdoc`; and the bun test renders it on the kit's own
modules (`Bun.plugin` maps `oculus/*` to `widget-kit/`). `tsconfig.json`
includes the gallery, so `tsc` checks it against `widget.d.ts`.

## Checking a widget before it is sent

`oculus widget check <file|->` runs `transpile` — the gate the chat and
`artifact save` use — and a widget that fails gets that one finding
(`rule` `transpile`, with its line and column when the error has one).
Otherwise `widgets::check` lints the oxc AST, with oxc's semantic scoping
deciding what is global, so a name the widget declares itself always
passes:

| Rule | Finding |
| --- | --- |
| `title` | The first non-blank line isn't `// title: <something>` (as `widget/title.ts` reads it) |
| `colour` | A string or template text holding `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa` (not after `&`, a letter or a digit, nor before a letter, digit or `_`), or `rgb(`, `rgba(`, `hsl(`, `hsla(`, `hwb(`, `lab(`, `lch(`, `oklab(`, `oklch(` (not after a letter, digit, `-` or `_`); a number written `0x` with six hex digits |
| `font` | An object key or JSX attribute named `fontFamily` or `font-family`; `font-family` followed by `:` in a string or template text |
| `tex` | A JSX attribute string holding `\\`, which JSX keeps as two backslashes |
| `sandbox` | A global `fetch`, `XMLHttpRequest`, `WebSocket`, `EventSource`, `localStorage`, `sessionStorage`, `indexedDB` or `eval`, bare or as a property of a global `window`, `globalThis` or `self`; a global `Function` called or constructed; `.cookie` on a global `document` (or `window.document`) |
| `cleanup` | See below |

`cleanup` defines a scheduling call as a call to the global
`setInterval`, `setTimeout` or `requestAnimationFrame` (bare, or on
`window`, `globalThis` or `self`) or to any `.addEventListener(…)`. It
passes only inside the callback — the first argument, an arrow or a
function expression — of a call whose callee is named `useEffect` or
`useLayoutEffect` (`React.useEffect` included), nested functions in that
callback included, and only when that callback returns a cleanup that
cancels it: a `return` in the callback's own body, or an arrow's
expression body, whose value is an arrow or function expression that
calls `clearInterval`, `clearTimeout`, `cancelAnimationFrame` or
`removeEventListener` respectively, by name or as a method. A cleanup
returned through a variable doesn't count. With effects nested, the
innermost is judged.

Findings print one a line, `<file>:<line>:<col> <rule>: <message>`, in
source order; `--json` prints them as `[{rule, line, col, message}]`. Any
finding exits 1, so the skill's loop is check → fix → check. The agent
guard treats `widget check` as a read.

## Rust side

`widget_transpile(source)` (`app/src-tauri/src/widgets/transpile.rs`) parses the
TSX with oxc, strips TypeScript and turns JSX into automatic-runtime
`react/jsx-runtime` calls, returning `{ js, pending }`: `pending` names the
components the module draws as JSX but never declares, which only a
streaming prefix has (omitted when empty). It keeps the widget's own
imports, restricted to `ALLOWED_IMPORTS`; relative paths, URLs, re-exports
(`export … from`) and dynamic `import()` are errors, and syntax errors read
`"<line>:<col> <message>"`.

The default export is bound after the transform, from the generated code
parsed again and edited at its top-level statements' spans, so text that looks
like an export (a template string) is never touched. `export default function
App() {}` (or a `class`) is kept and `const __oculusWidget = App;` follows the
module; an anonymous function or class and any expression become
`const __oculusWidget = <it>;`; `export { X as default, Y }` loses its default
specifier, keeps `export { Y }` and gets `const __oculusWidget = X;`. A module
without a default export is the error `a widget needs a default export (the
component to draw)`; two defaults are a syntax error.

`widgets::check` is [above](#checking-a-widget-before-it-is-sent).
`widget_ask` and `widget_ask_cancel` are
[above](#the-job-and-the-call).

The media server serves the kit at `/widget-kit/<path>` with no token
(`app/src-tauri/src/widgets/kit.rs`; [architecture.md](./architecture.md)):
from `app/dist/widget-kit` in debug, from the embedded frontend in release.
The frontend builds the URL from `media_server_info`.

## Gotchas

- Never add `allow-same-origin` to a widget frame — it hands untrusted code the app's IPC.
- Never subscribe the document build to `widgetStore` — every state save would reload the frame.
- A kit entry's file name is part of the import map; hash it and every widget's imports break.
- Don't import Mafs' `core.css` or `font.css`, or give a colour prop a CSS value — the plane would stop following the theme.
- Keep `ALLOWED_IMPORTS` in `transpile.rs`, `importMap()` in `srcdoc.ts` and the entries in `vite.widget-kit.config.ts` in lockstep; `widget.test.tsx` and `kit.test.tsx` compare them.
- Never let the frame decide an `oculus/ask` request or draw its Allow — the widget is untrusted code; the gate and the prompt live in the parent.
- Keep `MAX_PROMPT` and `MAX_SCHEMA` the same in `ask/limits.ts` and `widgets/ask/prompt.rs`, and the subset in `widgets/ask/schema/` the same as `JsonSchema` in `widget.d.ts` and `widget-kit/ask/schema.ts`.
- Never re-export r3f's own `Canvas` from `fiber.ts` — a widget's raw r3f scene would keep its context off-screen and bring back the sixteen-context cap.
- Never let OrbitControls zoom by default — its wheel handler would `preventDefault` and the chat would stop scrolling over a scene.
- Keep `DEFAULT_BINDING` the same in `transpile.rs` and `widget-kit/protocol.ts`; the frame mounts that name.
- Never boot a second frame for a streaming fence or a preview — the committed row keeps the first, and a 3D scene per update would run into the context cap.
- Never give a streaming row a key its committed row won't take (`replyKey`) — React would remount it and the frame would boot again at commit.
- Never let a preview act — `oculus/ask`, state saves and Save to Revision wait for the `final` program, in the frame and in the parent.
- Never shadow the timers in a final program or skip `commit` — its errors would stay quiet and its effects capped.
- Never let `closePrefix` close a loop's body — a repaired `while` can spin the frame with nothing to stop it.
- Don't clamp the frame's height below or scroll it inside — the budget is the only limit, and it is an error.
- Never copy a `widgetStore` entry into a saved artifact — its Allow belongs to that fence; save `entryState` or the last reported state.
- Never give a saved widget a chat key (`thread:row:fence`) — `prune` would drop its grant with that thread's.
- List every `oculus-widgets` file in `SKILLS` — `write_skill` deletes an unlisted one from the agent's skill folder.
- After changing a gallery example or the transpiler's output, run `WIDGET_BLESS=1 cargo test widget_gallery` — the bun test builds frames from the fixtures.
