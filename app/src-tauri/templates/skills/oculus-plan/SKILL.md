---
name: oculus-plan
description: Proposes projects, tasks, tags, calendar events and notes as editable confirmation cards in an Oculus chat. Use for planning work, breaking down an assignment, changing due dates, finishing or refiling tasks, scheduling study blocks, writing a note, or proposing sync, parse, transcription, lecture or download jobs.
---

# Planning with confirmation cards

Follow the Oculus chat instructions for confirmation policy and output
placement. This skill owns planning workflows and how to amend pending cards;
`oculus-ui` owns OpenUI syntax, positional signatures and permitted fields.
Read it before writing a card. Keep prose to a line; do not repeat its rows.

## Look up ids and the current values

- `oculus project list`, `project show ID`: project ids, fields and board
  columns. Column ids are board-specific; never guess an older board's ids.
- `oculus task list`, `task show ID`: task ids, fields, effective tags,
  related library paths and subtasks. No project filter spans all tasks.
- `oculus tag list`: numeric ids, names and palette colours.
- `oculus calendar`, `project events`: event ids; only `local_<id>` can be
  changed or deleted. Canvas events are read-only.
- `oculus files CODE`: exact library paths for task attachments and jobs.

## Break an assignment into a project and tasks

Use NewProject for a project and its tasks in one confirmation. Choose a
real subject code or null for Personal.

```openui
root = Stack([NewProject("Assignment 2", "INFO30006", [outline, draft], "2026-10-27")])
outline = Task("Outline the argument", "2026-10-25", 45, "todo", [Task("Read the brief"), Task("Collect sources")])
draft = Task("Write the first draft", "2026-10-26", 90)
```

For tasks on an existing project, use NewTasks(projectId, tasks). A task
needs no project: NewTasks(null, [Task("Email the tutor")]) files it nowhere.
Do not invent an Inbox project. Under(existingTaskId, [Task(…)]) adds
subtasks under an existing parent; look up the parent's real id first.

## Change only the fields that need changing

Read the item immediately before proposing. Change(id, from, to) carries
matching keys: `from` is exactly what the CLI returned, `to` is the desired
value. The card flags anything that changed since, and apply refuses stale
writes. `null` clears a field. Omitted keys stay untouched.

```openui
root = Stack([TaskChanges([Change(42, {due: "2026-10-27"}, {due: "2026-10-30"})])])
```

Use the permitted field keys in `oculus-ui`. Completing a task changes
its column to the board's done column. Refiling changes project; null unfiles.
A parent takes its subtasks with it; a lone subtask cannot be refiled. A move
goes at the end of its destination column. Archiving a project keeps its tasks.
Use deletion only when asked; finishing a task is a column change.

## Put study blocks around a deadline

Read the calendar before scheduling. A study block is a class event with
an end; a date-only start is all day.

```openui
root = Stack([NewEvents([Event("Past paper practice", "2026-10-26T14:00:00+11:00", "2026-10-26T15:30:00+11:00", "class", "INFO30006")])])
```

Use NewNote for a note, with the complete Markdown body in its card.

## Amend from the student's edits

The app appends "Cards in this thread" with pending cards and each newly
decided state. Use those current fields, including excluded rows, rather
than reconstructing the original proposal. To replace a pending card, pass
its app id (for example `"812.0"`) as the final `amends` argument. The old
card becomes Updated below; only the replacement can apply. Never invent a
card id. A decided card cannot be amended: use its made ids in a new change
card. Rewind removes later cards but does not undo their writes.

## Ask before a long or billed job

Choose the job from `oculus-ui` using these inputs:

- Sync: subject codes, plus whether to include lecture lists.
- Reparse: exact library paths, using Settings → Parsing and
  Embeddings; this can take minutes or use credits.
- Transcribe: a library video; the card names Settings' engine order
  and explains when Groq uploads audio.
- LectureChapters / LectureEnd: real lecture ids; the card
  names Settings' job models.
- Download: "aligner", "voices" for the starter pack, or a voice id
  from `oculus studio voices`; the card names its size.

If a previous run failed or the app closed, inspect the domain's result
before proposing another run.
