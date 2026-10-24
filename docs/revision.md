# Revision

Revision keeps condensed cheat notes, practice questions with spaced repetition,
named quizzes and saved interactive widgets. It is one rail section scoped
to one subject at a time.

## Where

| Piece | Location |
| --- | --- |
| Tables and indexes, registered as migration 50 | `app/src-tauri/src/db/revision/schema.sql`, `app/src-tauri/src/db/migrations.rs` |
| Topic tables, membership triggers and seed, migration 51 | `app/src-tauri/src/db/revision/topics.sql`, `app/src-tauri/src/db/revision/topics.rs` |
| Cheat tag registry and its triggers, migration 52 | `app/src-tauri/src/db/revision/tags.sql`, `app/src-tauri/src/db/revision/tags.rs`, `app/src/lib/revision/tags.ts` |
| Topics nav group, editor picker and matching | `app/src/components/revision/nav/`, `app/src/components/revision/TopicPicker.tsx`, `app/src/lib/revision/topics.ts` |
| Guarded Revision field edits (`EditItems`) | `app/src-tauri/src/db/proposals/edits.rs` |
| Rust validated row writers and CLI persistence | `app/src-tauri/src/db/revision/`, `app/src-tauri/src/bin/oculus/commands/revision/` |
| CLI arguments | `app/src-tauri/src/bin/oculus/args/revision.rs` |
| Frontend row types, reads, writes and scheduler | `app/src/lib/revision/` |
| Review transaction and widget batch-save commands | `app/src-tauri/src/revision.rs` |
| Section layout, subject scope and data snapshot | `app/src/layouts/RevisionLayout.tsx`, `app/src/stores/revision/revisionStore.ts` |
| Cheat sheet, Review, Quizzes and Artifacts | `app/src/pages/revision/` |
| Shared editor, source links, cheat entry and its tag chips | `app/src/components/revision/` |
| Durable cheat-note proposals | `app/src-tauri/src/db/proposals/cheats.rs`, `app/src/components/markdown/openui/proposals/` |
| Strict written-answer marker | `app/src-tauri/src/harness/app/revision.rs` |
| Course-style drafting instructions | `app/src-tauri/templates/skills/oculus-revision/SKILL.md` |

## Every entry belongs to at least one topic

Topics are flat, subject-scoped buckets shared by cheat entries, questions,
quiz sets and artifacts. Each subject has one Unsorted
topic, created with the subject, which cannot be renamed or deleted.
Membership lives in one link table per content type, and an entry can belong
to several named topics.

SQLite triggers keep the invariant for every writer, app or CLI. A new row
lands in Unsorted; adding a named topic removes Unsorted; removing the last
named topic, or deleting the topic itself, returns the row to Unsorted.
Changing a row's subject resets it to that subject's Unsorted, and a topic
from another subject is refused. Migration 51 seeded named topics from the
existing note tags and question topic labels; those labels stay as metadata.

The nav column has the Revision heading, then the Subjects switcher
(`SubjectSwitcherMenu`) drawn as a field, without the new-file counts. The
subject is the URL's `subject`, else the one Revision last showed
(`app/src/lib/revision/lastSubject.ts`, kept apart from the Subjects
section's), else the first current subject.

Every entry and topic names a subject. The columns stay nullable from
migration 50, so the shared Rust writers (`input` and `topics::add` in
`app/src-tauri/src/db/revision/`) refuse a missing or null `subject_id`, and
the CLI requires `--subject` on every add. A `NewCheatNotes` entry without a
code files under its thread's subject and fails without one. Things saved from
chat take the thread's subject, else Revision's last, else the first current
subject (`app/src/lib/revision/subject.ts`). Under the four pages, a Topics group lists Unsorted
and each named topic with its entry count, as Tasks lists tags. A row filters
every page through the `topic` URL parameter and a second click clears it.
The group's `+` adds a topic and a row's `…` renames or deletes it. Editors
pick topics with `TopicPicker`, and a new entry inherits the selected topic.

Assigning topics is an ordinary edit, not a separate action. Writers take a
`topic_ids` field beside the content fields; `[]` means Unsorted. The agent
sorts revision with one `EditItems([Edit(entity, id, from, to)])` card, the
same guarded from/to flow used for titles and bodies, and Undo restores the
memberships from the journal. Editing a question's topics leaves its FSRS
state alone.

## Cheat entries are database rows

`revision_notes` holds a title, markdown body, subject, tag names, real-valued
position and provenance. Entries are grouped by topic or tag and narrowed by
local search. An entry (`cheat/CheatEntry.tsx`) reads as rendered markdown
whose title and body select like any content;
Edit, or a double-click, edits it in place without changing its size: the
title becomes a field in the heading's type, the body the compact, inline
Live-mode `NoteField` (the rendered body's type, maths and gaps, with only its
toolbar added above), and the topic and tag chips gain pickers in the same
chip size. Each part saves as it changes or blurs; Done, or a press outside
the entry, only leaves editing — the entry's own pickers and dialogs and the
note field's popups on `<body>` (`.cm-tooltip`) count as inside.
Only one entry edits at a time: the page holds which, so editing another (or
a new entry) ends the open one, which saves its title and body as it closes.

Entries reorder from the grip in the left gutter on `useCardDrag`, or with the
arrow keys on a focused grip; the cards themselves never drag. Holding a card
near the page's top or bottom edge scrolls it, easing in toward the edge
(`autoScroll`). A moved note
gets a position between its neighbours. Grouped by topic, a drop into another
group also moves the entry to that topic (or to Unsorted) in the same write;
grouped by tag, a drop into another group springs back.

## Cheat tags have their own registry

`revision_tags` gives each cheat tag a palette colour and a name unique
ignoring case, as `task_tags` does for tasks; the two lists stay apart because
their kinds differ. It is seeded with Definition, Formula, Theorem, Method,
Worked example and Note. Entries keep names in `revision_notes.tags`, and
triggers do the rest for every writer: a new name joins the registry in
slate, an entry takes the registry's spelling with duplicates dropped, and a
renamed or deleted tag follows into every entry. Writers refuse an empty name,
one over 40 characters, or one with a `/`, so a path never becomes a tag.
`CheatTags` draws them with the task `TagChip` and `TagPicker`; a name created
in the picker is only a name until the entry is saved. `oculus cheat tags`
lists the registry with use counts, and the agent spells tags from it.

Entries do not write a file under the course folder. A chat addition cannot
replace an open note editor or overwrite the rest of a cheat sheet.
The source chip opens a library citation or the originating chat.

`NewCheatNotes([CheatEntry(...)])` is a durable proposal: title, body, subject,
tags and topics are editable before Add. Each entry (`CheatRow.tsx`) is drawn
as the cheat sheet draws one, sharing its type and chip sizes: title, rendered
body, then subject, topic, tag and source chips. Its pencil edits in place as
the cheat sheet does (the editor is fetched ahead, so nothing collapses while
it loads), and X restores the entry as it was. Tags use the same `CheatTags`
picker over the registry. While editing, a proposal entry keeps at least its
reading height: mounting and measuring CodeMirror must not briefly collapse
the row and clamp the chat's scroll position. The field mounts before paint.
The row holds its scroller through the DOM swap and toolbox/editor resize,
including a late toolbox mount; new wheel, touch, pointer or key input cancels
the hold immediately. The title input uses the heading's exact line height
as its height, with native appearance disabled, so WebKit cannot round it to
a different box when editing starts or ends.
Proposal versions and Reset use the shared
[card version contract](./harness.md#proposal-context-is-read-at-dispatch-and-confirmation-lives-in-rust).
`source` is the library path an entry came from, `#page=N` for a PDF page,
drawn here and on the cheat sheet as a file chip with the page. A path the
agent passed among the tags is moved into `source` when the card opens
(`sourceFromTags` in `proposals/model.ts`), with the page from a label like
"Lecture 17 p14" left where the path belonged. Topics are names, not ids: Add files
the entry under the subject's topic of that name (case-insensitive) and
creates a missing one (`topic_named` in `app/src-tauri/src/db/proposals/cheats.rs`),
so the card is the agent's only write. The proposal, its entries and any new
topics commit in one transaction; Undo removes a created topic unless other
entries have since been filed under it. Deciding twice cannot create
duplicates. Amendments preserve explicit inclusion ticks on matching entry
titles.

After Add the card lists each entry as it is now, with Open; an entry the
student deleted is struck through. Undo removes every added entry, edited ones
included, skips deleted ones, and returns the card to pending with the entries
as they were at Add (`apply::undo_in`), so adding again restores the proposal,
not the edits. Other cards still refuse an Undo over live edits.

## Draft questions enter Review only after acceptance

`revision_items` supports card, choice, numeric, short and structured questions.
It stores the model answer, options, absolute numeric tolerance, rubric points,
topic tags, difficulty label and the problem used as a style reference.

The agent skill reads the course's own assignments, quizzes, tutorial sheets,
past exams and Ed Lessons challenges before drafting. It paraphrases questions,
changes values, retains the notation and marking style and cites the source
problem. Difficulty has a fixed meaning: easy is recall; medium applies one
method; hard is a multi-step or unseen problem. A paraphrased exam question is
at least medium.

Agent CLI additions and saved Quiz/Flashcards widgets create drafts. Quizzes
lets the student edit, accept, suspend or delete them. Only active questions
enter Review. The human `level` is independent of `fsrs_difficulty`.
Choice answers are exact option text and must match exactly one option.
Numeric model answers must be finite; tolerances are absolute and nonnegative.

## A review and its FSRS state commit together

The frontend uses pinned `ts-fsrs` 5.4.2 with fuzz disabled. The package has no
runtime dependencies or install lifecycle scripts. It computes the next due
date, stability, scheduler difficulty, repetitions, lapses, state, last review
and learning steps. The Rust command writes the attempt and transition in one
transaction, refusing a draft or a stale repetition count from another view.

Cards flip and the student rates Again, Hard, Good or Easy. Choice and numeric
questions are marked locally. Scores map to ratings by fraction: below 40%
Again, below 70% Hard, below 95% Good, otherwise Easy. The student can select
another rating; the attempt records that override.

Every attempt retains the answer, rating, score/max, feedback, judge model,
override and elapsed seconds. The schema also reserves a working field for
attempt context. It does not implement drawing-canvas working or hints.

## Written answers are marked only on Submit

The `revisionMarking` job has a chain in Settings → Jobs, mirrored in Rust's
job registry. Its default is Codex `gpt-6-luna` at medium effort. Every provider
uses `Harness::one_turn`; opencode has a hidden, tool-less marker definition.
Settings only chooses a model and never invokes it.

The backend reads the saved question, model answer, rubric and subject, then
adds the submitted answer. The model must return strict JSON containing one
award/reason per rubric point, total, maximum, short feedback and the largest
omission. Rust checks the shape, bounds and sums. A question without a rubric
has one point worth one mark.

A failed provider or malformed response surfaces on the question. Mark it
myself explicitly reveals the model answer and lets the student enter a score
and rating. It records the override. The app never silently invents a grade.
Live-provider calibration is separate from the parser's offline tests.

## Practice quizzes leave due dates alone

Named sets order references to questions; one question can belong to several
sets. Taking a set uses accepted active questions. Each attempt carries the
set id and a fresh session id, with `scheduled = 0`; completing it does not
update FSRS state. Repeating a question within that session is rejected.
The Quizzes page displays attempt counts, the latest scored result and an
expandable score history per set. A quiz needs only a title and can start
empty; ticked active questions go into a new quiz or, with Add N ticked, an
existing one. New question (`app/src/components/revision/quiz/NewQuestion.tsx`)
writes a card, choice, numeric or written question by hand, active at once
since the student wrote it; structured, rubric-marked questions come from the
agent.

## Saved artifacts retain the widget state

Every rendered OpenUI fence offers Save to Revision on hover or keyboard
focus, except one holding a proposal or job card, which is part of the chat
rather than a widget to keep. `revision_artifacts` stores its source program and runtime snapshot,
including slider values and component progress. The Artifacts page renders
through the same lazy runtime, supports expansion, rename and deletion, and
saves state changes after a short debounce, flushing on unmount.

Ask about this appends the artifact's program to a new-chat draft and opens
Chat. The student sends it. Saved programs use the same fixed component
library as replies; arbitrary HTML is handled by the separate output embed.

## CLI writes refresh open Revision views

`oculus cheat` reads and manages entries, and `oculus cheat tags` lists the
tag registry; agent writes require a `NewCheatNotes` proposal instead. `oculus quiz add --batch` validates and saves
its draft questions atomically. `quiz list --due` reads active due questions
without computing a schedule. `quiz set add` creates an ordered set.
`oculus artifact save --program-file` persists a widget program and optional
state. `oculus topic` lists, adds, renames and removes topics; creation
commands take `--topics` ids and list commands filter by `--topic`. An
agent's updates, topic renames included, require an `EditItems` proposal;
deleting a topic or entry is left to the student. Exact flags are in [cli-reference.md](./cli-reference.md).

Frontend writes and completed harness `oculus cheat|quiz|artifact` calls emit
`oculus:revision-updated`. The mounted Revision layout reloads its shared snapshot
with a request generation guard. Each pane filters that snapshot using its own
URL subject scope. Due
counts refresh while a pane is idle, and FSRS learning repeats return when due.

## Gotchas

- Never use the scheduler's difficulty as the student's easy/medium/hard label.
- Never schedule a practice quiz attempt or activate an agent draft automatically.
- Choice model answers are option text, not option indices.
- Preserve `learning_steps` when reconstructing an FSRS card.
- A malformed marking response is an error with an explicit self-mark option.
- Do not edit an applied migration: SQLite's migration checksums include comments.
