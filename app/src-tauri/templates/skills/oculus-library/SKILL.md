---
name: oculus-library
description: Finds and reads coursework in an Oculus course library — searches slides and readings by meaning or by pattern, lists what a subject has, and prints a page. Use for "find where it says X", "which slide covers Y", "read page 12", "open the assignment brief", "what files are in this subject", or any question answered from a PDF, a Canvas page, an announcement or an Ed thread.
---

# Finding and reading

`oculus grep` and `oculus search` find, `oculus files` lists and `oculus
read` prints. **Choosing between `grep` and `search` is the whole skill** —
each covers a different half of the library, and a miss looks exactly like
an absence.

| What you are looking in | `oculus grep` | `oculus search` |
| --- | --- | --- |
| PDF and Office page text | yes, exactly | yes, by meaning |
| Spreadsheets (one page per sheet) | yes | **no** |
| Canvas pages, announcements, assignments, Ed threads | yes | **no** |
| Lecture transcripts and outlines | **no** | **no** |

Two consequences worth holding on to:

- **`search` only ranks slide decks and readings.** Ask it about an
  announcement or an Ed thread and it will rank nothing, however plainly the
  words are there.
- **Neither reaches a recording.** Lectures are files on disk that no index
  covers — that is the `oculus-lectures` skill, not this one.

And a third, for when a plain `rg` looks tempting: **per-page PDF text is not
on disk.** It lives in the database, and it is where most of the actual
teaching is. `rg` over `courses/` reads the markdown and silently misses
every slide deck's page text.

## The two passes

`oculus grep` is a **locator, not a reader**. Its snippets are truncated and
it has no context flag on purpose — what it gives you is an *address*:

```
courses/COMP30022_2026_SM2/ed/0068-workshop-swap.md:L200: Groundzero Universe…
courses/COMP30022_2026_SM2/files/SE_Project_Idea.pdf:p1: Design the experience…
```

`:L200` for markdown, **`:p1` for a PDF** — and that page number is the one
`oculus read -p` and the app's viewer use, so a hit opens directly. Nothing
else in the library can hand you that, because the page text is not in a file.

So the shape is always locate, then read:

- a `.pdf` hit → `oculus read <file> -p <page>`
- a `.md` hit → the file is right there on disk. Read it, or use native `rg`
  on **that path** if you want context lines. `rg` is the better tool once you
  know which file you are in; it just cannot find the file for you.

## `oculus grep` — exact, offline, free

```bash
oculus grep "sprint retrospective" -s INFO30006
```

It needs no network and no key, so it is the door that always works and the
fallback whenever `search` cannot run.

- **The pattern is a regular expression, and it is case-insensitive already.**
  There is **no `-i`** — asking for the default is an error. Pass
  `--case-sensitive` to match case exactly.
- There is **no `-C`/context flag**, by design — see the two passes above.
- It scans **subject by subject, then by path, and stops at the limit**, so a
  truncated result is biased rather than sampled: a broad pattern at `-n 6`
  returns six hits from the first subject and silence from the rest. It does
  say `stopped at N matches` when this happens — treat that as "narrow the
  pattern or add `-s`", not as "that is all there is".
- **`-c` narrows to one kind of thing before the limit bites** — `-c ed`,
  `-c announcement`, `-c assignment`, `-c file` (the slide decks and
  downloads), `-c page`, `-c module`, `-c quiz`, `-c syllabus`, `-c home`,
  `-c image`, `-c upload` (what the student added themselves). It is
  repeatable: `-c ed -c announcement`. This is the fix for the ordering bias
  above. A word that is **not a category is refused, naming the real ones**, so
  a typo never comes back as "not in the library"; a real category this
  subject happens not to have returns **no matches**, which is the honest
  answer. `oculus files` takes the same flag with the same spelling.
- `-F` treats the pattern as literal text — reach for it when the query has
  `.`, `(`, `*` or `?` in it.
- `-l` prints matching paths only; `-n` caps the hits (default 40).
- `-s` is **repeatable**: `-s INFO30006 -s COMP30026`.

## `oculus search` — by meaning, and it can fail

```bash
oculus search "constrained optimisation" -s MULT20015 -n 8
```

Finds a slide about Lagrange multipliers when asked for "constrained
optimisation", because the query is embedded by the same vision model that
embedded the page images.

- It **costs a network round trip and a key**, and it is the one read command
  that spends anything.
- **It fails loudly rather than returning nothing.** An empty index, or one
  built by a retired embedding model, exits non-zero and names
  `oculus index`. Do not read that as "the library has no answer" — it means
  try `oculus grep`, which searches the same text with no model at all.
- `-s` takes one subject code here, not a repeated list. `--full` prints each
  hit's whole page instead of a snippet.

## `oculus files` — what is there

```bash
oculus files INFO30006 --type pdf
```

```bash
oculus files INFO30006 -m assignment
```

Use it to *locate* before reading, when a filename is half-remembered.
`-c/--category` is the same flag `oculus grep` takes, spelled the same way and
repeatable: `file`, `page`, `announcement`, `ed`, `module`, `assignment`,
`quiz`, `syllabus`, `home`, `image`, `upload`, `document`, `canvas`, `other`. `--indexed` keeps only
what retrieval can see.

**The `indexed` column is how many pages are searchable.** A PDF showing none
has never been parsed — a real state, not a bug, and not something to work
around by reading the raw file. It is fixed by `oculus index <SUBJECT_CODE>`,
which takes minutes per file and is the student's call, not a thing to launch
mid-answer.

**A video's words are in `<video>.vtt` beside it**, when there is one. A video
without it has no transcript yet; `oculus transcribe <video>` makes one with
the first engine, in the order the student set in Settings, that can take it:
Groq when they saved a key, which spends their free-tier allowance; Whisper
run locally on a model downloaded in Settings; or the Mac's on-device speech
recognition, which takes about a minute an hour of audio. Run it only when
asked to.

## `oculus read` — the text of one file

```bash
oculus read 13.pdf --pages 30-35
```

Ranges are `12`, `12-15`, `12,14,20-22`, or `30-` for "30 to the end". These
are the **same page numbers** `search` reports and the app's viewer shows, so
a hit can be opened at its page directly.

`FILE` may be a full library path, a bare filename, or any distinctive
fragment. The lookup is **tiered, not fuzzy** — exact path, then exact
filename, then case-insensitive filename, then path substring — and only the
best tier that matched anything is considered. An ambiguous fragment **lists
the candidates rather than guessing**, so read the list and re-run with a
longer fragment or `-s`; do not pick for it.

## A notebook — the student's handwritten working

The student writes assignments and derivations by hand in an Oculus
**notebook**: a `.canvas` file under `canvases/`, one or more pages, sometimes
laid over a copy of the assignment. It is not text, so `oculus read` prints
its **page pictures** instead:

```bash
oculus read "Assignment 1 (notebook)" --pages 2
```

Each line is a page number, the path of that page's PNG and how long ago it
was redrawn (a few seconds after each stroke while the notebook is open). **Open
the PNG with your own image tool** and read what is written. Do not open the
`.canvas` file: it is JSON holding every stroke, and an assignment's pages
inside it are megabytes of base64.

- When the student says "my working", "my solution" or "the canvas", this is
  what they mean. A chat message names a notebook that is open, and the page in
  view; if none is named, `oculus files -c canvas` lists them.
- Give feedback on **their** working: say which line is right, which step
  does not follow and why, and what to try. Do not rewrite their page for them
  or paste the finished answer unless asked. If their handwriting is unclear,
  ask rather than guess.
- A picture older than the student's last edit is stale; if the age looks wrong
  for what they say they just wrote, say so rather than commenting on old work.
- A notebook is the student's. You cannot and should not write into `canvases/`.
- `oculus read` says so when a canvas has no pages (a whiteboard) or no
  pictures yet (never opened since it was made); relay that, do not retry.

## The argument that is not where you expect

`oculus files` and `oculus calendar` take subject codes as **positional
arguments**. `grep`, `search` and `read` take a **`-s` flag**. Writing
`oculus grep MULT20015 "cartesian axes"` searches for the pattern
`MULT20015` — it is not an error, it is a wrong answer.

Codes match on prefix everywhere: `MULT20015` finds `MULT20015_2026_SM2`.
