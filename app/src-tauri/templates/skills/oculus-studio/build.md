# Building a scene

How a scene helper builds, fixes or reviews one scene of a video. The
director's brief says which, and names the video's slug and the scene.
Section names in quotes ("Moving maths", "Lint") are `reference.md`'s.

## Contents

- Start: `oculus studio brief`
- Write: the component, its cues, its maths, a page crop
- Check as text: `lint` and `beats`
- Look: one strip or frame, read in the same message
- The bar: what every scene must be
- What is yours
- The reply
- Review: checking another builder's scene

## Start

Your first call is `oculus studio brief <slug> --scene <id>`. It prints your whole input: the scene's index, component and props; what
the scenes either side carry in and hand on, and whether their components
exist yet; your storyboard entry with its beat table; the notes files it
names; every narration word at the time `word()` returns for it; and the
exact `lint`, `beats`, `strip`, `frame` and `gallery` calls for the scene.
It replaces `storyboard.md`, `script.md` and the narration manifest: do not
read them. Read a notes file only when the brief names it and the scene
needs a fact from it (a number, a worked step). If the brief prints a
narration error instead of word times, stop and reply with it.

A fix brief adds a fix list and the last builder's reply: the component
is written, so make the fixes, then check and look as below.

## Write

1. Write the scene's components in `components/`. Write your own freely:
   the library's components are primitives to compose (a title card, a
   number), never templates to fill. A component made for one scene
   declares one prop, `narrated: bool().default(false)`: `video.json` sets
   it `true` on a narrated scene, so the beats cue on `word()`, and without
   it they fall back to fixed times (every recipe in "Moving maths" shows
   the pattern).
2. Move maths with the mechanisms in "Moving maths" (`Write`, `Steps`,
   the overlays); its recipes are worked examples of them, not layouts to
   copy. The picture is yours to draw for this scene's idea.
3. Cue every beat with `word("…")` on the words the beat table names,
   spelled as the brief prints them. The first element is on screen at a
   fixed time under half a second, so the scene never opens on an empty
   frame.
4. A pause (`4.10 [pause 1.0 s]` in the brief) is where an idea lands.
   Time the landing visual (the rewrite's last state, the highlight, the
   value in its slot) to settle just before `pause()`, then start no new
   motion until the next word: the frame holds through the silence.
5. Keys in and out are the ones the brief's neighbours name, so the magic
   move carries them.
6. A storyboard entry that names a page figure (file, page, block or
   `--find` text) is a `Slide`: make the crop with `oculus studio slide`
   as `reference.md` ("Slides") says, unless `assets/slides/` has it, then
   look at it before cueing its zoom and highlights.

`skills/oculus-studio/studio.d.ts` has every global's exact signature:
search it for the name rather than reading it whole.

## Check as text

Text first, pictures after. Run the brief's two calls:

- `lint … --scene <N>`: fix every error. Fix each warning, or say in your
  reply why the scene wants it. A note (an unsure or guessed word) is the
  director's.
- `beats … --scene <N>`: what changes on screen, when, and the words said
  then. Check the beats you just changed against the beat table: find
  their keys' lines. It runs to 80 lines, so do not read it end to end
  every time.

Edit until lint has no errors and the beats land on their words.

## Look

Run the brief's `strip` call (the motion: what moves, in which order,
where it settles) or its `frame` call with `--at` a beat's word time (one
moment, larger). Both write to the fixed `-o` path the brief prints. On
Claude Code, **Read that PNG in the same message as the command**, the
command first: it runs a message's calls in order. Elsewhere, Read it in
the next call.

Look rarely and on purpose: make every fix you can from lint and beats
between two looks. A strip's `--n` and `--scale` can go lower while it
still reads. If `strip` says a scene's narration is stale, tell the
director; never narrate.

## The bar

**Banned**, because each is a slide, not an explanation: a chain of cards
joined by arrows; a heading with lines of text appearing under it; a
formula that appears whole; a scene that shows only text; a number on
screen that no picture explains.

**Maths.** Every formula, number in a formula and variable is a `Math`,
never Text. Every Math is inside a `Write` cued to its words, or arrives
as a `Steps` state on its cue. A derivation is one place, rewritten, never
lines stacking under each other. A finished result moves aside, smaller,
and stays while it is used. Words in a formula are `\text{…}` inside it.

**Taste.** The picture carries the idea; words on screen are labels, never
the script. The main element is large and centred, and the frame is
filled. Every motion is cued to its words. With `"captions": true` in
video.json the captions take the bottom sixth of the frame (a plate 5% of
the height above the edge, one or two lines): keep key content above it,
and check with `frame`, which draws them. No colour, font or spacing
literals: read them from `theme` ("The theme").

## What is yours

Write only your own components, named after your scene (`PrimeDots`), and
ask the director for anything else. A component two scenes share is
written by the builder the director names, with at least two `examples`,
and checked with the brief's `gallery … --sheet` call (no narration
needed; "Looking"). `video.json`, `script.md`, `storyboard.md`, the notes,
other scenes' components and the libraries are not yours to edit.

## The reply

Exactly:

```
Scene: prime (3)
Components: PrimeDots (new)
Props: {"narrated": true} (as in video.json)
Strip: /…/out/prime-strip.png
Sheet: none (no shared component)
Keys in: d0-d11 (as planned). Keys out: row7, seven
Lint: no errors; warning still 12.4-15.1 kept: the pause before "prime" is the point
Narration changes: none
```

A narration change is the sentence as it is and as it should be, with why
("'refuse' has no beat; say 'won't make'"). The director edits
`script.md`, narrates again, and has the scene re-cued.

## Review

A review brief names a finished scene and its builder's reply. Edit no
file.

1. Run `oculus studio brief` for the scene, then its `lint` and `beats`
   calls.
2. Run its `strip` once, then at most three `frame`s at the moments the
   strip leaves in doubt, each Read the way Look says.
3. Check: every row of the beat table lands on its words; nothing new
   moves during a `pause` line of `beats`; the frame is
   filled and centred; nothing breaks the bar above; the keys out sit where
   the next scene's entry expects them; every lint warning is fixed or
   explained in the builder's reply.

Reply `Scene: <id> (<N>)`, then `PASS`, or a numbered fix list of at most
ten, each concrete: what, when (seconds or words) and where (key or place
in the frame):

```
Scene: squares (13)
1. 23.1 s, "Take one dot away": d15 fades but never turns `negative`; recolour it on "Take".
2. 0.4-4.5 s: the clock sits in the left half; centre it and draw it at h1 size.
```
