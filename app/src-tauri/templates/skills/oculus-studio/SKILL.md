---
name: oculus-studio
description: Make a short video in an Oculus library — an explainer, an animation, a narrated summary of a lecture, a recap of a topic — as scenes written in TSX, narrated, checked frame by frame and rendered to an mp4. Use for "make a video of lecture X", "a 60 second explainer of Y", "animate this idea", "narrate a summary of week 4", or any request whose result is a video.
---

# Videos

A video is a **folder** that `oculus studio` reads: `video.json` lists the
scenes, `components/` holds the TSX that draws them, and `script.md` holds
what is said. You write those files; the CLI does everything else — it
narrates, checks, draws frames and renders the mp4. Every frame is a pure
function of the folder and the time, so the same call always draws the
same pixels.

Two files sit beside this one, in `skills/oculus-studio/` under your
working directory. Open them when you write, not before:

- `reference.md` — every field, prop, helper and lint message, and what to
  do about each.
- `studio.d.ts` — the globals a component can use, typed. A component
  imports nothing; these are simply there.

## Where things are

From your working directory (`agents/`):

```
../studio/videos/<slug>/      one video; you write here
  video.json  script.md  components/<Name>.tsx  assets/
  out/                        PNGs from frame, strip and gallery
  renders/                    draft.mp4, final.mp4
../studio/library/            the global library: read, never edit
../studio/subjects/<CODE>/    a subject's library: read, never edit
```

Every `oculus studio` command takes the video's **slug** where it says
`DIR`, so you never type the folder. Your file tools need the folder:
`oculus studio new` prints its absolute path.

**Never edit a library file directly.** A component or theme reaches a
library only through `oculus studio components promote` or `oculus studio
themes promote`, which keep the old text in `.history/`.

## The loop

One command per call, every time (see "Shell rules" below).

**1. Read the lecture.** Use the `oculus-lectures` skill: resolve the
recording with `oculus list -l CODE`, then read `outline.md` by span. For
the slides, `oculus files CODE -t pdf`, then `oculus read <file> --pages
N-M -s CODE`. Pick the one idea the video is about before you write anything.

**2. Make the folder.**

```bash
oculus studio new gradient-descent --subject COMP30027 --title "Gradient descent"
```

The slug is lowercase letters, digits and `-`. `--subject` takes the
subject code you pass to `-s`. It refuses a slug that exists: pick another,
or `oculus studio list --subject COMP30027` to see what is there. Add
`--portrait` only when asked for a vertical video.

**3. Plan, then write.** For about 60 seconds: 5 to 8 scenes and about
150 spoken words in all (speech runs at about 2.5 words a second, and each
narrated scene adds about a second of quiet around its clip).

- See what you can use: `oculus studio components list gradient-descent`,
  then `oculus studio components show gradient-descent Steps` for one's
  props and examples. The starter set is in `reference.md`.
- Write `video.json`: one scene per idea, each with an `id`, a
  `component` and its `props`. `new` wrote one silent `TitleCard` scene
  (`id` `title`, `hold` 4); keep it or replace it.
- Write `script.md`: one `## <id>` section per narrated scene, in spoken
  form (numbers and symbols as words). A scene with no section is silent
  and needs a `hold` in seconds.
- A scene no existing component fits: write one in the video's
  `components/` (step 8).

**4. Lint.**

```bash
oculus studio lint gradient-descent --json
```

Before `narrate`, lint always fails: each narrated scene reports kind
`narration` ("no narration for its current text … run `oculus studio
narrate`"), and the command exits 1 with `{"error": "N problems in …"}`.
That is expected. Fix every other kind, above all `script` (each is a word
the voice would say wrong), then narrate. `[]` is required only after
narrating. `reference.md` says what to do about each kind.

**5. Narrate.**

```bash
oculus studio narrate gradient-descent
```

It reads Settings → Voice, so pass no `--server`, `--model` or `--voice`
unless the student asks for another voice. If it fails because no server,
model or voice is set, or the aligner model is not downloaded, **stop and
tell the student** what to set in Settings → Voice (the server, model and
voice, or the word-timing model's Download button). Do not run `oculus
studio aligner download` yourself: it is 378 MB and the student's call. If
the server does not answer, say so; do not retry in a loop.

Per scene it prints the clip's length and word count, and warns about
words the aligner was unsure of and speech the script lacks. Re-run it after
editing `script.md`: only changed scenes are made again.

**6. Lint again, then look.** Lint now renders the narrated scenes and
checks their word cues. When it prints `[]`, look at the result. The
default strip is 12 frames, one every 5 s of a 60 s video; take 16:

```bash
oculus studio strip gradient-descent --n 16 --cols 4
```

```bash
oculus studio frame gradient-descent --t 12.5 --scale 0.5
```

Each prints the PNG's path. **Open that path with your file-reading tool**
(the one that shows you images) and look: is the text readable, does
anything overflow or overlap, does each scene say one thing? `strip
--scene N` covers one scene (N counts from 0). Fix, lint, look again.

**7. Render.**

```bash
oculus studio render gradient-descent
```

A fast 540p draft at `renders/draft.mp4`. Tell the student its path. When
they are happy, or asked for it, render the full-size final:

```bash
oculus studio render gradient-descent --final
```

**8. A new or tuned component** goes in the video's own
`components/<Name>.tsx`, where it wins over a library one of the same
name. Give it at least two `examples` and check them:

```bash
oculus studio gallery gradient-descent Takeaway --sheet
```

Open the sheet's PNG and look. Copy it into a library only when the student
wants to keep it for other videos:

```bash
oculus studio components promote gradient-descent Takeaway --to subject
```

## Taste

- **One idea per scene.** A scene that needs two headings is two scenes.
- **Words on screen summarise; the narration explains.** Never put the
  script on screen. A heading and three short points at most.
- **Fill the frame.** The scene's main element is large and centred in the
  space below the heading, never parked in the top third.
- **The key number or phrase is display size** (`theme.type.display`).
- **Something meaningful is on screen within a second** of a scene
  starting. Do not open on an empty frame or a slowly drawing line.
- **Cue motion to words.** An element that illustrates a phrase enters as
  it is said: `at={word("gradient")}`.
- **Keep magic-move keys stable.** An element that carries across two
  scenes keeps the same `key` in both, so it moves instead of fading. When
  consecutive scenes each have a heading, give every heading the same key
  (`key="heading"`), or the two headings cross-fade on top of each other.
- **No colour, font or spacing literals.** Read them from `theme`, and
  cite only token names that exist: "The theme" in `reference.md` and
  `studio.d.ts` list them.
- **Look before you report.** Never hand over a render you have not seen
  as frames.

## Shell rules

- **One command per call.** No `for`, `while`, `$(…)`, backticks, shell
  variables or `cd`. Write values out: the subject code and the slug are
  short.
- `oculus` is on `PATH`. Call it by name.
- Add `--json` to a command for output you parse; lint with `--json`
  prints `[]` when clean.
- **On opencode, `bash` is allow-listed to `oculus …` and `ls …` only.**
  Write files with the write and edit tools, and open PNGs with the read
  tool, by the path the command printed.
