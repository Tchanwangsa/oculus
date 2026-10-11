# Studio

Studio makes lecture videos from scenes written as TSX components.
`app/studio-core/` is its renderer: a video folder plus a time in, one RGBA
frame out, the same pixels on every machine, scene transitions included.
A narrated scene lasts as long as its narration and can cue elements to its
words, from clips and timings `oculus studio narrate` makes with a TTS server
and a local aligner; encoding to video is not built yet. `oculus studio`
renders single frames and contact sheets, lints, and narrates.

## Where

| Piece | Location |
| --- | --- |
| The crate (`oculus-studio-core`, lib `studio_core`), its own cargo root | `app/studio-core/` |
| Public API: `Renderer`, `Engine`, `Frame`, `Time`; motion blur | `app/studio-core/src/render/` (`blur/`) |
| Contact sheets (`strip`): parallel frames, the tiled sheet and its labels | `app/studio-core/src/strip/` |
| Video folder, component lookup, timeline, lint | `app/studio-core/src/project/` (`lint/`) |
| `script.md`, the narration manifest, scene lengths and word cues | `app/studio-core/src/project/` (`script/`, `narration/`, `scenes.rs`), `app/studio-core/src/host/prelude/words.js` |
| Scene transitions: the cascade, compositing two scenes | `app/studio-core/src/transition/` |
| Magic move: matching and blending two laid-out trees, OKLab colour | `app/studio-core/src/morph/` |
| Easings shared by transitions and the morph (the prelude's, in Rust) | `app/studio-core/src/easing/` |
| TSX → JS (oxc), the QuickJS sandbox, the prelude every component sees | `app/studio-core/src/host/` (`prelude/*.js`, one module scope, concatenated in order) |
| The element tree and its style keys | `app/studio-core/src/scene/` |
| Layout (taffy, parley) and paint (vello_cpu); Path trim and Text reveal | `app/studio-core/src/layout/`, `app/studio-core/src/paint/` (`path/`, `text/`) |
| Props schemas and their validation | `app/studio-core/src/schema/` |
| Themes and bundled fonts | `app/studio-core/themes/`, `app/studio-core/fonts/`, `app/studio-core/src/theme/`, `app/studio-core/src/fonts/` |
| Every error and its message | `app/studio-core/src/error.rs` |
| Fixture videos and golden frames | `app/studio-core/tests/fixtures/`, `app/studio-core/tests/golden/` |
| Narration audio decode and word timing (the aligner, its model download) | `app/src-tauri/src/studio/` (`audio/`, `timing/`), `app/src-tauri/src/runtime/onnx/` |
| Narrating a video: the TTS client, the `voice` settings row, the cache, the manifest writer | `app/src-tauri/src/studio/` (`tts/`, `voice/`, `narrate/`) |
| `oculus studio` | `app/src-tauri/src/bin/oculus/commands/studio/` (`narrate.rs`, `aligner.rs`), `app/src-tauri/src/bin/oculus/args/studio.rs` |

## A video is a folder, and a frame is a pure function of it and `t`

```
video.json
components/   <Name>.tsx, one component per file
assets/       images, named by <Image src>
script.md     the narration, one `## <id>` section per narrated scene
narration/    manifest.json and the clips it names
```

```json
{
  "size": { "width": 1920, "height": 1080 },
  "theme": "default",
  "motion": { "transition": { "kind": "push", "dur": 0.6 }, "magicMove": "auto" },
  "voice": { "voice": "af_heart", "speed": 1.0 },
  "scenes": [
    { "id": "title", "component": "TitleCard", "props": { "title": "Gradient descent" }, "hold": 4.0 },
    { "id": "steps", "component": "Steps", "transition": { "kind": "wipe", "direction": "up" } }
  ]
}
```

- A scene's length is its `hold` in seconds, or for a narrated scene the
  length of its narration (see "Narration"). A global `t` maps to a scene
  and a scene-relative `t` by summing lengths (`Timeline`, from
  `Project::timeline`); a `t` past the end is an error that states the
  video's length.
- `id` (`[a-z0-9-]+`, unique) names the scene's section in `script.md`. It
  is required on every scene once the folder has a `script.md`; a scene
  whose id has no section is silent and needs a `hold`.
- `voice` (optional: `model`, `voice`, `speed`) overrides Settings -> Voice
  for this video. studio-core only stores it; the app's narrator reads it.
- `theme` is a bundled theme's name or a `.json` path relative to the folder.
- `motion` (optional) overrides the theme's transition and motion-blur
  defaults for this video; a scene's `transition` (optional) overrides
  both. See "Transitions" and "Motion blur".
- `video.json` refuses unknown keys, an empty `scenes`, a non-positive `hold`,
  a bad or repeated `id`, a non-positive `voice.speed` and sides outside
  1–8192 px.
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
  from the scene's start and `duration` the scene's length, frozen. Function components used inside the JSX
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
  `parallel`. The motion components are in "Motion" below; `word` and
  `wordEnd` are in "Narration".

## Narration sets a scene's length and cues its elements

`script.md` holds what is said, and `narration/manifest.json` the timings of
the clips made from it. studio-core reads both and never the audio. The
manifest is written by `oculus studio narrate` (see "Narration is made by
`oculus studio narrate`").

```markdown
## steps

We follow the *gradient* downhill, one small step at a time.
```

- **`script.md`** is one `## <id>` section per narrated scene, written as it
  is said. A section's text is its prose with `*`, `_` and backticks
  removed and whitespace collapsed; nothing else is interpreted (`#` and
  `###` lines are prose). Text before the first heading, a heading that is
  not an id, a repeated id and an empty section fail `Project::load` with
  the line. A section no scene's `id` matches is a lint error.
- **The manifest** is `{ version: 1, scenes: { <id>: { key, text, audio,
  duration, words: [{ text, start, end, confidence }], gaps: [{ after,
  start, end, heard }] } } }` (`Manifest`). `text` is the section text the
  clip says, `audio` the clip's file name in `narration/`, `key` its content
  address (sha256 of the text, server, model, voice, speed and aligner
  model), times are seconds from the clip's start, `confidence` is the
  aligner's (mean log-probability of the word's letter frames; higher is
  surer), and `gaps` (optional) is speech the aligner heard that the
  script does not have, after word index `after` (`null`: before the first
  word). Unknown keys, another
  version, non-finite times and a word or gap ending before it starts fail
  the load.
- **Length.** A narrated scene lasts `max(hold, lead + duration + tail)`:
  `hold` is optional and only a minimum. `lead` and `tail` are the theme's
  `narration` tokens (0.25 s and 0.6 s in the bundled theme); the clip
  starts `lead` into the scene. A silent scene (no section) lasts its
  `hold`, and one without a `hold` has no length.
- **Stale narration fails, it is never estimated.** A scene with script
  text whose manifest entry is missing or was made for other text makes
  `Project::timeline`, `Renderer::new` and `strip` fail with
  `StudioError::Narration` (kind `narration`): ``scene `steps`: no
  narration for its current text (…); run `oculus studio narrate <dir>` ``.
  `Project::load` still succeeds, so lint reports it with everything else.
- **Word cues.** `word(phrase, nth = 1)` is the scene time (lead included)
  at which the nth occurrence of `phrase` starts, and `wordEnd(phrase, nth
  = 1)` when its last word ends, so `<Enter at={word("gradient")}>` enters
  as it is said. Words match lowercased with only letters, digits and
  apostrophes kept; a phrase is consecutive words. A phrase that is not
  said, an `nth` past the last occurrence and a call in a silent scene
  throw at the call's TSX line, with the closest phrase that is said:
  ``Steps.tsx:12:20: `word("gradiant")`: no such word in scene `steps`;
  closest: gradient``. Each call gets its own scene's words (`SceneCall::
  words`, from `Project::word_cues`), so in a transition the outgoing scene
  cues to its words and the incoming one to its own.

## Narration timing is forced alignment on the CPU

The app crate times a clip's words (`app/src-tauri/src/studio/timing/`):
CTC forced alignment of the script against wav2vec2's letter emissions, run
by ONNX Runtime on the CPU (`app/src-tauri/src/runtime/onnx/`). The model
is Xenova's fp32 ONNX export of `facebook/wav2vec2-base-960h` (Apache-2.0,
credit facebook/wav2vec2-base-960h), pinned to one commit, 378 MB. It is
downloaded on request (`oculus studio aligner download`) to
`<app data>/models/align/` and never bundled; the download is checked
against its pinned size and sha256 before it gets its name. ONNX Runtime
itself is linked statically. `STUDIO_ALIGN_MODEL=<model.onnx>` points the
aligner at a copy elsewhere instead, for development and the parity tests.

- **Input.** The server's WAV is decoded with hound (any integer or float
  format, downmixed to mono) and resampled to 16 kHz with rubato
  (`app/src-tauri/src/studio/audio/`). A clip whose whole RMS is under
  -80 dBFS, or that holds a second or more of exact zeros, is digitally
  silent: a failed narration, not a pause.
- **Words.** Each script word is upper-cased to A-Z and `'`. A word with no
  letters (a dash, `&`) is left out of the result rather than given a
  made-up time; each word carries its index in the script.
- **Times.** 20 ms frames. A word starts at its first letter frame minus
  65 ms (clamped at 0) and ends after its last letter frame, unbiased. The
  bias was measured against PyTorch wav2vec2, which the fp32 ONNX model
  matches frame for frame.
- **Confidence** is the mean log-probability of the word's letter frames;
  below `LOW_CONFIDENCE` (-8.0) the word was mangled or not said.
- **Gaps.** A word the voice said but the script lacks lowers no
  confidence, so the aligner also reports each stretch between words with
  4 or more frames whose best label is a letter, with what it heard there
  ("THICK").
- **Limits.** A clip over `MAX_NARRATION_SECS` (60 s) is refused: memory
  grows with the square of its length (0.7 GB at 17 s, 2.2 GB at 57 s). Each
  alignment opens its own session and drops it, since a reused one keeps
  growing. Threads: half the cores, at most 4. No timeouts.

## Narration is made by `oculus studio narrate`

`narrate` (`app/src-tauri/src/studio/narrate/`) turns each scene's
`script.md` section into a clip and its word times, and writes the
manifest. Scenes run one at a time, in video order.

- **Voice.** Each of server, model, voice and speed comes from the first
  that sets it: the CLI flags, the video's `voice` block, then the `voice`
  settings row (`{ baseUrl, model, voice, speed }`, which Settings -> Voice
  will write; `studio::voice`). A missing or ill-typed field in the row
  reads as unset; only speed has a default (1.0). Server, model and voice
  have none, since ids differ per server (Kokoro-FastAPI `kokoro`, mlx-audio
  `mlx-community/Kokoro-82M-bf16`, OpenAI `tts-1`): narrate fails with
  ``set a TTS server in Settings -> Voice, or pass --server`` and the same
  for `--model` and `--voice`.
- **The TTS server** is OpenAI-compatible (Kokoro-FastAPI, mlx-audio):
  `POST <base>/v1/audio/speech` with `{ model, input, voice,
  response_format: "wav", speed }`, one request per clip, no timeout
  (`studio::tts`). The base URL is accepted with or without a trailing
  `/v1`, normalised once. Keyless servers only: no API key is sent. A
  non-2xx answer fails with its status and a short excerpt of the body; a
  429 is retried with back-off (1, 2, 4, 8 s; five tries). A 200 whose body
  is empty, is not a WAV, or is digitally silent (see "Input" above) is a
  failure, never a pause: Kokoro answers an unknown voice with 200 and
  nothing.
- **Cache.** A clip's `key` is the sha256 hex of the section text, the
  normalised server URL, model, voice, speed and the aligner's `MODEL_ID`,
  each length-prefixed. A scene whose manifest entry has the same key and
  whose `narration/<key>.wav` exists is kept (`cached`) unless `--force`;
  editing one paragraph re-requests that one scene.
- **A new clip** is saved byte for byte as the server sent it to
  `narration/<key>.wav`; its duration comes from the decoded samples, a clip
  over `MAX_NARRATION_SECS` is refused before alignment ("split the
  paragraph"), and the rest is resampled to 16 kHz and timed against the
  section's whitespace-separated words. The aligner model is checked before
  the first TTS request, so a missing one costs no synthesis and fails with
  ``run `oculus studio aligner download` ``.
- **Writes.** The manifest is rewritten atomically after each scene, so a
  failure keeps the scenes before it; the error names the scene and the
  command exits 1. After the last scene, entries whose id is no longer a
  narrated scene are dropped and every `.wav` (or interrupted `.wav.part`)
  in `narration/` that no entry names is deleted; other files are left. A
  manifest this build cannot read (another version, damaged) is rebuilt
  from nothing, with a warning.
- **Output.** Per scene: the id, `narrated` or `cached`, the clip's length,
  its word count, and as warnings each word under `LOW_CONFIDENCE` and each
  gap (times into the clip; lint fails on the same ones, at scene time).
  `--json` gives `{ dir, voice, scenes: [{ id, outcome, audio, duration,
  words, low_confidence, gaps, timings }], dropped, deleted, ms }`.
- `oculus studio tts "<text>"` makes one clip the same way and writes it
  (`tts.wav` unless `-o`), for hearing a voice. `oculus studio aligner
  status|download|delete` manages the aligner model.
- **Measured** on Kokoro (mlx-audio, `af_heart`, M-series Mac, debug
  build): three scenes of 4 to 8 s narrated in 1.8 s with a 0.67 GB peak;
  a re-run after one paragraph edit took 0.5 s. Kokoro's clips start with
  about 0.2 s and end with about 0.4 s of exact zeros, well under the 1 s
  zero-run limit.

## Four primitives and a closed set of style keys

`Box`, `Text`, `Image` and `Path` are the host elements, and `Morph` holds
two layouts of one place (see "Magic move"); `Stack` (a column) and `Row`
are `Box` with `gap`, `align`, `justify` and `wrap` shorthands. Elements take
only `style`, `key`, children, `src` on `Image`, `d`, `viewBox`, `fill`,
`stroke` and `trim` on `Path`, and `p` on `Morph`.

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
  Gradients are dithered (see "Gradients are dithered").
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

## Transitions play over the incoming scene's head

A scene after the first comes in through a transition: `{ kind, dur,
easing, direction, morphColor }`, where `kind` is `cut`, `dissolve`, `push`, `wipe`,
`zoom` or `magic`, `dur` is seconds, `easing` is a theme easing name, a
prelude easing (`linear`, `easeOutCubic`, `easeInOutCubic`) or `[x1, y1,
x2, y2]`, and `direction` (`left`, `right`, `up`, `down`) is where the
incoming scene comes from for `push` and `wipe` (`left` pushes the old scene
out to the left). `morphColor` is under "Magic move". `video.json`'s
`motion` block takes `transition`, `magicMove`, `morphColor` and
`motionBlur`.

- **Timing.** It plays over `[0, dur)` of the incoming scene B, with the
  outgoing scene A held at its last moment (the left limit of its length,
  since Sequence and Stagger windows are half-open). The video's length
  stays the sum of scene lengths, and past `dur` a frame is B alone. `p =
  easing(t / dur)`. Each frame calls both components, each in its own fresh
  context, so a transition frame is seekable like any other. A's logs come
  first, prefixed `scene N:`.
- **Cascade, field by field:** the scene's `transition`, then `video.json`'s
  `motion.transition`, then the theme's `motion.transition`, then dissolve,
  0.7 s, easeInOutCubic, left. The bundled theme ships exactly that.
- **Automatic magic move.** When the incoming scene sets no `transition`
  of its own, `magicMove` (video `motion`, else theme `motion`, else
  `auto`) is `auto`, and A's and B's trees share a key, the transition is
  `magic` instead, with the cascade's `dur` and `easing`. `"off"` keeps the
  cascade's kind.
- **Compositing.** Both scenes are laid out and painted onto one canvas,
  each wrapped in a full-frame node whose style carries the effect: `cut` is
  B; `dissolve` draws B over A at opacity `p` on the theme background, so A
  never shows through B's transparent areas; `push` moves A out by `p` of
  the frame and B in from the other side; `wipe` reveals B (on the theme
  background) from its edge; `zoom` grows A to 1.08 as it fades and settles
  B in from 0.94, fading in over A from `p = 0.3`, with B's root left
  full-frame so only its contents scale; `magic` draws the morph of the two
  trees.
- A bad value fails every frame of its scene, with an error naming the level
  it came from (`scene 3 transition.kind`, `video.json
  motion.transition.dur`, `theme motion.transition.easing`); lint reports
  it once.

## Magic move morphs laid-out trees

The morph engine (`app/studio-core/src/morph/`) takes two laid-out trees (A
at `p = 0`, B at `p = 1`) and returns one, which paint draws like any other.

- **Matching.** Roots match. A key matches the same key anywhere in the
  other tree, at any depth, which is how a card moves from one row to
  another. Inside a matched pair the unkeyed children match by index only
  when both sides have the same unkeyed children (count and kinds);
  otherwise they are unmatched. Keys are unique per scene tree, not just
  among siblings as in React: a key on two nodes is an error when a morph
  needs the tree, and lint reports it either way.
- **Matched pairs** blend per property: the border box (children ride along,
  since rects are absolute), radius, fill, border, shadow, opacity,
  translate, scale, rotate, transformOrigin, clipInset, blur, text colour and
  reveal progress. Two fills that are not both colours become gradients on
  the union of their stops (a colour is a flat gradient, no fill a clear
  one) and blend stop by stop, the angle turning the short way, so nothing
  is cross-faded. A side without a border or shadow grows it from zero width
  or fades it in.
- **Colour** (fills, gradient stops, borders, shadows, text and strokes)
  blends in OKLab with alpha premultiplied, along the path the `morphColor`
  motion token picks. `oklch`, the default, is polar between two saturated
  colours (chroma interpolates, the hue turns the short way), because the
  straight OKLab line between near-complementary colours such as indigo and
  orange passes close to grey; with a near-grey end it is the straight line.
  `oklab` is always the straight line. A scene's `transition.morphColor`
  sets it for the magic move into that scene, over `video.json`'s
  `motion.morphColor`, over the theme's; an in-scene `<Morph>` uses the
  video's, else the theme's. An unknown value is an error naming its level,
  like the other motion values.
- **Unmatched** A nodes fade out over `p` 0 to 0.4 and unmatched B nodes in
  over 0.4 to 1 (easeInOutCubic), at their own tree's place, carried by
  their nearest matched ancestor's move. A new or removed box that holds
  matched nodes fades its own fill, border and shadow only, so what moves
  through it stays fully visible. The output follows B's paint order, each
  unmatched A node just before its nearest following matched sibling, else
  last.
- **Content.** The same string in the same family and line count re-shapes
  at the interpolated size, weight, line height and letter spacing, broken
  at the interpolated width (no bitmap scaling). Different strings or line
  counts swap in halves: A's text fades out over 0 to 0.5 and B's in over
  0.5 to 1, so they are never visible together, while the box still morphs.
  The same image morphs its geometry, a different one swaps in halves; the
  same path data morphs viewBox, fill, stroke and trim, different data swaps
  in halves.

`<Morph p={...}>{a}{b}</Morph>` uses the same engine inside one scene, which
also gives FLIP-style layout animation. The tree lays out twice, once with
every Morph replaced by its first child and once by its second, and the two
layouts morph: inside each Morph at its own `p`, matching by key within that
Morph; outside every Morph, nodes match by tree path and move at the first
Morph's `p`, so siblings that shift because a Morph changes size glide too.
`p` is clamped to 0 to 1. A Morph takes exactly two elements, and a Morph
inside another is an error for now.

## Motion blur averages sub-frames, off unless asked

`RenderOptions::motion_blur` (`MotionBlur { fps, shutter, max_samples }`,
default 30 fps, a 180° shutter of 0.5 and at most 32 samples) makes
`Renderer` average sub-frames spread evenly over the shutter, `t ± shutter
/ fps / 2`, kept inside the frame's scene so a cut stays a cut. It is off
by default (`None`); `Engine` renders single calls and ignores it.

- **Adaptive N.** Before drawing, the renderer lays the frame out at the
  shutter's start and end (both scenes and the morph, during a
  transition), matches nodes by `key` (anywhere in the tree) else by their
  path of child indices from the nearest keyed ancestor, and takes the
  largest move of any node's transformed bounds in output px: `N =
  clamp(ceil(px), 1, max_samples)`, one sub-frame per px, so a fast edge
  smears instead of doubling. N = 1 is exactly the sharp frame, for two
  extra layouts and no extra paint. Only boxes count, so a fade,
  a reveal's rise inside its box or a trim does not add samples.
- Sub-frames average premultiplied in integers, so a blurred frame is
  deterministic and seekable like any other. `Frame::samples` says how many
  were drawn; the logs are the middle sample's.
- **The `motionBlur` token** (theme `motion`, overridden by `video.json`'s
  `motion`; the bundled theme ships `finals`) says which renders blur:
  `finals` the export, `always` drafts and previews too, `off` none.
  `Project::motion_blur(Purpose::Final | Purpose::Draft)` resolves it into
  the `RenderOptions` value, and the export (a later stage) asks for
  `Final`. A bad value is an error naming its level (`video.json
  motion.motionBlur`), and lint reports it. `oculus studio frame` and
  `strip` are inspection tools and blur only with `--motion-blur`, whatever
  the token says.
- At the default shutter, sharp edges moving fast show as N copies about 6
  px apart in a single frame; in motion they read as blur.

## A strip is a contact sheet of a motion

`strip(project, StripOptions { from, to, n, cols, scale, threads,
motion_blur })` renders `n` frames evenly spaced over `[from, to]` (both
included; `n = 1` is `from`, and a `to` at or past the end is clamped to the
video's last moment, as a transition holds the outgoing scene) and tiles
them `cols` a row on a neutral grey, which is neither theme's background. It
is how an agent or a reviewer checks a motion without a video: what moves,
in which order, and where it settles.

- Under each tile, never over the frame, a label bar gives the global time
  (`1.250 s`) and, in a video of more than one scene, `scene N · t`. A time
  just inside a scene's end reads `1.999`, never the end itself. Labels are
  an element tree drawn by studio-core with bundled Inter at 13 px and
  tabular digits, through the normal layout and text paint.
- Tiles are the frames' pixels copied in, identical to `Renderer::frame` at
  the same scale, between 8 px gutters. When a tile is too narrow for the
  time and the scene on one line, every bar stacks them on two. A sheet over
  64 Mpx or 65,535 px a side is refused before any frame renders.
- Frames render on worker threads, one `Renderer` per thread (never vello's
  own threads, which panic with filters), dealt round-robin and put back by
  index, so the sheet is byte-identical at any thread count. The default is
  half the cores, at most one per frame, and the count is capped so the
  renderers' estimated memory (64 MB each plus their frame buffers and
  decoded images) stays under 2 GB.
- A frame that fails fails the strip with `StudioError::Frame`, which names
  the global time, the scene and the time within it, and carries the frame's
  own error (its `kind`, component and TSX line). The earliest failing frame
  is the one reported, whatever the thread count.

## Gradients are dithered

An 8-bit linear ramp across a dark range bands: the dark theme's `#0e0f13`
to `#1c1e26` across 1920 px has 15 to 20 levels a channel, and undithered
its flat runs are 87 px wide even averaged over 8×8 px, with 1-level steps
between them. So studio-core rasterises gradient fills itself
(`app/studio-core/src/paint/gradient.rs`): the colour is evaluated in f32 at
every device pixel, premultiplied, and dithered to 8 bits with an 8×8 Bayer
matrix fixed to the canvas, so a still gradient never shimmers from frame
to frame. vello draws the result as an image paint sampled at its own pixel
grid, under the shape's anti-aliased coverage. Dithered, the widest run of
equal 8×8 means is 4 px, no step exceeds 1 level, and every column averages
to the ideal ramp within a fraction of a level (`tests/dither.rs`, on the
`dark-gradient` fixture). A full-frame gradient costs about 4 ms more per
1080p frame than vello's own gradient paint. vello_cpu's `f32_pipeline` does
not help: it still writes 8-bit pixels with no dither, so its output bands
the same.

## Blur means what it means in CSS

- An element's `blur` is `filter: blur()`: the value is the gaussian sigma.
- `shadow.blur` is `box-shadow`'s blur radius: sigma is half of it.

## Text is never snapped to a pixel

Glyphs draw with vello's atlas cache and hinting off, parley does not quantise,
and taffy's rounding is disabled; no transform rounds either, nor does a
reveal's rise. Text that glides must not step a whole pixel at a time, and
`tests/subpixel.rs` measures that: a line moving 0.25 px a frame, a word
rising in through `reveal`, text under a panning Camera, text pushed in by a
transition and text carried by a magic move must each move their ink
centroid smoothly.

## Fonts and themes are bundled, never the system's

- The fonts are Inter (variable: `opsz`, `wght`) and Manrope (variable:
  `wght`), compiled in with their OFL licences beside them. System fonts are
  never loaded, so a frame is the same on every machine; an unknown
  `fontFamily` is an error naming the bundled families.
- A theme is tokens only: `color`, `font` (`heading` and `body` families),
  `type` (style fragments to spread into `style`), `space`, `radius`,
  `motion` (`duration`, `easing` as cubic-bezier points, `spring` presets,
  and optionally `transition`, `magicMove`, `morphColor` and `motionBlur`,
  the bottom of the motion cascade; the bundled theme sets `morphColor:
  "oklch"` and `motionBlur: "finals"`) and `narration` (`lead` and `tail`,
  non-negative seconds; see "Narration"). Components read it as
  `ctx.theme` and it is frozen.
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

## Lint renders every scene at both ends and mid-transition

`Project::lint` loads each component once, checks every scene's props and
every component's `examples`, and renders each scene at `t = 0` and just
before its length, at scale 0.25, writing nothing; a key on two nodes of a
scene's tree is reported there too. When a scene and the one before it both
render cleanly, it also renders the middle of the transition between them,
so a morph error shows. It checks the transition values through the cascade
(reported at the first scene that uses a bad one, naming its level) and the
video's `morphColor` and `motionBlur`, a
`transition` on the first scene, and a `dur` longer than the incoming
scene's length. A scene whose length is unknown (stale narration, or silent
with no `hold`) is reported and not rendered. It collects every problem
with its scene, scene id and component instead of stopping at the first;
an error is reported once even if several renders hit it.

For a narrated video lint also reports, each naming the scene id:

- a `script.md` section no scene matches (kind `script`, with its line);
- script text that is not spoken form (kind `script`, `script.md:<line>:`
  and the token): digits, `%`, `^`, `=`, `/`, `+`, `<`, `>`, a lone `*`, maths
  symbols such as `×`, `÷`, `±`, `√`, `∑`, `∫`, `∈`, `≤`, `≠`, `≈`, Greek
  letters, any other symbol, and dotted abbreviations (`e.g.`, `i.e.`,
  `etc.`, `U.S.`), each with what to write instead (`` say `%` as
  `percent` ``). Letters and `, . ; : ! ? ' " - — ( )` pass, and `*` or `_`
  inside a word is emphasis. The rules are in
  `app/studio-core/src/project/lint/spoken.rs`;
- a clip longer than `MAX_NARRATION_SECS` (60 s): the aligner's memory grows
  with the square of the clip's length, so the paragraph should be split;
- each word whose aligner confidence is below `LOW_CONFIDENCE` (-8.0,
  measured on Kokoro, where clean words stayed above -4.83; re-check it for
  another TTS engine), since a mangled word smears its neighbours' timing;
- each manifest gap: ``the audio has unscripted speech after `word` (heard
  "…")``, or ``before the first word``.

All of these are errors: lint has no warning level, so any of them makes
`oculus studio lint` exit 1.

## `oculus studio` renders and lints from the terminal

- `oculus studio frame <DIR>` writes one PNG, by default to
  `<DIR>/out/frame-<t>.png` with `t` global; `--t S` or `--scene N --at S`
  pick the moment. It prints the path, the scene, its component, the
  scene-relative `t`, the size and the render time, then any component logs.
- `oculus studio strip <DIR>` writes a contact sheet, by default the whole
  video in 12 frames, 4 a row, at quarter scale, to
  `<DIR>/out/strip-<from>-<to>.png`; `--from`/`--to` (global seconds) or
  `--scene N` pick the stretch, `--n`, `--cols`, `--scale` and `--threads`
  shape it. It prints the path, then the frame count, the time range, the
  sheet and tile sizes, the render time and the thread count; `--json`
  gives `{path, times, width, height, tile: {width, height}, cols, ms,
  threads}`.
- `frame` and `strip` take `--motion-blur` (30 fps, 180° shutter); `frame`
  then reports the sub-frames it averaged.
- `frame` and `strip` read `script.md` and the narration manifest; stale or
  missing narration stops them with the `Narration` error, while `lint`
  lists it with the rest.
- `oculus studio narrate <DIR>` makes the narration (`--scene ID`,
  `--force`, and `--server`, `--model`, `--voice`, `--speed` over the
  video's and Settings' voice); `oculus studio tts` makes one clip; `oculus
  studio aligner` manages the aligner model. See "Narration is made by
  `oculus studio narrate`".
- `oculus studio lint <DIR>` prints each problem and exits 1 if there is
  any. With `--json` it prints the issues as an array of `{scene, id,
  component, example, kind, message, loc}`; `scene` and `component` are
  null for a script section no scene matches.

Flags are in [cli-reference.md](./cli-reference.md#oculus-studio).

## Golden frames pin the pixels

`tests/golden.rs` renders the fixtures in `tests/fixtures/` at fixed times and
compares them with `tests/golden/*.png`: a pixel differs when a channel is
more than 2 off, and at most 0.1% may differ. A failure writes the actual
frame and a diff image to `app/studio-core/target/golden-failures/`.
`STUDIO_BLESS=1 cargo test` rewrites the goldens. `tests/determinism.rs`
checks two renderers produce identical bytes, and `tests/isolation.rs` that
a frame (a sequenced timeline, a transition and a Morph included) is the
same whatever the engine rendered before it. `tests/transitions.rs` covers
timing, the cascade, automatic magic move, the changed-text halves and
lint's transition checks. `tests/strip.rs` pins a contact sheet of the
transitions fixture and its sameness across thread counts,
`tests/motion_blur.rs` the sample counts, a blurred push and the token, and
`tests/dither.rs` the dark-gradient measurement. The transition and strip
goldens are 640x360 or smaller to keep the repo small; there is no
dark-gradient golden, since a dithered 1080p frame is a 2.4 MB PNG.
`tests/narration.rs` covers scene lengths, stale and missing narration, the
load errors and `word`/`wordEnd`; the `narrated` fixture (a hand-written
manifest, no audio) pins a card entering at its word. In the app crate, `cargo test --lib studio::` covers the TTS client
against a loopback fake server (WAV, empty, non-WAV, silent, 404, 429),
the voice resolution, the cache key and `narrate` with a stubbed aligner
(caching, one-paragraph edits, per-scene writes, pruning, failures).

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
- Dithered gradients compress badly: a golden whose frame is mostly
  gradient grows several-fold when reblessed (`title-card` from 0.4 to
  2.1 MB). The pre-dither goldens still pass, since dithering moves no
  channel more than 2 levels and the comparison allows 2.
- The theme's default dissolve lasts 0.7 s, so a scene held shorter than
  that is a lint error naming `theme motion.transition.dur` until the scene
  sets a shorter `dur` or a `cut`, or the video's `motion` a shorter `dur`.
- Enter, Exit and a hidden Sequence change their children's root styles, so
  a root's own `opacity`, `translateX/Y`, `scale` and `blur` combine with
  theirs rather than being replaced. A root that is a `Morph` passes them
  to both of its layouts.
