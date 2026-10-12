---
name: oculus-studio
description: Makes a short video in an Oculus library (an explainer, an animation, a narrated summary of a lecture, a recap of a topic) by writing TSX scenes and a spoken script, then narrating, checking frame by frame and rendering an mp4 with `oculus studio`. Use for "make a video of lecture X", "a 60 second explainer of Y", "animate this idea", "narrate a summary of week 4", or any request whose result is a video.
---

# Videos

A video is a **folder** that `oculus studio` reads: `video.json` lists the
scenes, `components/` holds the TSX that draws them, and `script.md` holds
what is said. You write those files; the CLI narrates, checks, draws frames
and renders the mp4.

## The two files beside this one

Both are in `skills/oculus-studio/` under your working directory. Do not
open them before you start writing.

- **`reference.md`**: read it in full at step 3, before you write the first
  file of a video. Afterwards, go back to the section a lint message or a
  question points at; its Contents list names every section.
- **`studio.d.ts`**: open it when you write or fix a component's TSX and
  need a global's exact signature (an element's props, a style key, a
  helper's arguments). Search it for the name rather than reading it whole.
  A component imports nothing; every global it declares is simply there.

## Where things are

From your working directory (`agents/`):

```
../studio/videos/<slug>/      one video
  video.json                  you write: the scenes, in order
  script.md                   you write: what is said, one section per narrated scene
  components/<Name>.tsx       you write: the video's own components
  assets/                     you write: images, named by <Image src>
  narration/                  made by narrate: manifest.json and the clips
  out/                        made by frame, strip and gallery: PNGs
  renders/                    made by render: draft.mp4, final.mp4
  .cache/                     render segments; never edit
../studio/library/            the global library: read, never edit
../studio/subjects/<CODE>/    a subject's library: read, never edit
../studio/subjects/<CODE>/lexicon.json   the one library file you edit: pronunciations (step 6)
```

Every `oculus studio` command takes the video's **slug** where its usage says
`DIR`. Your file tools need the folder: `oculus studio new` prints its
absolute path.

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

## Making a video

Copy this checklist and tick it off as you go:

```
Video progress:
- [ ] 1. Read the lecture and pick the one question the video answers
- [ ] 2. Make the folder (oculus studio new)
- [ ] 3. Read reference.md, plan the argument, write video.json, script.md and any component
- [ ] 4. Check each new component (oculus studio gallery --sheet), look, fix
- [ ] 5. Lint until only `narration` problems are left
- [ ] 6. Narrate
- [ ] 7. Lint until [], then look at a strip and frames; fix and repeat
- [ ] 8. Render the draft and give the student its path
- [ ] 9. Render the final when the student is happy or asked for it
```

**Step 1: Read the lecture.** Use the `oculus-lectures` skill: resolve the
recording with `oculus list -l CODE`, then read `outline.md` by span. For
the slides, run `oculus files CODE -t pdf`, then `oculus read <file> --pages
N-M -s CODE`. Pick the one idea the video is about before you write
anything.

**Step 2: Make the folder.** Run:

```bash
oculus studio new gradient-descent --subject COMP30027 --title "Gradient descent"
```

The slug is lowercase letters, digits and `-`. `--subject` takes the
subject code you pass to `-s`. It refuses a slug that exists: pick another,
or run `oculus studio list --subject COMP30027` to see what is there. Add
`--portrait` only when asked for a vertical video. `new` writes one silent
`TitleCard` scene (`id` `title`, `hold` 4); keep it or replace it.

**Step 3: Plan, then write.** Read `reference.md`. Then write the argument
first, as plain sentences in your head or your reply: the question the
video answers, each step with its *reason*, and how the end answers the
question. Then cut it into scenes by "The story" below.

For about 60 seconds: 5 to 8 scenes and about 150 spoken words in all
(speech runs at about 2.5 words a second, and each narrated scene adds
about a second of quiet around its clip). An explained step takes 15 to 20
seconds; if the idea needs more steps than the length allows, cover less
ground, or tell the student how long it needs, rather than rushing.

- See what you can use: run `oculus studio components list
  gradient-descent`, then `oculus studio components show gradient-descent
  Steps` for one's props and examples. The starter set is at the end of
  `reference.md`.
- `video.json`: one scene per idea, each with an `id`, a `component` and
  its `props`.
- `script.md`: one `## <id>` section per narrated scene, in spoken form
  (numbers and symbols as words). A scene with no section is silent and
  needs a `hold` in seconds.
- A scene no existing component fits: write one in the video's
  `components/<Name>.tsx`, where it wins over a library one of the same
  name. Give it at least two `examples`.

**Step 4: Check each new component.** Skip this when you wrote none. Run:

```bash
oculus studio gallery gradient-descent Takeaway --sheet
```

Open the sheet's PNG and look, as in step 7. Fix the component and run it
again until every example reads.

**Step 5: Lint.** Run:

```bash
oculus studio lint gradient-descent --json
```

Before `narrate`, lint always fails: each narrated scene reports kind
`narration` ("no narration for its current text … run `oculus studio
narrate`"), and the command exits 1 with `{"error": "N problems in …"}`.
That is expected. Fix every other kind, above all `script` (each is a word
the voice would say wrong), and lint again until only `narration` is left.
`reference.md` ("Lint") says what to do about each kind.

**Step 6: Narrate.** Run:

```bash
oculus studio narrate gradient-descent
```

It reads Settings → Voice, so pass no `--engine`, `--server`, `--model` or
`--voice` unless the student asks for another voice. Per scene it prints the clip's
length and word count, and warns about words the aligner was unsure of and
speech the script lacks. Run it again after editing `script.md`: only
changed scenes are made again.

If it fails because no server, model or voice is set, or the aligner model
is not downloaded, **stop and tell the student** what to set in Settings →
Voice (the server, model and voice, or the word-timing model's Download
button). Do not run `oculus studio aligner download` yourself: it is 378 MB
and the student's call. If the server does not answer, say so; do not retry
in a loop.

If it says Kokoro's voice pack or G2P pack is not downloaded, **stop and
tell the student** the same way: `oculus studio voices download` fetches
both (103 MB), and that is their call too.

**Check the guessed words.** With the Kokoro engine, narrate ends with a
`note: … check pronunciation: "Nesterov" is guessed as /nˈɛstəɹɔv/` line
for each word its dictionary lacks (lint prints the same notes). A note
does not fail anything, but the guess may be wrong. For each one:

1. Run `oculus studio g2p "Nesterov" --video <slug>` to see the reading,
   and listen to that scene's clip if you can play audio; otherwise judge
   from the phonemes and tell the student which words were guessed.
2. If it is wrong, add an entry to the subject's
   `../studio/subjects/<CODE>/lexicon.json` (a JSON object; create it
   when missing). Prefer a respelling in dictionary words, `"Nesterov":
   "NESS-ter-off"`: each `-` or space part is a dictionary word, and an
   all-caps part takes the stress. Use raw phonemes in slashes,
   `"Nesterov": "/nɛstˈɛɹɔf/"`, only when no respelling works.
   `reference.md` ("Pronunciation") has the rules.
3. Run `oculus studio g2p` again until the word reads `lexicon` with the
   reading you want, then `narrate` again: only scenes whose reading
   changed are made again.

A lint problem naming `lexicon.json` is an entry you wrote that does not
parse: fix it before narrating.

**Step 7: Lint again, then look.** Lint now renders the narrated scenes and
checks their word cues. Fix and lint until it prints `[]`. Then run a
strip (the default is 12 frames, one every 5 s of a 60 s video; take 16):

```bash
oculus studio strip gradient-descent --n 16 --cols 4
```

and a frame for a closer look at one moment:

```bash
oculus studio frame gradient-descent --t 12.5 --scale 0.5
```

Each prints the PNG's path. **Open that path with your file-reading tool**
(the one that shows you images) and check:

- Is the text readable?
- Does anything overflow or overlap?
- Does each scene say one thing, and keep the taste rules below?

`strip --scene N` covers one scene (N counts from 0). Fix, lint, look
again, until nothing is wrong.

**Step 8: Render the draft.** Run:

```bash
oculus studio render gradient-descent
```

It writes a fast 540p draft to `renders/draft.mp4`. Tell the student its
path.

**Step 9: Render the final** when the student is happy with the draft, or
asked for it:

```bash
oculus studio render gradient-descent --final
```

## Keeping a component

**Never edit a library file directly.** A component or theme reaches a
library only through `oculus studio components promote` or `oculus studio
themes promote`, which keep the old text in `.history/`. Promote only when
the student wants to keep it for other videos:

```bash
oculus studio components promote gradient-descent Takeaway --to subject
```

## The story

The student watches to understand, not to be told. Each of these is a
sentence or a clause, not a recap: do it lightly, but do it.

- **Explain, don't assert.** Every claim gets its reason. Not "factoring
  is hard", but why: the only general way is to try divisors, and their
  number grows with the size of the number. A claim you cannot back in one
  sentence does not belong in the video.
- **Link each scene to the last.** Open each scene's narration with a
  clause that picks up what was just shown ("That gap is the whole lock:
  …"), so a new number or picture lands as the next step of the same
  argument, not a new topic.
- **Make every number mean something** for the argument. "A 768-bit key"
  says nothing; "a 768-bit key, a number about two hundred and thirty
  digits long, was factored in two thousand nine" does.
- **Define a term the first time**, in passing: "nation states, that is,
  governments with whole computing centres".
- **Example, then the rule.** After a worked example, say the general rule
  in one sentence and put it on screen, so the example is not left as a
  trick that happened to work.
- **Spell out the payoff.** When a step "gives away" or "breaks"
  something, show how, in one concrete line.
- **End on the opening question.** The last scene answers what the first
  scene raised, naming the steps that got there in a clause each.
- **Say it the way the voice will run it.** Speech is one stream, so words
  fuse across their gaps: "R S A rests" is heard as "RS arrests". Read
  each sentence as sounds and rephrase a fusion ("R S A depends on…").
  `reference.md` ("script.md") has the spoken-form table and the acronym
  rule.

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
