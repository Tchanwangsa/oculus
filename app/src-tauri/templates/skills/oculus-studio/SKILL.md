---
name: oculus-studio
description: Makes an explainer video in an Oculus library (an animated lecture topic, a worked derivation, a narrated walk through an idea) in the style of 3Blue1Brown, by directing helpers that read the lecture and build one scene each, writing a storyboard, a spoken script and TSX scenes, then narrating, checking frame by frame and rendering an mp4 with `oculus studio`. Use for "make a video of lecture X", "an explainer of Y", "animate this proof", "narrate a walk through week 4", or any request whose result is a video.
---

# Videos

A video is a **folder** that `oculus studio` reads: `video.json` lists the
scenes, `components/` holds the TSX that draws them, and `script.md` holds
what is said. You write those files; the CLI narrates, checks, draws frames
and renders the mp4.

The target is a 3Blue1Brown video, not a slide deck. The picture *is* the
explanation: a formula is written term by term as each term is said, then
rewrites itself into the next line in place; a finished result slides
aside to make room; a definition is shown working on a real example before
it is named. Nothing is asserted that the picture does not show.

## Contents

- The files beside this one; where things are; shell rules
- Helpers: who does what, and how your CLI starts one
- Making a video: the checklist
- Research: what a research helper reads and writes
- Direct: the storyboard, its template and a worked scene
- Build: what a scene helper gets, writes and returns
- Assemble: narration, the whole-video check, the render
- The story; Taste; Moving maths

## The files beside this one

Both are in `skills/oculus-studio/` under your working directory.

- **`reference.md`**: the authoring rules (video.json, script.md, the
  component format, elements, maths, motion, cues, lint). A scene builder
  reads it in full before writing TSX; the director reads its "video.json"
  and "script.md" sections before writing those files. Afterwards, go to the
  section a lint message or a question points at.
- **`studio.d.ts`**: open it when you write TSX and need a global's exact
  signature. Search it for the name rather than reading it whole. A
  component imports nothing; every global it declares is simply there.

## Where things are

From your working directory (`agents/`):

```
../studio/videos/<slug>/      one video
  storyboard.md               the director writes: the plan, scene by scene
  notes/<nn>-<topic>.md       research helpers write: notes on the source
  video.json                  the director writes: the scenes, in order
  script.md                   the director writes: what is said, per scene
  components/<Name>.tsx       scene builders write: the video's components
  assets/                     images, named by <Image src>
  narration/                  made by narrate
  out/                        made by frame, strip and gallery: PNGs
  renders/                    made by render: draft.mp4, final.mp4
../studio/library/            the global library: read, never edit
../studio/subjects/<CODE>/    a subject's library: read, never edit
```

Every `oculus studio` command takes the video's **slug** where its usage
says `DIR`. File tools need the folder: `oculus studio new` prints it.

## Shell rules

- **One command per call.** No `for`, `while`, `$(…)`, backticks, shell
  variables or `cd`. Write values out.
- `oculus` is on `PATH`. Add `--json` for output you parse; `lint --json`
  prints `[]` when clean.
- **On opencode, `bash` is allow-listed to `oculus …` and `ls …` only.**
  Write files with the write and edit tools, read text and PNGs with the
  read tool, search with the grep tool.

## Helpers

Three roles make a video. You are the **director**: you never read the
whole lecture and you never build every scene yourself.

| Role | Who | Does |
| --- | --- | --- |
| Research | one helper per slice of the source, on the research model | reads its slice, writes one notes file |
| Direct | you, on your own model | writes the storyboard, the script and video.json; assembles; reviews |
| Build | one helper per scene, on the scene model | writes and checks that scene's components |

The models are the student's choice in Settings → Jobs. Find out yours and
how your CLI starts a helper:

```bash
oculus studio agents --agent claude
```

Pass your own CLI: `claude`, `codex`, `opencode` or `antigravity`. For each
role it prints the model and the exact call to make. Use it as printed:

- **Claude Code**: the `Agent` tool with the printed `subagent_type` and
  `run_in_background: false`. Start a batch in one message so they run at
  once; a background helper would end your turn early.
- **Codex**: `spawn_agent` with exactly the printed `model` and
  `reasoning_effort`, `fork_turns: "none"` and the brief as `message`. This
  skill asks you to set the model, so set it. At most 3 at once; collect each
  with `wait_agent`, then `close_agent` it before starting another.
- **opencode**: the `task` tool with the printed `subagent_type`.
- **A role printed as off, or an agent that cannot start helpers** (agy):
  do that role's items yourself, one after another, in the same order and
  to the same standard. If a start is refused, do the same.

A helper starts with nothing but its brief: no memory of this chat, no
lecture, no storyboard. Every brief names the video's slug and folder, the
files to read, the file to write and what to reply. Briefs are below.

## Making a video

Copy this checklist and tick it off as you go:

```
Video progress:
- [ ] 1. Make the folder (oculus studio new)
- [ ] 2. Map the source and start one research helper per slice
- [ ] 3. Read the notes; write storyboard.md
- [ ] 4. Show the student the storyboard and wait for their go-ahead
- [ ] 5. Write video.json and script.md; lint until only `narration` is left; narrate
- [ ] 6. Start one scene builder per scene (in batches)
- [ ] 7. Review each builder's strip; send fixes to a new builder for that scene
- [ ] 8. Lint until []; strip the whole video and look; fix
- [ ] 9. Render the draft and give the student its path
- [ ] 10. Render the final when the student is happy or asked for it
```

**Step 1.** Pick a slug (lowercase letters, digits, `-`) and run:

```bash
oculus studio new shor-breaks-rsa --subject MULT20015 --title "How Shor breaks R S A"
```

It refuses a slug that exists (`oculus studio list` shows them). `--portrait`
only when asked for a vertical video. It writes one silent `TitleCard`
scene; keep or replace it.

**Step 2.** Map the source cheaply: `oculus list -l CODE` for the recording,
then only the slide-change lines of its `outline.md` (the `oculus-lectures`
skill says how), and `oculus files CODE -t pdf` for the deck. Cut it into
slices of about ten minutes or ten slides, each one topic, and start one
research helper per slice ("Research" below has the brief).

**Step 4.** Show the student the storyboard's question, ladder and scene
list (a line each) and the path to the full file. Stop there. Build only
after they say go, folding in what they change. Skip the stop only when the
student said to go straight through.

Steps 3, 5 to 8 are in "Direct", "Build" and "Assemble". Steps 9 and 10:

```bash
oculus studio render shor-breaks-rsa
```

writes a fast 540p `renders/draft.mp4`; tell the student its path.
`--final` writes `renders/final.mp4` at full size.

## Research

**The brief** (the director fills in the brackets):

> You are a research helper for the Oculus Studio video `[slug]`, folder
> `[absolute path]`. Your slice: lecture `[full UUID]` from `[mm:ss]` to
> `[mm:ss]`, and slides `[N-M]` of `[file]` in subject `[CODE]`. Write your
> notes to `[folder]/notes/[nn]-[topic].md` in the format in
> `skills/oculus-studio/SKILL.md`, section "Research". Reply with the path
> and three lines: what the slice teaches, what it assumes, what it leaves
> for later.

**The notes file**, one per slice, always in this shape:

```markdown
# [nn] [topic] (lecture [mm:ss]-[mm:ss], slides [N-M])

## Definitions
- **prime**: a whole number above 1 whose only factors are 1 and itself. (slide 4, 03:12)

## Worked examples
- Factor 15 with gcd: pick a = 7; 7^4 = 2401 ≡ 1 (mod 15), so r = 4;
  gcd(7^2 - 1, 15) = gcd(48, 15) = 3 and gcd(7^2 + 1, 15) = gcd(50, 15) = 5.
  Every number and every step as the lecture gives it. (slide 9, 14:40)

## Figures
- slide 7: a clock face with 15 marks, powers of 7 landing on 7, 4, 13, 1. (page 7)

## Assumes
- modular arithmetic (used at 12:05, never defined in this slice)

## Leaves for later
- how a quantum computer finds r: "next week" (21:30)

## Quotes worth keeping
- 15:02 "the period is the whole secret"
```

Read the slice in full, not a summary of it. Copy numbers exactly; never
fill a gap with your own example and present it as the lecture's. Write
"not in this slice" rather than guess.

## Direct

Read every notes file (not the lecture), then write `storyboard.md` in the
video's folder. It is the whole plan: a builder reads only its scene and
the scenes either side, so each scene must stand on its own page.

**The question.** One sentence the video answers, asked in the first scene
and answered in the last.

**The viewer.** A first-year who missed the lecture. Every term is defined,
on screen and in words, before its first use, with an example before the
name: show seven dots refusing to make a rectangle, then say "prime". The
**ladder** lists every term in the order the video defines it; a scene may
use only terms above it. Basics get their own scenes: if the video uses
gcd, a scene shows what gcd is on two small numbers and another shows how
Euclid's steps find it, step by step.

**Many short scenes.** One idea each, as many as the explanation needs.
There is no length budget: never cut a definition or a step to save time,
and never cram two ideas into one scene to save a scene.

**Each scene** has: its id and component name; what it carries in and hands
on (the elements and their keys, so the morph between scenes is planned,
not hoped for); the idea in a sentence; the picture; a narration draft; and
a **beat list**, one row per sentence or clause, saying exactly what appears
or moves on screen as those words are said.

**Banned**, because each is a slide, not an explanation:

- a chain of cards joined by arrows;
- a heading with lines of text appearing under it;
- a formula that appears whole;
- a scene that shows only text;
- a number on screen that no picture explains.

**The maths rule.** Every spoken step of a derivation is a visible change
to the previous line, cued to its words: terms move to their new places,
what cancels fades, what is new is written in as it is said. A finished
result moves aside, smaller, and stays on screen while it is used.

**When the source stops short**, say so in the narration ("the lecture
leaves how the period is found for next week") rather than invent the step
or skip it silently.

### The storyboard template

````markdown
# How Shor breaks R S A

Question: How can finding a repeating pattern break the lock on the internet?
Viewer: a first-year who missed lecture 11.
Notes: notes/01-rsa.md, notes/02-period.md, notes/03-shor.md
Not in the source: how the quantum computer finds the period (lecture 12).

## Ladder
1. factor: a number that divides another exactly (scene `factors`)
2. prime: only the factors 1 and itself (scene `prime`)
3. gcd: the largest factor two numbers share (scene `gcd`)

## Scenes

### prime (component PrimeDots)
Carries in: twelve dots d0-d11 in a 3 by 4 rectangle, from `factors`.
Hands on: the row of seven (keys d0-d6, label `seven`), shrunk to the top left, to `gcd`.
Idea: a prime's dots make no rectangle except one row.
Picture: a dot grid that regroups into rows as each grouping is said.
Narration: Some numbers refuse to make a rectangle. Take seven dots. Two rows
leave one dot over, and three rows leave one over too. Only one row of seven
works, so seven's only factors are one and itself, and that is what prime
means. Twelve is different: three rows of four.

| Said | On screen |
| --- | --- |
| "refuse to make a rectangle" | the 3 by 4 rectangle stays; dots d7-d11 fade out |
| "Take seven dots." | d0-d6 glide into one centred row |
| "Two rows leave one dot over" | the row regroups into rows of 3 and 3; d6 drops below, turns `negative` |
| "three rows leave one over too" | regroups into 2, 2, 2; d6 again alone |
| "Only one row of seven works" | back to one row; d6 back to `text` colour |
| "factors are one and itself" | under the row, Math `eq7` `7 = 1 \times 7` is written on with `Write`, a `\k` group per term on its words |
| "that is what prime means" | `\text{prime}` in `\color{accent}`, a group of the same Math, writes on beside the 7 |
| "Twelve is different" | a `Steps` state: the row, its label and `eq7` shrink to the top left (keys `row7`, `seven`, `eq7`); twelve dots enter at centre |
| "three rows of four" | the twelve dots fall into 3 rows of 4 |
````

## Build

Start builders in batches (Claude: one message; Codex: up to 3). Tell each
builder which scenes share keys with its own, and which keys.

**The brief:**

> You are building scene `[id]` (scene [index from 0] of [count]) of the
> Oculus Studio video `[slug]`, folder `[absolute path]`. Read
> `skills/oculus-studio/SKILL.md` section "Build", then `reference.md` in
> full, then `storyboard.md` (scene `[id]`, and `[previous id]` and
> `[next id]`) and `[notes files]`. Write `components/[Name].tsx`.
> Keys in from `[previous id]`: `[keys]`. Keys out to `[next id]`: `[keys]`.
> The narration is made; `oculus studio strip [slug] --scene [index]`
> draws your scene. Reply as "Build" says.

**What a builder does:**

1. Writes the scene's components in `components/`. Write your own freely:
   the library's components are primitives to compose (a title card, a
   number), never templates to fill. A component made for one scene
   declares one prop, `narrated: bool().default(false)`: `video.json` sets
   it `true` on a narrated scene, so the beats cue on `word()`, and without
   it they fall back to fixed times (every recipe in `reference.md`
   "Moving maths" shows the pattern).
2. Cues every beat with `word("…")` on the words the beat list names, in
   the scene's `script.md` section as written. The first element is on
   screen at a fixed time under half a second, not at a late cue, so no
   scene opens on an empty frame.
3. Builds maths term by term as `Math`, never Text ("Moving maths" below,
   `reference.md` "Maths" and "Moving maths").
4. Draws and looks, until every beat reads:

   ```bash
   oculus studio strip shor-breaks-rsa --scene 3 --n 16 --cols 4
   ```

   ```bash
   oculus studio frame shor-breaks-rsa --scene 3 --at 4.5 --scale 0.5
   ```

   Open each PNG with your image-reading tool. Check every beat lands on
   its words, nothing overlaps or overflows, text is readable, and the
   shared keys sit where the neighbours expect them. `lint` is the director's:
   it covers the whole video, where other builders' scenes are still being
   written. If `strip` says some scene's narration is stale, tell the
   director; do not narrate.
5. Writes only its own components, named after its scene (`PrimeDots`),
   and asks the director for anything else. A component two scenes share
   is written by the builder the director names, with at least two
   `examples`, and checked with `oculus studio gallery <slug> <Name>
   --sheet` (no narration needed; `reference.md` "Looking").

**The reply**, exactly:

```
Scene: prime (3)
Components: PrimeDots (new)
Props: {"narrated": true} (as in video.json)
Strip: /…/out/strip-12.4-31.0.png
Sheet: none (no shared component)
Keys in: d0-d11 (as planned). Keys out: row7, seven
Narration changes: none
```

A narration change is the sentence as it is and as it should be, with why
("'refuse' has no beat; say 'won't make'"). The director edits
`script.md`, narrates again, and tells the builder to re-cue.

## Assemble

**Before building** (step 5): write `video.json` (one scene per storyboard
scene: its `id`, its component name, and `"props": { "narrated": true }`
on a narrated scene or `{}` and a `hold` on a silent one) and `script.md` (each scene's narration, in spoken form; see
`reference.md` "script.md"). Then:

```bash
oculus studio lint shor-breaks-rsa --json
```

Before the components exist, lint reports `component_not_found` for each
scene and `narration` for each narrated one; fix every other kind, above
all `script`. Then narrate:

```bash
oculus studio narrate shor-breaks-rsa
```

It reads Settings → Voice; pass no `--server`, `--model` or `--voice`
unless the student asks. If it fails for want of a server, model, voice or
the word-timing model, **stop and tell the student** what to set in
Settings → Voice. Never run `oculus studio aligner download` (378 MB, their
call). If the server does not answer, say so; do not retry in a loop.

**After building** (steps 7 and 8): read each builder's strip as it lands
and send fixes back: a new scene builder with the same brief plus the fix
list and the last reply, since the first one is gone. Then lint until `[]` and strip the whole video, about one frame
every four seconds:

```bash
oculus studio strip shor-breaks-rsa --n 32 --cols 4
```

Check the joins: each shared key moves from one scene into the next
instead of fading; the scene after a magic move has no `transition`.

## Keeping a component

**Never edit a library file directly.** A component or theme reaches a
library only through `oculus studio components promote` or `oculus studio
themes promote`, which keep the old text in `.history/`. Promote only when
the student wants it kept for other videos:

```bash
oculus studio components promote shor-breaks-rsa PrimeDots --to subject
```

## The story

The student watches to understand, not to be told.

- **Explain, don't assert.** Every claim gets its reason, shown. Not "gcd
  is fast", but why: each two steps of Euclid's method at least halve the
  larger number, so even a 600-digit pair takes a few thousand steps.
- **Link each scene to the last.** Open each scene's narration with a
  clause that picks up what was just shown ("That leftover dot is the whole
  idea: …").
- **Make every number mean something.** "A 768-bit key" says nothing; "a
  768-bit key, a number about two hundred and thirty digits long" does.
- **Example, then the rule.** After a worked example, say the general rule
  in one sentence and show it, so the example is not a trick.
- **Spell out the payoff.** When a step "breaks" something, show how, in
  one concrete line.
- **End on the opening question**, naming the steps that got there.
- **Say it the way the voice will run it.** Speech is one stream, so words
  fuse: "R S A rests" is heard as "RS arrests". Read each sentence as
  sounds and rephrase a fusion. A letter said alone ("q") may be timed as a
  word ("cue"), so a beat cued on it can miss; cue the word beside it
  ("the prime q": `word("prime")`). `reference.md` ("script.md") has the
  spoken-form table and the acronym rule.

## Taste

- **The picture carries the idea; words on screen are labels.** Never put
  the script on screen.
- **Fill the frame.** The scene's main element is large and centred.
- **Cue every motion to its words.** An element that illustrates a phrase
  enters or moves as it is said.
- **Keep magic-move keys stable.** An element that carries across scenes
  keeps its key in both, so it moves instead of fading.
- **No colour, font or spacing literals.** Read them from `theme`; "The
  theme" in `reference.md` lists the token names.
- **Look before you report.** Never hand over a render you have not seen
  as frames.

## Moving maths

Every formula, number in a formula and variable on screen is a
`<Math tex="…" key="eq">`: LaTeX typeset as one keyed glyph per piece, so
it can be written on and rewritten term by term. It moves as it is said:

| The narration says | On screen | Built with |
| --- | --- | --- |
| a formula, term by term | each term is written on as its words are said | `<Write at={{ lhs: word("…"), … }}>` round the Math, one `\k{name}{…}` group per spoken term |
| the next step | the line rewrites itself in place: shared terms glide, what cancels fades, what is new arrives beside its neighbours | `<Steps at={[word("…"), …]} lag={0.1}>`, one state per line, the same Math `key` in every state, `\k` names on the terms that must travel |
| "this term", "the period" | the term is pointed at as it is named | `Brace` with a label, `Surround`, `Underline`, `Indicate`, `Arrow from to`, each on `target="eq/name"` |
| "keep that", then the next idea | the finished result shrinks into a corner and stays there while it is used | a later Steps state: the same Math key, smaller, `position: "absolute"`, no `lag` |
| a value put into a formula | the number flies into the symbol's slot; the symbol fades | one Math holding the formula and the given, the number `\k{val}{…}` in both states |
| the next scene uses it | it carries over term by term | the same Math `key` in the next scene's tree, and no `transition` on that scene |

`reference.md` "Maths" has the element (props, backslashes, `\k` keys,
theme colours, size) and "Moving maths" a working component for each row;
start a scene from the closest one.

**Never:**

- a formula that appears whole: every Math is inside a `Write` cued to its
  words, or arrives as a Steps state on its cue;
- lines stacking under each other: a derivation is one place, rewritten;
- text pretending to be maths: `2^{4}` or `7 = 1 × 7` as Text, a power
  as a raised Text run. Words that belong in a formula are `\text{…}`
  inside its Math.
