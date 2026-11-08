---
name: oculus-widgets
description: Writes interactive illustrations for an Oculus chat reply as ```widget fences — a small TSX program on the Oculus kit (sliders, typed formulas, steppers on pages, 2D planes, 3D scenes, mathjs) that the student plays with inside the reply — and checks each one with `oculus widget check`. Use when a concept is easier to play with than to read, such as "how does X work", "explain X", "what happens when", "show me", "walk me through", a mechanism, a cycle, an algorithm's steps or a function to explore; when an Ask to fix message says a widget failed; and when saving a widget to Revision with `oculus artifact save --format widget`.
---

# Widgets

A ` ```widget ` fence in your reply is one TSX module that Oculus runs in a
sandboxed frame inside the reply, built from the Oculus kit so it looks
like the app, follows its light and dark themes, and keeps its state. The
student drags, types and steps through it where they are reading.

## Contents

- Draw the mechanism
- The fence
- Writing one: the checklist
- The gallery
- `oculus/ask`: a model from inside the widget
- When a widget fails
- Saving to Revision

The files beside this one are in `skills/oculus-widgets/` under your working
directory:

- **`widget.d.ts`**: the kit's contract: every module, component and prop.
  Open it for the props of a part you haven't used.
- **`bloch-sphere.tsx`**, **`derivative-steps.tsx`**, **`bubble-sort.tsx`**,
  **`vector-field.tsx`**: the gallery, each a complete fence (below).

The Oculus chat instructions own output choice and fence placement. Use
this skill once the answer calls for an interactive illustration; the kit,
visual design and validation below are specific to widgets.

## Draw the mechanism

- **The thing itself, not a diagram about it.** The state on the sphere,
  the bars of the array swapping, the arrows of the field — not boxes
  labelled "state" and "evolve" joined by arrows.
- **Give the picture the control the real system has.** Time, a parameter,
  an input, a starting point: the student learns by moving it. A picture
  with no control is a static image; put it in `outputs/` instead.
- **A small budget.** One idea per widget, two to four controls, a few
  colours. Colour means something — a state (compared, swapped, done) or an
  intensity — never just which item is which; position and labels carry
  order. Labels are a word or two, or maths.
- **Split a dense subject** into several widgets, each its own fence, with
  prose between them.
- **The explanation is the reply's text.** Before the fence, say what to try
  ("drag θ to 0, then press Evolve"); after it, what that showed. Inside the
  frame: no heading (the card shows the title), no paragraphs, no
  instructions longer than a short `Note`.
- **Never narrate the making.** The reply doesn't mention the check, the
  file, the kit or that "the widget is ready" — the student sees the
  subject, what to try and what it shows.

## The fence

````
```widget
// title: Bubble sort, one comparison at a time
import { Widget, Slider } from "oculus/ui";
import { useWidgetState } from "oculus/state";

export default function BubbleSort() {
  const [step, setStep] = useWidgetState("step", 0);
  return <Widget><Slider label="Step" value={step} onChange={setStep} min={0} max={27} step={1} /></Widget>;
}
```
````

- **First line `// title: …`**, a short noun phrase: the card's title.
- **The default export is a component taking no props**, and its root is
  `<Widget>`.
- **Root first, helpers after.** The student watches the widget draw while
  your reply streams, top to bottom. After the imports and the constants it
  reads, write the default export, then the components and functions it
  uses. A component not written yet shows as a placeholder, so the card
  fills in as you write; helpers written first draw nothing until the end.
  The gallery is in this order.
- **Imports**: only `react`, `oculus/ui`, `oculus/state`, `oculus/2d`,
  `oculus/math`, `oculus/3d`, `oculus/ask`, and `three` and
  `@react-three/fiber` as published (no `three/addons/…`, no drei). Nothing
  else loads: no CDN, no relative files, no dynamic `import()`. The frame
  has no network and no storage.
- **Colours are theme tokens.** A kit part (`oculus/2d`, `oculus/3d`) takes a
  token name: `foreground`, `muted`, `brand`, `success`, `warning`,
  `danger`, `chart-1` … `chart-5`. A plain element's style takes
  `var(--color-<token>)`: `foreground`, `muted-foreground`, `brand`,
  `success`, `warning`, `destructive`, `border`, `card`, `chart-1` …
  `chart-5`. No hex, `rgb()` or `hsl()`, and no font family: the frame
  already has the app's fonts. With tokens only, dark mode needs no work:
  check that nothing you wrote is a colour of its own.
- **Height.** The frame is exactly its content's height and never scrolls.
  Past 560 CSS px it is clipped and shows an error. A `Plane` or `Scene3D`
  of about 300 px, two sliders and a row of values fits. Longer content goes
  on `Pages` (one child per page; the frame is the tallest page's height, so
  paging never jumps) or `Tabs` — never a scrolling box.
- **Width.** The reply can be 300 px wide. `Row` wraps; don't fix widths.
- **State.** `useWidgetState(key, initial)` for what the student set —
  slider values, typed text, the page, the step. It survives reloads, theme
  changes, the expanded view and Save to Revision. A 3D scene's canvas is
  released when it scrolls away and its inner state starts over, so keep
  what matters in `useWidgetState` above the scene. Plain `useState` for
  what shouldn't come back after a reload (a playing animation).
- **Maths** is TeX: `<Tex>{"\\frac{a}{b}"}</Tex>`, `tex={"\\theta"}`,
  `<Label at={p}>{"\\theta"}</Label>`. Never `tex="\\theta"`: a JSX
  attribute string keeps both backslashes.
- **Typed input is validated as it is typed.** `formula(text, ["x"])` from
  `oculus/math` never throws; show its `error` in a
  `<Note tone="danger">` under the `Field`, keep drawing the last good
  result or nothing, and refuse to page or step past it.
- **Timers and listeners** go in `useEffect` (or `useLayoutEffect`) and the
  effect returns a cleanup that cancels them: `return () => clearInterval(id)`,
  `cancelAnimationFrame`, `removeEventListener`. An animation can also be a
  time slider, which needs no timer.
- **Accessible.** Every control has its `label`. A picture made of plain
  elements gets `role="img"` and an `aria-label` that says its state in
  words. Never make colour the only difference between two states.

## Writing one: the checklist

Copy this and tick it off:

```
- [ ] 1. Check the chat output rules: does this answer need a widget?
- [ ] 2. Name the mechanism and its controls; open the nearest gallery example
- [ ] 3. Write the TSX to ./outputs/<name>.tsx
- [ ] 4. oculus widget check ./outputs/<name>.tsx
- [ ] 5. Fix every finding and run the check again, until it says no problems
- [ ] 6. Read it over: under 560 px, tokens only, every control labelled,
         state in useWidgetState, no prose in the frame, input validated
```

The reply includes what to try, the fence (the file exactly), and what it
shows — nothing about the check or the file.

`oculus widget check <file>` compiles the widget the way the chat does,
then lints it: a fixed colour, a font family, a `\\` in a JSX attribute
string, `fetch` or storage or `eval`, a timer or listener with no cleanup
that cancels it, a missing title. Each finding has a line and column and
says what to write instead; `--json` prints them as a list. It exits 0 only
when there are none.

## The gallery

Four complete widgets, each a different part of the kit. Read the nearest
one before writing; copy its shape, not its subject.

- **`bloch-sphere.tsx`** (`oculus/3d`): a qubit's state on the Bloch sphere,
  θ and φ on sliders, an Evolve button that turns the phase. A
  `requestAnimationFrame` loop with its cleanup; state kept above the scene.
- **`derivative-steps.tsx`** (`oculus/math`, `Field`, `Pages`): the student
  types f(x); pages show the rule the outermost operation calls for, the
  derivative put together, then simplified and checked against a secant's
  slope. Validation that refuses to page.
- **`bubble-sort.tsx`** (`oculus/ui`, `oculus/state`): frames of the sort
  computed up front, Back/Next/Play over them, bars drawn as plain elements
  coloured by state with `var(--color-…)`; `setInterval` with its cleanup.
- **`vector-field.tsx`** (`oculus/2d`): a field drawn as a grid of `Vector`s
  with opacity for strength, a `MovablePoint` start whose path is traced and
  drawn with `Plot`, divergence and curl at the point.

## `oculus/ask`: a model from inside the widget

`ask(prompt, schema)` answers with JSON matching a JSON Schema (a subset:
`widget.d.ts` lists the keywords); `send(text)` posts `text` into the chat
as the student's next message. Both are billed to the student and gated:

- The first request waits until the student presses **Allow** on the card;
  one allowance covers 20 requests, one runs at a time, and the student can
  stop it. Until the student picks a model for Widget questions in
  Settings → Jobs, every request fails saying so.
- Use it only when the widget can't work without a model — new steps for a
  formula `oculus/math` can't take apart, feedback on a sentence the student
  wrote. Anything mathjs can compute, compute.
- Call it on a click, never on mount, in an effect or per keystroke.
- Use `useAsk()` in a component: `{ ask, send, busy, error }`; its calls
  never reject (a failure resolves `undefined` and sets `error`), so show
  `busy` on the button and `error` in a `Note`.
- `send` works only in a conversation: on the Revision Artifacts page there
  is no thread and it is refused.

```tsx
const { ask, busy, error } = useAsk();
const more = async () => {
  const r = await ask<{ steps: string[] }>(`Three steps to differentiate ${text}, as TeX`, {
    type: "object",
    properties: { steps: { type: "array", items: { type: "string" }, maxItems: 6 } },
    required: ["steps"],
  });
  if (r) setSteps(r.steps);
};
// <Pill onClick={more} disabled={busy}>More steps</Pill>{error && <Note tone="danger">{error}</Note>}
```

## When a widget fails

A widget that fails to compile, throws, overflows the 560 px budget or
draws wider than its frame shows one line under its title with **Ask to
fix**, which sends you: `The widget "<title>" failed: <error>. Fix it and
resend the widget.` with the fence. Fix the cause and run the check. The
reply includes the whole corrected fence. Common causes:

- "isn't a theme colour": a kit part was given a colour that isn't a token.
- Overflow: too much stacked; move part of it onto `Pages`.
- `Label3D goes inside a Scene3D or a Canvas`: a label outside its scene.
- "a widget needs a default export": the component isn't `export default`.

## Saving to Revision

Read `oculus-revision` for saving artifacts, including the subject and topic
rules. The program file is the TSX you checked above; that skill owns the
save command and format flag.
