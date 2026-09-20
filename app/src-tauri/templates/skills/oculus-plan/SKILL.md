---
name: oculus-plan
description: Put work on the student's board in an Oculus course library — create a project, break an assignment into tasks, move or refile something. Use for "put this on the board", "break this down", "make a plan for the assignment", "mark that done", or any request that should end as rows the app draws rather than as a written-out list.
---

# Planning

`oculus project` and `oculus task` write the **same rows the app's board
draws**, so a task added here appears there immediately. Nothing upstream has
a copy of this — unlike a course file, a deleted task does not come back on
the next sync.

## The shape of a breakdown

Three calls, not thirty:

```bash
oculus project create "INFO30006 Assignment 2" -s INFO30006 --due 2026-10-03T23:59:00Z --brief "Censorship and evasion report, 2000 words, in pairs"
```

```bash
oculus project show 7
```

```bash
oculus task add -p 7 --batch -
```

`project show` is not optional. It prints each column's **id** in brackets
after the heading, and `--column` takes that id. A new project opens with
`backlog`, `todo`, `doing`, `done`, but an older one or a hand-made board need
not, and an unknown column is **refused** — a task filed under a column that
does not exist is drawn by nothing, in any view.

## The `--batch` contract

`--batch -` reads a JSON array from stdin and writes it in **one
transaction**. Anything past two or three tasks goes in this way; a command
per task is what leaves half a plan on the board when one is rejected.

```json
[
  {"title": "Read the brief", "column": "todo", "due": "2026-09-22T23:59:00Z"},
  {"title": "Outline", "key": "outline"},
  {"title": "Draft intro", "parent": "outline"}
]
```

Per task: `title` (**required**), `column`, `body`, `due`, `starts`,
`estimate` (minutes), `parent`, `key`.

- `parent` is either an existing task's numeric id or the `key` of an
  **earlier item in the same array** — that is how a parent and its subtasks
  go in together. `key` is never stored.
- Subtasks are **one level deep**. A subtask cannot be given children.
- An unknown field is a hard error, not a silent no-op.
- All or nothing: a bad item writes none of them and says which one failed.
  Fix it and re-send.

Write the JSON to a file and pass the path when it is long. Do **not** put a
newline inside a shell argument — a multi-line `--body` or `--brief` is
refused before the CLI ever sees it.

## Moving, finishing, filing

- **`oculus task move` is the only thing that changes where a task sits.** Its
  column, its order and whether it is finished are one fact; `task update`
  deliberately cannot touch them. **Finishing something is moving it into a
  `done` column**, not setting a flag.
- **`oculus task refile` is the only thing that changes which project a task
  is on**, and it takes the task's **subtasks with it**. A lone subtask is
  refused and told to refile its parent. The column maps across by *kind*, and
  a destination with no column of that kind is refused rather than given the
  nearest one; the task lands at the end, so `task move` is how it is placed.
- `oculus task update` is for the title, notes, dates and estimate. An **empty
  string clears** a field: `--due ""`.

## A task does not need a project

```bash
oculus task add "email the tutor about the extension"
```

With no `-p` this is an **unfiled** task — the absence of a project, not a
project called Inbox. It is the right answer to "write this down, I have not
decided where it goes", and it is what the app's Tasks page writes by default.
`oculus task refile 12 -p 7` files it later.

**Do not invent an "Inbox" project.** There is nothing to clean up if an
unfiled task is never filed.

`oculus task list` with no `-p` spans the library, unfiled first;
`--unfiled` is that pile alone. `--column` still needs `-p`, since a column id
only means something against one board.

## Before you write

Rows written here are marked `source: agent`, so the board can show what it
did not write itself — but that is a label, not an undo. `oculus task rm` has
none at all. When a breakdown is a guess rather than a request, say what you
would add and let the student agree to it first.
