---
name: oculus-plan
description: Proposes projects, tasks, tags, calendar events and notes as editable confirmation cards in an Oculus chat. Use for planning work, breaking down an assignment, changing due dates, finishing or refiling tasks, scheduling study blocks, writing a note, or proposing sync, parse, transcription, lecture or download jobs.
---

# Planning with confirmation cards

The student reviews a card, edits or unticks rows, then clicks Create or
Apply. Always propose first, even when asked to "just add them". The app
applies the selected items together and offers Undo. Do not call CLI write
verbs: chat runs with `OCULUS_AGENT=1`, which refuses them.

Read `oculus-ui` for OpenUI syntax and the exact positional signatures. One
proposal per fence, at the root inside `Stack`. A malformed fence stays code,
so use only documented components and functions. A line of prose around a
card is enough; do not repeat its rows in the reply.

## Look up ids and the current values

- `oculus project list`, `project show ID`: project ids, fields and board
  columns. Column ids are board-specific; never guess an older board's ids.
- `oculus task list`, `task show ID`: task ids, fields, effective tags,
  related library paths and subtasks. No project filter spans all tasks.
- `oculus tag list`: numeric ids, names and palette colours.
- `oculus calendar`, `project events`: event ids; only `local_<id>` can be
  changed or deleted. Canvas events are read-only.
- `oculus files CODE`: exact library paths for task attachments and jobs.

Project/task/tag ids are numbers; event and lecture ids are strings. A
subject is its code, a path starts at the library root (`courses/…`). Use
ISO dates; a time includes its intended offset or `Z`. Estimates are whole
minutes. Optional middle arguments use `null`, optional trailing ones can
be omitted. Tags are names; a missing name is registered on Create.

## Break an assignment into a project and tasks

Use NewProject for a project and its tasks in one confirmation. Choose a
real subject code or null for Personal; subtasks nest one level only.

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

Task fields: title, body, due, starts, estimate, tags, files, column, project.
Completing a task is changing column to the board's done column. Refiling
changes project; null unfiles. A parent takes its subtasks with it; a lone
subtask cannot be refiled. A move goes at the end of its destination column.

Project fields: name, subject, due, starts, brief, tags, event, status
(active or archived). Archiving keeps its tasks. Event fields: title, start,
end, kind, subject, notes. Tag fields: name, color. Unknown keys are refused.
TagChanges also accepts NewTag(name,color) and DeleteTag(numericId).
DeleteTasks([ids]) lists the subtasks that go too. DeleteEvents takes local
ids only. Use deletion only when asked; finishing a task is a column change.

## Put study blocks around a deadline

Read the calendar before scheduling. Event kinds are note (default), due
and class. Only class accepts an end; a study block is a class event. A
date-only start is all day. Each event may name a subject or null.

```openui
root = Stack([NewEvents([Event("Past paper practice", "2026-10-26T14:00:00+11:00", "2026-10-26T15:30:00+11:00", "class", "INFO30006")])])
```

NewNote(subject,title,body) writes a note only on Create. Preserve Markdown
with `\n` in strings and double every TeX backslash. No direct course writes.

## Amend from the student's edits

The app appends "Cards in this thread" with pending cards and each newly
decided state. Use those current fields, including excluded rows, rather
than reconstructing the original proposal. To replace a pending card, pass
its app id (for example `"812.0"`) as the final `amends` argument. The old
card becomes Updated below; only the replacement can apply. Never invent a
card id. A decided card cannot be amended: use its made ids in a new change
card. Rewind removes later cards but does not undo their writes.

## Ask before a long or billed job

Use the job cards, one per fence:

- Sync(subjects,lectures?): codes to sync, true for lecture lists.
- Reparse(files): exact library paths, using Settings → Parsing and
  Embeddings; this can take minutes or use credits.
- Transcribe(file): a library video; the card names Settings' engine order
  and explains when Groq uploads audio.
- LectureChapters(lecture) / LectureEnd(lecture): real lecture ids; the card
  names Settings' job models.
- Download(item): "aligner", "voices" for the starter pack, or a voice id
  from `oculus studio voices`; the card names its size.

Jobs have their own progress and error, no amendments or Undo. If a previous
run failed or the app closed, inspect the domain's result before proposing
another run. Never call auth or keyd from chat. Memory commands and Studio
video work remain the agent's workspace and need no proposal card.
