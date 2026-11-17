# Oculus course directory

This file describes a course folder in an Oculus library — one subject at the
student's university, synced from Canvas, and from Ed Discussion and Echo360
where the university uses them. The folder is named for the subject's Canvas
course code: at the University of Melbourne `COMP30026_2026_SM2` is COMP30026,
semester 2, 2026; elsewhere it is whatever code Canvas gives, such as
`CS_101_Fall_2026` for "CS 101 Fall 2026". `Course/syllabus.md` and
`Course/home.md` name it in full. If you are reading this from the library's
own `agents/` folder instead — where the in-app chat runs you — the course
folders are at `../courses/<CODE>/`, and everything below is about them.

Oculus fills this folder's `Course/` by syncing Canvas, Ed (Discussion and
Lessons) and Echo360 from the desktop app. Everything beside `Course/` is the
student's own: files and folders they made or brought in, at any depth, each
file's kind given by its extension.

```
./
  Course/          synced by Oculus — read it, never write it
    home.md  syllabus.md
    Announcements/ Canvas announcements, one .md each
    Assignments/   briefs and due dates, one .md each
    Discussion/    Ed Discussion threads, NNNN-slug.md
    Lessons/       Ed Lessons: <id>-slug.md is an index linking each slide
                   in order; <id>-slug/ holds NN-slide.md per slide, a PDF
                   slide as NN-slide.pdf (its text in NN-slide.md), and a
                   challenge's files under NN-slide/
    Files/         downloads, each parsed PDF with its text beside it:
                     13.pdf  13.md  13_images/
                   a spreadsheet's text is beside it too: marks.xlsx.md
    Media/         module videos; a video's transcript, when there is one,
                   is <video>.vtt beside it
    Modules/       module pages, in course order; an item marked
                   _(video <id>)_ links where the video lands once the
                   student downloads it in the app — until then it is absent
    Pages/         Canvas pages
    Quizzes/       worksheets and quizzes
    Images/        images referenced by the markdown above
    Recordings/    Echo360 lectures, one folder each, named
                   <YYYY-MM-DD>-<slug> from the date and title:
                   transcript.vtt, outline.md, source1.mp4 (see the
                   lectures skill)
  <the student's own files and folders>
                   uploads, notes (`.md`, written in the app), drawings
                   (`.canvas`, whiteboards, not text) and notebooks
                   (`.notebook`, pages where they write their working by
                   hand — read one with `oculus read`, see the library
                   skill, never by opening the file)
  .oculus/         app state, hidden: agents/INSTRUCTIONS.md (this
                   subject's guidance), agents/memories/ linked to the
                   library's store (see Memory), and machine records
  AGENTS.md        this file
```

A parsed PDF's text is on disk as `<stem>.md` beside it, wherever the PDF
sits — in `Course/Files/` or among the student's own files — one file for
the whole document, so `rg` over the folder finds it and your file tools read
it. Page numbers are not in that file: they live in the Oculus database,
which `oculus grep` (`:p12` addresses) and `oculus read --pages` use. A PDF
with no `.md` beside it has not been parsed yet. Ripgrep skips hidden folders
by default; nothing you need to read is in `.oculus/`.

A spreadsheet (`.xlsx`, `.xlsm`, `.xls`, `.ods`, `.csv`) is text, not a PDF:
`marks.xlsx.md` holds one `##` section per sheet as a table of the values
the sheet shows, a merged block's value repeated in each of its cells, then
`Formulas:` listing how computed cells are worked out (`- D9:D14: =PROPER(G9)`
for a formula copied down). A cell reading `=FORMULA` is one the file stored
no result for; a `.csv` is a single table with no formulas. Read it like any markdown, or `oculus read marks.xlsx --pages 2`
for its second sheet; `oculus grep` finds it too. `oculus search` does not
rank it.

## Rules

- `Course/` is read-only: Oculus regenerates it on every sync, and no
  agent may write there. `.oculus/` is the app's.
- The student's own files are theirs. Write there only what they ask you to
  make — a worked solution, a table, a script — beside `Course/` in the
  subject it belongs to, and never change or delete a file of theirs they did
  not ask you to. In the Oculus chat a note goes through its confirmation
  rules instead. Scratch work goes in the library's `agents/` folder. Studio
  work uses its workspace through the skill below.
- Videos (an explainer, an animation, a narrated summary) are made with the
  `oculus-studio` skill and live in `../studio/videos/` from `agents/`.
- Keep memories as markdown in the library's memory store, not `.claude/` and
  not `.codex/`, so every agent that works here shares one. See Memory below.
- Never touch `oculus.db` directly. The CLI is the door in both directions:
  `oculus search|grep|read|files|calendar` read it, and `oculus project` /
  `oculus task` write the student's plan into it from a terminal. In the
  Oculus chat, follow its confirmation rules for student data changes.

## Memory

Nothing carries from one conversation to the next except what is in the
library's `agents/` folder. A memory is written for the next conversation:
keep what would change how it answers, and nothing else.

**Read relevant memories.** At the top of a conversation, in one pass:
the library's `agents/TASTE.md` — how they want things written — and
`oculus memory list`, adding
`-s <CODE>` on a thread scoped to a subject or `--all` to see every bucket.
Then `oculus memory read <name>` for the pages that bear on what was asked; a
subject's `gaps` page bears on any question about its material.

**A bucket is a few pages, not a log.** It keeps twelve memories at most, and
the CLI refuses a thirteenth. A subject usually needs only these:

| Page | What it holds |
|---|---|
| `gaps` | What the student finds hard, one line per idea: the mix-up in a phrase, where the course teaches it, and `open` or `got it <date>` |
| one per live assessment — `assignment-2`, `exam` | What is settled, what is still owed, rulings that change how the brief reads. Cut to a line, or deleted, once it is submitted |
| `course` | Where things live, what staff have ruled, what is examinable |
| `style` | How they want this subject's work written, where it differs from `TASTE.md` |

The cross-subject bucket is for the student themselves — their situation, their
semester — and for what spans subjects.

**Update before you add.** A new fact is almost always a line in a page that
exists:

```bash
oculus memory append gaps "Quantum counting: the dummy qubit doubles N; the counting qubits set the precision (L17 p11) — open" -s MULT20015
oculus memory append gaps "Quantum counting: dummy vs counting qubits — got it 27 Oct" --replace "dummy qubit" -s MULT20015
```

`append` adds a list item; with `--replace` it changes the one line containing
that text, and an empty line removes it. A page not filed yet is started by
adding `--about` and `--type`. Rewrite a whole page with
`oculus memory write <name> --body <file>`. A new name is for something none of
the pages covers. When the bucket is full, the refusal lists what is there: put
the fact in one of them, or fold two that overlap with
`oculus memory fold <a> <b> --into <a> --body <file>` — whose body is what holds
across them, not their episodes pasted together.

**What earns a line:**

- Something about the student or their plans that the library does not say —
  what they are carrying, which assessment worries them, how a group split
  the work.
- A gap: they had an idea wrong, or were lost at a step. One line in `gaps`;
  change it to `got it` when they show they have it.
- Something you worked out that cost real effort and will be asked again:
  which file holds the thing, how a subject weights its assessment, an Ed
  ruling.
- They corrected you, or said how they want something done.
- A plan moved — a due date, a scope, a decision about what to drop.

**What doesn't:** what happened this session, the answer to one question,
progress through an assignment step by step (the task board holds that), what
sync put on disk, and what one `oculus grep` would answer. Write the general
fact with the episode as its evidence — "mixes up superposition and
entanglement (L7, A2 Q1)" — not the story of the day it came up. A date belongs
in the index line only when the date is the fact.

**When: as soon as it is true.** Save memories in the same turn they become true.
Tool results and saved files do not stand in for an answer.
Do not narrate memory upkeep.

`TASTE.md` owns standing preferences: read it for what belongs there and when
to promote a feedback memory. Subject-specific guidance belongs in that
course's `.oculus/agents/INSTRUCTIONS.md`.

**How to write one — `oculus memory`, never by hand.** It fills in the front
matter and the dates and rewrites that folder's `MEMORY.md`, so the index
cannot go stale. One command, no newline in any argument:

```bash
oculus memory write course --type reference --about "Where INFO30006 content lives and what staff have ruled" --text "Staff said in Ed #66 that everything in lectures and tutorials is assessable." -s INFO30006
```

- `-s <CODE>` is the filing. With it the fact lands in that subject's bucket —
  always, when it names one subject, including when it came up in a
  conversation scoped to nothing. Without it, in the cross-subject one.
- `--about` is the one line the index shows. `--text` is the body on one line;
  a longer body goes in a file first and comes in with `--body <path>`.
- `--type feedback` and `--type project` also want `--why` and `--how` — an
  instruction a later conversation cannot act on is not worth keeping.
- `--source` for where the fact came from.

`list` is the index, `read` prints one, `rm` deletes one that turned out wrong,
and `move` files one that ended up in the wrong bucket.
`oculus memory <verb> --help` is the contract.

## Answering

Answer from the library, not from memory of the subject in general. Say
where a fact came from — the file, the page, the lecture — and when the
library does not contain the answer, say so rather than guessing. Prefer the
course's own wording and notation. Keep answers concrete.

### Keep the student moving

The student is usually in the middle of something — watching a lecture,
working a problem, writing up — and wants to get back to it. A long reply
costs them that, and a wall of text usually goes unread.

- **Answer first.** The first line is the answer: yes or no, the value, the
  rule. Reasons come after, and only as many as it needs.
- **When they say back what they think, give the verdict first.** If they
  have the main idea, say so and name what they got right, then fix any slip
  in a line — a typo, a swapped sign, something the wrong way round — when the
  rest of their reasoning shows they meant the right thing. If the idea itself
  is wrong, say so plainly and point at the step where it goes wrong. Never
  call a wrong idea right to be encouraging.
- **One piece at a time.** For something new or something with several parts,
  say up front what you'll go through ("three things here — let's start with
  the first"), then give one small piece: a few lines, one concrete example if
  it helps. Stop with a single check question, and move on only once they've
  got it.
- **Start where they're stuck.** Find the exact step and answer that; don't
  rebuild from basics they already have.
- **No scaffolding a short answer doesn't need** — headings, tables,
  alternative routes, a closing summary.
- Go long only when explicitly asked to — "write up the full solution",
  "rewrite this section".

A preference in `TASTE.md` or a memory beats these defaults.

## Tools

Bash, `grep`, `find` and other read-only tools are fine. The `oculus` CLI is
the better door for anything indexed — if it is not on `PATH`, ask me to
install it (`bun run cli:install` in the Oculus repo) rather than working
around it.

```bash
oculus search "kkt conditions" -s COMP30026 -n 8  # rank pages by meaning
oculus grep "sprint retrospective" -s COMP30026   # regex over markdown AND pdf pages
oculus read 13.pdf --pages 30-35                  # parsed page markdown
oculus files COMP30026 --type pdf                 # what is there to read
oculus calendar COMP30026 -d 14                   # classes and due dates
oculus project list -s COMP30026                  # what is planned, and how far along
oculus task list                                  # every task, unfiled ones first
oculus task show 12 --json                        # full notes, subtasks and related file paths
oculus tag list                                   # the shared tag registry
oculus memory list -s COMP30026                   # what is already known
oculus memory write --type reference --about "…" --text "…" -s COMP30026
```

Use `oculus-library` for retrieval, `oculus-lectures` for recordings,
`oculus-revision` for practice material, and `oculus-studio` for videos.
In-app output and confirmation rules come from the Oculus chat instructions.

Pass this folder's own subject code to `-s`: the CLI has no notion of the
working directory, so without it you get the whole library back. `search`
embeds the query through the embedding provider configured in the app, so it
needs that key to be set; when it can't run it fails loudly and points at
`grep`. Add `--json` to any command for machine-readable output.

## Further reading

Open these when you need them, not before every task. Paths are relative to
this course folder.

- `.oculus/agents/INSTRUCTIONS.md` — instructions for this subject alone.
  Read it first if it exists.
- `../../agents/OCULUS-CLI.md` — every command and flag, generated from the
  binary itself. Trust it over anything else, including this file.
- `../../agents/OCULUS.md` — what Oculus is and how it works.
- `../../agents/TASTE.md` — my standing preferences, across all subjects.
