# Studio

Studio makes lecture videos from scenes written as TSX components.
`app/studio-core/` is its renderer: a video folder plus a time in, one RGBA
frame out, the same pixels on every machine, scene transitions included.
A narrated scene lasts as long as its narration and can cue elements to its
words, from clips and timings `oculus studio narrate` makes with Kokoro run
inside the app or a TTS server, and a local aligner. `oculus studio render`, or the app's `studio_render`
command, encodes a video to an mp4 with ffmpeg, one cached segment per
scene. `oculus studio` also renders single
frames and contact sheets, lints (errors, and warnings about how a
scene looks over time), prints a scene's timeline of changes (`beats`),
crops library PDF pages into slides (`slide`), and narrates.

## Where

| Piece | Location |
| --- | --- |
| The crate (`oculus-studio-core`, lib `studio_core`), its own cargo root | `app/studio-core/` |
| Public API: `Renderer`, `Engine`, `Frame`, `Time`; motion blur | `app/studio-core/src/render/` (`blur/`) |
| Contact sheets (`strip`): parallel frames, the tiled sheet and its labels | `app/studio-core/src/strip/` |
| Video folder, component lookup, the whole-frame timeline, scene fingerprints, lint | `app/studio-core/src/project/` (`timeline.rs`, `frames.rs`, `fingerprint.rs`, `lint/`) |
| What a scene shows over time, from laid-out trees: sampling, the paint-order walker, lint's look warnings and their thresholds, `beats` | `app/studio-core/src/look/` (`sample.rs`, `walk.rs`, `items.rs`, `entities.rs`, `limits.rs`, `warn/`, `beats/`) |
| `script.md`, the narration manifest, scene lengths and word cues | `app/studio-core/src/project/` (`script/`, `narration/`, `scenes.rs`), `app/studio-core/src/host/prelude/words.js` |
| Scene transitions: the cascade, compositing two scenes | `app/studio-core/src/transition/` |
| Magic move: matching and blending two laid-out trees, OKLab colour, lag and fades | `app/studio-core/src/morph/` (`clock.rs`, `fade.rs`) |
| Burned-in captions: phrasing, timing, the caption's element tree, the WebVTT file, the theme's `captions` tokens | `app/studio-core/src/captions/` (`phrase.rs`, `timing.rs`, `tree.rs`, `subtitles.rs`, `style.rs`) |
| Overlays bound to keys (Brace, Surround, Underline, Indicate, a keyed Arrow), drawn after layout and morphing | `app/studio-core/src/overlay/`, `app/studio-core/src/host/prelude/overlays.js` |
| Easings shared by transitions and the morph (the prelude's, in Rust) | `app/studio-core/src/easing/` |
| TSX → JS (oxc), the QuickJS sandbox, the prelude every component sees | `app/studio-core/src/host/` (`prelude/*.js`, one module scope, concatenated in order) |
| The element tree and its style keys | `app/studio-core/src/scene/` |
| Layout (taffy, parley) and paint (vello_cpu); Path trim and Text reveal | `app/studio-core/src/layout/`, `app/studio-core/src/paint/` (`path/`, `text/`) |
| `<Math>`: KaTeX through math-core, the layout of its HTML tree, keys, the expansion into glyphs, `<Write>`, KaTeX's brace; the KaTeX fonts | `app/studio-core/src/math/` (`walk/` one file per construct, `write.rs`, `brace.rs`), `app/studio-core/fonts/katex/` |
| Props schemas and their validation | `app/studio-core/src/schema/` |
| Component and theme libraries: the starter set, seeding, the snapshot write path and removal, `library.json` (a library's default theme), promoting a video's own file | `app/studio-core/src/library/`, `app/studio-core/library/` (the starter files), `app/src-tauri/src/studio/libraries/` (the app's data-dir roots, `promote.rs`, `defaults.rs`) |
| Videos in the data directory: `new`, `list`, slug resolution, slugs from titles, the app's scope | `app/src-tauri/src/studio/videos/` (`slug.rs`), `app/src-tauri/src/library/paths/studio.rs` (`studio_dir`, `agent_write_dirs`) |
| Galleries: each example of a component as a still, and its contact sheet | `app/studio-core/src/gallery/` |
| Themes and bundled fonts | `app/studio-core/themes/`, `app/studio-core/fonts/`, `app/studio-core/src/theme/`, `app/studio-core/src/fonts/` |
| Every error and its message | `app/studio-core/src/error.rs` |
| Fixture videos and golden frames | `app/studio-core/tests/fixtures/`, `app/studio-core/tests/golden/` |
| Narration audio decode and word timing (the aligner, its model download) | `app/src-tauri/src/studio/` (`audio/`, `timing/`), `app/src-tauri/src/runtime/onnx/` |
| Narrating a video: the `Speaker` trait, the TTS client, the `voice` settings row, the cache, the manifest writer | `app/src-tauri/src/studio/` (`tts/`, `voice/`, `narrate/`) |
| The Kokoro engine: its pack, reader, chunking and synthesis | `app/src-tauri/src/studio/tts/kokoro/` |
| Text to Kokoro phonemes (the G2P), the lexicon, the guesser and its pack; the lint of a video's narration that needs them | `app/src-tauri/src/studio/voice/g2p/` (`guess/`, `lexicon.rs`), `app/src-tauri/src/studio/voice/lint/` |
| The pronunciation review: suggestions stored one file each, the lexicon writer, finding a word in a video's narration; its Tauri commands and wrappers | `app/src-tauri/src/studio/voice/review/` (`key.rs`, `lexicon.rs`, `locate.rs`), `app/src-tauri/src/studio/commands/pronunciations.rs`, `app/src/lib/studio/pronunciations.ts` |
| Settings -> Voice: the `voice` row and its engine, the server's model list, Kokoro's packs and sample, the pronunciation review, the aligner download; its Tauri commands | `app/src/pages/settings/library/VoicePage.tsx`, `app/src/components/settings/voice/` (`pronunciations/`), `app/src/lib/studio/` (`voice.ts`, `voicePacks.ts`, `aligner.ts`), `app/src-tauri/src/studio/commands/` (`voice.rs`, `kokoro.rs`, `aligner.rs`, `progress.rs`), `app/src-tauri/src/studio/tts/models.rs` |
| Rendering a video: the export presets, ffmpeg's toolkit and the encoder choice, plan, workers and the reorder buffer, ffmpeg, the segment cache, the narration track, mux, the `.vtt` beside each narrated render, each preset's status, one render per folder | `app/src-tauri/src/studio/render/` (`preset.rs`, `toolkit.rs`, `settings.rs`, `export.rs`, `status.rs`), `app/src-tauri/build.rs` (`STUDIO_RENDER_BUILD`) |
| Rendering from the app: `studio_render`, `studio_render_cancel`, `studio_exports`, the `studio-render-progress` event and their wrappers; the page over them is [studio-page.md](./studio-page.md) | `app/src-tauri/src/studio/commands/render.rs`, `app/src/lib/studio/render.ts` |
| The Studio page's commands: the video list, `studio_new_video`, a video's detail, stills and thumbnails; the `studio_videos` link table; a video thread's brief | `app/src-tauri/src/studio/commands/videos/`, `app/src-tauri/src/db/store/studio_videos.rs`, `app/src-tauri/src/harness/manager/studio.rs`, `app/src/lib/studio/videos.ts` |
| The playground's and Themes page's commands: the theme and component lists, source, frame, validated write, history, restore and removal, each library's default theme; the shared snapshot listing; a new video's default theme | `app/src-tauri/src/studio/commands/components/`, `commands/themes/`, `commands/levels.rs`, `commands/history.rs`, `app/src-tauri/src/studio/libraries/defaults.rs`, `app/src/lib/studio/components.ts`, `app/src/lib/studio/themes.ts` |
| The agent flow: the `oculus-studio` skill and the prelude's globals, typed | `app/src-tauri/templates/skills/oculus-studio/` (`SKILL.md`, `reference.md`), `app/studio-core/studio.d.ts` (its test: `src/host/tests/globals.rs`) |
| Slides: a region of a library PDF page as a video asset, the blocks to pick it by, edges moved off ink, the sidecar and its lint; the bundled sample crop | `app/src-tauri/src/studio/slides/` (`blocks.rs`, `geometry.rs`, `snap.rs`, `raster.rs`, `sidecar.rs`, `preview.rs`), `app/studio-core/library/components/Slide.tsx`, `app/studio-core/src/images/`, `app/studio-core/library/assets/` |
| `oculus studio` | `app/src-tauri/src/bin/oculus/commands/studio/` (`mod.rs`, `videos.rs`, `lint.rs`, `brief.rs`, `beats.rs`, `render.rs`, `narrate.rs`, `voices.rs`, `g2p.rs`, `pronounce.rs`, `aligner.rs`, `gallery.rs`, `components.rs`, `themes.rs`, `slide.rs`), `app/src-tauri/src/bin/oculus/args/studio.rs` |

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
  "fps": 30,
  "theme": "default",
  "motion": { "transition": { "kind": "push", "dur": 0.6 }, "magicMove": "auto" },
  "voice": { "voice": "af_heart", "speed": 1.0 },
  "scenes": [
    { "id": "title", "component": "TitleCard", "props": { "title": "Gradient descent" }, "hold": 4.0 },
    { "id": "sorts", "component": "Bars", "props": { "labels": ["\\text{bubble}", "\\text{insertion}", "\\text{merge}"], "values": [500, 250, 9], "diff": [0, 2], "diffLabel": "\\times 55", "diffAt": 2.2 }, "transition": { "kind": "wipe", "direction": "up" } }
  ]
}
```

- A scene's length is its `hold` in seconds, or for a narrated scene the
  length of its narration (see "Narration"), rounded up to whole frames at
  the video's `fps` (see "Frames"). A global `t` maps to a scene and a
  scene-relative `t` by summing lengths (`Timeline`, from
  `Project::timeline`); a `t` past the end is an error that states the
  video's length.
- `fps` (optional, default 30) is 24, 25, 30, 50 or 60; any other value
  fails the load with the rates.
- `id` (`[a-z0-9-]+`, unique) names the scene's section in `script.md`. It
  is required on every scene once the folder has a `script.md`; a scene
  whose id has no section is silent and needs a `hold`.
- `voice` (optional: `engine` (`kokoro` or `server`), `model`, `voice`,
  `speed`) overrides Settings -> Voice for this video. studio-core only stores it; the app's narrator reads it.
- `theme` is a bundled theme's name, a library theme's name (see "Libraries
  cascade") or a `.json` path relative to the folder.
- `subject` (optional) is the course the video belongs to: a course code's
  folder name (letters, digits, `-` and `_`, up to 64 characters), whose
  library (`<app data>/studio/subjects/<CODE>/`) is read before the global
  one.
- `captions` (optional, default `false`) burns word-highlight captions into
  every frame of a render (see "Captions are burned in from the word
  timings"). `oculus studio new` and the app write `true`.
- `motion` (optional) overrides the theme's transition and motion-blur
  defaults for this video; a scene's `transition` (optional) overrides
  both. See "Transitions" and "Motion blur".
- `video.json` refuses unknown keys, an empty `scenes`, a non-positive `hold`,
  a bad or repeated `id`, a non-positive `voice.speed` and sides outside
  1–8192 px.
- Component names are PascalCase letters and digits and resolve to
  `components/<Name>.tsx` through the `ComponentSource` trait
  (`FolderComponents`); a missing one is an error listing the ones there are.

## Frames: scenes are whole frames, and a scene has a fingerprint

The timeline counts frames at the video's `fps`: each scene's length rounds
up to whole frames (`ceil(length * fps)`, at most one frame longer, with a
millionth of a frame of slack so `0.1 s * 30` is 3 frames), and every
consumer (`Renderer`, `strip`, lint, `ctx.duration`, word cues, the
transitions) reads that one timeline.

- `Project::fps()`, `Project::frames()` (the frame count),
  `Project::scene_frames(i) -> Range<u64>` (global frame indices) and
  `Project::frame_time(k) -> Time` are what an encoder walks; `Timeline` has
  the same methods. Frame `k` of scene `i` is `Time::Scene { index: i, t: j
  / fps }` for its `j`th frame, so a scene's frames sample the same
  scene-relative times wherever it sits. The last frame is one frame
  before the video's end, never at it; a `k` past it is
  `StudioError::FrameOutOfRange`.
- A global `t` on the frame grid (`k / fps`) locates to exactly
  `frame_time(k)`, so `oculus studio frame --t` at a frame's time is that
  frame of the video. A `t` between frames renders as given.
- `Project::scene_fingerprint(i)` is what in the folder scene `i`'s frames
  depend on, as canonical JSON bytes (keys sorted, so key order in a file
  never matters), for a per-scene segment cache key: the size and fps, the
  theme's tokens (its `captions` block included), `video.json`'s `motion`
  and `captions`, and for scene `i` and scene
  `i - 1` (whose last moment the transition into `i` shows) the scene spec
  (`id` included), the narration words as components see them (`text`,
  `start`, `end`), the frame count and, for a scene without an `id` only,
  its index (`Math.random` is seeded by component and id, or index when
  there is no id). Words' confidence, the clip's file and `voice` reach no
  frame and are left out. The caller adds the rest: the contents of
  `components/` and `assets/`, the render settings (quality, scale, motion
  blur, encoder arguments) and the build of studio-core. An id'd scene keeps
  its fingerprint when scenes are inserted, removed or reordered around it,
  as long as the scene before it is the same; an id-less scene, or one
  right after an id-less scene, gets a new fingerprint when it moves.

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
  `token(kind)`, `latex()` (a string of LaTeX source, checked by typesetting
  it, so a bad formula or `\color` is a `Props` error with KaTeX's message
  and position), each with
  `.default(v)` and `.optional()`. Before every call the scene's props are
  checked: a missing required prop, a wrong type or an unknown prop is a
  `Props` error naming the component, the prop path, what was expected and
  what came. Defaults are filled in.
- `token("color")` takes a token *name* (`"accent"`) and hands the component
  its value; any theme section works, dotted ones too (`token("motion.spring")`).
- `examples` are named prop sets. Lint checks them against the schema and
  requires each required prop in every example (see "Lint"); `gallery`
  renders each one as a still.
- Motion helpers are prelude globals and pure functions of `t`:
  `interpolate(t, inputs, outputs, { easing, clamp })`, `spring(t, { stiffness,
  damping, mass })` (closed form, so any `t` samples directly), `linear`,
  `easeOutCubic`, `easeInOutCubic`, `cubicBezier(a, b, c, d)`, `mix(a, b, p)`
  for hex colours, `clamp`, `lerp`, `counter`, `revealProgress`, `series` and
  `parallel`. The motion components are in "Motion" below; `word` and
  `wordEnd` are in "Narration".

## Videos live in the data directory

Everything Studio keeps is under `<app data>/studio/`
(`library::paths::studio_dir`): `videos/<slug>/` (one folder per video),
`library/` (the global library), `subjects/<CODE>/` (each subject's) and
`pronunciations/` (the readings waiting for the student's review, see
"The student reviews the agent's suggested readings").
`courses/` stays read-only to agents, so no library lives there. It is the
one folder besides `agents/` and the database an agent thread may write
([harness.md](./harness.md#every-thread-runs-from-agents-and-that-is-the-containment)).

- `oculus studio new <slug> [--subject CODE] [--title T] [--portrait]`
  (`studio::videos::create_in`) makes `videos/<slug>/`: `video.json` at
  1920x1080 (1080x1920 with `--portrait`), 30 fps, the default theme, the
  subject, `"captions": true`, and one silent `TitleCard` scene (`id` `title`, 4 s) whose title
  is `--title` or the slug with spaces; an empty `script.md`; and empty
  `components/` and `assets/`. The default theme is the subject library's
  `library.json` `defaultTheme`, else the global library's, else `default`
  (see "Libraries cascade"), and video.json names it, so a later change of
  default leaves the video as it was. A slug is lowercase letters, digits and `-`, at most 64
  characters; one that exists is refused and left alone. It prints the
  folder (`--json`: `{dir, slug}`).
- `oculus studio list [--subject CODE]` prints each video's slug, subject,
  scene count, whether `narration/manifest.json` exists (`narrated`; `lint`
  says whether it is current), the files in `renders/` and the newest change
  to video.json, script.md or `components/`. A folder whose video.json does
  not parse is listed with the reason. `--json` gives `[{slug, dir,
  subject, scenes, narrated, renders, modified, problem}]`.
- Every command that takes a video folder (`frame`, `strip`, `lint`,
  `gallery`, `components list|show|promote`, `themes promote`, `render`,
  `narrate`, `g2p --video`, `pronounce suggest`, `pronounce list --video`)
  also takes a bare slug (`studio::videos::resolve`): a path
  that exists is used as given, otherwise a slug names `videos/<slug>/`, and
  anything else fails naming both places.
- The app's commands take a slug and reach only a video (`videos::scoped_in`,
  `video_in`): a bare slug that names a folder in `videos/` wins over a path
  of that name, and a path must, once symlinks are followed, be a folder
  directly inside `videos/`. `videos/`, `components/` and any folder
  elsewhere are refused ("is not a video").
- A title makes a slug (`slug_from_title`): ASCII letters and digits
  lowercased, any other run one `-`, at most 56 characters (room for a
  suffix), `untitled` when none is left. `unique_slug` appends `-2`, `-3`...
  until no folder has the name.
- video.json has no title field. The Videos list and the detail call a video
  by its slug spelled out (`title_of`: `gradient-descent` -> `Gradient
  descent`); the scaffold's `TitleCard` carries the title the student typed,
  and an agent may rewrite it.

## Agent flow: a chat agent makes a video with the `oculus-studio` skill

A video is made in a Chat thread by the student's own CLI agent, through
the `oculus` CLI and its file tools; the Studio page's New video starts that
thread beside the video's player ([studio-page.md](./studio-page.md)).
The `oculus-studio` skill ([harness.md](./harness.md#the-brief-is-appended-except-on-opencode))
is the whole brief, and `AGENTS.md` only points at it. The target is a
3Blue1Brown-style explainer: maths written term by term as it is said and
rewritten in place, every basic defined on an example before it is named,
no slide shapes (card-and-arrow chains, a heading over lines, a formula
that appears whole, a text-only scene), and no length budget.

`SKILL.md` splits the work into roles, each helper started as a
[subagent](./harness.md#subagents) on the model Settings → Jobs gives that
agent (`oculus studio agents` prints it and the call to make):

- **Research**, one helper per slice of the lecture (about ten minutes or
  ten slides), writes `notes/<nn>-<topic>.md` in the video's folder in a
  fixed shape: definitions, worked examples with every number, figures
  with page refs, what the slice assumes, what it leaves for later.
- **Direct**, the thread's own agent, reads only the notes and writes
  `storyboard.md`: the question, the viewer (a first-year who missed the
  lecture), a ladder of every term in the order it is defined, and many
  short scenes, each with what it carries in and hands on by key, the
  idea, the picture, a narration draft and a beat list (each clause and
  what appears or moves as it is said). It stops for the student's
  go-ahead on the storyboard, then writes `video.json` (each one-scene
  component's only prop, `narrated`, set on narrated scenes) and
  `script.md`, placing a `[pause]` where an idea lands (a result shown, a
  definition complete, a rewrite finished, the end of a scene's argument),
  then lints and narrates before any scene is built, so builders cue to
  real word times.
- **Build**, one helper per scene, works from `build.md`: its first call is
  `oculus studio brief <slug> --scene <id>`, which stands in for the
  storyboard, script and manifest; it writes the scene's components, cues
  each beat with `word()` on the brief's word times, lands the beat a
  pause follows just before `pause()` and holds it through the silence,
  checks as text with
  `lint --scene` (every error fixed, each warning fixed or explained) and
  `beats --scene`, then looks with the brief's `strip` or `frame` call,
  reading the PNG at its fixed `-o` path in the same message (a missing
  component elsewhere does not stop them; stale narration anywhere does).
  A component two scenes share gets examples and a `gallery --sheet`. It
  replies with its components, props, strip, kept warnings and any
  narration change, which the director makes and re-narrates. The
  director's brief is a few lines: slug, scene, component, shared keys and
  the student's rules for the video.
- **Review**, one helper per finished scene on the same scene role, runs
  `brief`, `lint --scene` and `beats --scene`, looks at one strip and a few
  frames, and replies `PASS` or a numbered fix list of at most ten (what,
  when, where); it edits nothing. The director reads verdicts, not strips,
  and sends a fix list to a fresh builder with the build brief and the
  last reply.

The director then lints until no errors are left, takes its one look at a
strip of the whole video for the joins, and renders a draft, then
`--final`. An agent whose role is off, or that cannot start helpers (agy),
does the items itself in order.
When `narrate` fails for want of a voice or the aligner, the agent stops and
tells the student what to set in Settings -> Voice; it never downloads the
aligner or the voice packs. After narrating with Kokoro it judges each
guessed-word note, suggests a reading for each doubtful word with `oculus
studio pronounce suggest` (it never edits a lexicon), tells the student to
review them in Settings -> Voice, and carries on: a decision applies at the
next narrate, and a suggestion marked wrong comes back in the notes with
the student's note for another try. The story rules (explain with reasons, link each scene to the
last, numbers given meaning, example then rule, the payoff shown, the end
answering the opening question, phrasing the voice cannot fuse) and the
taste rules (the picture carries the idea, the frame filled, every motion
cued, stable magic-move keys, tokens over literals) and a map of moving
maths close `SKILL.md`: every formula a `Math`, written on per `\k` group
with `Write`, rewritten in place with `Steps`, pointed at with the
overlays, shrunk into a corner, a value flying into its slot, carried to
the next scene by key; never a formula appearing whole, stacked lines or
maths as Text.

`build.md` is the scene helper's procedure (brief, write, check as text,
look, the bar every scene meets, the reply, review). The `studio-scene`
role prompt carries it with the builder's sections of `reference.md`; on
Codex the director's brief has the helper read them by file
([harness.md](./harness.md#subagents)).

`reference.md` is this page's authoring rules distilled for an agent
(video.json, spoken form and pauses, the component contract, elements and style keys,
type sizes, motion, `Math` with its backslash rule, `\k` groups, theme
colours and size, moving maths with `Write`, `Steps` and the overlays as
five whole components checked with `strip` plus their gotchas, cues,
transitions, each lint kind and what to do about it, and the commands'
outputs). `studio.d.ts` types every prelude global;
`src/host/tests/globals.rs` lists the globals a fresh sandbox context gains
from the prelude (QuickJS's own `Object.getOwnPropertyNames(globalThis)`
before and after, less the host's `__studio*` entry points) and fails when
they differ from the file's top-level `declare`s, either way. The app ships
the file with `include_str!`.

## Libraries cascade: project, subject, global

A video's components and themes resolve through up to three libraries, and
the first hit wins:

| Level | Components | Themes |
| --- | --- | --- |
| `project` | the video folder's `components/` | (a `.json` path in the folder) |
| `subject <CODE>` | `<app data>/studio/subjects/<CODE>/components/` | `<app data>/studio/subjects/<CODE>/themes/` |
| `global` | `<app data>/studio/library/components/` | `<app data>/studio/library/themes/` |

A component resolves by name (`CascadeComponents`, in the order above). A
theme name resolves through the subject's library, then the global one, then
the bundled set. A subject's library is read only when the libraries are
given (`Library::subjects`); every app and CLI path that loads a video
gives them (`studio::libraries::open`, `libraries`), narrate included, and
the `Project::load` API does not. `gallery` takes `--subject` and `--theme`,
and `components list` and `show` take `--subject`, which replace
video.json's for that load. A component that no level has is an error
listing every name in every level.

The starter set (`app/studio-core/library/`) is compiled into the crate:
drawing primitives: `NumberLine`, `Axes` (any formula string, several
curves, `Scatter` with a regression line, `Area`, `Riemann` rectangles, a
`Tangent` or shrinking secant, `Dot`), `Graph` (nodes and edges laid out
on a circle, as a tree, in columns or by hand; a traversal and a walk),
`Grid` (a matrix, DP table or board of cells with fills, marks and
pointers), `Bars` (a bar chart or histogram), `Code` (highlighted source
on a fixed column grid) and `Table` (keyed cells), plus `Slide` (a crop of
a lecture PDF page; see "Slides crop a library PDF page") and `TitleCard`, and
the theme `warm` beside `default`. Components cannot import one another, so
each primitive file holds the primitive as a helper function (keys under
an `id` prefix, times as numbers a scene passes from `word()`) and a
default export that demos it; a scene copies the helper into its own
component. The starters are primitives, never scene templates: a heading
over lines, two text columns or a stat card is the slide shape the
Studio skill bans, so none ships. A primitive earns its place when it
solves a layout or mapping problem a scene would otherwise redo (data to
px, a fitted grid, columns of characters) and keys every piece it draws,
so two with one `id` glide in a `Steps` and an overlay can point at a
part. Each takes its colours and fonts from the theme and has at least
two examples; only `Code` reads `font.mono`. The default export of `Axes`,
`Graph`, `Grid`, `Bars` and `Code` sizes itself from `ctx.size`, so the demo
fits 1280x720, 1920x1080 and 1080x1920 frames; their helpers take explicit
`width` and `height` (`Code`: `maxWidth`, `maxHeight`), and a scene that
copies one passes its own. Bad input to one of them throws an error naming
the prop, the value and what is allowed (a ragged `cells`, a duplicate
`keys`, a `path` that leaves the edges, a `diff` off the bars), never a
blank or broken frame. `tests/starter.rs` lints the set
in both themes, and `tests/gallery_golden.rs` pins each starter as one
sheet, `starter-<Component>.png`, a row per example and a column per
moment of the 6 s preview (laid out at 1920x1080, drawn at a quarter), so
the draw-on is pinned too. A load copies each starter
file the global library has never been offered, then lists it in the
library's `.seeded` as `path<TAB>sha256 of the text written` (a legacy line
has no tab and no hash). A listed file is never copied again, so a deleted
starter stays deleted, while one added to a later build still arrives. A copy
whose bytes still hash to the recorded text was never edited: when the
starter has changed, the load rewrites it (the old text goes to `.history/`)
and records the new hash. Any other text is the user's edit, left alone, and
its hash is never recorded, so no later update can overwrite it. A legacy
line has no hash to compare, so its copy counts as untouched only if it
equals a version the repo has ever shipped (`library/shipped.rs`, SHA-256 of
every blob in the starter files' git history; `bun run studio:hashes` in
`app/` regenerates it, and a unit test fails until a starter edit's hash is
in it); any other text stays hashless and is never updated. A copy
that equals the current starter gets the current hash recorded. A starter
dropped from the set is neither updated nor deleted: a library's existing
copy stays, edited or not, and its `.seeded` line stays as it was, so a
video that uses it keeps rendering. Such a copy
named like a prelude element (`Steps.tsx`) shadows nothing: each
component is its own module and is called only by name from video.json,
so inside every component `<Steps>` is still the element. `oculus studio
components seed --reset` writes the starter text over every file that
differs or is missing, recording the hashes it writes. Every write
to a library file first copies the previous text to `.history/` beside it
(`<Name>.<unix-ms>.tsx` or `.json`, the millisecond moved on past any
snapshot already there); nothing prunes `.history`. `oculus studio
components list <DIR>` prints each name with the level it resolves from, and
`components show <DIR> <Name>` its source.

**A library's default theme** is `library.json` at its root
(`<app data>/studio/library/library.json`, `<app data>/studio/subjects/<CODE>/library.json`),
`{"defaultTheme": "<name>"}` and nothing else (`studio_core::LibrarySettings`,
`deny_unknown_fields`); a library without the file has no default. A new
video that names no theme (`oculus studio new`, the app's New video with no
theme picked) takes its subject library's default, else the global
library's, else the bundled `default`
(`studio::libraries::default_theme`), and video.json gets that name. A
default naming a theme its cascade no longer has is passed over; a
`library.json` that does not parse stops the video being made, naming the
file. Agents may edit the file by hand; the Themes page sets it with
`studio_theme_set_default`. A theme's name is its file stem, lowercase
letters, digits and `-`, at most 64 characters
(`studio_core::valid_theme_name`), the one rule for a scaffold's theme, a
library write, a promote and a lookup.

A library grows through a video: a component is written and tuned in the
video's own `components/`, where it wins the cascade, checked with
`gallery` and `strip`, then copied into a library with `oculus studio
components promote <DIR> <Name> --to global|subject`
(`studio::libraries::promote_component`). Only a component the video itself
has (level `project`) can be promoted; `--to subject` writes to the subject
in video.json and fails when there is none. `oculus studio themes promote
<DIR> <FILE> --to global|subject` does the same for a theme file inside the
video (a `.json` path such as `themes/chalk.json`, checked to load as a
theme) as `themes/<stem>.json`, the stem following the theme name rule, so
any video can name it. Both go through the snapshotting write,
write nothing when the library already has the same text, and leave the
video's copy in place.

The render segment key covers the source of every component a scene uses, as
the cascade resolves it, so an edit to a library component re-renders the
scenes that use it.

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
  removed, pause markers cut out and whitespace collapsed; nothing else is
  interpreted (`#` and `###` lines are prose). Text before the first
  heading, a heading that is not an id, a repeated id and an empty section
  fail `Project::load` with the line. A section no scene's `id` matches is
  a lint error.
- **Pauses.** `[pause]` anywhere in a section's prose holds the picture in
  silence for the theme's `narration.pause` seconds, and `[pause 1.5]` for
  that many: more than 0 and at most `MAX_PAUSE` (5 s); `pause` in any
  case. A marker is not said and splits the words either side of it;
  `ScriptSection::pauses` records each one's place (how many words come
  before it) and length, and at the end of a section it holds the last
  picture before the cut. A bracket opening with `pause` that is neither
  form (`[pause x]`, `[pause 9]`, no `]`) is cut from the text too and is
  a lint error (`project/script/pause.rs`). `ScriptSection::clip_text`
  writes the pauses back into the text at their lengths (`It lands. [pause
  1] Then it stays.`), a bare one at the theme's; without pauses it is the
  text. That is what a clip is made from and what its manifest entry
  matches, so moving a marker or changing its length, or the theme's,
  makes the scene stale.
- **The manifest** is `{ version: 1, scenes: { <id>: { key, text, audio,
  duration, words: [{ text, start, end, confidence }], gaps: [{ after,
  start, end, heard }], pauses: [{ after, start, end }] } } }`
  (`Manifest`). `text` is the section's `clip_text`, `audio` the clip's
  file name in `narration/`, `key` its content
  address (see "Cache" under "Narration is made by `oculus studio
  narrate`"), times are seconds from the clip's start, `confidence` is the
  aligner's (mean log-probability of the word's letter frames; higher is
  surer), and `gaps` (optional) is speech the aligner heard that the
  script does not have, after word index `after` (`null`: before the first
  word). `pauses` is the exact silence each `[pause]` put in the clip,
  after word index `after` (`null`: before the first word); it is left out
  when there are none, so a manifest from before pauses reads unchanged.
  Unknown keys, another version, non-finite times and a word, gap or pause
  ending before it starts fail the load.
- **Length.** A narrated scene lasts `max(hold, lead + duration + tail)`:
  `hold` is optional and only a minimum, and `duration` includes the
  clip's pauses. `lead`, `tail` and `pause` are the theme's `narration`
  tokens (0.25 s, 1.0 s and 1.0 s in the bundled themes); the clip starts
  `lead` into the scene. `pause` may be left out of a theme file and reads
  as `DEFAULT_PAUSE` (1.0 s), so a theme copied into a library before it
  existed still loads. A silent scene (no section) lasts its `hold`, and
  one without a `hold` has no length. Either rounds up to whole frames:
  4.25 s is 128 frames at 30 fps, 4.267 s.
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
  apostrophes kept. A hyphen or dash splits a word into parts, in the phrase
  and the narration alike, so `word("ninety one")` and `word("ninety-one")`
  both find the said `ninety-one`; a phrase is consecutive parts that start
  and end on whole said words (`word("one")` does not find it), since only
  words have times. A phrase that is not said, an `nth` past the last
  occurrence and a call in a silent scene throw at the call's TSX line. A
  phrase not said names the said run of words nearest it by edit distance,
  however far, spelt with its hyphens (`suggest::nearest`): ``Descent.tsx:12:20:
  `word("gradiant")`: no such word in scene `descent`; closest: gradient``.
  Each call gets its own scene's words (`SceneCall::words`, from
  `Project::word_cues`), so in a transition the outgoing scene cues to its
  words and the incoming one to its own.
- **Pause cues.** `pause(nth = 1)` is the scene time (lead included) at
  which the nth `[pause]`'s silence starts (`Project::cued_pauses`), so a
  beat can land just before it and hold through it. A scene without that
  many pauses, and a silent scene, throw at the call's line: ``scene `a`
  has 2 pauses, so there is no pause 3``.

## Narration timing is forced alignment on the CPU

The app crate times a clip's words (`app/src-tauri/src/studio/timing/`):
CTC forced alignment of the script against wav2vec2's letter emissions, run
by ONNX Runtime on the CPU (`app/src-tauri/src/runtime/onnx/`). The model
is Xenova's fp32 ONNX export of `facebook/wav2vec2-base-960h` (Apache-2.0,
credit facebook/wav2vec2-base-960h), pinned to one commit, 378 MB. It is
downloaded on request (Settings -> Voice, or `oculus studio aligner
download`) to
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
- **Long clips run in windows** (`timing/windows.rs`), since the model's
  self-attention makes its memory grow with the square of the audio it sees
  (over 2 GB for a 57 s clip in one pass). A clip of up to 40 s runs in one
  pass. A longer one is cut into `ceil(length / 30 s)` windows of even
  length, each cut moved to the quietest 20 ms frame (least energy, ties to
  the nearest) within 3 s of its even position, and no window under 15 s.
  Each window runs with 2 s of context audio on both sides, normalised on
  its own, and keeps only its own frames, so the stitched emissions have
  exactly the frames one pass would (a frame per 320 samples whose 400
  sample receptive field fits). The Viterbi then runs once over the whole
  clip: words never meet a seam. No run sees more than 40 s, so peak
  memory stays about 1.3-1.6 GB at any length.
- **Windows change a few edges.** wav2vec2 attends over everything it is
  given, so a window's emissions differ from one pass's throughout, not only
  near its cuts (the cuts themselves sit in silence). On a 57 s clip, two
  windows moved 15 of 304 word edges by one frame (20 ms) and none further,
  and confidences by up to 1.1; shorter windows move more (20 s: 10%, some
  by two frames). Appending a second of silence to a one-pass clip moves an
  edge too.
- **Sessions and threads.** Each model run (the clip, or each window) opens
  its own session and drops it, since a reused one keeps growing (two
  windows of a 57 s clip: 1.8 GB on one session, 1.3 GB on one each); a
  load is about 0.2 s. Threads: half the cores, at most 4. No timeouts.
- **The Viterbi** keeps two bits of backpointer per frame and state (two
  states per letter or word break, plus one), four to a byte: about 1.5 MB
  per minute squared of speech, 40 MB at 5 minutes, 155 MB at 10.
- **Measured** (debug build, Kokoro `af_heart`): a 170 s clip timed in
  9.2 s and a 371 s clip (13 windows) in 22.5 s, both at a 1.6 GB peak,
  no word under -4.2, no gaps, and the last word ending within 40 ms of
  the speech.

## Narration is made by `oculus studio narrate`

`narrate` (`app/src-tauri/src/studio/narrate/`) turns each scene's
`script.md` section into a clip and its word times, and writes the
manifest. Scenes run one at a time, in video order.

- **Voice.** Each of engine, voice and speed, and for the server engine
  the server and model, comes from the first that sets it: the CLI flags
  (`--engine`, `--voice`, `--speed`, `--server`, `--model`), the video's
  `voice` block, then the `voice` settings row (`{ engine, baseUrl, model,
  voice, speed }`; `studio::voice`). A missing or ill-typed field in the
  row reads as unset, and so does an engine other than `server` or
  `kokoro` (in video.json it fails the load); a row with a `baseUrl` and
  no `engine` key reads as `server`. Only speed has a default
  (`voice::DEFAULT_SPEED`, 0.9: a little under the voice's own pace, since
  a player can always speed a lecture up); a stored speed is kept.
  There is no default engine, since Kokoro needs a download and a server
  a URL: narrate fails with ``choose Kokoro or a TTS server in Settings ->
  Voice, or pass --engine``. The server engine needs a server, model and
  voice, whose ids differ per server (Kokoro-FastAPI `kokoro`, mlx-audio
  `mlx-community/Kokoro-82M-bf16`, OpenAI `tts-1`): narrate fails with
  ``set a TTS server in Settings -> Voice, or pass --server`` and the same
  for `--model` and `--voice`. The Kokoro engine needs only a voice, an id
  of lower-case letters, digits and `_` (it names a file in the voice
  pack); it ignores the server fields, which stay set for when the server
  engine is chosen again.
- **Engines.** `narrate` asks for clips through the `Speaker` trait
  (`narrate::Speaker`): `ready()` is asked once, beside the aligner's,
  before the first clip; `key(text)` is asked for every scene, cached or
  not, and gives the parts of its cache key, engine first; `speak(text)`
  gives a WAV and its decoded samples, never silent. Both get the
  section's text without its pauses. `ServerSpeaker` is
  the TTS client below and has no readiness check of its own;
  `KokoroSpeaker` runs Kokoro inside the app (see "Kokoro narrates inside
  the app"). A video has one engine: when it fails, the narrate fails and
  says why, with no fallback to the other.
- **The TTS server** is OpenAI-compatible (Kokoro-FastAPI, mlx-audio):
  `POST <base>/v1/audio/speech` with `{ model, input, voice,
  response_format: "wav", speed }`, one request per clip, no timeout
  (`studio::tts`). The base URL is accepted with or
  without a trailing `/v1`, normalised once. Keyless servers only: no API
  key is sent. A non-2xx answer fails with its status and a short excerpt of the body; a
  429 is retried with back-off (1, 2, 4, 8 s; five tries). A 200 whose body
  is empty, is not a WAV, or is digitally silent (see "Input" above) is a
  failure, never a pause: Kokoro-FastAPI answers an unknown voice with 200
  and nothing.
- **Cache.** A clip's `key` is the sha256 hex of `oculus-narration-2`, the
  section's `clip_text` (its text with each pause at its length), the
  speaker's key parts and the aligner's `MODEL_ID`, each length-prefixed
  (`narrate/key.rs`). Without pauses `clip_text` is the text, so a section
  keeps the key it had before pauses existed and its clip is kept. The server's parts are `server`, the
  normalised URL, model, voice and speed. Kokoro's are `kokoro`, its model
  id (`kokoro-82m-v1.0-onnx-int8@1939ad2a`), voice, speed and the text's
  phoneme string, so a lexicon edit re-narrates only the scenes whose
  phonemes it changes. The text is in every key because a manifest entry
  is current only for the text it was timed against. A scene whose
  manifest entry has the same key and whose `narration/<key>.wav` exists
  is kept (`cached`) unless `--force`; editing one paragraph re-makes that
  one scene. A scene whose key cannot be made (Kokoro: the G2P pack is
  missing, a lexicon entry is bad, a word has no phonemes) fails naming
  the scene.
- **A new clip** is saved byte for byte as the speaker made it to
  `narration/<key>.wav`; its duration comes from the decoded samples, and
  it is resampled to 16 kHz and timed against the section's
  whitespace-separated words, at any length. A section with pauses
  (`narrate/clip.rs`) is said and timed the same way, whole, and then each
  pause is spliced in as exact zeros: before the clip for a pause before
  the first word, after it for one after the last, else at the quietest
  10 ms (ties to the middle) between the end of the last word before it
  and the start of the next, so the cut falls in the silence the voice
  already left there. Words and gaps after a cut move on by its length,
  each pause goes into the manifest as `{ after, start, end }`, and the
  clip is written as a 16-bit mono WAV at the speaker's rate. The words
  are said whole because Kokoro paces an input by its length: the same
  scene said in parts between its pauses came out up to 11% slower, part
  by part, than said whole (`af_heart` at 0.9, the first of three parts
  1.7 s behind over 16 s); a server is asked once, too, and the aligner
  never sees the inserted silence. The silence check runs on the clip as
  spoken, never on the spliced one, so a pause is not mistaken for a
  failed clip. The speaker and the aligner
  model are checked before the first clip, so a missing one costs no
  synthesis; the aligner fails with ``download it in Settings -> Voice or
  run `oculus studio aligner download` ``.
- **Guessed words.** With Kokoro, each word the guesser read in the scenes
  the run keyed is printed after the scenes as ``note: scene 1 `momentum`
  script.md:9: check pronunciation: "Nesterov" is guessed as
  /nˈɛstəɹɔv/; ...`` (`voice::lint::notes`), ending with the state of the
  word's suggestion when one is stored (see "The student reviews the
  agent's suggested readings"). A note never changes the exit code; the
  fix is a suggested reading the student accepts, then another `narrate`.
- **Writes.** The manifest is rewritten atomically after each scene, so a
  failure keeps the scenes before it; the error names the scene and the
  command exits 1. After the last scene, entries whose id is no longer a
  narrated scene are dropped and every `.wav` (or interrupted `.wav.part`)
  in `narration/` that no entry names is deleted; other files are left. A
  manifest this build cannot read (another version, damaged) is rebuilt
  from nothing, with a warning.
- **Output.** Per scene: the id, `narrated` or `cached`, the clip's length,
  its word count, its pauses when it has any, and as warnings each word under `LOW_CONFIDENCE` and each
  gap (times into the clip; lint notes the same words and fails on the
  same gaps, at scene time),
  then the notes. `--json` gives `{ dir, voice, scenes: [{ id, outcome,
  audio, duration, words, pauses, low_confidence, gaps, timings }], notes: [{
  scene, id, line, word, phonemes, guessed }], dropped, deleted, ms }`,
  `voice` being `{ engine: "server", server, model, voice, speed }` or `{
  engine: "kokoro", voice, speed }`, and each note also has `review`
  (`null`, or `{ status, reading, note }`).
- `oculus studio tts "<text>"` makes one clip the same way and writes it
  (`tts.wav` unless `-o`), for hearing a voice; with Kokoro it reads the
  global library's lexicon only. `oculus studio g2p "<text>"` shows how
  Kokoro reads text without synthesising. `oculus studio voices` and
  `oculus studio aligner status|download|delete` manage the downloaded
  models.
- **Measured** on the server engine (mlx-audio Kokoro, `af_heart`,
  M-series Mac, debug build): three scenes of 4 to 8 s narrated in 1.8 s
  with a 0.67 GB peak; a re-run after one paragraph edit took 0.5 s.
  Kokoro's clips start with about 0.2 s and end with about 0.4 s of exact
  zeros, well under the 1 s zero-run limit. The Kokoro engine's numbers
  are in the next section.

## Kokoro narrates inside the app

The `kokoro` engine (`app/src-tauri/src/studio/tts/kokoro/`) runs
Kokoro-82M on the ONNX Runtime the aligner uses, with no server and no
side process. It needs two packs, downloaded together on request (Settings
-> Voice, or `oculus studio voices download`; one download at a time, and a
cancel between the packs stops the second) to `<app data>/models/voice/`,
checked against their
pinned sizes and sha256 before they get their names, and never bundled:

| Pack | Files | Size |
| --- | --- | --- |
| `voice-kokoro` | `kokoro-v1.0-int8.onnx` (onnx-community/Kokoro-82M-v1.0-ONNX's `model_quantized.onnx`, Apache-2.0) and the voices `af_heart.bin` and `am_michael.bin`, at commit `1939ad2a` | 93 MB |
| `g2p-en-us` | misaki's `us_gold.json` and `us_silver.json` (Apache-2.0, commit `e820629b`), and the guesser's `g2p-encoder.onnx`, `g2p-decoder.onnx` and `g2p-config.json` (PeterReid/graphemes_to_phonemes_en_us, PR #1 at `9470bafd`) | 9 MB |

The voices are US English, as the dictionary and the guesser are. Another
voice is one more `<id>.bin` in the folder at a voice file's size (510
rows of 256 f32s); `oculus studio voices` lists it. The model is int8:
fp16 and q4f16 exports come out silent with no error.

- **One speaker per run.** `KokoroSpeaker::ready` checks both packs and
  the voice's file. The reader (the dictionary, the video's lexicon and
  the guesser; `reader.rs`) loads on the first key and the model session
  on the first clip; both are dropped with the speaker when the run ends,
  never held idle. No timeouts on either.
- **Reading.** Each section's text is read to phonemes by the G2P (next
  section). A `$` is refused: it is Kokoro's pad.
- **Chunking** (`chunk.rs`). An input holds at most 509 tokens, since the
  voice file has a style row per input length. The phoneme string is cut
  after each sentence end (`.`, `!`, `?`, `…`, with any closing quote or
  bracket) that whitespace follows, and sentences are packed greedily
  under the limit; a sentence too long alone is cut at `,`, `;` and `:`
  the same way, and a part still too long fails naming its sentence's
  words. Whitespace runs read as one space. A character outside Kokoro's
  115-symbol vocabulary (`vocab.rs`) fails naming its word, since Kokoro
  would drop it silently.
- **Synthesis** (`synth.rs`). Each input runs as `[0, ids..., 0]` with the
  voice's style row `len(ids)` and the speed, on the shared CPU session
  (`runtime::onnx::session`, arena on, half the cores up to 4). An input
  that comes out digitally silent fails naming its words. The samples join
  with no gap and are written as a 16-bit mono 24 kHz WAV, the shape a
  server's clip has; the silence check runs on the float samples first.
- **Measured** (`tts/kokoro/tests/e2e.rs`: two scenes, 19.3 s of audio,
  `af_heart`, M-series Mac with other builds running, debug build around
  a static ORT 1.28): synthesis at a real-time factor of 0.40 (0.61 on a
  busier run), the first clip's figure including the session load; 0.9 s
  of alignment; 0 unsure words and no gaps; a 1.12 GB peak for the whole
  run, aligner included, and 1.27 GB after two more runs in the same
  process. Synthesis and alignment therefore share one pass per scene. On
  ORT 1.28 most of the time is one op, `ConvInteger`; ORT 1.31 runs the
  same model about four times faster.

## Kokoro reads text through the G2P

`app/src-tauri/src/studio/voice/g2p/` turns a section's text into
Kokoro's phonemes and one record per word, `{ word, phonemes, source,
guessed }`. The text is already spoken form (lint forbids digits and
symbols), so numbers and currency are not read. It is a port of misaki
0.9.4's dictionary path without spaCy or espeak, and matches its oracle's
golden files byte for byte (`tests/golden/`, with a dictionary trimmed to
the entries they touch). Each word, or each part of a merged word
(`PyTorch`, `scikit-learn`, of which only the unknown part is guessed), is
read by the first of:

1. **The lexicon** (`source: lexicon`), below.
2. **misaki's dictionary** (`dict`): `us_gold.json`, then `us_silver.json`,
   grown as misaki's `grow_dictionary` does, with its special cases (a, an,
   the, to, I, by, in, used, dotted acronyms), case handling (an unknown
   all-caps word is spelt out), possessives, the `-s`, `-ed` and `-ing`
   stems, and three rules misaki lacks: `-ied` read from `-y`
   ("multiplied"), an `-izer` agent noun read as its `-ize` verb + `əɹ`
   ("optimizers"), and the British `-ise` family read as `-ize`
   ("renormalise", "factorisation", "optimisers").
3. **The heteronym table** (`rule`, `heteronym.rs`): read, lead, live,
   content, used, that and close, and the generic verb, noun or adjective
   reading, picked from the neighbouring words without a tagger; `lead`
   also has the metal reading the dictionary lacks.
4. **The guesser** (`guess`, `guess/`): a 3 MB BART (one encoder and one
   decoder layer) run as two ONNX sessions, greedy, at most 63 steps.

Stress is applied as misaki does, and `ɾ` and `ʔ` become `T` and `t`. A
word with a letter in it and no phonemes is an error naming it: a dropped
word would wreck its neighbours' timing.

**The lexicon** is a `lexicon.json` object from a word to its reading, in
two layers, the subject's over the library's:
`<app data>/studio/library/lexicon.json` and
`<app data>/studio/subjects/<CODE>/lexicon.json` for the video's
`subject`. A value in slashes is raw phonemes in Kokoro's alphabet
(`"Cauchy": "/kˈOʃi/"`), any of the 115 symbols but the pad `$`. Any other
value is a respelling in dictionary words (`"softmax": "SOFT-max"`): each
`-` or space part is read through the dictionary, an all-caps part longer
than one letter takes primary stress and the rest secondary. Keys starting
`_` are comments. A word finds its entry by exact key, then its lower
case, then (ending `'s`) its stem's entry plus `-s`, then any key equal in
lower case; `’` reads as `'` in keys and in the words looked up. Every
entry is checked on load, and a bad phoneme or a respelling part the
dictionary lacks fails naming the file and the key. The app writes the
lexicon files, from the student's review (next section); the agent's skill
tells it never to edit them.

**Guessed words are notes.** A guess never stops narration, but every
guessed word is printed by `narrate` and `lint` as a `check pronunciation`
note with its scene and script.md line. The agent suggests a reading for
the doubtful ones and the student decides (next section); only the scenes
whose phonemes a decision changed are made again at the next narrate.

`oculus studio g2p "<text>"` prints each word, its phonemes and its
source, marking guesses, then the phoneme string; `--json` gives `{ text,
phonemes, subject, lexicon: [<files>], words: [{ word, phonemes, source,
guessed }] }`. It needs only the `g2p-en-us` pack.

## The student reviews the agent's suggested readings

`app/src-tauri/src/studio/voice/review/` holds the readings an agent
suggested for words it doubts, until the student decides on each.

- **A suggestion is one file**,
  `<app data>/studio/pronunciations/<CODE or _library>/<key>.json`: the
  video's subject, or `_library` for a video without one, and the word
  lower-cased with every character outside `a-z`, `0-9` and `-` written
  `_<hex>_` (`don't` is `don_27_t`). Its id is `<folder>/<key>`. The CLI
  and the app are two processes with no lock between them, so neither ever
  rewrites a list the other holds: each writes one file, atomically
  (`runtime::atomic_write::json`). The file is `{ word, reading, original,
  guessed, subject, video, sentence, scene, line, status, note, created }`:
  the word as the script spells it, the suggested lexicon value, the
  phonemes Kokoro read it with when it was suggested, whether the guesser
  read it, the subject (or `null`), the video's slug, the spoken sentence
  it is in (whitespace collapsed), the scene id and script.md line,
  `pending` or `wrong`, the student's note (or `null`) and the UTC time
  (`YYYY-MM-DDTHH:MM:SS`). A new suggestion for the same subject and word
  replaces the file, back to `pending`.
- **`oculus studio pronounce suggest <DIR> <WORD> <READING>`** checks the
  reading as a lexicon value (`review::check`, the lexicon's own
  validation and wording, naming `the reading` as its file), then reads
  the video's narrated sections with the reader `narrate` would use and
  finds the word in any case, `Word's` counting as `Word`
  (`review::locate`): its first guessed place, else its first place. A
  possessive's original is the stem read alone. A word the narration never
  says fails; a word that is not a guess is still stored, with the
  phonemes it reads with now. It prints the reading's phonemes, how the
  word reads now, the scene, line and sentence, and that the student
  reviews it in Settings -> Voice (`--json`: the item plus `phonemes`).
- **`oculus studio pronounce list [--video DIR]`** prints each item,
  newest first: its status, word, reading, subject, video, scene and line,
  and under a wrong one the student's note (`--json`: `[{ id, ...the
  file's fields }]`). A file that does not parse is skipped and logged.
- **Notes.** `narrate` and `lint` look up each guessed word's file under
  the video's subject, a possessive's under its stem (`Store::annotate`),
  and end its note with
  `(suggested "NESS-ter-off", awaiting review in Settings -> Voice)` or
  `(suggestion "NESS-ter-off" marked wrong: "<note>"; suggest again)`.
- **Deciding** (`Store::decide`) writes the word into the subject's
  `lexicon.json`, or the global library's for a video without a subject:
  `correct` writes the suggestion, `fine` the original as `/phonemes/`, so
  the word is no longer a guess. The value is checked first, against the
  dictionary of the `g2p-en-us` pack. The writer (`review::write_entry`)
  keeps every other key, `_` comments included, in its place (serde_json's
  `preserve_order`), keeps a replaced key's place, creates a missing file,
  and refuses a file that is not a JSON object rather than overwrite it.
  The item's file is then deleted, unless a new suggestion replaced it
  meanwhile. A decision applies at the video's next narrate.
- **Tauri commands** (`commands/pronunciations.rs`, wrapped in
  `lib/studio/pronunciations.ts`): `studio_pronunciations_list()` gives
  every item; `studio_pronunciation_decide(id, decision: "correct" |
  "fine")` gives `{ lexicon, word, reading }`;
  `studio_pronunciation_wrong(id, note?)` gives the item, a blank note
  stored as none; `studio_pronunciation_sample(id, which: "original" |
  "suggestion")` synthesises the item's sentence as `studio_voice_sample`
  does, in Settings -> Voice's voice and speed, reading the subject's
  lexicon with one entry over it that pins the word to the original
  phonemes or to the suggestion (`LexiconFiles::pin`), and gives the WAV's
  bytes. Settings -> Voice's Pronunciations section is built on them
  (next section).

## Settings -> Voice chooses the engine and holds the models

Settings -> Voice (Library group) has three sections. Settings never makes a
billed call: its one network call to a server is listing models, which is
free, and the only sample is Kokoro's, made on this Mac.

- **Narration voice** edits the `voice` row. **Engine** is a bordered list
  of two rows, "Kokoro (built in)" and "TTS server", each with a radio dot
  and a status line (Kokoro: whether it runs now or after its download, or
  the download's percentage; server: its URL and model, or "No server
  set"). A whole row picks its engine. Nothing is chosen until the user
  picks (the app never preselects an engine), and with none chosen a
  warning says Studio can't narrate. Only the chosen engine's fields show,
  then Speed (a slider, 0.5 to 2.0 in 0.05 steps), which both share.
  Switching engine keeps the other's fields stored; Voice is one field for
  both.
  - **Kokoro**: the packs' row, "Kokoro voice packs", with each pack's size
    (Voice model 93 MB, Pronunciation 9 MB) and one Download for both, then
    a percentage, a progress bar and Cancel, or Delete behind a
    confirmation, as Word timing's row has; a Voice select listing
    `kokoro::voices` by name ("Heart · American, female" for `af_heart`):
    the pack's two, then any other voice file in the folder (a voice
    stored for the server that is not one stays in the select, flagged);
    and **Sample**, which plays a fixed sentence in the chosen voice at the
    slider's speed. It is disabled until both packs and a voice are there,
    shows "Making…" while the clip is made (a few seconds: the model loads
    each time) and Stop while it plays; the credit line follows.
  - **TTS server**: Server URL, Model and Voice (typed: the OpenAI API
    cannot list voices), and the note that API keys are not supported. "List
    models" calls `studio_voice_models` with the URL as typed: `GET
    <base>/v1/models`, the `data[].id`s in the server's order, with only
    ureq's connect timeout (`studio::tts::list_models`). A non-empty list
    turns the Model field into a select (the stored model stays in it when
    the server does not list it); an empty list or a failure leaves the text
    field, with the reason inline. Editing the URL drops the list. A server
    has no sample button: it could bill.

  A text field saves on blur or Enter, a select or the slider on change;
  each save sends the whole row to `studio_set_voice_settings`, which trims
  it, unsets blanks, normalises the URL (`tts::normalise_base`), refuses a
  speed that is not a positive number, and answers with what it stored. A
  refusal shows under its row.
- **Pronunciations** (`components/settings/voice/pronunciations/`) lists
  the review's items, newest first, under a one-line hint. It loads on
  mount and again whenever the window regains focus, since the CLI adds
  items while the page is open; with none it says "No words waiting for
  review." A row shows the word and its suggested reading, the sentence
  muted with the word picked out, and `<subject or Library> · <video> ·
  scene <id>`. On the right, **Original** and **Suggestion** play the
  sentence read each way (`studio_pronunciation_sample`; "Making…" while
  Kokoro makes it, a click again stops). One clip plays at a time across
  the section (`useClipPlayer`, which the voice sample also uses; blob URLs
  revoked at the end, on Stop and when the page goes). Beneath, three
  verdicts: **Suggestion correct** and **Pronunciation is fine** decide,
  and the row gives way to a muted line saying which lexicon it went into
  and that it applies when the video is next narrated, until the next
  reload; **Suggestion wrong** opens "How is it said? (optional)", a text
  field with Send and Cancel (Enter and Escape). A wrong row stays listed,
  muted, with its samples and "Sent back to the agent" and the student's
  note in place of the verdicts, until the agent's next suggestion replaces
  it as pending. A row's buttons wait while its request runs, and a failed
  request or sample shows in red on the row.
- **Word timing** is the aligner model's row, like a Local Whisper model:
  "wav2vec2 base (English)", its 378 MB size and status, Download (then a
  percentage, a progress bar and Cancel) or Delete behind a confirmation,
  and the credit line.

Both downloads run the same way (`commands/progress.rs`;
`usePackDownload` and `PackDownloadRow` on the page). The status commands
read the disk alone: `studio_aligner_status` gives `{ status: missing |
downloading | present, bytes }`, and `studio_voice_packs_status` gives the
same for both packs together (present only when both are) plus `packs: [{
id, status, bytes }]` and `voices: [{ id, downloaded }]`. The download
commands (`studio_aligner_download`, `studio_voice_packs_download`) resolve
when the files are on disk, reject `cancelled` on the matching cancel
command, and emit `studio-aligner-progress` or `studio-voice-progress` `{
phase: downloading | complete | cancelled | error, received, total, error?
}` about every 150 ms; Kokoro's counts bytes over both packs. The delete
commands stop a download and return the bytes freed; Kokoro's leaves any
other voice file. A download started on an earlier visit shows at 0 %
until its next event.

`studio_voice_sample(voice, speed, text?)` (`commands/kokoro.rs`) checks the
voice id and speed as narrate does, makes a `KokoroSpeaker` reading the
global library's lexicon, synthesises `text` or a fixed spoken-form
sentence, and returns the 24 kHz WAV's bytes; the speaker, its reader and
its model session are dropped on return. The page plays them from a blob
URL (`useVoiceSample`), released on Stop, at the clip's end, or when the
page goes. Measured on a debug build: about 6.5 s of audio in 3.6 to 4.5 s,
model load included.

There is no narrate command in the app yet: narration runs from `oculus
studio narrate`.

## Four primitives and a closed set of style keys

`Box`, `Text`, `Image` and `Path` are the host elements, `Morph` holds
two layouts of one place (see "Magic move"), `Math` is LaTeX that
becomes primitives before layout (see "Math") and the overlays are drawn
from keyed nodes after it (see "Overlays"); `Stack` (a column) and `Row`
are `Box` with `gap`, `align`, `justify` and `wrap` shorthands. Elements take
only `style`, `key`, children, `src` on `Image`, `d`, `viewBox`, `fill`,
`stroke` and `trim` on `Path`, `p`, `lag`, `fade` and `ease` on `Morph`,
and `tex` and `display` on `Math`.

- `flexDirection` defaults to `row`, as in CSS. Use `Stack` for a column.
- `Text` children must be strings or numbers; to mix styles put several
  `Text` in a `Row`. A `Text`'s baseline is its first line's, so a `Row`
  with `alignItems: "baseline"` lines texts of different sizes (and `Math`)
  up on it.
- `Image src` is relative to `assets/`. A missing or undecodable file is an
  error, never a placeholder. Decoded images are cached per renderer. A
  `src` starting `starter:` is an image bundled in the crate
  (`images::BUNDLED`, files in `app/studio-core/library/assets/`), so a
  starter's examples render with no assets folder.
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

## Math typesets LaTeX as keyed glyphs

`<Math tex="\frac{a}{b}" display key style>` is LaTeX typeset by the
maths engine's KaTeX port (`app/math-core/katex`, KaTeX 0.18.5, the app's
own maths), so a formula looks as it does in the app. `display` (default
`true`) is display style, limits above and below; `false` is inline maths.

- **Expansion.** After the component returns and before `select` and
  layout, each `Math` becomes a Box the formula's size (its width by its
  height plus depth) holding one absolutely placed child per piece: a
  filled `Path` per glyph (its outline from the bundled KaTeX fonts, read
  with skrifa), a filled `Box` per rule (fraction bars, over- and
  underlines, `\rule`, `\boxed` frames, `\colorbox` backgrounds) and a
  `Path` per KaTeX SVG (root signs, tall and stretchy delimiters, braces,
  arrows; `\cancel` strokes). Its first child is an empty strut as tall as
  the formula's height, so the Box's baseline is the formula's.
- **Layout.** `render_to_html_tree` gives KaTeX's HTML tree; `math/walk/`
  lays it out as katex.css would in a browser: inline spans on one baseline
  (margins, padding, widths, relative shifts, `vertical-align`), vlists by
  their rows' `top` and pstrut, `width: 100%` bars and root tails across
  their vlist, `text-align` per class, the size ladder, and each class's
  font. SVG is mapped by its `viewBox` and `preserveAspectRatio` and
  clipped to its `overflow: hidden` boxes by clamping its points, since
  KaTeX's paths run far past what shows. The fonts have no kerning or
  ligatures, so glyphs advance by their widths alone. Against KaTeX in
  WebKit at 200 px per em (the `webkit` test in `src/math/tests/`), every
  glyph, rule and SVG box of a corpus of fractions, scripts, roots,
  delimiters, limits, matrices, accents, braces, boxes, laps and `\cancel`
  is within 0.006 em.
- **Size and colour.** `style.fontSize` (default 16) is the size of the
  text the formula sits with; maths is set 1.21 times larger, as KaTeX's CSS
  does. `style.color` (default the theme's text colour) inks every piece;
  `\color{…}` and `\textcolor{…}{…}` take a theme colour's name
  (`\color{accent}`) or a hex colour, and a CSS colour name is an error
  listing the theme's. What `\phantom` hides (KaTeX's `transparent`) takes
  its space and draws no piece. The other text keys are ignored, so a `theme.type`
  token can be spread in; `width`, `height`, their min and max, `padding`,
  `border`, the flex container keys, `textAlign`, `reveal` and `objectFit`
  are errors (wrap the Math in a Box to size or pad it). Placement and
  paint keys (`margin`, `position`, `opacity`, transforms, …) apply to the
  Box.
- **Keys** make a `<Morph>` between two formulas move term by term, as
  manim's TransformMatchingTex does. Every piece is keyed `<prefix>/<term>`.
  A term is its label: its character (KaTeX's minus is `−`), `frac` for a
  fraction bar, `sqrt` for a root sign, a box side `box`, and KaTeX's path
  name for other SVG; then `^` or `_` for each script it sits in,
  outermost first (a superscript, a limit above an operator and a root's
  index are `^`; a subscript and a limit below are `_`); then, from the
  term's second appearance, `#n`. So `x^2 - 5x` is `x`, `2^`, `−`, `5`,
  `x#2`, and `x_{i_0}` ends in `0__`: the exponent of `x^2` never flies
  into the `2` of `(x - 2)`, and the nth baseline `x` meets the nth.
  Fraction terms keep their level (no mark), so a numerator `a` meets a
  baseline `a` when both sides are multiplied out. A script's key is not
  covered by its base's as an overlay target (`eq/4^` is not under
  `eq/4`).
  `\k{name}{…}` (a macro over `\htmlData{k=name}`, with trust on) names a
  group: its pieces are `name`, `name.1`, `name.2`, … in reading order, the
  innermost group winning. Reading order is the tree's, with each vlist
  read top to bottom: numerator, bar, denominator; a root sign before its
  radicand. A group named like a term of the same formula is an error. The
  prefix is the Math's `key`; two formulas given one key in a Morph glide
  the terms they share and fade the rest, and a Math elsewhere never
  collides with them. A Math without a key inside a Morph is prefixed by
  its place, `~<morph>.<n>` (the nth Math of that Morph's child), so the
  two sides match with no key; anywhere else its pieces are unkeyed. The
  keys are ordinary keys: one shared by consecutive scenes makes their
  transition a magic move, so a formula carries across scenes term by
  term, and two Math elements with one key in one tree are a duplicate-key
  error.
- **Errors.** A formula KaTeX cannot parse fails the frame naming the
  element's tree path and TSX line, with KaTeX's message and position
  (``<Math tex="\frac{a}{">: Unexpected end of input in a macro argument,
  expected '}' at end of input: \frac{a}{``); so does a character no
  bundled KaTeX font has, a line break outside an environment and
  `\includegraphics`.
- **Cache.** Each renderer keeps typeset formulas by source and display
  mode (up to 512, then it starts over), so a formula is typeset once, not
  every frame.
- **Write** (`<Write at dur lag stroke>`, manim's Write; `math/write.rs`)
  writes every Math inside it on: each piece's outline is stroked on (a
  `trim` of its outline, `stroke` em wide, default 0.025, in its fill
  colour, round caps) over the first half of its run, then its fill fades
  in as the stroke fades over the second; a rule wipes on from the left and
  a stroked path (`\cancel`) draws on. `at` is seconds, writing the whole
  formula, or `{ group: seconds }` naming `\k` groups, each cued on its own
  word (`at={{ lhs: word("two to the fourth"), minus: word("minus") }}`); a
  piece in no listed group writes with the nearest listed group before it
  in reading order, else with the first, and a group lists the pieces of
  the groups inside it too. `dur` (a number or `{ group: seconds }`,
  default `theme.motion.duration.slow`) is each group's run; inside a group
  pieces go one after another in reading order, `lag` apart (default
  manim's `min(4 / (n + 1), 0.2)`). Before its cue a piece is invisible but
  keeps its place. The prelude sends each group's linear progress and Rust
  places the pieces, since only it knows them. A name that is not a group
  of the formula is an error listing its groups. Text cannot be written
  (its glyphs are a shaped run, not outlines); `\text{…}` inside a Math can.
- `Math` is JS's own `Math` object: the JSX factory knows it by identity,
  so `Math.max` and the seeded `Math.random` are untouched. `studio.d.ts`
  types `<Math>` through a call signature merged into the `Math` interface.

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
- `<Steps at dur ease lag fade>` holds N states of one place (children,
  or render props), a derivation rewritten in place. It is a `<Morph>`
  between the pair the time picks: before `at[0]` the first state shows,
  and transition k (state k to k + 1) runs from `at[k]` for `dur` (default
  `theme.motion.duration.base`), cut short by the next cue so it always
  finishes; so each cue is continuous, the Morph at `p = 1` and the next at
  `p = 0` showing the same state. `p` goes in linearly and `ease` (default
  easeInOutCubic) shapes each piece's own run, so `lag` staggers eased
  pieces. Pairs are morphed one at a time: a key in states 0 and 2 but not
  in 1 fades out over the first transition and back in, at its state-2
  place, over the second. A Steps inside a Morph is a nested Morph, an
  error.
- `<Arrow points curve progress head stroke>` strokes through `points`
  (frame px; `curve` 0 is straight segments, otherwise a Catmull-Rom curve)
  up to `progress`, with a head (`head` px, default 4.5 × the stroke width,
  0 for none) riding the drawn end along the shaft's tangent there, so the
  arrow reads as being drawn. The shaft ends inside the head, a fixed
  pull-back behind its tip, so the round cap never pokes past it; the head
  grows in from nothing over the first head length of shaft. The head is
  placed by `__studioPathEnd`, a native that measures arc length as `trim`
  does (`paint/path/trim.rs`), so the two cannot drift apart. It is two
  absolute, full-frame Paths with the frame as viewBox, so its parent must
  fill the frame. With `from` and `to` instead of `points` it is an overlay
  (see "Overlays").

## Transitions play over the incoming scene's head

A scene after the first comes in through a transition: `{ kind, dur,
easing, direction, morphColor, lag, fade }`, where `kind` is `cut`, `dissolve`, `push`, `wipe`,
`zoom` or `magic`, `dur` is seconds, `easing` is a theme easing name, a
prelude easing (`linear`, `easeOutCubic`, `easeInOutCubic`) or `[x1, y1,
x2, y2]`, and `direction` (`left`, `right`, `up`, `down`) is where the
incoming scene comes from for `push` and `wipe` (`left` pushes the old scene
out to the left). `morphColor`, `lag` and `fade` are under "Magic move"; they act only on
a magic move. `video.json`'s `motion` block takes `transition`,
`magicMove`, `morphColor` and `motionBlur`.

- **Timing.** It plays over `[0, dur)` of the incoming scene B, with the
  outgoing scene A held at its last moment (the left limit of its length,
  since Sequence and Stagger windows are half-open). The video's length
  stays the sum of scene lengths, and past `dur` a frame is B alone. `p =
  easing(t / dur)`. Each frame calls both components, each in its own fresh
  context, so a transition frame is seekable like any other. A's logs come
  first, prefixed `scene N:`.
  A frame loads A's component only while the transition covers it; when A
  has no component file, B is drawn alone, as after a cut, so a scene can
  be checked before the scene before it is built. Lint still reports the
  missing file, and a full render fails on A's own frames.
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
  or fades it in. A pair is drawn under B's ancestors, so when its A
  ancestors' opacity differs from what B's give it at `p = 0` (a `3` in a
  table row still at opacity 0, keyed to a formula elsewhere), it starts
  at its effective opacity in A (its own times its A ancestors') and
  blends to its own in B; it can never show more than its B ancestors do.
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
- **Clocks** (`morph/clock.rs`). Every node runs on its own progress. A
  scope's leaves (childless nodes that paint) are numbered in reading
  order: A's in tree order, then B's arrivals in theirs, a matched B leaf
  taking its partner's number; for a Math that is its key order. A node's
  window spans the numbers below it (a pair's, both sides'). With `lag`
  (0 to 1, manim's `lag_ratio`) piece i starts `i · lag` piece-runs in and
  the whole takes `(n - 1) · lag + 1` of them; a container runs from its
  first piece's start to its last's end. Each window's linear progress
  then goes through the scope's `ease`. Without lag every window is `p`.
- **Unmatched** nodes fade by `fade` (`morph/fade.rs`, easeInOutCubic on
  their own progress). `overlap`, the default, fades A's out over 0 to 0.6
  and B's in over 0.4 to 1, so they cross while matched pieces move, and a
  piece (a leaf: a glyph, a text) keeps its place beside its nearest
  matched sibling (the one before it in its parent, else after; an empty
  strut does not count) and moves with it, an arriving one where that
  sibling's B place would put it and a leaving one where its A place would.
  So the `4` of `2^4` leaves riding on its `2`, and `(2^2 + 1)` slides in
  behind the `1` instead of waiting at its end place over the gliding
  terms, as TransformMatchingTex reads. `sequential` fades out over 0 to
  0.4 and in over 0.4 to 1, in place, never together. Everything is also
  carried by its nearest matched ancestor's move. A new or removed box that
  holds matched nodes fades its own fill, border and shadow only, so what
  moves through it stays fully visible. The output follows B's paint
  order, each unmatched A node just before its nearest following matched
  sibling, else last.
- **Overshoot.** `p` may pass 0 or 1 (a spring `ease`): rects, radius and
  transforms follow it; opacity, blur, insets, colours, content swaps and
  fades stop at the ends.
- **Content.** The same string in the same family and line count re-shapes
  at the interpolated size, weight, line height and letter spacing, broken
  at the interpolated width (no bitmap scaling). Different strings or line
  counts swap in halves: A's text fades out over 0 to 0.5 and B's in over
  0.5 to 1, so they are never visible together, while the box still morphs.
  The same image morphs its geometry, a different one swaps in halves; the
  same path data morphs viewBox, fill, stroke and trim, different data swaps
  in halves.

`<Morph p={...} lag fade ease>{a}{b}</Morph>` uses the same engine inside
one scene, which also gives FLIP-style layout animation. `ease` is a
function, cubic-bezier points or a theme easing name, sampled by the
prelude into a 65-point table so each piece eases on its own clock; with
`lag`, pass a linear `p` and an `ease` (an eased `p` would be cut into
slices, each piece stopping at speed). `Steps` does this for you. A magic
move between scenes takes `lag` and `fade` from its transition cascade
(scene, `video.json`, theme; default 0 and overlap), with the
transition's `easing` as each piece's `ease`. The tree lays out twice, once with
every Morph replaced by its first child and once by its second, and the two
layouts morph: inside each Morph at its own `p`, matching by key within that
Morph; outside every Morph, nodes match by tree path and move at the first
Morph's eased `p` (no lag), so siblings that shift because a Morph changes
size glide too. A Morph takes exactly two elements, and a Morph inside
another is an error for now.

A shrink-to-corner is a Steps whose last state lays the same Math key out
smaller, absolutely placed in a corner: its glyph Paths keep their path
data, so each rect glides and scales and the outline scales with it.

## Overlays are drawn on the finished tree

`Brace`, `Surround`, `Underline`, `Indicate` and an `Arrow` with `from`
and `to` point at terms (`src/overlay/`, `prelude/overlays.js`). Each is
an `overlay` host node, absolutely placed so it takes no space (a Brace's
label is its child, laid out where it sits). After layout and after any
morph has blended the tree, `overlay::resolve` finds each target's box on
the composed frame and draws, so an overlay follows its target while it
moves in a Morph or Steps. Two overlays matched across a Morph lerp
between their two targets' boxes.

- **Targets.** A key or a key prefix: `eq/lhs` covers `eq/lhs`,
  `eq/lhs.1`, … and `eq/lhs/…`, not `eq/lhs2`. The box is the union of the
  outermost covered nodes (the Math box for `eq`), through every transform
  above and on them, mapped into the overlay's parent's space; a fading
  node counts by its opacity, so the box eases as terms fade in and out,
  and an overlay whose target has faded out fades with it. A target no key
  covers fails the frame naming the closest key. Defaults scale with the
  target's em, the px per em of its Math pieces (or a Text's font size;
  else its height / 1.2).
- **Timing.** `progress` (0 to 1) draws it on; or `at` and `dur` (default
  `theme.motion.duration.base`) with `ease` (default easeInOutCubic) give
  it from `ctx.t`; with neither it is drawn.
- `<Brace target side label gap size color>`: KaTeX's own brace
  (`math/brace.rs`: the three `\overbrace`/`\underbrace` SVG slices, at
  least 1.6 em wide) along `side` (`below`, the default, `above`, `left`,
  `right`), 0.12 em off, its thickness 0.548 of its em (`size`, default
  the target's), text-coloured. It grows from its centre with `progress`
  and its label fades in over the second half, 0.15 em beyond it. A
  string label is `theme.type.body` text scaled about its centre to 0.7 of
  the target's em, as manim's brace labels sit beside their formula (it is
  laid out before the em is known, so `overlay/draw.rs` scales it); an
  element label keeps its own size.
- `<Surround target pad radius width color>`: a rounded rectangle 0.2 em
  out with 0.12 em corners, stroked 0.045 em in the accent, drawn on
  clockwise from its top left.
- `<Underline target gap width color>`: a line 0.1 em under the target,
  drawn on from the left.
- `<Indicate target scale color at dur>` changes its targets themselves:
  over its run (`dur` default `theme.motion.duration.slow`) they scale to
  `scale` (1.2) about their joint centre and flash to `color` (the accent)
  and back, along easeInOutCubic there and back. It runs before the other
  overlays, so they measure the pulsed boxes.
- `<Arrow from to bend head gap stroke>`: each end a key or a point in its
  parent's px. A keyed end starts `gap` (0.1 em) off its target's box on
  the line between the two ends' centres. `bend` (degrees) arcs the shaft
  (one quadratic whose end tangents turn half the bend from the chord). It
  draws on as the prelude's Arrow does, head and all (`overlay/arrow.rs`).
- Across scenes an overlay is drawn in each scene before the magic move,
  so its Paths morph like any others: a Surround or Underline whose box
  changed, or a Brace of a new width, swaps in halves rather than gliding.

## Captions are burned in from the word timings

With video.json's `captions` on, `Renderer` ends every frame of a narrated
scene with a caption: the phrase being said on a rounded plate, the word
being said highlighted, the words still to come faded (`src/captions/`).
`renderer.without_captions()` leaves them out; `strip` and the app's stills
(thumbnails, the scene strip) use it, and `gallery`, lint and the
playground draw through `Engine` and never have them. `oculus studio frame`
shows them, so a builder can check what a frame will look like.

- **Phrases** (`phrase.rs`) come from one scene's words (`cued_words`, the
  script's spelling and punctuation as said, spoken forms such as "thirteen
  mod twelve" included), never across scenes, so a scene's captions depend
  on its own words and the theme only. A `[pause]`, a silence of 0.6 s or
  more, and a sentence's end (`.`, `!`, `?`, `…`) always end a phrase.
  Longer stretches take as many words as fit the lines, ending at the last
  clause break (`,` `;` `:` a dash, or a 0.25 s silence) that leaves the
  phrase at least half full, and give a word back rather than leave one
  word alone at the end. A phrase is set in as few lines as hold it, cut so
  the longest line is shortest (a tie puts more on the later line), then
  each cut moves to the nearest clause break that keeps every line within
  the limit and the longest at most 9 characters longer ("eleven, / wrap
  round", not "eleven, wrap / round"); a word longer than a line gets a
  phrase to itself.
- **Timing** (`timing.rs`): a phrase shows 0.15 s before its first word
  (never before the one before it hides) and stays until the next shows;
  before a silence of 0.6 s or more, and at the scene's end, it hides 0.4 s
  after its last word, so a long pause has no caption. The current word is
  the last one that has started, lit until 0.3 s after it ends; before the
  first word nothing is lit.
- **Drawing** (`tree.rs`): the caption is an element tree like a
  component's, laid out at the frame's size and painted as the last layer,
  each word its own `Text` in a padded box so the highlight moves without
  reflowing the line. Under motion blur every sub-frame takes the caption
  of the frame's own time, so it is drawn once and sharp.
- **Style** is the theme's `captions` block (`style.rs`). A theme without
  one gets it filled from its own tokens, so it is in `ctx.theme` too:
  `fontFamily` (default the theme's body font), `fontSize` 44 and
  `fontWeight` 600, `color` (`onInk`), `upcoming` (0.6, the opacity of
  words not yet said), `highlight` (`accent`), `highlightText`
  (`onAccent`), `highlightStyle` (`pill`, a pill of the highlight behind
  the word in `highlightText`, or `color`, the word in the highlight),
  `plate` (`ink`, or `"none"`) at `plateOpacity` (0.8), `position`
  (`bottom` or `top`), `margin` (0.05 of the height from that edge),
  `maxChars` (42 a line) and `maxLines` (2). A colour is a `color` token's
  name or a `#hex`. Sizes are px on a 1080 px short side and scale with the
  frame's shorter side, so portrait text is as big as landscape; a line on
  a frame narrower than 16:9 holds proportionally fewer characters (24 on
  9:16, never under 12). Each value is checked when the theme loads, and a
  bad one is named (``captions.position = `middle`: must be `bottom` or `top` ``).
- **Subtitles.** `Project::caption_cues()` is the same phrases in video
  seconds, captions on or not, and `captions::vtt` writes them as each
  narrated render's `.vtt` (see "Rendering a video").
- **Not linted.** Lint does not check what a scene draws under the caption
  band; the skill tells builders to keep key content above the bottom
  sixth of the frame when captions are on.

## Motion blur averages sub-frames, off unless asked

`RenderOptions::motion_blur` (`MotionBlur { fps, shutter, max_samples }`;
`MotionBlur::at(fps)` is a 180° shutter of 0.5 and at most 32 samples) makes
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
  the `RenderOptions` value at the video's fps; a `--final` render asks
  for `Final` and a `--draft` one for `Draft`. A bad value is an error naming its level
  (`video.json motion.motionBlur`), and lint reports it. `oculus studio
  frame` and `strip` are inspection tools and blur only with
  `--motion-blur`, whatever the token says.
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

- The fonts are Inter (variable: `opsz`, `wght`), Manrope (variable:
  `wght`) and JetBrains Mono (variable: `wght`, upright only), compiled in
  with their OFL licences beside them, and the twenty KaTeX faces (`fonts/katex/`, OFL, from the katex npm package the app
  uses), which only `Math` reads; `Text` cannot name them. System fonts are
  never loaded, so a frame is the same on every machine; an unknown
  `fontFamily` is an error naming the bundled families.
- Monospace is for code. JetBrains Mono's advance is 0.6 em, so a column
  of characters is a fixed number of px; the typesetter turns off its `calt`
  feature (ligatures), so `<=` and `!=` show as typed (`layout/text.rs`).
  Numbers, even in a table, take `fontVariantNumeric: "tabular-nums"`, not
  the mono face.
- A theme is tokens only: `color`, `font` (`heading`, `body` and `mono`
  families), `type` (style fragments to spread into `style`; the bundled
  themes have `display`, `h1`, `h2`, `h3`, `body`, `label`, `caption` and
  `code`, the monospace fragment, 28 px), `space`, `radius`,
  `motion` (`duration`, `easing` as cubic-bezier points, `spring` presets,
  and optionally `transition`, `magicMove`, `morphColor` and `motionBlur`,
  the bottom of the motion cascade; the bundled theme sets `morphColor:
  "oklch"` and `motionBlur: "finals"`), `narration` (`lead` and `tail`,
  non-negative seconds; see "Narration") and `captions` (how burned-in
  captions look; see "Captions are burned in from the word timings").
  Components read it as
  `ctx.theme` and it is frozen.
- `color.bg` is the canvas under every frame, `color.text` the colour and
  `font.body` the family of `Text` that sets none.
- `font.mono` and `type.code` are optional in a theme file: a file without
  them gets `JetBrains Mono` and the bundled `code` fragment before it is
  validated (`theme/schema.rs`, `fill_defaults`), so a theme written before
  the face existed still loads, and a component can always read them.
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
  reseeded before every call from the component name and the scene's `id`,
  or its index when it has none (`host::seed`; a tag byte keeps an id seed
  from ever hashing as an index seed), so an id'd scene draws the same
  numbers wherever it sits.
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

Each `LintIssue` has a level: `Error` (everything below unless it says
otherwise; any error makes `oculus studio lint` exit 1), `Warning` (how a
scene looks over time, below) or `Note`. Notes carry a noun and a short
form (`group`) so the CLI prints a scene's repeats as one line; warnings
print one per line in full, since their quoted words say where to fix
them. `LintReport::is_clean` counts errors only. `Project::lint_scene(i)`
lints scene `i` alone: it renders that scene and the one before it (for the
transition's middle) and keeps only scene `i`'s issues, its component's
own (tokens, props, examples) and the video's `morphColor` and
`motionBlur`; a component that fails to load is reported at scene `i`
even when the scene before uses it too.

Lint also reads the source and declared props of each component a scene
uses (`project/lint/tokens.rs`, `project/lint/props.rs`). Each problem is an
error at the literal's `file.tsx:line:col`:

- **Colour literals.** `#rgb`, `#rrggbb` or `#rrggbbaa`, `rgb(`, `rgba(`,
  `hsl(`, `oklch(` and the other functional notations in any string, and a
  whole string that is a CSS colour name where a colour goes
  (`project/lint/tokens/places.rs`): the value of a `color`, `fill`,
  `stroke`, `background` or `stops` key, or of any key ending in `Color`,
  whether an object key (`color: "red"`) or a JSX attribute (`fill="red"`),
  including the arrays, calls and ternaries inside it; or the value of a
  key named after a `token("color")` prop, unless the theme has a colour by
  that name. Elsewhere a colour name is a word on screen (`rightValue:
  "purple"`) and passes. The argument of `token(...)`, and a `.default(...)`
  chained on it, may hold a name, not a literal; the props check rejects a
  default the theme has no token for. The message names the theme
  colour to use and the three nearest by RGB distance (all colour names for a
  functional or named notation): ``colour `#5e6ad3` is a literal; use
  theme.color.accent (nearest: accent, accentStrong, muted)``.
- **Font families.** A bundled family's name as a whole string, or any string
  after `fontFamily:`: ``font family `Manrope` is a literal; use
  theme.font.heading`` (`JetBrains Mono` names `theme.font.mono`).
- **Style numbers.** A number that is a length in a `style` object and
  equals a theme `space` or `radius` entry: ``style value 24 equals
  theme.space.m; use theme.space.m``. A length is the value of a length
  key (`width`, `height`, the `min`/`max` sizes, `gap`, `padding`,
  `margin`, `left`, `top`, `right`, `bottom`, `radius`, `blur`,
  `translateX`, `translateY`, `fontSize`, `letterSpacing`): the value
  itself, a branch of it (an array element as in `padding: [0, 16]`, a
  ternary's arm, a side of `||`, `??` or `&&`) or a term added or
  subtracted (`space.s + 4`). Space is searched before radius, so 16 names
  `space.s` when `radius.m` is also 16. Unitless keys (`opacity`, `scale`,
  `rotate`, `flexGrow`, `flexShrink`, `fontWeight`, `lineHeight`,
  `transformOrigin`, `clipInset`) are never checked, so `opacity:
  clamp(pop * 4, 0, 1)` passes, and neither is a factor (`width: 24 * s`),
  a side of a comparison, a call's argument, a number inside a nested
  object (`shadow`, `border`, `fill`, `reveal`), or a negative number
  (`project/lint/tokens/style.rs`).

One more check is a note, not an error (`mono_note`): a component whose
name neither starts nor ends with `Code` that reads `theme.font.mono` or
`theme.type.code` gets ``theme.font.mono is used outside a Code component;
monospace is for code, and numbers take fontVariantNumeric:
"tabular-nums"`` at the first place, grouped as `monospace outside code`.

The check reads the TSX with a scanner, not a parser: comments are skipped,
strings and template literals are read as strings (a literal inside a
template's `${...}` is not checked), and JSX text is read token by token.

- **Required props.** A prop with no `.default()` and not `.optional()` must
  be set in every `examples` entry: ``example `short` omits required prop
  `note`, which has no default``. A component with required props and no
  `examples` is reported once.

For a narrated video lint also reports, each naming the scene id:

- a `script.md` section no scene matches (kind `script`, with its line);
- script text that is not spoken form (kind `script`, `script.md:<line>:`
  and the token): digits, `%`, `^`, `=`, `/`, `+`, `<`, `>`, a lone `*`, maths
  symbols such as `×`, `÷`, `±`, `√`, `∑`, `∫`, `∈`, `≤`, `≠`, `≈`, Greek
  letters, any other symbol, and dotted abbreviations (`e.g.`, `i.e.`,
  `etc.`, `U.S.`), each with what to write instead (`` say `%` as
  `percent` ``). Letters and `, . ; : ! ? ' " - — ( )` pass, `*` or `_`
  inside a word is emphasis, and `[pause]` markers pass. The rules are in
  `app/studio-core/src/project/lint/spoken.rs`;
- a malformed pause marker (kind `script`, with its line): ``[pause x]` is
  not a pause; write `[pause]` for the theme's pause or `[pause 1.5]` for
  seconds``, or that a pause is more than 0 and at most 5 seconds;
- as a note, not an error, each word whose aligner confidence is below
  `LOW_CONFIDENCE` (-8.0, measured on Kokoro, where clean words stayed
  above -4.83; re-check it for another TTS engine), since a mangled word
  smears its neighbours' timing;
- each manifest gap: ``the audio has unscripted speech after `word` (heard
  "…")``, or ``before the first word``.

When the voice `narrate` would resolve (without flags) is Kokoro, lint
also reads every section as `narrate` will (`studio::voice::lint`; the
server engine has nothing to check):

- the `g2p-en-us` pack missing, as one problem (kind `lint`) saying how to
  download it;
- a lexicon entry that is not valid (kind `lint`, naming the file and the
  key; the student fixes the file);
- a word with no phonemes, a `$`, or a sentence too long for Kokoro with
  no comma to cut at (kind `script`, at the section's line).

All of these but the unsure words are errors: any of them makes `oculus
studio lint` exit 1. A voice that does not resolve is left to `narrate` to
report. Each word Kokoro's guesser reads is a note: ``note: scene N `id`:
script.md:<line>: check pronunciation: ...``, with the state of the word's
suggestion when one is stored. Notes never change the exit code.

### Look warnings: what a director would catch on a strip

`crate::look` reads a scene's laid-out trees without painting them. Each
scene that rendered cleanly is laid out alone (no transition) every 0.2 s
and at each cue word's start, on worker threads (one `Engine` each, at most
8, half the cores), and each tree is walked as paint composes it
(`look/walk.rs`, which motion blur's `measure` shares): transforms,
effective opacity, and ancestors' `clip` and `clipInset`. A node counts as
shown when it paints something (fill, border, shadow, text, path, image),
is above 5% effective opacity, and covers under 90% of the frame (a
background). A path's box is its own outline as trimmed, stroke included,
not the box it is fitted into. Shown nodes gather into entities: a key
with its unkeyed descendants, a Math with all its pieces (the Box
`math::build` makes, found by its zero-width strut), or a TSX line for
nodes with no key near them. Every threshold is in `look/limits.rs`. A
layout that throws mid-scene, which the renders at both ends miss, is an
error naming its time.

Each warning names the scene, the time span, the entity (its key, or
`Box at Comp.tsx:line:col`) and the words said in that span:

- **`off_frame`**: an entity more than 4 px past an edge, settled (moved
  under 1 px) for two samples or more, after the transition in. Nodes
  under a camera (a frame-filling ancestor with `scale`) are cropped on
  purpose and skipped.
- **`still`**: while the narration plays (`lead` to `lead + clip`), more
  than 2.5 s where no entity's place (to the pixel), opacity (to 2%),
  colours or shape changes, not counting the time inside `[pause]`
  silences, which hold the picture on purpose; and nothing on screen by
  0.5 s (``nothing is on screen until 3.2 s``, or ``nothing is ever on
  screen``).
- **`half_frame`**: for more than 3 s, everything shown sits in one half
  of the frame (reaching at most 3% past the centre line) and is narrower
  than half the frame; top and bottom too for a portrait video.
- **`fragment`**: at the scene's last sample, once the last word is said
  (and within its last 1.5 s), a Math showing at most 2 of its pieces and
  under 25% of them (a stray `= .` left from a rewrite), or a node under 6
  px both ways with nothing shown within 40 px.

Calibration: on a 22-scene lecture video, a scene a reviewer judged good
drew warnings only for its opening (a lone clock for 4 s, in the left half
for 6.5 s); lint and its warnings take about 0.2 s a scene.

### `beats` is the same samples as a timeline

`SceneLook::beats` lists, per entity, when it appears, vanishes, moves
(its centre more than 8 px), resizes (more than 8 px), recolours or is
rewritten (its text, path, trim, reveal or a Math's shown pieces change),
with 2-4 words said at that moment. A change over consecutive samples is
one beat with a start and an end; an entity's changes that start together
are one beat (`move+resize`); a change starting as it appears is part of
the appear, which reports where it settles. The same beat at the same
sample on keys that differ only in a trailing number shares a line
(`d0-d3,d5`). A Math's rewrite lists the pieces it gains and loses
(`+val -add`). Each `[pause]` is a beat of its own at its start, its
length and the words before it: `  4.10  pause    1.0 s  "… lands on
one."`.

## Rendering a video

`render` (`app/src-tauri/src/studio/render/`) turns a video folder into
one export preset's mp4: H.264, yuv420p, BT.709, 48 kHz AAC narration,
`+faststart`. It never narrates and never
touches the network: stale or missing narration fails with the `Narration`
error before any frame renders.

- **Presets** (`preset.rs`), each written to `<dir>/renders/<name>.mp4`:

  | Preset | Writes |
  | --- | --- |
  | `draft` | mp4, 540 px on the shorter side (never upscaled), on the fast encoder |
  | `final` | mp4 at the video's size, crf 18 |
  | `small` | mp4, 720 px on the shorter side, crf 23 |

  Motion blur follows the `motionBlur` token for the purpose: `draft` is
  `Draft`, every other preset `Final`, so with the bundled `finals` only
  `draft` is sharp. An odd output side is an error naming the size and
  scale, since yuv420p needs even sides (nothing is padded), and so is a
  side over 8192 px. Portrait and square are a different video size, not a
  preset.
- **Encoders** (`toolkit.rs`). Before anything renders, the ffmpeg is asked
  once per binary and process for its `-encoders`, `-decoders` and
  `-demuxers`. A preset needs the `rawvideo` demuxer and decoder and
  the `concat` demuxer; narration needs the `wav` demuxer, the `pcm_s16le`
  decoder and the `aac` encoder. A missing one fails at once, naming it
  ("this ffmpeg was built without the `concat` demuxer, ..."). `draft`
  takes the first of `h264_videotoolbox -q:v 65`, `h264_mf` (Windows Media
  Foundation, at 0.1 bits a pixel a frame) and `libx264 -preset veryfast
  -crf 23 -threads 8`; the others take the first of `libx264 -preset medium
  -crf <18|23> -threads 8`, `h264_mf` (0.3 or 0.15 bits a pixel, `-rate_control
  u_vbr`) and `h264_videotoolbox` (`-q:v` 80 or 65). The chosen encoder
  and its arguments are part of every segment's key. macOS's ffmpeg has
  VideoToolbox and libx264; the Windows LGPL build has only `h264_mf` (see
  [windows.md](./windows.md)). The hardware encoders can soften the
  gradients' dither, which x264 at crf 18 keeps.
- **Colour.** Frames go to ffmpeg as raw RGBA on stdin (`-f rawvideo
  -pix_fmt rgba -s WxH -r fps -i -`), converted with `-vf
  scale=out_color_matrix=bt709:out_range=tv -pix_fmt yuv420p` and tagged
  `-colorspace bt709 -color_primaries bt709 -color_trc bt709 -color_range
  tv`. Untagged, swscale converts with BT.601 and the indigo shifts.
- **Segments.** Each scene is one segment, `.cache/segments/<key>.mp4`,
  covering its whole frames (`scene_frames`), each frame sampled at
  `Time::Scene { t: j / fps }`, so a scene's segment does not depend on
  where it sits. The key is the sha256 of the scene's fingerprint
  (`scene_fingerprint`, which covers the scene before it), a hash of every
  file under `components/` and `assets/`, the render settings (quality,
  scale, size, fps, blur, filter, codec and tag arguments) and
  `STUDIO_RENDER_BUILD`, which `app/src-tauri/build.rs` hashes from
  studio-core's `src/`, `fonts/`, `themes/` and `library/assets/` (the
  bundled `starter:` images), the KaTeX port's `src/`,
  `data/` and `build.rs` (`app/math-core/katex/`), `src/studio/render/` and
  `app/src-tauri/Cargo.lock` (the versions of vello_cpu, parley, taffy and
  the rest), so a renderer, font or dependency change in development never
  reuses an old segment. Editing one scene re-renders it
  and the scene after it (whose transition shows it); the rest print
  `cached`.
- **Frames in parallel, written in order.** Worker threads (half the cores,
  or `--threads`, capped so the renderers, their frames and the reorder
  slots are estimated under 2 GB: `strip`'s policy) each build their own
  `Renderer` and claim frames from a reorder buffer that holds at most two
  per worker; the calling thread takes them in order and writes each to the
  current segment's ffmpeg, one process per segment. The earliest failing
  frame stops the render with `StudioError::Frame` (its time and scene).
  `--threads` bounds only these renderers. The encoder runs beside them on
  its own threads: x264 always on 8 (see "Encoders"), so a
  `--final --threads 3` render was measured at 553% CPU. The fixed count
  keeps x264's memory known and its output, and so the segment cache, the
  same at any `--threads`.
- **Joining.** The segments are joined by the concat demuxer as a stream
  copy (`-f concat -safe 0 -c:v copy`, absolute paths in
  `.cache/concat-<preset>.txt`). x264's B-frames do not drop or repeat a
  frame at the joins: the joined file decodes (`-f framemd5`) to each
  segment's frames in turn, and a render that reused cached segments
  decodes to the same frames as a cold one.
- **Narration track.** Mixed in Rust, not with `amix` (which divides by its
  input count): each clip is decoded, resampled to 48 kHz mono (rubato) and
  added into one buffer as long as the video at sample `round((first frame
  / fps + lead) * 48000)`. It is written as 16-bit WAV to
  `.cache/narration-<key>.wav` (keyed by the clips and their offsets) and
  encoded with `-c:a aac -b:a 160k`. A video without narration has no audio
  stream.
- **Output.** `Preset::default_out` (`-o` for another path). A narrated
  video's render also writes its captions as WebVTT beside the mp4, the
  same name with `.vtt` (`renders/draft.vtt`), whether or not they are
  burned in, for the player's CC. Segments, the track and the output are
  written as `.part` and renamed. After a render, `.cache/renders.json`
  records, under the preset's name, the segments and track it used and the
  file it wrote; segments and tracks no preset's last render used, and any
  `.part`, are deleted. A record of a preset since removed is dropped with
  them.
- **Status** (`status.rs`, `render::exports`). For each preset: the file
  at its default path, if any, its mtime, `current` when the file is there
  and the record a render would write now (the segment keys, track and
  file, planned without rendering) equals the recorded one, and `problem`
  when the preset cannot render now (stale narration, an encoder ffmpeg
  lacks, a size too large).
- **Failure and cancel.** ffmpeg's non-zero exit is an error quoting its
  last stderr lines. Cancel (an `&AtomicBool`, checked before every frame
  and while joining) kills ffmpeg, deletes the partial segment and returns
  `Finished::Cancelled`, which is not an error. No timeouts.
- **Progress.** A sync `&mut dyn FnMut(Progress)`: `Planned` (segments,
  cached, frames to render, threads), `Segment`, `Frame` (frame of frames
  and frames a second), `Done` per segment in video order, `Muxing`.
- **From the app.** `studio_render(dir, preset)` (a preset's name, to its
  default path, half the cores) runs `render` on a
  blocking thread, finding ffmpeg like the CLI does plus the app's
  resources. It resolves to the report (`--json`'s `report`), or `null`
  when cancelled; it rejects with the failure's message. One render per
  folder (a slug or a video's folder; any other folder is refused, see "The
  Studio page's commands"): `render::RENDERS` maps each canonical folder to its preset and
  an `Arc<AtomicBool>` cancel flag, and a second start for the same folder
  is refused at once ("a draft render of <dir> is already running"), with
  no event. A guard frees the folder however the render ends, before the
  last event. `studio_render_cancel(dir)` sets the flag (false when none
  runs). Each step is a `studio-render-progress` event `{ dir, preset,
  phase, ... }`, `dir` as given: `planning` at the start and again with `plan:
  {segments, cached, frames, threads}`; `rendering` with `{segment, of,
  scene, frame, frames, fps}` (1-based segment and frame), at most one per
  100 ms besides each segment's first and last frame; `muxing`; then
  `complete` with `{path, report}`, `cancelled`, or `error` with `{error}`.
  Each ending is logged.
- **Measured** (debug build, 18-core M-series Mac, a 15.5 s narrated
  three-scene 1080p video at 30 fps, 465 frames, 9 render threads; the
  spread is other load on the machine): `draft` (960×540) in 1.7 to 1.8 s
  wall, 390 to 700 frames/s, 0.2 GB peak for oculus and ffmpeg together;
  `final` in 2.3 to 3.8 s, 135 to 220 frames/s, 1.2 GB peak (oculus 0.49
  GB, x264 0.75 GB; x264's default threads held 1.1 GB on their own).
  After one paragraph edit (and its narration), `final` re-rendered that
  one scene and reused the other two. The mixed track put each clip on its
  exact sample (cross-correlated against the clip), and a card cued to a
  word appeared on the first frame at or after the word, matching the frame
  `oculus studio frame` draws at that frame's time (41 to 48 dB PSNR, 23 dB
  against its neighbour). The scenes were mostly still; motion-blurred
  motion costs up to 32 sub-frames a frame.

## The Studio page's commands

`studio::commands::videos` is what the Studio page calls
(`app/src/lib/studio/videos.ts` is the typed wrapper; JSON is camelCase).
Each takes a slug, resolves it with `videos::video_in`, and returns what is
on disk now; nothing is cached but thumbnails.

| Command | Does |
| --- | --- |
| `studio_videos(subject?)` | `videos::list_in` plus `title` and `threadId`, newest change first (a video whose video.json does not load sorts last and carries `problem`) |
| `studio_new_video(options)` | `{title?, subject?, theme?, portrait?, voice?}` -> `{slug, dir}`. The slug is `unique_slug(title)`; `theme` defaults to the subject's default theme (see "Libraries cascade"); `voice` is video.json's `voice` block (`{engine?, model?, voice?, speed?}`, `engine` being `kokoro` or `server`). The scaffold has `"captions": true`. It is loaded over the libraries before it is returned, and removed again if it does not load (a theme nothing defines, say) |
| `studio_video_detail(slug)` | the row, `fps`, `width`, `height`, `theme`, `voice`, `captions`, `scenes` and `renders` |
| `studio_render_captions(slug, preset)` | the WebVTT text beside that preset's render (`renders/<preset>.vtt`), for the player's CC |
| `studio_set_captions(slug, on)` | sets video.json's `captions`: the `true`/`false` literal is replaced in the text, or the key is added before `scenes` on a line of its own, so the rest of the file keeps its formatting (`videos::set_captions`) |
| `studio_exports(dir)` | each preset's status (`render::exports`, see "Rendering a video") as `{preset, path, modified, current, problem}`; it asks ffmpeg for its toolkit, renders nothing and makes no network call |
| `studio_frame(slug, at, width)` | a PNG `width` px wide (1 to 4096), the layout of the full-size frame, as bytes through `tauri::ipc::Response` |
| `studio_thumbnail(slug)` | the video's cached thumbnail PNG |
| `studio_render`, `studio_render_cancel`, `studio_exports` | see "Rendering a video"; they take a slug or a video's folder and refuse any other |

- **The detail's voice** is video.json's `voice.voice`, else Settings ->
  Voice's voice, else null. **Scenes** are `{id, component, start, duration,
  text}`; `text` is the scene's `## id` section of `script.md` (null when it
  has none). `start` and `duration` are seconds from `Project::timeline`, so
  they are known once every scene has a length (a hold, or narration that
  matches the script) and null for every scene until then, stale narration
  included. **Renders** are each preset's file that exists (`draft.mp4`,
  `final.mp4`, `small.mp4`), as `{preset, path, modified, captions}` with
  the absolute path, mtime in Unix seconds, and whether the render's `.vtt`
  is beside it.
- **A frame is of the video as narrated.** `at` is `{scene, t}` (seconds into
  a scene, counting from 0) or `{global}`. Stale or missing narration makes
  `Renderer::new` fail, and the command rejects with studio-core's message
  (``run `oculus studio narrate` ``); it never falls back to a silent frame.
- **A render plays from the media server.** `lectures/media.rs` serves
  files under `studio/videos/` (Range requests, so a seek works), and the
  asset protocol's scope in `tauri.conf.json` names `$APPDATA/studio/videos/**`;
  the page reaches `renders/draft.mp4` with `mediaSrc(path)`.
- **Stills share render slots**, half the cores (`slot.rs`, as the PDF
  viewer's `on_render_slot`), so a page of thumbnails queues without holding
  threads, and each renders on the blocking pool.
- **Thumbnails** are `<video>/.cache/thumbs/<key>.png`, 640 px on the long
  side. The key is `render::still_key` over scene 0 (its fingerprint, the
  components it can use, `assets/`, the build) and the shot, so an edit to
  the scene, a component or an asset is a new file and writing it deletes
  the others. A narrated video (every scene has a length) shows scene 0 one
  second in. One that is not draws scene 0 silent from a copy of the project
  without `script.md` and manifest, every scene given its hold (3 s if it
  has none), at the end of its hold. A scene that cues `word()` cannot draw
  silent, so its video has no thumbnail until it is narrated: the command
  rejects with ``no thumbnail for `slug`: `` and the reason, and the page
  shows a placeholder. `renders/` is never written.

### A video has a thread

A video made in the app is made by an ordinary harness thread, so it is also
in Chat. `studio_videos(slug PK, thread_id, length_secs, created_at)`
(migration 46) links them; `thread_id` references `harness_threads` `ON
DELETE SET NULL` (deleting the thread keeps the video), is unique, and the
row is written only by `db::store::link_studio_video`. It lives in the
database, not in video.json, which is `deny_unknown_fields` and writable by
the agent.

`harness_send` takes `SendOptions.studio` (`{slug, lengthSecs?}`; the TS
`format` is not read, since the brief takes the format from video.json's
size). When the send creates the thread it checks the folder exists
(`video_in`), creates the thread, then links it; a link that fails deletes
the thread again. Every send reads the link back (`StudioBrief::read`) and
the thread's brief gets a "The video being made" section, rebuilt like the
lecture's: the folder (absolute) and that the app made it, the format
(landscape or portrait, from the size), the theme, the subject and its
library, the length target, and the instruction to direct the video with
this skill (its step 1, the folder, done), render a draft and say it is
ready, and ask before the final. The agent is told not to run `oculus
studio new`. See
[harness.md](./harness.md#a-studio-thread-carries-its-video).

### The playground's commands

`studio::commands::components` and `studio::commands::themes` serve the
Components and Themes pages and the style chip
(`app/src/lib/studio/components.ts` and `themes.ts` are the typed wrappers;
JSON is camelCase). They read and write
only the two libraries, `<data>/studio/library/` and
`<data>/studio/subjects/<CODE>/`: a command takes a component name, a level
(`subject` or `global`) and a subject code, never a path
(`commands/levels.rs`), and a name that is not a PascalCase identifier (for a
theme, lowercase letters, digits and `-`) or a subject that is not a course
code's folder name is refused. Without a subject only the global library is
read. Each call loads the global library first, so the starter set is
there. Nothing is cached.

| Command | Does |
| --- | --- |
| `studio_themes(subject?)` | one row per theme name: `{name, level, swatch, tokens, problem, shadows}` |
| `studio_theme_source(name, subject?)` | `{level, text}` from the level that wins; a bundled theme's text as compiled in |
| `studio_theme_write(name, level, subject?, text)` | writes once the text parses as a theme; returns `{level, text}` |
| `studio_theme_delete(name, level, subject?)` | removes a library theme, snapshotting it first |
| `studio_theme_history(name, level, subject?)`, `studio_theme_restore(name, level, subject?, id)` | as for components, over `<name>.<unix-ms>.json` |
| `studio_theme_defaults(subject?)` | `{global, subject, effective}`: each library's `defaultTheme` as written, and the theme a new video there takes |
| `studio_theme_set_default(level, subject?, name?)` | writes (or with no name clears) that library's `defaultTheme`; returns the defaults |
| `studio_components(subject?)` | one row per component name: `{name, level, kind, props, examples, problem, shadows, starter}` |
| `studio_component_source(name, subject?)` | `{level, text}` from the nearest level that has it |
| `studio_component_frame(name, subject?, theme, themeText?, props?, example?, t, width, duration?)` | a PNG `width` px wide (1 to 4096), as bytes through `tauri::ipc::Response` |
| `studio_component_write(name, level, subject?, text)` | writes once the text loads; returns `{level, text}` |
| `studio_component_history(name, level, subject?)` | `[{id, savedAt}]`, newest first |
| `studio_component_restore(name, level, subject?, id)` | writes snapshot `id` back; returns `{level, text}` |
| `studio_component_delete(name, subject)` | removes the subject's own version, snapshotting it first |

- **Themes** are the bundled names, `themes/*.json` in the global library and
  in the subject's, resolved as a video resolves its theme: the subject's
  over the global one over the bundled one, one row per name with the level
  that won (`bundled`, `global`, `subject`), `default` first, then by name.
  `tokens` is the parsed theme file and `swatch` up to four CSS colours from
  its `color` tokens in the order `bg`, `text`, `accent`, `surface`. A file
  that does not parse is a row with `problem` (the `Theme::parse` message),
  no `tokens` and an empty `swatch`, not a failed call. `shadows` is the
  level a library file replaces under the same name (a subject's own
  version of a global theme is `global`), else null.
- **Components** are read through the cascade without a video: the subject's
  `components/` over the global one, `level` being the one that won. Every
  one is loaded in a single engine for its `props` (the builders' own JSON
  in declaration order, each prop `{name, kind, optional, default?}` with
  `of` for a list, `values` for `oneOf` and `token` for a token) and its
  `examples` (name to props, in declaration order); a component that does
  not load is a row with `problem`, an empty `props` and no examples.
  `PropSpec` serialises to that JSON in studio-core. `kind` is `shell` when
  the name ends in `Shell` (a library's own; no starter is one), else
  `component`: nothing in a component's code
  says which, so the name does. `shadows` is true for a subject's file that
  replaces a global component, the subject's own version. `starter` is true
  when the name is in the starter set (`STARTER_COMPONENTS`), whichever level
  holds the file and whether it was edited.
- **A frame** draws the component alone in a 1920x1080 frame scaled to
  `width`, in the theme `theme` names (resolved through the same levels).
  `t` is seconds into the component and must be in `[0, duration)`;
  `duration` is what the component sees as `ctx.duration` (default 4 s).
  The props are `props` if given, else the example `example` names, else the
  first example, else none. They are checked against the schema, so a wrong
  one is refused with the engine's sentence and nothing is drawn as a guess;
  so is a theme that does not parse or an error in the TSX. There is no
  narration, so `word()` finds no cues. It takes a render slot
  (`videos/slot.rs`) and builds one `Engine` for the call, since QuickJS is
  not Send: a 480 px frame took about 3 ms and a 1920 px one about 6 ms
  (a starter component, test build). With `themeText` the theme is that
  text instead (at most 256 KB), parsed as a write would parse it, so the
  Themes page draws an unsaved edit; `theme` then only names it in errors.
- **A write** loads the text as the component first (a fresh engine; at most
  512 KB), so a text with a syntax error, no default export or a bad
  `props` is refused with the engine's message and the file, its history and
  the library are untouched. Then it goes through `studio_core::write_library`,
  which copies the text it replaces to `<library>/.history/` (see
  "Libraries cascade"). A subject write creates the subject's version of a
  global component; a global write may create a new component. The same text
  again writes nothing.
- **History and restore.** `history` lists `<Name>.<unix-ms>.tsx` files in
  the level's `.history/`; `id` is the milliseconds as a string and `savedAt`
  the same number. `restore` takes an id from that list, loads the old text
  (it must still load) and writes it through the same path, so the text it
  replaces is snapshotted as the newest entry and an Undo can be undone.
  `.history` is shared by both kinds of file; a component's snapshots are
  `.tsx` and a theme's `.json`, so one name never sees the other's.
- **A theme write** checks the text with `Theme::parse` (at most 256 KB) and
  goes through the same snapshotting write. A bundled theme's name
  (`default`) is refused: a library file of that name would replace it for
  every video, so the page saves a copy under a new name instead.
- **Removing** (`studio_theme_delete` at either level,
  `studio_component_delete` only at the subject level, the global library
  being what a subject falls back to) copies the file to `.history` first
  (`studio_core::remove_library_file`), so restore brings it back.

## Slides crop a library PDF page

A scene shows "page 12, Figure 3" as the real thing: `oculus studio slide`
crops a region of a library PDF page into the video's `assets/`, once, and
the `Slide` starter animates it in. Rendering never opens a PDF.

- **Picking the region.** FILE resolves as for `oculus read` (an Office
  document means its converted PDF). `--find TEXT` matches, in any case,
  the text of each block in the parse record's `blocks`
  ([parsing.md](./parsing.md#blocks-where-each-rendered-item-sits)), on
  `--page` or every page: one figure (`image`, `chart`, `table`) among the
  matches wins; else one text block next to a figure is its caption and
  both are taken (the nearer neighbour by vertical gap); else one match is
  taken alone. No match, or a tie at the tier that decided, is an error
  listing each candidate as `page P block I (kind): text`. `--block I,J`
  takes blocks by index, `--bbox x0,y0,x1,y1` any box. Boxes are the
  record's convention, fractions of the rendered page (crop box, rotated)
  from its top left; a MinerU figure box can leave out a caption printed
  under it, so the crop is worth a look. `--pad` (default 0.02 of the
  page's shorter side, the same in points on every side) grows the union
  of the boxes, clamped to the page; under 2 pt a side is refused.
- **No edge slices ink** (`snap.rs`, unless `--exact`). A probe render of
  the region with 6% of the page round it marks each row and column that
  holds ink (a pixel more than 40 a channel from the background, the
  commonest colour on the probe's border). Then each edge: where the
  blocks' box itself cuts ink (ink on both sides of it, past a row of
  anti-aliasing), it moves out to the next blank row within 6% of the page
  and then checks its pad as below, else pulls back to the middle of the
  blank before the ink, else stays and is a `cut`. Where ink lies in the
  pad, it moves out over that ink to the next blank row when it is a
  figure's caption or label (the pick holds a figure, chart or table, the
  ink starts within 34 pt, about two lines, and the nearest block to it
  along that axis is a picked one, so another block's text never joins),
  else pulls back halfway to it. After moving out it keeps as much pad as
  fits before the next ink. Top and bottom are decided over the padded
  columns, then left and right over the rows decided. The sidecar's
  `edges` says what each did (`pad`, `trim`, `extend`, `cut`); an edge
  left at its pad keeps the exact padded value. `--bbox` follows the same
  rule, with no picked blocks, so only a cut line moves its edge out.
- **`oculus studio blocks FILE [--page N] [--png OUT]`** lists each page's
  size in points and its blocks: index, kind, box, and the block's text on
  one line (`blocks::plain`: an image's `![caption](...)` as its caption,
  tags and heading marks dropped, cut to 80 characters). `--png` writes
  the page 1600 px wide with each box outlined (figures indigo, the rest
  grey) and its index in a tab, drawn with a 3x5 bitmap font
  (`preview.rs`). `--json` gives `{path, png, pages: [{page, width,
  height, blocks: [{index, kind, bbox, text}]}]}`.
- **Rendering.** `slides::make` runs hayro on a thread with the
  renderer's stack (`RENDER_STACK`), every call guarded, each raster under
  a render-budget reservation (the viewer's lane). Only the region is
  interpreted, into a context its size (`hayro::render_into` with a
  translate and scale before the page's own transform), so a small figure
  at 2x never rasterises the whole page at that scale. The crop's default
  width is twice what fills the video's frame (`video.json`'s size), at
  most 6 megapixels; `--width` asks for one and is refused past 8192 px a
  side or 6 megapixels. The whole page is also rendered fitted to the
  frame at 1x, for the zoom. PNGs are RGB, opaque on white, encoded at
  best compression: the same inputs write the same bytes, so the segment
  cache, which hashes `assets/`, keeps its segments.
- **Files.** `assets/slides/<name>.png`, `<name>.page.png` and
  `<name>.json`. The name is `--name` (letters, digits, `-`, `_`) or made
  from the page and the ask (`p12-figure-3`, `p12-b3-4`, `p12-region`). A
  name already holding a crop of another PDF is refused unless
  `--replace`. The sidecar (`sidecar::Sidecar`) holds `source` (the PDF's
  data-relative path), `sha256` of its bytes, `page`, `pageSize` (points),
  `bbox` (the crop as made), `select` (`find`, `blocks` or `bbox`), `pad`,
  `exact` when set, `edges`,
  `width` when one was asked, `image` and `pageImage` (`src`, `width`,
  `height`), `blocks` (each block at least half inside the crop: index,
  kind, box as fractions of the crop, text) and `props`, the `Slide`
  props for the crop (`src`, `aspect`, `page`, `pageAspect`, `bbox`, and a
  `source` label from the file name), which the command also prints.
- **Lint.** `oculus studio lint` reads every sidecar under
  `assets/slides/` and warns when the PDF at `source` no longer has the
  sidecar's sha256, naming the scene whose props hold the crop's `src`
  (else the video) and the `oculus studio slide` call that makes it again.
  A PDF it cannot read is not checked (`slides::lint`).
- **`Slide`** (`library/components/Slide.tsx`) draws the crop on a paper
  card as large as the frame allows, a caption beside a crop that would
  leave a third of the width empty and below a wide one, the source label
  under it. With `page`, `pageAspect` and `bbox` it opens on the whole page
  fitted to the frame and zooms into the crop: the card's rect goes from
  the crop's place on the page to its own (size by powers, centre
  linearly), the page drawn round it fading out while the card's edge and
  shadow grow, so the crop is seamless at first. Without them the card
  comes in with an `Enter` preset. `highlight` boxes, in fractions of the
  crop (the sidecar's `blocks` give them), draw on as a rounded outline
  with a tint at `highlightAt`. The sandbox reads no files, so the props
  carry the aspect ratios and boxes. Keys are under `id`, so two Slides in
  a `Steps` morph one crop's card into the other's. Its examples use
  `starter:slide-figure.png` and `starter:slide-page.png`, a crop of a
  lecture slide the app's tests build with lopdf; the ignored test
  `slides::tests::make::write_the_starter_sample` remakes them.

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
- `frame` and `strip` take `--motion-blur` (a 180° shutter at the video's
  fps); `frame` then reports the sub-frames it averaged. `-o` is relative
  to the working directory, and its folder is made.
- `oculus studio brief <DIR> --scene <ID|N>` prints one scene builder's
  whole input (`app/src-tauri/src/studio/brief/`): the scene's index,
  component and props and the folder; for the scenes before and after it,
  their component, whether it resolves yet, and their storyboard entries'
  `Carries in:` and `Hands on:`; its own `storyboard.md` entry verbatim
  (from a `### <id>` heading, then the end of the line, a space or `(`, to
  the next `#` to `###` heading or `---` line, outside code fences), less
  its `Narration:` paragraph (to a blank line, a table row or a heading)
  when the words below are printed; the
  `notes/` paths the entry names, else every file in `notes/`; its length
  and words, one line per sentence, each as `start word` at the time
  `word()` returns (`Project::cued_words`), with each pause as `start
  [pause 1.0 s]` after the word it follows at the time `pause()` returns
  (`Project::cued_pauses`), or the narration error and the script text
  (its `clip_text`); and its `lint --scene`, `beats`, `strip`, `frame` (at the
  scene's middle) and `gallery` calls with absolute `-o` paths, naming the
  video by slug when it is in the data directory.
  `--json` gives the same as `{index, count, id, component, props, dir,
  previous, next, storyboard, storyboard_missing, notes: {named, paths,
  missing}, narration: {kind: silent|spoken|unavailable, ...}, commands}`.
- `frame` and `strip` read `script.md` and the narration manifest; stale or
  missing narration stops them with the `Narration` error, while `lint`
  lists it with the rest.
- `oculus studio narrate <DIR>` makes the narration (`--scene ID`,
  `--force`, and `--engine`, `--voice`, `--speed`, `--server`, `--model`
  over the video's and Settings' voice); `oculus studio tts` makes one
  clip; `oculus studio g2p` shows how Kokoro reads text; `oculus studio
  pronounce suggest|list` puts a reading up for the student's review and
  lists the waiting ones (see "The student reviews the agent's suggested
  readings"); `oculus studio aligner` manages the aligner model. See
  "Narration is made by `oculus studio narrate`".
- `oculus studio voices` lists each Kokoro pack's status and size and each
  voice, downloaded or not, marking the one Settings -> Voice names;
  `voices download` fetches both packs with the aligner's progress lines,
  `voices delete` removes them. `--json` gives `{ dir, packs: [{ id,
  status, bytes }], voices: [{ id, downloaded, chosen }] }`, `{ status,
  dir, seconds }` and `{ deleted_bytes }`.
- `oculus studio agents [--agent A]` prints, for each agent or one, the
  model and effort its `studio-research` and `studio-scene` subagents run
  on and the call that starts one (`--json`: `[{agent, roles: [{role, job,
  model, effort, spawn}]}]`). It reads only the settings; see
  [harness.md](./harness.md#subagents).
- `oculus studio render <DIR>` writes one export preset's mp4:
  `--preset draft|final|small`, with `--draft` (the
  default) and `--final` for the first two; `-o`, `--threads` for the frame
  renderers, not the encoder. It prints the plan, then per scene `rendered`
  with its frames, seconds and frames a second, or `cached`, then the paths
  written (the mp4, then its `.vtt`) and a summary; `--json`
  gives `{dir, report: {path, preset, width, height, fps, frames, duration,
  audio, threads, segments: [{scene, id, outcome, frames, ms, fps}],
  sidecars, pruned, ms}}`. See "Rendering a video".
- `oculus studio lint <DIR>` (`--scene <ID|N>` for `lint_scene`; the
  voice checks are cut to that scene too) prints each error in full, one a
  line, then the warnings, then the notes, and exits 1 if there is any
  error. A scene's warnings or notes of one kind (`still span`, `unsure
  word`, `guessed word`, ...) print as one line when there are several:
  ``warning: scene 5 `mod` (ClockMod): 4 still spans: 0.9-7.7 s,
  16.0-19.0 s, 27.8-32.8 s, …``. With no errors it ends `no errors` and the
  counts, or `no problems`. With `--json` it prints one array: each issue
  as `{level: "error"|"warning"|"note", scene, id, component, example,
  kind, message, loc}` (`kind` is `off_frame`, `still`, `half_frame` or
  `fragment` for a warning; `scene` and `component` null for a script
  section no scene matches, or for a problem of the whole video, such as a
  slide no scene shows whose PDF changed), then each
  guessed word as `{level: "note", scene, id, line, kind: "guessed",
  message, word, phonemes, guessed, review}`.
- `oculus studio beats <DIR> --scene <ID|N>` prints the scene's timeline
  (see "`beats` is the same samples as a timeline"), one beat a line as
  `  6.20  move     d12-d14  → (1180,420)…(1180,660) over 0.8 s  "… stands
  up"`, at most 80 lines and how many were cut; `--json` gives every beat
  as `{index, id, component, length, beats: [{t, end, event, who, detail,
  words}]}`.

- `oculus studio gallery <DIR> <Component>` renders each example of a
  component as `<DIR>/out/gallery/<Component>-<example>.png` (`--out`),
  at `--t` seconds of the component's scene (default: the middle of its
  silent scene's hold, or 3 s of a 6 s preview), and with `--sheet` a
  contact sheet `<Component>.png` (`--cols`). It never reads the narration,
  so it runs before `narrate`. `--subject` and `--theme` replace video.json's
  for the load; `--threads` sets the render threads. See "Galleries render
  every example as a still".
- `oculus studio components list <DIR>` and `components show <DIR>
  <Component>` read through the cascade (see "Libraries cascade"),
  `components promote` and `themes promote` copy a video's own file into a
  library, and `components seed [--reset]` copies the starter set into the
  global library.
- `oculus studio slide <DIR> <FILE>` crops a region of a library PDF page
  into the video's `assets/slides/`, and `oculus studio blocks <FILE>`
  lists the blocks to pick it by (see "Slides crop a library PDF page").
- `oculus studio new` and `list` make and list the videos in the data
  directory, and every command above takes a video's slug in place of its
  folder (see "Videos live in the data directory").

Flags are in [cli-reference.md](./cli-reference.md#oculus-studio).

## Galleries render every example as a still

`gallery::render(project, component, t, options)` renders each entry of the
component's `examples` as one still, in declaration order. Each still is the
component called with that example's props at one moment: `t`, by default the
middle of the first scene that uses the component when that scene is silent
with a `hold` (its length is `ctx.duration`), else the middle of a 6 s
preview (`gallery::PREVIEW`), long enough for entrances on fixed fallback
times to have settled. An explicit `t` must lie inside that length.

A gallery reads neither the narration nor the timeline: a narrated scene's
length would need its narration, and a stale scene elsewhere in the video
does not concern the component. So it runs before `narrate`, and the CLI
loads the video with `Project::load_unnarrated_with`
(`studio::libraries::open_unnarrated`), so a missing or unreadable manifest
does not stop it. `word()` and `wordEnd()` in a gallery throw ``a gallery
renders without narration, so there are no words to cue; take the phrases
in an optional `cues` prop, …`` (`WordCues::unavailable`). An example is
checked against the props schema when it renders, and a failure names the
example.

Examples render in parallel, one engine per thread, at most one engine per
example and within the strip's 2 GiB budget. Each still is put back at its
declaration's index, so one thread and many give the same bytes
(`tests/gallery_golden.rs`). `Gallery::write_sheet` tiles the stills, two
or more a row, labelled with their example names.

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
`tests/frames.rs` covers the whole-frame timeline at other rates, frame
times against global times, blur at the video's fps, strips across thread
counts, which edits move which scene fingerprints, and (the `random`
fixture) that an id'd scene using `Math.random` keeps its fingerprint and
its exact pixels when moved behind the same scene.
`tests/narration.rs` covers scene lengths, stale and missing narration, the
load errors, `word`/`wordEnd` (hyphenated words, and the nearest phrase on
every miss) and a gallery of a narrated video with no manifest; the `narrated` fixture (a hand-written
manifest, no audio) pins a card entering at its word. In the app crate, `cargo test --lib studio::` covers the TTS client
against a loopback fake server (WAV, empty, non-WAV, silent, 404, 429),
the `/v1/models` listing (parsing, 404, non-JSON, unreachable), the Settings
row's validation (`studio::commands`),
the voice resolution, the cache key and `narrate` with a stubbed speaker
and aligner (caching, one-paragraph edits, per-scene writes, pruning,
failures). `studio::voice::g2p` matches every golden row (1030) on the
trimmed dictionary; `studio::tts::kokoro` covers chunking, the vocabulary
and pad refusals, silence, the WAV and the pack; `studio::voice::lint`
covers the missing pack, a bad lexicon entry and the notes' lines;
`studio::voice::review` covers the store (replace, list order, wrong,
decide), the lexicon writer, finding a word and the notes' review state.
Ignored
tests run the real models: the full dictionary (`STUDIO_G2P_DATA`), the
guesser's goldens and a real lint (`STUDIO_G2P_GUESSER`), and Kokoro's
end-to-end narrate (`STUDIO_KOKORO`, `STUDIO_G2P_GUESSER`,
`STUDIO_ALIGN_MODEL`), which prints its real-time factor and peak memory,
and the review's two samples differing (`STUDIO_KOKORO`,
`STUDIO_G2P_GUESSER`).
`cargo test --lib studio::render` covers the presets' sizes and names,
the encoder choice from canned `-encoders` lists (macOS's build, the
Windows LGPL build before and after Studio's additions, VideoToolbox or
x264 alone), the render plan, segment keys and tree hashes, the audio
placement, the reorder buffer and pruning, and, through the real ffmpeg
(each test skipped, saying why, without one or when it lacks the encoder,
demuxer or libx264 the test needs), the join and cache check above, a
narrated fixture's audio onset, cancel, a failing frame, and the exports'
`.vtt` sidecar and `current` status, including captions turned on.
`studio::videos` covers the captions edit of video.json.

`tests/captions.rs` pins two captioned frames of the `captions` fixture
(the `narrated` one with `captions` on: a word lit mid-phrase and late in
it), that captions are off without the switch and on no silent scene, that
a blurred frame's caption matches the sharp frame's, the fingerprint, and
the subtitle cues. `src/captions/tests/` covers phrasing, timing, the style
tokens and the VTT text.

`tests/math.rs` pins a corpus of KaTeX constructs beside text labels on
their baselines (`math-corpus`) and a Morph from `2^4 - 1` to
`(2^2 - 1)(2^2 + 1)` as a contact sheet (`math-factor-strip`); the unit
tests in `src/math/tests/` cover keys, the Morph's glide and fades, the
prefixes, errors, colours, `latex()` props and the layout against WebKit's.

`tests/gallery_golden.rs` pins the stills of the `gallery` fixture's examples
and checks that their pixels do not depend on the thread count.

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
