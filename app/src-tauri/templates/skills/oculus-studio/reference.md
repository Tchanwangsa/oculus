# Studio reference

The authoring rules for a video folder. `SKILL.md` has the loop; this file
has the details. `studio.d.ts` types every global named here. Every command
takes the video's slug as `DIR`; `oculus studio <command> --help` has the
full flags.

## The folder

```
video.json          the scenes, in order
script.md           what is said: one `## <id>` section per narrated scene
components/<Name>.tsx   the video's own components, one per file
assets/             images, named by <Image src>
narration/          made by `narrate`: manifest.json and the clips
out/                PNGs from frame, strip and gallery
renders/            draft.mp4 and final.mp4
.cache/             render segments; never edit
```

You write `video.json`, `script.md`, `components/` and `assets/`. The rest
is made by the CLI.

## video.json

```json
{
  "size": { "width": 1920, "height": 1080 },
  "fps": 30,
  "theme": "default",
  "subject": "COMP30027",
  "scenes": [
    { "id": "title", "component": "TitleCard", "props": { "title": "Gradient descent" }, "hold": 3 },
    { "id": "idea", "component": "Steps", "props": { "heading": "The idea", "steps": ["Measure the slope", "Step against it"] } },
    { "id": "rate", "component": "BigNumber", "props": { "value": "0.01", "label": "a typical step size" },
      "transition": { "kind": "push", "direction": "left" } }
  ]
}
```

| Key | What it is |
| --- | --- |
| `size` | `{ width, height }` in px. `new` writes 1920x1080 (1080x1920 with `--portrait`). |
| `fps` | 24, 25, 30, 50 or 60. Default 30. |
| `theme` | A theme name (`default`, `warm`, or one in a library), or a `.json` path in the folder. A subject library's `themes/default.json` replaces `default` for that subject's videos. |
| `subject` | The subject code. Its library is read before the global one. |
| `scenes` | The scenes in order (below). At least one. |
| `motion` | Optional: `transition`, `magicMove`, `morphColor`, `motionBlur` for the whole video (see "Transitions"). |
| `voice` | Optional: `{ model, voice, speed }` over Settings → Voice for this video. Leave it out unless the student asks for a voice. |

A scene:

| Key | What it is |
| --- | --- |
| `id` | Required on every scene (the folder has a `script.md`). Lowercase letters, digits and `-`, unique. Names the scene's `script.md` section. |
| `component` | A component name (PascalCase), resolved through the libraries (see "Libraries"). |
| `props` | The component's props, checked against its schema. Missing defaults are filled in. |
| `hold` | Seconds. Required on a silent scene; on a narrated one only a minimum. |
| `transition` | Optional: how this scene comes in (see "Transitions"). Never on the first scene. |

Unknown keys are refused.

**A scene's length.** A silent scene lasts its `hold`. A narrated scene
lasts `max(hold, lead + clip + tail)`, where `lead` (0.25 s) and `tail`
(0.6 s) are the theme's `narration` tokens: the clip starts `lead` into the
scene. Lengths round up to whole frames. The video's length is the sum of
its scenes' lengths.

## script.md

```markdown
## idea

We follow the slope *downhill*, one small step at a time.

## rate

A typical step size is zero point zero one.
```

- One `## <id>` section per narrated scene, matching a scene's `id`. A
  scene without a section is silent.
- Nothing may come before the first heading, a heading must be a scene id,
  each id appears once, and no section is empty: any of these stops every
  command from loading the video, naming the line.
- `*`, `_` and backticks are dropped; whitespace is collapsed. Everything
  else is read aloud.

**Spoken form.** Write what the voice should say. Lint flags each of these
with what to write instead:

| Written | Write |
| --- | --- |
| Digits: `0.01`, `2024`, `3rd` | Words: `zero point zero one`, `twenty twenty-four`, `third` |
| `%` `=` `+` `/` `^` `<` `>` `*` `&` `@` `#` `~` | `percent`, `equals`, `plus`, `over` or `per`, `squared` or `to the power of`, `less than`, `greater than`, `times`, `and`, `at`, `number`, `about` |
| Maths symbols: `×` `÷` `±` `√` `∑` `∫` `∈` `≤` `≠` `≈` `∞` `→` | `times`, `divided by`, `plus or minus`, `the square root of`, `the sum of`, … |
| Greek letters: `α` `θ` `λ` `σ` | `alpha`, `theta`, `lambda`, `sigma` |
| `e.g.` `i.e.` `etc.` `vs.` `w.r.t.` | `for example`, `that is`, `and so on`, `versus`, `with respect to` |

Letters and `, . ; : ! ? ' " - — ( )` pass. Words on screen (props) may use
digits and symbols; only the script is spoken.

## Components

A component is one file, `components/<Name>.tsx`, named as the component.
It imports nothing and exports three things:

```tsx
export const props = {
  heading: str(),
  points: list(str()),
  cues: list(str()).optional(),
  accent: token("color").default("accent"),
};

export const examples = {
  three: { heading: "Why the slope matters", points: ["It points uphill", "Its size is the steepness", "Zero means flat"] },
  one: { heading: "In short", points: ["Step against the slope"] },
};

export default ({ heading, points, cues, accent }, { theme }) => {
  const { color, space, radius, type } = theme;
  // With cues, each point enters as its phrase is said; without (gallery), every 0.6 s.
  const at = (i) => (cues ? word(cues[i]) : 0.4 + i * 0.6);
  return (
    <Stack gap={space.xl} justify="center" style={{ flexGrow: 1, padding: space.xxl, fill: color.bg }}>
      <Text key="heading" style={{ ...type.h1, color: color.text }}>{heading}</Text>
      <Stack gap={space.l}>
        {points.map((point, i) => (
          <Enter key={`point-${i}`} preset="rise" at={at(i)}>
            <Row gap={space.m} align="center">
              <Box style={{ width: 18, height: 18, radius: radius.pill, fill: accent }} />
              <Text style={{ ...type.h2, color: color.text }}>{point}</Text>
            </Row>
          </Enter>
        ))}
      </Stack>
    </Stack>
  );
};
```

- **`props`** is required, even when empty (`{}`): lint and the app are
  built from it. A scene's props are checked against it before every call.
- **`examples`** are named prop sets. Give at least two; each must set
  every required prop. `gallery` draws each one.
- **The default export** is `(props, ctx) => element` and returns exactly
  one element. `ctx` is `{ t, duration, theme, size }`: `t` is seconds into
  the scene, `duration` its length, `size` the frame's `{ width, height }`.
- The root element fills the frame unless it sets its own size; give it
  `flexGrow: 1` and the scene's background `fill`.
- A helper component in the same file is called the same way, `(props,
  ctx)`; a `Sequence` or `Stagger` above it shifts its `ctx.t`.
- A component is a pure function of its props and `t`. There is no `Date`,
  no `import` and no I/O; `Math.random` is seeded per scene, and anything at
  module level starts fresh on every frame. Derive everything from `t`.
- `console.log` output is printed by `oculus studio frame`.

**Prop builders:** `str()`, `num()`, `bool()`, `list(x)`, `oneOf([...])`,
`latex()` (a string; there is no equation element yet), and `token(section)`,
which takes a token's *name* and hands the component its value:
`token("color").default("accent")` set to `"positive"` in a scene's props
gives `theme.color.positive`. Each takes `.default(v)` or `.optional()`.

Props are flat: there is no object builder, so a prop is never an object
and `list(x)` holds what `x` does (strings, numbers, booleans, `oneOf`
values, tokens or lists). Describe a list of records as parallel lists:
`labels: list(str())`, `values: list(num())`, `tones:
list(token("color"))`, read together by index.

## Elements

| Element | Props | Notes |
| --- | --- | --- |
| `Box` | `style`, `key`, children | Children are elements. `flexDirection` defaults to `row`. |
| `Stack` | `gap`, `align`, `justify`, `wrap`, `style`, `key`, children | A column Box. |
| `Row` | the same | A row Box. |
| `Text` | `style`, `key`, children | Children are strings or numbers. To mix styles, several Text in a Row. Wraps at its box's width. |
| `Image` | `src`, `style`, `key` | `src` is relative to `assets/`. A missing file is an error. |
| `Path` | `d`, `viewBox`, `fill`, `stroke`, `trim`, `style`, `key` | SVG path data fitted into its box. `stroke` is `{ width, color, cap, join, dash, dashOffset }`; `trim: [start, end]` draws that fraction of the length. |
| `Morph` | `p`, two children | Two layouts of one place, blended at `p` (0 to 1). No `key`. |

An element takes only these props; anything else is an error that names
the allowed ones. Style keys go inside `style`.

- **A Box is a row.** `justifyContent` places children across, `alignItems`
  up and down; in a Stack it is the other way round.
- **Text wraps at its box's width.** A box with `position: "absolute"` and
  no `width` takes its text's width. Give it a `width` and the text wraps
  inside it, so make the width generous: a label centred on a point is a
  wide box centred on that point with `justifyContent: "center"`, as in the
  diagram below.

**Style keys** (a closed set; an unknown key names the closest one):

| Group | Keys |
| --- | --- |
| Size | `width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `maxHeight`: px, `"N%"` or `"auto"` |
| Flex | `flexDirection`, `flexWrap`, `flexGrow`, `flexShrink`, `gap`, `alignItems`, `alignSelf`, `justifyContent` |
| Space | `padding`, `margin`: `n`, `[vertical, horizontal]` or `[top, right, bottom, left]` |
| Position | `position` (`relative`, `absolute`), `left`, `top`, `right`, `bottom` |
| Paint | `fill` (a colour or `{ linear: { angle, stops: [[0, c1], [1, c2]] } }`), `radius`, `border` `{ width, color }`, `shadow` `{ x, y, blur, spread, color }`, `blur`, `opacity`, `clip` |
| Transform | `translateX`, `translateY`, `scale`, `rotate` (degrees), `transformOrigin` (`[fx, fy]` or `"top left"`), `clipInset` `[top, right, bottom, left]` fractions |
| Text | `fontFamily`, `fontSize`, `fontWeight`, `color`, `lineHeight`, `letterSpacing`, `textAlign`, `fontVariantNumeric` (`"tabular-nums"`), `reveal` |
| Image | `objectFit` (`fill`, `cover`, `contain`) |

Colours are `#rgb`, `#rrggbb` or `#rrggbbaa`, always read from the theme.

## The theme

`ctx.theme` holds tokens, the same names in both bundled themes:

- `color`: `bg`, `surface`, `surfaceAlt`, `text`, `muted`, `faint`,
  `border`, `accent`, `accentStrong`, `accentSoft`, `onAccent`, `ink`,
  `onInk`, `shadow`, `positive`, `warning`, `negative`.
- `type`: `display`, `h1`, `h2`, `h3`, `body`, `label`, `caption`, each a
  style fragment to spread: `{ ...type.h1, color: color.text }`.
- `space`: `xxs` 4, `xs` 8, `s` 16, `m` 24, `l` 40, `xl` 64, `xxl` 112.
- `radius`: `s` 8, `m` 16, `l` 28, `xl` 44, `pill` 999.
- `font`: `heading` (Manrope), `body` (Inter). No other fonts exist.
- `motion`: `duration` (`fast`, `base`, `slow`, `stagger`), `easing`
  (`standard`, `enter`, `exit`), `spring` (`gentle`, `snappy`, `bouncy`).

`default` is neutral grey with an indigo accent; `warm` is cream with a
burnt-orange accent. For a new look, copy `../studio/library/themes/warm.json`
to the video's `themes/<name>.json`, change values (keep every section),
and set `"theme": "themes/<name>.json"`.

## Motion

Everything moves as a function of `t`.

- `<Enter preset at dur easing>` and `<Exit …>` animate their children in
  or out. Presets: `fade`, `rise`, `slide-left`, `slide-right`, `scale`,
  `blur`, `mask` (a left-to-right wipe). `dur` defaults to
  `theme.motion.duration.base`; `at` to 0 for Enter and to the end for
  Exit. They do not shift time.
- `<Sequence from dur hold>` gives its subtree `t - from`, clamped to
  `[0, dur]`. Outside its window the subtree is hidden but keeps its place,
  so nothing jumps; `hold` keeps it shown after. Children may be
  `(ctx) => element` to read the shifted `t`.
- `<Stagger each at>`: child i starts at `at + i * each`.
- `<Camera x y zoom rotate>`: a full-frame box that puts frame point
  `(x, y)` at the centre, zoomed. Use it as the root.
- `<Arrow points curve progress head stroke>`: a stroke through frame-px
  points drawn up to `progress`, with a head. Its parent must fill the
  frame.
- Helpers: `interpolate(t, [0, 1], [0, 200], { easing })` (easing must be
  a function here), `spring(t, { stiffness, damping, mass })`, `mix(c1, c2,
  p)` for colours, `clamp`, `lerp`, `linear`, `easeOutCubic`,
  `easeInOutCubic`, `cubicBezier(...theme.motion.easing.enter)`, `series`
  and `parallel` (start times from durations).
- `counter(t, 0, 92, { at, dur, decimals })` returns the number as a
  string; pair it with `fontVariantNumeric: "tabular-nums"`.
- Text `reveal: { progress: revealProgress(t, at, dur), by: "word", from:
  "rise" }` shows the text unit by unit (`char`, `word`, `line`; `fade`,
  `rise`, `blur`).

`easing` props take a function, `[x1, y1, x2, y2]`, or a theme easing name
(`"enter"`, `"exit"`, `"standard"`).

## A diagram scene

Cards placed absolutely from `ctx.size`, an arrow between them, and a label
that enters on a word. `Arrow` draws in frame pixels with the frame as its
box, so its parent must fill the frame: here the root `Box`, with
`flexGrow: 1`.

```tsx
export const props = {
  heading: str(),
  from: str(),
  to: str(),
  label: str(),
  cues: list(str()).optional(),
};

export const examples = {
  bank: { heading: "One message, two ends", from: "Your phone", to: "The bank", label: "Encrypted" },
  chat: { heading: "Same road, any message", from: "Alice", to: "Bob", label: "Private" },
};

export default ({ heading, from, to, label, cues }, { t, theme, size }) => {
  const { color, space, radius, type } = theme;
  // cues[0]: when the arrow draws; cues[1]: when the label enters.
  const tArrow = cues ? word(cues[0]) : 0.6;
  const tLabel = cues ? word(cues[1]) : 1.6;
  const cardW = 520;
  const cardH = 280;
  const top = (size.height - cardH) / 2 + space.xl;
  const xs = [space.xxl, size.width - space.xxl - cardW];
  const midY = top + cardH / 2;
  const card = (text, x) => (
    <Box style={{ position: "absolute", left: x, top, width: cardW, height: cardH, radius: radius.l,
      fill: color.surface, border: { width: 2, color: color.border }, justifyContent: "center", alignItems: "center" }}>
      <Text style={{ ...type.h1, color: color.text }}>{text}</Text>
    </Box>
  );
  return (
    <Box style={{ flexGrow: 1, fill: color.bg }}>
      <Box key="heading" style={{ position: "absolute", left: space.xxl, top: space.xl, width: size.width - 2 * space.xxl }}>
        <Text style={{ ...type.h1, color: color.text }}>{heading}</Text>
      </Box>
      <Enter preset="scale">{card(from, xs[0])}</Enter>
      <Enter preset="scale" at={0.2}>{card(to, xs[1])}</Enter>
      <Arrow
        points={[[xs[0] + cardW + space.m, midY], [xs[1] - space.m, midY]]}
        progress={interpolate(t, [tArrow, tArrow + 0.8], [0, 1], { easing: easeOutCubic })}
        stroke={{ width: 10, color: color.accent }}
      />
      <Box style={{ position: "absolute", left: 0, top: top - space.xxl, width: size.width, justifyContent: "center" }}>
        <Enter preset="rise" at={tLabel}>
          <Box style={{ padding: [space.s, space.l], radius: radius.pill, fill: color.accent }}>
            <Text style={{ ...type.h2, color: color.onAccent }}>{label}</Text>
          </Box>
        </Enter>
      </Box>
    </Box>
  );
};
```

## Word cues

`word("phrase")` is the scene time at which the narration starts saying
`phrase`; `wordEnd("phrase")` when it finishes. `word("phrase", 2)` is the
second time. Words match lowercased, letters, digits and apostrophes only,
consecutive words for a phrase, so cue the words as `script.md` spells
them: `word("zero point zero one")`.

- A phrase the narration does not say throws, naming the closest one it
  does say. Lint reports it at the TSX line.
- `word()` throws in a silent scene, and in `gallery`, which renders
  without narration. Put the cue phrases in props (an optional `cues:
  list(str())`, as in the example above) and fall back to fixed times only
  when they are absent, so the examples still draw.
- Cues exist only after `narrate`: a narrated scene is not rendered by
  lint, `frame` or `strip` until its narration is current.

## Transitions and magic move

Every scene after the first comes in through a transition, played over the
start of the incoming scene:

```json
"transition": { "kind": "push", "dur": 0.6, "easing": "easeInOutCubic", "direction": "left" }
```

- `kind`: `cut`, `dissolve`, `push`, `wipe`, `zoom` or `magic`.
- `dur` in seconds; it must fit inside the incoming scene.
- `easing`: `linear`, `easeOutCubic`, `easeInOutCubic`, a theme easing
  name, or `[x1, y1, x2, y2]`.
- `direction` (push and wipe): `left`, `right`, `up`, `down`, where the
  new scene comes from.
- Each field falls back to `video.json`'s `motion.transition`, then the
  theme's (a 0.7 s dissolve). A silent scene held under 0.7 s needs a
  shorter `dur` or a `cut`.

**Magic move.** When two consecutive scenes share a `key` and the incoming
one sets no `transition`, the transition becomes `magic`: elements with the
same key move, resize and recolour from one layout to the other, and the
rest fade. Keys must be unique within a scene. Keep a key the same on an
element you want to carry across; `"motion": { "magicMove": "off" }` in
`video.json` turns it off. `<Morph p>` does the same inside one scene.

`motion` in `video.json` also takes `morphColor` (`oklch` or `oklab`) and
`motionBlur` (`finals`, `always`, `off`; the default blurs only `--final`).

## Lint

`oculus studio lint <slug> --json` prints every problem as `{ scene, id,
component, example, kind, message, loc }` and `[]` when clean. It exits 1
on any problem. Read `message`: it says what is wrong and what is valid.

| `kind` | What it means | What to do |
| --- | --- | --- |
| `narration` | "no narration for its current text": the scene's script changed or was never narrated | Run `oculus studio narrate <slug>`. |
| `narration` | "the aligner is unsure of `word`" | Reword the sentence around it (a rare name, an acronym, a spelled-out symbol), then narrate again. |
| `narration` | "unscripted speech" | The voice said something the script lacks; reword, then narrate again. |
| `script` | A section no scene's `id` matches | Rename the heading or the scene id. |
| `script` | Text not in spoken form, with `say … as …` | Write the words it suggests. |
| `component` | A colour, font or spacing literal (`colour #5e6ad2 is a literal; use theme.color.accent`) | Use the token it names. |
| `component` | The component threw, or a key is on two nodes | Fix the TSX at `loc`. A bad `word()` phrase names the closest one said. |
| `lint` | An example omits a required prop | Add it to the example, or give the prop a `.default()`. |
| `props` | A scene's props do not match the schema | Fix the scene's props in `video.json` (path, expected, got). |
| `style` | An unknown style key or a bad value | Use the key or value it names. |
| `transition` | A bad transition value, one on the first scene, or a `dur` longer than its scene | Fix the field at the level it names (scene, `video.json` motion, theme). |
| `project` | A silent scene without `hold` | Give it a `hold`, or a `script.md` section. |
| `component_not_found` | No component by that name | Use one the message lists, or write it in `components/`. |
| `transpile` | The TSX does not compile, or uses `import` | Fix the syntax at `loc`. |
| `image`, `font`, `budget` | A missing asset, an unknown font, or a component doing too much work | Add the file; use `theme.font`; remove the loop. |

**The literal rules, exactly.** Lint reads each component's source:

- A colour is a `#` hex string, `rgb(`, `hsl(`, `oklch(` and the like, or a
  string that is a CSS colour name, anywhere outside `token(...)` and its
  `.default(...)`.
- A font is a bundled family's name as a string, or any string after
  `fontFamily:`.
- A number is checked only inside a `style={{ … }}` (or `style: { … }`)
  object, at any depth of brackets or calls, so the `8` in `width:
  Math.max(8, w)` and both numbers in `padding: [0, 16]` count. It is an
  error only when it equals a theme `space` or `radius` value (4, 8, 16,
  24, 28, 40, 44, 64, 112, 999 in the bundled themes; space is named
  first). Numbers inside a nested object (`shadow`, `border`, a gradient
  `fill`), after a minus sign, outside `style` (a `const`, `Arrow` points,
  a `stroke` prop) and every other value, such as 56 or 18, pass.

A broken `video.json` or `script.md` is not an issue but a failure: the
command prints the file, the line and the reason, and nothing else runs.

## Looking

Each command prints the PNG's absolute path. Open it with your
file-reading tool and look.

- `oculus studio frame <slug>` draws one frame to `out/frame-<t>.png`.
  `--t S` is seconds into the video; `--scene N --at S` is seconds into
  scene N (scenes count from 0). `--scale 0.5` halves it.
- `oculus studio strip <slug>` draws 12 frames of the whole video, 4 a row,
  at quarter size, to `out/strip-<from>-<to>.png`, each labelled with its
  time and scene. `--scene N` covers one scene; `--from`/`--to`, `--n`,
  `--cols` and `--scale` shape it. Use it to check a motion: what moves, in
  which order, where it settles.
- `oculus studio gallery <slug> <Component>` draws each example of a
  component to `out/gallery/<Component>-<example>.png`; `--sheet` adds all
  of them as `<Component>.png`. `--theme warm` tries another theme.

`frame` and `strip` stop on stale narration; lint lists it.

## Narrate

`oculus studio narrate <slug>` turns each section into a clip with the TTS
server set in Settings → Voice, times its words, and writes
`narration/manifest.json`. Unchanged scenes print `cached`. `--scene <id>`
does one scene (an id, unlike `strip --scene`, which takes an index);
`--force` makes it again. A failure keeps the scenes before it and names
the failed one.

When it fails because nothing is set (`no TTS server`, `no TTS model`, `no
TTS voice`) or the aligner model is missing, stop and ask the student to
set it in Settings → Voice. Never run `oculus studio aligner download`.

## Render

`oculus studio render <slug>` writes `renders/draft.mp4`: 540 px high and
fast. `--final` writes `renders/final.mp4` at full size, with motion blur
(by the default `motionBlur`, only finals blur). Each scene is cached, so a second render redoes only the
scenes an edit touched. It never narrates: stale narration stops it, naming
the scene. Report the path it prints.

## Libraries

A component name resolves through three levels; the first hit wins:

1. the video's `components/`,
2. the subject's library, `../studio/subjects/<CODE>/components/`,
3. the global library, `../studio/library/components/`.

Themes resolve the same way through `themes/`. `oculus studio components
list <slug>` prints each name and its level; `components show <slug>
<Name>` prints its source.

Library files change only through promote, which keeps the old text in
`.history/`:

```bash
oculus studio components promote <slug> <Name> --to subject
```

```bash
oculus studio themes promote <slug> themes/chalk.json --to global
```

Only a file in the video's own folder can be promoted. `--to subject` needs
a `subject` in `video.json`. Promote only what the student wants to keep.

**The starter components** (global library):

| Component | Props | For |
| --- | --- | --- |
| `TitleCard` | `title`; optional `eyebrow`, `subtitle` | The opening card, dark |
| `Steps` | `steps` (a list); optional `heading` | 3 to 5 steps of a few words. Packed at the top, so fewer leave the lower half empty |
| `BigNumber` | `value`, `label`; optional `unit` | One figure, large |
| `LowerThird` | `name`; optional `role` | A name caption, bottom left |
| `SplitShell` | `heading`, `left` and `right` (lists); `leftTitle`, `rightTitle` (default `Left`, `Right`) | 3 to 5 short lines a side. Its body-size text sits in cards that fill the frame, so one or two lines leave them mostly empty |

Each also takes `accent`, a colour token's name (default `"accent"`). All
five are static and text only: nothing moves and nothing is cued. For
motion, word cues, a diagram, or a comparison of a few big words, write the
video's own component.
