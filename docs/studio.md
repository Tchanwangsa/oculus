# Studio

Studio makes lecture videos from scenes written as TSX components.
`app/studio-core/` is its renderer: a video folder plus a time in, one RGBA
frame out, the same pixels on every machine. Narration, transitions and
encoding to video are not built yet; `oculus studio` renders single frames
and lints.

## Where

| Piece | Location |
| --- | --- |
| The crate (`oculus-studio-core`, lib `studio_core`), its own cargo root | `app/studio-core/` |
| Public API: `Renderer`, `Engine`, `Frame`, `Time` | `app/studio-core/src/render/` |
| Video folder, component lookup, timeline, lint | `app/studio-core/src/project/` |
| TSX → JS (oxc), the QuickJS sandbox, the prelude every component sees | `app/studio-core/src/host/` (`prelude/*.js`, one module scope, concatenated in order) |
| The element tree and its style keys | `app/studio-core/src/scene/` |
| Layout (taffy, parley) and paint (vello_cpu); Path trim and Text reveal | `app/studio-core/src/layout/`, `app/studio-core/src/paint/` (`path/`, `text/`) |
| Props schemas and their validation | `app/studio-core/src/schema/` |
| Themes and bundled fonts | `app/studio-core/themes/`, `app/studio-core/fonts/`, `app/studio-core/src/theme/`, `app/studio-core/src/fonts/` |
| Every error and its message | `app/studio-core/src/error.rs` |
| Fixture videos and golden frames | `app/studio-core/tests/fixtures/`, `app/studio-core/tests/golden/` |
| `oculus studio` | `app/src-tauri/src/bin/oculus/commands/studio.rs`, `app/src-tauri/src/bin/oculus/args/studio.rs` |

## A video is a folder, and a frame is a pure function of it and `t`

```
video.json
components/   <Name>.tsx, one component per file
assets/       images, named by <Image src>
```

```json
{
  "size": { "width": 1920, "height": 1080 },
  "theme": "default",
  "scenes": [
    { "component": "TitleCard", "props": { "title": "Gradient descent" }, "hold": 4.0 }
  ]
}
```

- `hold` is a scene's length in seconds. A global `t` maps to a scene and a
  scene-relative `t` by summing holds (`Timeline`); a `t` past the end is an
  error that states the video's length.
- `theme` is a bundled theme's name or a `.json` path relative to the folder.
- `video.json` refuses unknown keys, an empty `scenes`, a non-positive `hold`
  and sides outside 1–8192 px.
- Component names are PascalCase letters and digits and resolve to
  `components/<Name>.tsx` through the `ComponentSource` trait
  (`FolderComponents`); a missing one is an error listing the ones there are.

## A component is `(props, ctx) => element` with a declared props schema

```tsx
export const props = { title: str(), bullets: list(str()), accent: token("color").default("accent") }
export const examples = { short: { title: "Gradient descent", bullets: ["…"] } }
export default ({ title, bullets, accent }, { t, duration, theme, size }) =>
  <Stack gap={theme.space.m} style={{ padding: theme.space.xl }}>…</Stack>
```

- `ctx` is `{ t, duration, theme, size: { width, height } }`, `t` in seconds
  from the scene's start, frozen. Function components used inside the JSX
  are called as `(props, ctx)` with their subtree's ctx, which a `Sequence`
  or `Stagger` above them shifts (see "Motion").
- The default export returns exactly one element. A component with no
  `props` export is refused (`{}` declares none): the schema is what lint and
  the inspector are built from.
- Builders: `str()`, `num()`, `bool()`, `list(x)`, `oneOf([...])`,
  `token(kind)`, `latex()` (a string of LaTeX source), each with
  `.default(v)` and `.optional()`. Before every call the scene's props are
  checked: a missing required prop, a wrong type or an unknown prop is a
  `Props` error naming the component, the prop path, what was expected and
  what came. Defaults are filled in.
- `token("color")` takes a token *name* (`"accent"`) and hands the component
  its value; any theme section works, dotted ones too (`token("motion.spring")`).
- `examples` are named prop sets. Lint checks them against the schema; nothing
  renders them yet.
- Motion helpers are prelude globals and pure functions of `t`:
  `interpolate(t, inputs, outputs, { easing, clamp })`, `spring(t, { stiffness,
  damping, mass })` (closed form, so any `t` samples directly), `linear`,
  `easeOutCubic`, `easeInOutCubic`, `cubicBezier(a, b, c, d)`, `mix(a, b, p)`
  for hex colours, `clamp`, `lerp`, `counter`, `revealProgress`, `series` and
  `parallel`. The motion components are in "Motion" below.

## Four primitives and a closed set of style keys

`Box`, `Text`, `Image` and `Path` are the host elements; `Stack` (a column)
and `Row` are `Box` with `gap`, `align`, `justify` and `wrap` shorthands.
Elements take only `style`, `key`, children, `src` on `Image`, and `d`,
`viewBox`, `fill`, `stroke` and `trim` on `Path`.

- `flexDirection` defaults to `row`, as in CSS. Use `Stack` for a column.
- `Text` children must be strings or numbers; to mix styles put several
  `Text` in a `Row`.
- `Image src` is relative to `assets/`. A missing or undecodable file is an
  error, never a placeholder. Decoded images are cached per renderer.
- Style keys are `STYLE_KEYS` in `app/studio-core/src/scene/style.rs`: size
  and min/max, flex, `gap`, `padding`/`margin` (`n`, `[v, h]` or four
  sides), `position` with `left`/`top`/`right`/`bottom`, `fill`, `radius`,
  `border`, `shadow`, `blur`, `opacity`, `clip`, `translateX`/`translateY`,
  `scale`, `rotate` (degrees), `transformOrigin`, `clipInset`, and the text
  keys `fontFamily`, `fontSize`, `fontWeight`, `color`, `lineHeight`,
  `letterSpacing`, `textAlign`, `fontVariantNumeric` and `reveal`, plus
  `objectFit` for images. An unknown key is an error naming the closest valid
  one.
- `Path d` is SVG path data (parsed by kurbo; bad data is an error with the
  element's tree path and TSX line). It is sized like an image from its
  `viewBox: [x, y, w, h]` (default: the path's bounding box) and fitted into
  its box `xMidYMid meet`. `fill` is a colour or gradient (nonzero rule);
  `stroke` is `{ width, color, cap, join, dash, dashOffset }` with SVG
  defaults (butt, miter, limit 4) and widths in viewBox units. `trim: [start,
  end]` strokes only that part of the total arc length, across subpaths in
  order, cut exactly so caps land on the ends; dashes keep their place as the
  trim moves. Trim does not affect `fill`.
- Lengths are px, `"N%"` or `"auto"`. Colours are `#rgb`, `#rrggbb` or
  `#rrggbbaa` only; a theme colour is just the value read from `theme`.
- `fill` is a colour or `{ linear: { angle, stops: [[offset, colour], …] } }`
  with a CSS angle (0 is to top, 90 to right) and offsets rising from 0 to 1.
- Transforms apply about `transformOrigin`: `[fx, fy]` fractions of the
  border box or keywords (`"center"`, `"top left"`, …), default the centre.
  `opacity` and `blur` composite the node and its children as one group;
  `clip` clips children to the node's rounded rect; `clipInset: [top, right,
  bottom, left]` (fractions of the border box) clips the node's whole paint,
  shadow and border included, and a side at 0 does not clip.
- `fontVariantNumeric: "tabular-nums"` gives every digit one advance (the
  `tnum` feature, part of the shaping key); a theme type token may set it.

## Motion

Classic JSX evaluates children before their parent, so a component cannot
shift time for what it wraps. Function components are therefore deferred:
`h` returns a marked element, and after the default export returns, the
prelude's resolver walks the tree from the root in source order, calling each
component with its subtree's ctx and flattening fragments and arrays. A
component's `key` lands on the host node it resolves to. The resolver and
every component body share the call's step budget. An error raised while
resolving outside the component's own code (a bad `Sequence` prop, say)
carries the element's TSX line.

- `<Sequence from dur hold>` gives its subtree `t - from`, clamped to `[0,
  dur]`, and `ctx.duration = dur` (default: the rest of the parent's).
  Outside `[from, from + dur)` the subtree still resolves but every root is
  composed with `opacity: 0`: it keeps its layout slot, so nothing in a flex
  parent jumps. `hold` keeps it shown at `t = dur` after the window. A
  negative or non-finite `from` or `dur` is an error at its line. Children
  may be render props, `(ctx) => element`, to read the shifted time inline.
- `<Stagger each at>`: direct child i (fragments flattened) is a Sequence
  from `at + i * each`. `series(durs, { gap, at })` returns back-to-back start
  times, `parallel(durs, { at })` returns `at` for each.
- `<Enter preset at dur easing>` and `<Exit …>` compose onto their children's
  resolved roots, never wrapping them: opacity and scale multiply, translate
  and blur add. Presets: `fade`, `rise`, `slide-left`, `slide-right`,
  `scale`, `blur`, `mask` (a left-to-right `clipInset` wipe). `dur` and
  `easing` default to `theme.motion.duration.base` and
  `theme.motion.easing.enter`/`exit`; `at` to 0 for Enter and to `duration -
  dur` for Exit. An exit carries on in the entry's direction. They do not
  shift time; wrap them in a Sequence for that. `easing` is a function,
  cubic-bezier points or a theme easing name.
- `<Camera x y zoom rotate>` is a full-frame Box whose transform puts frame
  point `(x, y)` (default the centre) at the frame centre, zoomed and rotated
  about it. Its children lay out as in the root; use it as the root or a
  full-frame child.
- `counter(t, from, to, { at, dur, easing, decimals })` returns the number as
  a fixed-decimals string (easeOutCubic over 1 s by default); pair it with
  `fontVariantNumeric: "tabular-nums"`.
- Text `reveal: { by, progress, stagger, from }` shows N units — `char`
  (non-space characters), `word` (default) or `line`, in text order — as
  `progress` goes 0 to 1. Unit i starts at `i·d` and lasts `u`, with `(N-1)·d
  + u = 1` and `d = stagger·u` (`stagger` 0.5 by default; 0 is all together,
  1 strictly one after another), eased with easeOutCubic. `from` is `fade`
  (default), `rise` (up from 0.35 em below) or `blur` (from 0.25 em); all
  fade. It is paint only: layout and wrapping are the whole text's, so the
  text's space is taken from the start. Settled units draw in the normal
  glyph run; only units in flight draw on their own.
  `revealProgress(t, at, dur, easing?)` feeds `progress`.
- `<Arrow points curve progress head stroke>` strokes through `points`
  (frame px; `curve` 0 is straight segments, otherwise a Catmull-Rom curve)
  up to `progress`, and fades and grows a head (`head` px, default 4.5 ×
  the stroke width, 0 for none) in over the last 15%. It is two absolute,
  full-frame Paths with the frame as viewBox, so its parent must fill the
  frame.

## Blur means what it means in CSS

- An element's `blur` is `filter: blur()`: the value is the gaussian sigma.
- `shadow.blur` is `box-shadow`'s blur radius: sigma is half of it.

## Text is never snapped to a pixel

Glyphs draw with vello's atlas cache and hinting off, parley does not quantise,
and taffy's rounding is disabled; no transform rounds either, nor does a
reveal's rise. Text that glides must not step a whole pixel at a time, and
`tests/subpixel.rs` measures that: a line moving 0.25 px a frame, a word
rising in through `reveal`, and text under a panning Camera must each move
their ink centroid smoothly.

## Fonts and themes are bundled, never the system's

- The fonts are Inter (variable: `opsz`, `wght`) and Manrope (variable:
  `wght`), compiled in with their OFL licences beside them. System fonts are
  never loaded, so a frame is the same on every machine; an unknown
  `fontFamily` is an error naming the bundled families.
- A theme is tokens only: `color`, `font` (`heading` and `body` families),
  `type` (style fragments to spread into `style`), `space`, `radius` and
  `motion` (`duration`, `easing` as cubic-bezier points, `spring` presets).
  Components read it as `ctx.theme` and it is frozen.
- `color.bg` is the canvas under every frame, `color.text` the colour and
  `font.body` the family of `Text` that sets none.
- The bundled `default` theme is neutral greys with the app's indigo
  `#5e6ad2` accent, Manrope headings and Inter body.

## The sandbox makes a component a bounded, pure function

- One QuickJS runtime per `Renderer` (or `Engine`), never shared across
  threads: render in parallel with one renderer per thread.
- Every load and call gets a fresh context: the prelude, the theme and the
  component's module are evaluated anew, and the context is freed and its
  garbage collected before the entry returns. A top-level `let`, a mutated
  global, `Math` or `Array.prototype` lasts one call, so a frame depends only
  on the component, props, theme, size, `t` and scene index, whatever was
  rendered before it. The prelude and each component are compiled to
  bytecode once; the fresh state costs about 0.15 ms a call (release build),
  under 3% of a 1080p frame, and the heap is the same after 2,000 calls as
  after one.
- 256 MB heap, 1 MB stack and a step budget counted by the interrupt
  handler, reset on every entry (a module load; a call, and again before its
  default export runs). Running out is a `Budget` error naming the
  component, not a hang.
- No `Date`, `performance`, `WeakRef` or `FinalizationRegistry`, no module
  loader (an `import` is an error at its line), no I/O. `Math.random` is
  reseeded before every call from the component name and scene index.
- `console.log` and friends go to `Frame::logs`, at most 200 lines a call.

## Errors are written for an agent to act on

Each `StudioError` says what is wrong, where, and what would be valid. One
with a TSX position reads like a compiler's — `Typo.tsx:4:5: <message>`, then
that line of source indented below — with positions mapped back through oxc's
source map. `kind()` names the variant for JSON. The CLI prints the message
unchanged.

## The engine renders one component without a project

`Engine` takes a theme, a size, an optional assets folder and a scale, then
`load_component(name, tsx)` and `render(SceneCall)`. `Renderer` wraps it for a
video and loads each component on first use; the Components playground and a
gallery drive `Engine` directly.

- Loading a name again replaces that component in the same engine: the hot
  reload path, tested in `tests/reload.rs`. A load that fails keeps the
  previous version.
- `scale` draws the same layout under a root transform: 0.5 is a 540p proxy
  of a 1080p video, identical in layout.

## Lint renders every scene at both ends

`Project::lint` loads each component once, checks every scene's props and
every component's `examples`, and renders each scene at `t = 0` and just
before `hold`, at scale 0.25, writing nothing. It collects every problem
with its scene and component instead of stopping at the first; an error is
reported once even if both ends hit it.

## `oculus studio` renders and lints from the terminal

- `oculus studio frame <DIR>` writes one PNG, by default to
  `<DIR>/out/frame-<t>.png` with `t` global; `--t S` or `--scene N --at S`
  pick the moment. It prints the path, the scene, its component, the
  scene-relative `t`, the size and the render time, then any component logs.
- `oculus studio lint <DIR>` prints each problem and exits 1 if there is
  any. With `--json` it prints the issues as an array of `{scene, component,
  example, kind, message, loc}`.

Flags are in [cli-reference.md](./cli-reference.md#oculus-studio).

## Golden frames pin the pixels

`tests/golden.rs` renders the fixtures in `tests/fixtures/` at fixed times and
compares them with `tests/golden/*.png`: a pixel differs when a channel is
more than 2 off, and at most 0.1% may differ. A failure writes the actual
frame and a diff image to `app/studio-core/target/golden-failures/`.
`STUDIO_BLESS=1 cargo test` rewrites the goldens. `tests/determinism.rs`
checks two renderers produce identical bytes, and `tests/isolation.rs` that
a frame (a sequenced timeline included) is the same whatever the engine
rendered before it.

From `app/studio-core`: `cargo test`, `cargo clippy --all-targets -- -D
warnings`, `cargo fmt`. CI runs these in the `studio-core` job on macOS.

## Gotchas

- Each call starts from clean state: a top-level `let` resets every frame, so a counter or cache in module scope never accumulates — derive everything from `t`. A top-level `console.log` shows in every frame's logs, since the module runs again.
- Goldens are rendered on Apple Silicon; bless them on one, and keep the CI job on macOS.
- The crate's `[profile]` sections apply only to its own `cargo test`; built into `oculus`, src-tauri's profiles win.
- `flexDirection` defaults to `row`; a `Box` of lines lays them side by side.
- The JSX factory is `__studio_h`, so a component's own `h` variable is safe.
- A component element is not a host node until it resolves: `<Text>` still
  rejects a component child, and a component returning a string or a
  function is an error at its element's line.
- Enter, Exit and a hidden Sequence change their children's root styles, so
  a root's own `opacity`, `translateX/Y`, `scale` and `blur` combine with
  theirs rather than being replaced.
