# Studio reference

The authoring rules for a video folder; `SKILL.md` has the roles, the
workflow and the folder's layout, and `build.md` a scene helper's steps.
`studio.d.ts` types every global named here. Every command takes the
video's slug as `DIR`; `oculus studio <command> --help` has the full flags.

## Contents

- video.json: its keys, a scene's keys, how long a scene lasts
- script.md: sections, the spoken-form table, acronyms and fusions
- Components: the file's three exports, prop builders, flat props
- Elements: element props, layout, the style keys
- The theme: token names and type sizes, making a new theme
- Motion: Enter and Exit, Sequence, Stagger, Camera, Arrow, the helpers
- Maths: `Math`, backslashes, `\k` groups and keys, theme colours, size,
  errors
- Moving maths: `Write`, `Steps`, the overlays; five recipes (a derivation
  in place, writing on per group, pointing at a term, a result shrunk
  aside, a number into a symbol's slot); carrying a formula into the next
  scene; gotchas
- Word cues: `word()`, `wordEnd()`, cues in props
- Transitions and magic move
- Lint: each kind and what to do, the literal rules exactly
- Looking: frame, strip and gallery
- Narrate
- Pronunciation: guessed words, `oculus studio g2p`, suggesting a reading with `oculus studio pronounce`
- Render
- Libraries: how a name resolves, promoting, the starter components

## video.json

```json
{
  "size": { "width": 1920, "height": 1080 },
  "fps": 30,
  "theme": "default",
  "subject": "COMP30027",
  "scenes": [
    { "id": "title", "component": "TitleCard", "props": { "title": "Gradient descent" }, "hold": 3 },
    { "id": "factor", "component": "FactorTwelve", "props": { "narrated": true } },
    { "id": "sorts", "component": "Bars",
      "props": { "labels": ["\\text{bubble}", "\\text{insertion}", "\\text{merge}"],
                 "values": [500, 250, 9], "diff": [0, 2], "diffLabel": "\\times 55", "diffAt": 2.2 },
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

Lint checks symbols, not sound. Two things it cannot see:

- **Acronyms said letter by letter** are written as spaced capitals,
  `R S A`; an acronym said as a word is written as one, `NASA`.
- **Fusions across words.** The voice runs words together, so a spelled
  letter or a short word can merge with its neighbour: `R S A rests` is
  heard as "RS arrests", `a part` as "apart". After a spelled acronym,
  avoid a next word that starts with a vowel sound or repeats the
  acronym's last sound; rephrase (`R S A depends on`, `R S A's security
  rests on`).

## Components

A component is one file, `components/<Name>.tsx`, named as the component.
It imports nothing and exports three things:

```tsx
export const props = {
  count: num().default(7),
  cols: num().default(7),
  cues: list(str()).optional(),
};

export const examples = {
  row: { count: 7, cols: 7 },
  rectangle: { count: 12, cols: 4 },
};

export default ({ count, cols, cues }, { theme }) => {
  const { color, space, radius, motion } = theme;
  // With cues, the dots start entering as cues[0] is said; without (gallery), at 0.3 s.
  const start = cues ? word(cues[0]) : 0.3;
  const rows = Math.ceil(count / cols);
  return (
    <Stack align="center" justify="center" gap={space.m} style={{ flexGrow: 1, fill: color.bg }}>
      {Array.from({ length: rows }, (_, r) => (
        <Row gap={space.m}>
          {Array.from({ length: Math.min(cols, count - r * cols) }, (_, c) => {
            const i = r * cols + c;
            return (
              <Enter preset="scale" at={start + i * motion.duration.stagger}>
                <Box key={`d${i}`} style={{ width: 48, height: 48, radius: radius.pill, fill: color.accent }} />
              </Enter>
            );
          })}
        </Row>
      ))}
    </Stack>
  );
};
```

- **`props`** is required, even when empty (`{}`): lint and the app are
  built from it. A scene's props are checked against it before every call.
- **`examples`** are named prop sets; each must set every required prop.
  `gallery` draws each one. A component made for one scene may take no
  required props and have no examples (its scene's strip is its check); one
  meant for several scenes gets at least two.
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
`latex()` (a string of LaTeX source, for a `<Math tex>`),
and `token(section)`, which takes a token's *name* and hands the component
its value:
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
| `Math` | `tex`, `display`, `style`, `key` | LaTeX as one keyed piece per glyph ("Maths"). |
| `Morph` | `p`, `lag`, `fade`, `ease`, two children | Two layouts of one place, blended at `p` (0 to 1). No `key`. |
| `Steps` | `at`, `dur`, `ease`, `lag`, `fade`, two or more children | States of one place, the pair picked by time ("Moving maths"). |
| `Write` | `at`, `dur`, `lag`, `stroke`, children | Writes the Math inside it on ("Moving maths"). |
| `Brace`, `Surround`, `Underline`, `Indicate` | `target`, `at`, `dur`, `ease`, `progress`, and their own | Overlays on a key, drawn after layout ("Moving maths"). |
| `Arrow` (with `from` and `to`) | `from`, `to`, `bend`, `head`, `gap`, `stroke`, `at`, `dur`, `ease`, `progress` | An overlay between two keys or `[x, y]` points in its parent's px, drawn after layout; without `from` and `to` it is the points arrow ("Moving maths"). |

An element takes only these props; anything else is an error that names
the allowed ones. Style keys go inside `style`.

- **A Box is a row.** `justifyContent` places children across, `alignItems`
  up and down; in a Stack it is the other way round.
- **Text wraps at its box's width.** A box with `position: "absolute"` and
  no `width` takes its text's width. Give it a `width` and the text wraps
  inside it, so make the width generous: a label centred on a point is a
  wide box centred on that point with `justifyContent: "center"`.

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
- `type`: `display`, `h1`, `h2`, `h3`, `body`, `label`, `caption`, `code`,
  each a style fragment to spread: `{ ...type.h1, color: color.text }`.
  Sizes in the bundled themes, in px on the 1920x1080 frame: `display` 104,
  `h1` 72, `h2` 52, `h3` 38, `body` 32, `label` 24, `caption` 22, `code`
  28 (monospace).
- `space`: `xxs` 4, `xs` 8, `s` 16, `m` 24, `l` 40, `xl` 64, `xxl` 112.
- `radius`: `s` 8, `m` 16, `l` 28, `xl` 44, `pill` 999.
- `font`: `heading` (Manrope), `body` (Inter), `mono` (JetBrains Mono). No
  other fonts exist. Mono is for code only: set it with `font.mono` or
  `type.code` in a `Code`-named component, never for numbers (they take
  `fontVariantNumeric: "tabular-nums"`). Lint notes `monospace outside
  code` otherwise. A theme without `font.mono` or `type.code` gets the
  bundled values.
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
  points drawn up to `progress`. Its parent must fill the frame.
  `<Arrow>`'s head rides the drawn end of the shaft, pointing along the
  shaft's tangent there, so the arrow reads as being drawn; it grows in
  from nothing over the first head length of shaft and settles on the last
  point at `progress` 1. With `from` and `to` instead of `points` it is an
  overlay that points at a term ("Moving maths").
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

## Maths

`<Math tex="…" key="eq">` typesets LaTeX with KaTeX, as the app does, and
draws each glyph, fraction bar, root sign and tall bracket as its own keyed
piece. That is what lets a formula be written on term by term and rewrite
itself into the next line. Every formula, every number in a formula and
every variable is a Math, never Text.

| Prop | What it is |
| --- | --- |
| `tex` | The LaTeX source. |
| `key` | Prefixes every piece's key: `eq` gives `eq/…`. Give every Math one; the same key in two states of a `Steps`, or in two scenes, is what makes it move. |
| `display` | `true` (default): display style, limits above and below. `false`: inline. |
| `style` | `fontSize` and `color`, plus placement and paint (`margin`, `position`, `left`, `top`, `opacity`, transforms). |

- **Backslashes.** In a JSX attribute a backslash is itself:
  `tex="\frac{a}{b}"`. In a JS string or template literal it is written
  twice: `tex={"\\frac{a}{b}"}`, ``tex={`\\k{n}{${n}}`}``. Use attributes
  unless the formula is built from a prop or a constant.
- **Groups and keys.** `\k{name}{…}` names a term: its pieces are keyed
  `eq/name`, `eq/name.1`, `eq/name.2`, … in reading order, and the
  innermost group wins. A piece in no group gets an automatic key from its
  character and how many came before it. Automatic keys suit a `=` or a
  lone bracket, but they can pair the wrong copy of a repeated digit across
  a rewrite, so name every term that must travel deliberately. Names are
  words (`lhs`, `exp`, `val`): a group named like a symbol of the same
  formula (`x` where the formula has an `x`) is an error. `Write` cues
  groups, overlays target them, and `Steps` matches by them.
- **Colour.** `style.color` inks the whole formula (default
  `theme.color.text`). Inside, `\color{accent}` and `\textcolor{accent}{…}`
  take a theme colour's name (any `theme.color` token) or hex; a CSS colour
  name is an error that lists the theme's.
- **Size.** `style.fontSize` is the size of the text the formula sits
  with; the maths draws 1.21 times larger, as KaTeX does. Spread a type
  token: `style={{ ...type.h1 }}` is 72 px text and 87 px maths. The Math's
  box is the formula's size and its baseline the formula's, so beside a
  Text in a Row use `align="baseline"`. `width`, `height`, `padding`,
  `border`, flex container keys, `textAlign` and `reveal` are errors: wrap
  the Math in a Box to size or pad it.
- **Words in a formula** are `\text{…}` inside it (`\text{or}`,
  `\textcolor{accent}{\text{prime}}`), since only Math can be written on.
- **Two lines** are an environment (`\begin{gathered} … \\ … \end{gathered}`,
  `aligned`); `\\` outside one is an error. `\phantom{…}` takes its space
  and draws nothing, which keeps a line in place when what was beside it
  goes.
- **Errors.** A formula KaTeX cannot parse fails the frame with KaTeX's
  message and position, and lint reports it at the TSX line:
  ``<Math tex="\frac{a}{">: Unexpected end of input …``. So does a
  character no KaTeX font has.
- `Math.max` and `Math.random` still work: the element is JS's own `Math`.
- A `latex()` prop is a string of LaTeX source for a Math's `tex`.

## Moving maths

The rule: a formula is written on term by term as it is said, rewrites
itself in place, and shrinks aside when done. These do it:

- **`<Write at dur lag stroke>`** writes every Math inside it on, manim's
  Write: each piece's outline strokes on, then its fill fades in, piece
  after piece in reading order. `at` is seconds for the whole formula, or
  `{ group: seconds }` naming `\k` groups, each cued on its own words. A
  piece in no listed group writes with the nearest listed group before it,
  else the first; a group includes the groups inside it. `dur` (a number or
  `{ group: seconds }`, default `motion.duration.slow`) is each group's
  run. Before its cue a piece is invisible but keeps its place. A name that
  is not a group of the formula is an error listing its groups.
- **`<Steps at dur ease lag fade>`** holds N states of one place, its
  children. Before `at[0]` the first state shows; transition k (state k to
  k + 1) starts at `at[k]` and runs `dur` (default `motion.duration.base`;
  `motion.duration.slow` reads better for a derivation), cut short by the
  next cue. `lag` (0 to 1) starts the pieces one after another in reading
  order, manim's lag_ratio. `ease` shapes each piece's run (default
  easeInOutCubic). A Steps inside another Steps or a Morph is an error.
- **How a rewrite reads.** A key matches the same key anywhere in the next
  state, so a term can move from one side to the other or into an
  exponent: it glides, resizes and recolours. A key the next state drops
  fades out and a new one fades in, at the same time as the glide, each
  riding beside its nearest matched neighbour, so `(2^2 + 1)` slides in
  behind what it follows instead of appearing at its final place.
  `fade="sequential"` fades out, then in, in place.
- **Overlays** point at a term after layout and follow it while it moves.
  Each takes a `target` (a key or a key prefix: `eq/lhs` covers `eq/lhs`,
  `eq/lhs.1`, …; `eq` is the whole formula) and draws on from `at` over
  `dur`, or at `progress` (0 to 1). Sizes default to the target's maths
  size.
  - `<Brace target label side>`: KaTeX's brace below (default), above,
    left or right, the label beyond it: a string is set at body size, so
    pass a Text for a label that matches large maths (recipe 3).
  - `<Surround target>`: a rounded rectangle drawn on round it, accent.
  - `<Underline target>`: a line drawn on under it, accent.
  - `<Indicate target at>`: the term itself pulses larger and flashes to
    the accent, then back. It needs `at` or `progress`.
  - `<Arrow from to bend>`: each end a key or a `[x, y]` point in its
    parent's px; it stops just short of a keyed end.
- **`<Morph p lag fade ease>`** is the two-state engine under Steps. Use it
  only when you compute `p` yourself; with `lag`, pass a linear `p` and an
  `ease`.

### Recipes

Each is a whole component, checked with `strip` on its fixed times. Its
`narrated` prop is `true` in `video.json` (cues on `word()`) and `false`
without narration (fixed times). The comment is the narration it is cued
to. Start a scene from the closest one and change the maths, not the
mechanism.

**1. A derivation rewritten in place**, written on first: 15 = 2⁴ − 1 →
(2²)² − 1 → (2² − 1)(2² + 1) → 3 × 5.

```tsx
export const props = {
  narrated: bool().default(false),
};

export const examples = {};

// Narration: "Fifteen is two to the fourth, minus one. Two to the fourth is
// two squared, squared. So this is a difference of squares: two squared minus
// one, times two squared plus one. That is three times five."
export default ({ narrated }, { t, theme, size }) => {
  const { color, type, motion } = theme;
  const cue = (phrase, fallback) => (narrated ? word(phrase) : fallback);
  const at = [cue("two squared", 3.0), cue("difference", 4.6), cue("three times five", 6.2)];
  const big = { ...type.display };
  const edge = size.width * 0.25;
  // The formula starts at a fixed left edge, so `15 =` never moves while the
  // right side is rewritten. Lag staggers the rewrites; the last step, where
  // a whole side becomes its value, goes at once.
  return (
    <Stack justify="center" align="flex-start" style={{ flexGrow: 1, fill: color.bg, padding: [0, edge] }}>
      <Steps at={at} dur={motion.duration.slow} lag={t < at[2] ? 0.1 : 0}>
        <Write at={{ lhs: cue("fifteen", 0.3), base: cue("two to the fourth", 0.9), less: cue("minus one", 1.8) }}>
          <Math key="eq" tex="\k{lhs}{15} = \k{base}{2}^{\k{exp}{4}} \k{less}{- 1}" style={big} />
        </Write>
        <Math key="eq" tex="\k{lhs}{15} = (\k{base}{2}^{2})^{2} \k{less}{- 1}" style={big} />
        <Math key="eq" tex="\k{lhs}{15} = (\k{base}{2}^{2} \k{less}{- 1})(2^{2} + 1)" style={big} />
        <Math key="eq" tex="\k{lhs}{15} = 3 \times 5" style={big} />
      </Steps>
    </Stack>
  );
};
```

**2. Writing on per group**, a word included: 7 = 1 × 7, then *prime*.

```tsx
export const props = {
  narrated: bool().default(false),
};

export const examples = {};

// Narration: "Seven is one times seven, and nothing else: its only factors
// are one and itself. That is what prime means."
export default ({ narrated }, { theme }) => {
  const { color, type } = theme;
  const cue = (phrase, fallback) => (narrated ? word(phrase) : fallback);
  return (
    <Stack justify="center" align="center" style={{ flexGrow: 1, fill: color.bg }}>
      <Write
        at={{
          lhs: cue("seven is", 0.3),
          one: cue("one times", 0.9),
          self: cue("times seven", 1.3),
          name: cue("prime", 2.6),
        }}
        dur={{ name: 1.2 }}
      >
        <Math
          key="eq7"
          tex="\k{lhs}{7} = \k{one}{1} \k{self}{\times 7} \qquad \k{name}{\textcolor{accent}{\text{prime}}}"
          style={{ ...type.display }}
        />
      </Write>
    </Stack>
  );
};
```

**3. Pointing at terms** as they are named: a brace with a label, a
surround, an arrow from a label and a pulse.

```tsx
export const props = {
  narrated: bool().default(false),
};

export const examples = {};

// Narration: "Seven to the fourth is two thousand four hundred and one, which
// is a hundred and sixty fifteens, plus one. So it leaves one over when
// divided by fifteen. The four that got us there is the period."
export default ({ narrated }, { theme }) => {
  const { color, type, motion } = theme;
  const cue = (phrase, fallback) => (narrated ? word(phrase) : fallback);
  const period = cue("period", 5.4);
  return (
    <Stack justify="center" align="center" style={{ flexGrow: 1, fill: color.bg }}>
      <Write
        at={{
          base: cue("seven to the fourth", 0.3),
          big: cue("two thousand", 1.1),
          mult: cue("hundred and sixty", 2.0),
          rest: cue("plus one", 2.8),
        }}
      >
        <Math
          key="eq"
          tex="\k{base}{7}^{\k{per}{4}} = \k{big}{2401} = \k{mult}{160 \times 15} \k{rest}{+ \k{one}{1}}"
          style={{ ...type.h1 }}
        />
      </Write>
      <Brace
        target="eq/mult"
        label={<Text style={{ ...type.h3, color: color.muted }}>a multiple of 15</Text>}
        at={cue("fifteens", 2.3)}
      />
      <Surround target="eq/one" at={cue("leaves one over", 3.8)} />
      <Enter preset="fade" at={period}>
        <Text
          key="periodName"
          style={{ ...type.h3, color: color.accent, position: "absolute", left: "18%", top: "16%" }}
        >
          the period
        </Text>
      </Enter>
      <Arrow from="periodName" to="eq/per" bend={-30} at={period} />
      <Indicate target="eq/per" at={period + motion.duration.base} />
    </Stack>
  );
};
```

**4. A result shrunk aside, then a new formula.** The second state lays
the same `rule` Math out small in the corner and writes the next formula
in the middle.

```tsx
export const props = {
  narrated: bool().default(false),
};

export const examples = {};

// Narration: "So a to the r leaves one over, mod N. Keep that in the corner:
// it means a to the r minus one is a multiple of N, and that splits into two
// factors."
export default ({ narrated }, { theme }) => {
  const { color, type, space } = theme;
  const cue = (phrase, fallback) => (narrated ? word(phrase) : fallback);
  const full = { width: "100%", height: "100%" };
  return (
    <Box style={{ flexGrow: 1, fill: color.bg }}>
      <Steps at={[cue("keep that", 2.4)]} dur={1.2}>
        <Stack justify="center" align="center" style={full}>
          <Write at={{ power: cue("a to the r", 0.3), rest: cue("leaves one over", 1.0) }}>
            <Math key="rule" tex="\k{power}{a^r} \k{rest}{\equiv 1 \pmod{N}}" style={{ ...type.display }} />
          </Write>
        </Stack>
        <Box style={full}>
          <Math
            key="rule"
            tex="\k{power}{a^r} \k{rest}{\equiv 1 \pmod{N}}"
            style={{ ...type.h2, position: "absolute", left: space.xl, top: space.l }}
          />
          <Stack justify="center" align="center" style={full}>
            <Write at={{ lhs: cue("minus one", 3.7), split: cue("splits", 5.0) }}>
              <Math
                key="next"
                tex="\k{lhs}{a^r - 1 = k N} \k{split}{= (a^{r/2} - 1)(a^{r/2} + 1)}"
                style={{ ...type.h1 }}
              />
            </Write>
          </Stack>
        </Box>
      </Steps>
    </Box>
  );
};
```

**5. A number flying into a symbol's slot.** The formula and the givens
are one Math, so each value keeps its `\k` key from the given line into
the formula.

```tsx
export const props = {
  narrated: bool().default(false),
};

export const examples = {};

// Narration: "The recipe wants a, r and N. Here a is seven, r is four, and N
// is fifteen. Put them in. Seven squared minus one is forty-eight, and the
// gcd of forty-eight and fifteen is three."
// The formula and the givens are one Math, so each value's \k key is the
// same in both states and the value flies into its slot. `aligned` with a
// leading & starts both lines at one left edge; a phantom keeps the lines
// where they were once the givens have gone.
const GIVEN = "\\k{ga}{a = \\k{av}{7},} \\quad \\k{gr}{r = \\k{rv}{4},} \\quad \\k{gn}{N = \\k{nv}{15}}";
const HIDE = "\\phantom{a = 7,}";
const lines = (top, bottom) => `\\begin{aligned} &${top} \\\\ &${bottom} \\end{aligned}`;

export default ({ narrated }, { theme, size }) => {
  const { color, type, motion } = theme;
  const cue = (phrase, fallback) => (narrated ? word(phrase) : fallback);
  const big = { ...type.h1 };
  const edge = size.width * 0.3;
  return (
    <Stack justify="center" align="flex-start" style={{ flexGrow: 1, fill: color.bg, padding: [0, edge] }}>
      <Steps
        at={[cue("put them in", 3.6), cue("forty-eight", 5.2), cue("is three", 6.8)]}
        dur={motion.duration.slow}
      >
        <Write
          at={{
            fn: cue("recipe", 0.3),
            ga: cue("is seven", 1.2),
            gr: cue("is four", 1.9),
            gn: cue("is fifteen", 2.6),
          }}
        >
          <Math key="eq" tex={lines("\\k{fn}{\\gcd}(a^{r/2} - 1,\\ N)", GIVEN)} style={big} />
        </Write>
        <Math key="eq" tex={lines("\\k{fn}{\\gcd}(\\k{av}{7}^{\\k{rv}{4}/2} - 1,\\ \\k{nv}{15})", HIDE)} style={big} />
        <Math key="eq" tex={lines("\\k{fn}{\\gcd}(\\k{prod}{48},\\ \\k{nv}{15})", HIDE)} style={big} />
        <Math key="eq" tex={lines("\\k{fn}{\\gcd}(\\k{prod}{48},\\ \\k{nv}{15}) = 3", HIDE)} style={big} />
      </Steps>
    </Stack>
  );
};
```

### Carrying a formula into the next scene

Give the next scene's tree a Math with the same `key` and `tex`, and set
no `transition` on that scene: the magic move glides every shared piece
from its last place to its new one, and the rest of both scenes fades. To
keep a corner formula still, lay it out at the same place and size. The
scene after recipe 4:

```tsx
export const props = {
  narrated: bool().default(false),
};

export const examples = {};

// Narration: "Each factor might share a piece of N."
// The scene after KeepAside, with no `transition`: `rule` stays in its
// corner and `next` glides up and shrinks, both by their keys; the new line
// is written below.
export default ({ narrated }, { theme }) => {
  const { color, type, space } = theme;
  const cue = (phrase, fallback) => (narrated ? word(phrase) : fallback);
  return (
    <Box style={{ flexGrow: 1, fill: color.bg }}>
      <Math
        key="rule"
        tex="\k{power}{a^r} \k{rest}{\equiv 1 \pmod{N}}"
        style={{ ...type.h2, position: "absolute", left: space.xl, top: space.l }}
      />
      <Stack justify="center" align="center" gap={space.xl} style={{ width: "100%", height: "100%" }}>
        <Math
          key="next"
          tex="\k{lhs}{a^r - 1 = k N} \k{split}{= (a^{r/2} - 1)(a^{r/2} + 1)}"
          style={{ ...type.h2 }}
        />
        <Write at={cue("share", 1.0)}>
          <Math key="share" tex="\gcd(a^{r/2} - 1,\ N) > 1" style={{ ...type.h1 }} />
        </Write>
      </Stack>
    </Box>
  );
};
```

### Gotchas

- **Anchor what stays.** A centred formula that changes width moves every
  term, even the unchanged ones, and with `lag` a number's digits drift
  apart and collide. Start the formula at a fixed left edge (recipes 1 and
  5), so `15 =` holds still while the right side is rewritten; for two
  lines use `aligned` with a leading `&`.
- **One `lag` per Steps, picked by time.** `lag` 0.1 reads well when terms
  travel; a step where a whole side becomes its value, a shrink to a
  corner and a substitution read best at 0. Pass `lag={t < at[2] ? 0.1 :
  0}` to change it at a cue (recipe 1); the joins stay seamless.
- **Write wraps one Math, not a Steps.** Every group named in `at` must be
  a group of every Math inside the Write, so wrap the first state (or a new
  Math inside a later state, recipe 4), never the whole Steps.
- **Write writes Math only.** A word that must be written on is
  `\text{…}` inside a Math; a Text label enters with `Enter`.
- **An overlay's target must be on screen.** A key that a later state
  drops fades out, and an overlay on it fades with it. Point at a key every
  state keeps, or at a group that names the term in each state.
- **Overlays do not travel between scenes.** A magic move swaps an overlay
  whose box changed in halves instead of gliding it; let each overlay do
  its job inside its scene.
- **Keys are per scene.** Two Math elements with one key in one state are
  a duplicate-key error, and two Math elements never match each other's
  terms, so a value and the slot it flies into must be one Math (recipe 5).
- **A key missing from a middle state** fades out and back in. Keep a term
  in every state it should stay on screen for.
- **Name the terms that travel.** Automatic keys follow characters, not
  meaning: with two `2`s on a line, which one goes where is decided by
  order. A `\k` name on both sides is a promise.
- **Cue on words, not letters.** `word("a to the r")` finds the phrase; a
  letter alone may be timed as the word it sounds like ("Word cues").


## Word cues

`word("phrase")` is the scene time at which the narration starts saying
`phrase`; `wordEnd("phrase")` when it finishes. `word("phrase", 2)` is the
second time. Words match lowercased, letters, digits and apostrophes only,
consecutive words for a phrase, so cue the words as `script.md` spells
them: `word("zero point zero one")`. A hyphen splits a word into parts, in
the phrase and the narration alike, so `word("ninety one")` and
`word("ninety-one")` both find a said "ninety-one"; a phrase must start and
end on whole said words (`word("one")` does not find it).

- A phrase the narration does not say, or an `nth` past its last
  occurrence, throws at the call's TSX line and names the said phrase
  nearest it. Lint reports it there.
- `word()` throws in a silent scene, and in `gallery`, which never reads
  the narration. A component used in several scenes takes its cue phrases
  in props (an optional `cues: list(str())`, as in the example above) and
  falls back to fixed times without them, so its examples still draw. A
  component made for one scene can take a `narrated` flag instead, as
  every recipe in "Moving maths" does.
- A letter said alone ("q") may be timed as the word it sounds like
  ("cue"); cue the word beside it.
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

`oculus studio lint <slug>` checks the whole video; `--scene <id|N>`
checks one scene, its component and the video settings it renders with.
Each issue has a level:

- `error`: lint exits 1. Fix every one; "lint until no errors" is the bar.
- `warning`: how the scene looks over time (below). Lint still passes; fix
  it, or know why the scene wants it.
- `note`: a word the aligner was unsure of, or a word Kokoro guessed (check
  each as "Pronunciation" says), or a component that reads `font.mono` or
  `type.code` without `Code` at the start or end of its name ("monospace
  outside code": use the body face, and `tabular-nums` for numbers).

Text output prints errors one a line in full; several warnings or notes of
one kind in a scene share a line with a count and the first three.
`--json` prints every issue in full as `{ level, scene, id, component,
example, kind, message, loc }` (`[]` when there is nothing), and each
guessed word as `{ level: "note", scene, id, line, kind: "guessed",
message, word, phonemes, guessed, review }`, `review` being `null` or the
word's suggestion, `{ status: "pending" | "wrong", reading, note }`. Read
`message`: it says what is wrong and what is valid.

Warnings name the time span and the words said in it:

| `kind` | What it means | What to do |
| --- | --- | --- |
| `off_frame` | A node sits more than 4 px past the frame's edge (not while sliding in or out, not under a `Camera` zoom) | Move or shrink it inside the frame. |
| `still` | Narration plays over more than 2.5 s where nothing on screen changes, or nothing is on screen by 0.5 s | Cue something to those words: a highlight, a rewrite, the next element. |
| `half_frame` | For more than 3 s, everything sits in one half of the frame | Spread the picture, or put the next element in the empty half. |
| `fragment` | At the end, a Math shows only one or two of its pieces (a stray `= .` from a rewrite), or a tiny mark sits alone | Hide the whole Math, or finish the rewrite; remove the mark. |

| `kind` | What it means | What to do |
| --- | --- | --- |
| `narration` | "no narration for its current text": the scene's script changed or was never narrated | Run `oculus studio narrate <slug>`. |
| `narration` (note) | "the aligner is unsure of `word`" | Listen; if it is mangled, reword the sentence around it (a rare name, an acronym, a spelled-out symbol), then narrate again. |
| `narration` | "unscripted speech" | The voice said something the script lacks; reword, then narrate again. |
| `script` | A section no scene's `id` matches | Rename the heading or the scene id. |
| `script` | Text not in spoken form, with `say … as …` | Write the words it suggests. |
| `component` | A colour, font or spacing literal (`colour #5e6ad2 is a literal; use theme.color.accent`) | Use the token it names. |
| `component` (note) | "theme.font.mono is used outside a Code component" | Mono is for code; use `theme.font.body` and `fontVariantNumeric: "tabular-nums"`. |
| `component` | The component threw, or a key is on two nodes | Fix the TSX at `loc`. A bad `word()` phrase names the closest one said. |
| `lint` | An example omits a required prop | Add it to the example, or give the prop a `.default()`. |
| `props` | A scene's props do not match the schema | Fix the scene's props in `video.json` (path, expected, got). |
| `style` | An unknown style key or a bad value | Use the key or value it names. |
| `transition` | A bad transition value, one on the first scene, or a `dur` longer than its scene | Fix the field at the level it names (scene, `video.json` motion, theme). |
| `project` | A silent scene without `hold` | Give it a `hold`, or a `script.md` section. |
| `component_not_found` | No component by that name | Use one the message lists, or write it in `components/`. |
| `transpile` | The TSX does not compile, or uses `import` | Fix the syntax at `loc`. |
| `image`, `font`, `budget` | A missing asset, an unknown font, or a component doing too much work | Add the file; use `theme.font`; remove the loop. |
| `lint` | Kokoro's G2P pack is not downloaded | Stop and tell the student (`oculus studio voices download`). |
| `lint` | A `lexicon.json` entry names a phoneme Kokoro lacks, or a respelling part the dictionary lacks | Tell the student which file and entry; never edit a lexicon yourself. |
| `script` | Kokoro: a word with no phonemes, a `$`, or a sentence too long to say in one go | Reword: spell it out, or split the sentence or add a comma. |

**The literal rules, exactly.** Lint reads each component's source:

- A colour is a `#` hex string, or `rgb(`, `hsl(`, `oklch(` and the other
  functional notations, in any string. A whole string that is a CSS colour
  name is a colour only where a colour goes: the value of a `color`,
  `fill`, `stroke`, `background` or `stops` key or of any key ending in
  `Color` (an object key or a JSX attribute, including arrays, calls and
  ternaries inside it), or of a key named after a `token("color")` prop
  unless the theme has a colour by that name. Elsewhere it is a word on
  screen (`label: "purple"`) and passes. `token(...)` and a `.default(...)`
  on it take a token's name, not a literal.
- A font is a bundled family's name as a string, or any string after
  `fontFamily:`; the message names `theme.font.heading`, `.body` or `.mono`.
- A number is checked only inside a `style={{ … }}` (or `style: { … }`)
  object, as the value of a length key: `width`, `height`, `minWidth`,
  `minHeight`, `maxWidth`, `maxHeight`, `gap`, `padding`, `margin`,
  `left`, `top`, `right`, `bottom`, `radius`, `blur`, `translateX`,
  `translateY`, `fontSize` or `letterSpacing`. It counts when it is the
  value itself, a branch of it (an array element, as both numbers in
  `padding: [0, 16]`; a ternary's arm, `gap: big ? 24 : 16`; a side of
  `||`, `??` or `&&`) or a term added or subtracted (the `4` in `space.s
  + 4`). It is an error only when it equals a theme `space` or `radius`
  value (4, 8, 16, 24, 28, 40, 44, 64, 112, 999 in the bundled themes;
  space is named first). A unitless key (`opacity`, `scale`, `rotate`,
  `flexGrow`, `flexShrink`, `fontWeight`, `lineHeight`,
  `transformOrigin`, `clipInset`) is never checked, so the `4` in
  `opacity: clamp(pop * 4, 0, 1)` passes. So does a factor (`width: 24 *
  s`), a side of a comparison, a call's argument (`Math.max(8, w)`), a
  number inside a nested object (`shadow`, `border`, a gradient `fill`,
  `reveal`), a negative number, a number outside `style` (a `const`,
  `Arrow` points, a `stroke` prop), and every other value, such as 56 or
  18.

A broken `video.json` or `script.md` is not an issue but a failure: the
command prints the file, the line and the reason, and nothing else runs.

## Looking

Each command prints the PNG's absolute path.

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
  of them as `<Component>.png`. `--theme warm` tries another theme. It
  never reads the narration, so it runs before `narrate`: each still is the
  middle of the first silent scene that uses the component (its `hold`),
  else the middle of a 6 s preview, so entrances on fixed fallback times
  have settled. `--t S` picks another moment inside that length.

`oculus studio brief <slug> --scene <id|N>` prints one scene's whole input:
its index, component and props; what the scenes either side carry in and
hand on, and whether their components exist yet; its storyboard entry; the
notes it names; its words at the times `word()` returns; and its exact
`lint`, `beats`, `strip`, `frame` and `gallery` calls. Read it instead of
`storyboard.md` or the manifest.

`oculus studio beats <slug> --scene <id|N>` prints what changes on screen,
as text, with the words said then:

```
  0.20  appear   d0-d15  (centre (960,540))  "Fifteen is …"
  5.10  vanish   d15  "… loses a corner"
  6.20  move     d12-d14  → (1180,420)…(1180,660) over 0.8 s  "… stands up"
  7.40  move+resize+rewrite eq  (1248,349) → (1346,349); 390x66 → 577x65; +lap +plus -add over 0.8 s  "… one whole lap …"
```

Each line is a key (or a Math under its key, or `Box:12` for nodes with no
key, by TSX line): `appear`, `vanish`, `move`, `resize`, `recolour`,
`rewrite`. Check the beat table against it before you look at a strip, and
spend strips on how it looks.

`frame` and `strip` stop when any narrated scene's narration is stale, even
with `--scene` on another scene; lint lists it. A missing component in
another scene does not stop them.

## Narrate

`oculus studio narrate <slug>` turns each section into a clip with the TTS
server set in Settings → Voice, times its words, and writes
`narration/manifest.json`. Unchanged scenes print `cached`. `--scene <id>`
does one scene (an id, unlike `strip --scene`, which takes an index);
`--force` makes it again. A failure keeps the scenes before it and names
the failed one: `no TTS server`, `no TTS model` and `no TTS voice` mean
Settings → Voice is unset (`SKILL.md` step 5 says what to do).

The engine is Kokoro, run inside the app, or a TTS server; Settings →
Voice or the video's `voice` block (`"engine": "kokoro"`) picks it. With
Kokoro, narrate ends with a `note:` per guessed word, with the state of any
suggestion for it (`--json`: each note's `review`).

## Pronunciation

Kokoro reads each word from the lexicon, then its dictionary, then a
guesser. A guessed word is worth checking. The student decides how a word
is said; you suggest, and never edit a `lexicon.json`.

- `oculus studio g2p "<text>" --video <slug>` prints each word, its
  phonemes and who read it: `lexicon`, `dict`, `rule` (a word like "read"
  or "lead" read by its neighbours) or `guess`. It does not synthesise.
- `oculus studio pronounce suggest <slug> <word> <reading>` puts a reading
  up for review, for a word the video's narration says (any case;
  `Word's` counts as `Word`). It checks the reading, prints the phonemes
  it gives, and records how Kokoro reads the word now and the sentence it
  is in; a new suggestion for the same word replaces the last. `--json`
  gives the stored item plus `phonemes`.
- In Settings → Voice the student hears the sentence read both ways and
  picks one: the suggestion is correct (it goes into the lexicon), the
  guess is fine (the guess goes into the lexicon, so it is no longer
  flagged), or the suggestion is wrong, with a note on how the word is
  said. A decision applies at the next `narrate`: only scenes whose
  reading changed are made again.
- `oculus studio pronounce list [--video <slug>]` lists what is waiting
  (`pending`) and what was marked `wrong`, with the student's note;
  `--json` gives `[{ id, word, reading, original, guessed, subject, video,
  sentence, scene, line, status, note, created }]`. For a wrong one,
  suggest again following the note. Notes show the same state.
- The lexicon is two `lexicon.json` files, the subject's over the global
  one: `../studio/subjects/<CODE>/lexicon.json` (a video with a subject
  writes here) and `../studio/library/lexicon.json` (a video without
  one). Read them; the app writes them.
- **A respelling** is dictionary words joined by `-` or spaces:
  `SOFT-max`, `NESS-ter-off`. Every part must be in the dictionary (else
  the suggestion fails naming it); an all-caps part longer than one letter
  takes the main stress, the others a lighter one.
- **Raw phonemes** go in slashes, in Kokoro's alphabet: `/kˈOʃi/`. `ˈ`
  marks the main stress, `ˌ` a lighter one; copy symbols from words `g2p`
  prints rather than guessing them. A symbol Kokoro lacks, or `$`, fails
  naming it.
- A word finds its entry by exact key, then lower case; `Cauchy's` takes
  `Cauchy`'s entry plus `-s`.

## Render

`oculus studio render <slug>` writes `renders/draft.mp4`: 540 px high and
fast. `--final` writes `renders/final.mp4` at full size, with motion blur
(by the default `motionBlur`, only finals blur). Each scene is cached, so a
second render redoes only the scenes an edit touched. It never narrates:
stale narration stops it, naming the scene. Report the path it prints.

## Libraries

A component name resolves through three levels; the first hit wins:

1. the video's `components/`,
2. the subject's library, `../studio/subjects/<CODE>/components/`,
3. the global library, `../studio/library/components/`.

Themes resolve the same way through `themes/`. `oculus studio components
list <slug>` prints each name and its level; `components show <slug>
<Name>` prints its source.

Library files change only through promote, which keeps the old text in
`.history/`. A theme is promoted by its path:

```bash
oculus studio themes promote <slug> themes/chalk.json --to global
```

Only a file in the video's own folder can be promoted. `--to subject` needs
a `subject` in `video.json`. Promote only what the student wants to keep.

**The starter components** (global library) are drawing primitives, not
scenes. A component cannot import another, so each file holds its primitive
as a helper function and a default export that demos it: read it with
`components show <slug> <Name>`, copy the helper (and the small functions
beside it) into your scene component, and call it there with times from
`word()`. Every part is keyed under an `id` prop (default the name in the
table), so two calls with one id in a `Steps` glide part to part, and an
overlay can target a part (`bars/b2`) or the whole (`dots`).

| Primitive | Helper props | What it draws |
| --- | --- | --- |
| `NumberLine` | `from`, `to`, `step`, `labelEvery`, `width`, `at`, `dur`, `points: [{ value, name, at, moves: [[t, v]], color }]`, `span: { from, to, at, label, color }`, `id` (`nl`) | A line that draws on with its ticks and numbers, points that pop in and glide along it, an interval drawn on as a bar |
| `Axes` | `formula("sin(x)/x")` makes a function from text; `axes({ x: [lo, hi, step], y, origin, width, height })` maps data to px; `<Axes ax at dur id>` (`ax`) holds `<Plot ax f from to at dur color label head key>`, `<Dot ax x y at color label guides place key>`, `<Scatter ax xs ys at each color fit fitAt key>`, `<Area ax f g from to at dur color label key>`, `<Riemann ax f from to counts at refineAt color label key>` and `<Tangent ax f x0 x1 h h1 at moveAt dur color label key>` | Two axes drawn on with numbers; curves stroked on (breaking off-range), a dot riding the end, points with dashed guides, a scatter with its least-squares line and r², a shaded area, rectangles refined count by count, a tangent or shrinking secant with its slope. Flat props: `fns` (formula strings), `tones`, `labels`, `marks`, `px`/`py`, `fit`, `shade`, `rects`, `tangent`, `secant` |
| `Graph` | `layout(kind, nodes, edges, { width, height, root, columns, xs, ys, pad })` gives node px; `<Graph nodes edges weights directed kind root columns xs ys tones at edgesAt visit visitAt visitStart each via trace path walkAt walkEach radius tone pathTone id>` (`graph`) | Nodes and edges on a `circle`, `tree`, `layers` or `manual` layout, directed edges with heads, weights, self-loops; `visit` recolours nodes in order and lights the edge each came by, `path` walks a marker along nodes. Keys `graph/n<i>`, `graph/e<a>-<b>`, `graph/here` |
| `Grid` | `<Grid cells keys tones appear rowLabels colLabels brackets cell fitRows fitCols enter at each marks pointers id>` (`grid`); `marks: [{ kind: "row"/"col"/"cell"/"block", index, tone, at, off, moves }]`, `pointers: [{ label, side, index, at, tone }]` | A matrix, DP table, board or pixel image of TeX cells: fills, row/column/block marks that slide, pointers along a side; with `keys` a cell follows its value, so a Steps between two arrangements slides it. Keys `grid/c<r>_<c>` (or `grid/k<key>`) |
| `Bars` | `bars({ labels, series, to, changeAt, orientation, histogram, stacked, sort, unit, width, height, ... })` maps data to px; `<Bars ch id at barsAt each names tones line lineLabel lineAt highlight highlightAt dim diff diffLabel diffAt>` (`bars`) | A bar chart or histogram, grouped or stacked, vertical or horizontal: nice value axis, bars growing, value labels counting up, states gliding to the next (`sort` re-ranks), a reference line, a highlighted subset, a difference arrow. Keys `bars/b<i>`, `bars/l<i>`, `bars/v<i>` |
| `Code` | `tokenize(lines, lang)`; `<Code lines lang keys numbers first fontSize focus caret reveal notes trace added removed diffAt id maxWidth maxHeight>` (`code`) | Source on a panel, every character on a fixed column grid, coloured for `python`, `javascript`, `c`, `sql` or `plain`; a focus band that dims the rest, a caret, lines revealed one by one, notes beside lines, a trace table of variable values, a diff. `keys` keep a line's identity across versions, so a Steps slides the lines that persist (`code/l<key>`). The one starter in monospace |
| `Table` | `header`, `rows` (TeX), `at`, `each`, `by` (`row`/`col`), `marks: [{ rows, color, at }]`, `hide`, `colWidth`, `id` (`table`) | Cells of maths revealed row by row, rows banded; a Math elsewhere keyed like a cell (`table/r1c2`) takes its glyphs, so a value lifts out of the table |
| `TitleCard` | `title`; optional `eyebrow`, `subtitle`, `accent` | The opening card, dark. A scene on its own, the only one |

Each demo's flat props are in `components show`; `gallery` draws its
examples. A primitive draws; it never explains. The scene around it (what
is said, what moves on which word, what stays) is yours.
