---
name: oculus-plan
description: Manages projects and tasks in an Oculus course library, including Markdown descriptions, dates, calendar links, tags and related library files. Use for requests to put work on the board, break down an assignment, update project properties, attach files, move or refile tasks, or mark work done.
---

# Planning

`oculus project` and `oculus task` write the **same rows the app's board
draws**, so a task added here appears there immediately. Nothing upstream has
a copy of this — unlike a course file, a deleted task does not come back on
the next sync.

## The shape of a breakdown

Create the project, inspect its board, then add its tasks:

```bash
oculus project create "INFO30006 Assignment 2" -s INFO30006 --due 2026-10-03T23:59:00Z --tags "Assignment" --brief-file agents/brief.md
```

```bash
oculus project show 7
```

```bash
oculus task add -p 7 --batch agents/tasks.json
```

`project show` is not optional. It prints each column's **id** in brackets
after the heading, and `--column` takes that id. A new project opens with
`backlog`, `todo`, `doing`, `done`, but an older one or a hand-made board need
not, and an unknown column is **refused** — a task filed under a column that
does not exist is drawn by nothing, in any view.

## Markdown and project properties

Write multiline Markdown to a UTF-8 file under `agents/`, then pass its path:

```bash
oculus project update 7 --brief-file agents/brief.md
oculus task update 12 --body-file agents/task-notes.md
```

`project create` also takes `--brief-file`; `task add` takes `--body-file`.
Both accept `-` for stdin when the calling tool can supply it. The file's
newlines, lists, maths and code fences are preserved. Inline `--brief` and
`--body` suit short text; avoid literal newlines in shell arguments because
an agent's shell permission analyser can refuse them. Do not flatten a
formatted description into one paragraph to get around that.

- Create with `-s CODE` for a subject, or omit it for Personal. On an existing
  project, `project update 7 -s CODE` changes subject and `--personal` clears
  it; its tasks stay on the project.
- Project create/update accept `--due`, `--starts` (ISO 8601) and `--tags`.
  Dates with a time should include the intended timezone offset or `Z`.
- `project update 7 --status archived` archives it; `--status active` restores
  it. Progress is calculated from tasks, never set directly.
- `oculus project events -s CODE` lists calendar ids, including past events.
  Omit `-s` to reach other subjects and personal events. Pass an exact id to
  `project create ... --event ID` or `project update 7 --event ID`; `--event ""`
  unlinks it. Tasks and hidden fallback recordings cannot be pinned.
- An empty inline value or empty file clears About/notes on update. Omitted
  flags leave existing fields alone. Choose inline text or file input per field.
- Project icons and colours are app appearance preferences; board columns
  use the project's existing configuration, as `project show` prints it.

## The `--batch` contract

`--batch -` reads a JSON array from stdin and writes it in **one
transaction**. Anything past two or three tasks goes in this way; a command
per task is what leaves half a plan on the board when one is rejected.

```json
[
  {"title": "Read the brief", "column": "todo", "due": "2026-09-22T23:59:00Z", "body": "## Read\n\n- Mark requirements\n- Note questions", "tags": ["Assignment"], "files": ["courses/INFO30006/assignments/brief.pdf"]},
  {"title": "Outline", "key": "outline"},
  {"title": "Draft intro", "parent": "outline"}
]
```

Per task: `title` (**required**), `column`, `body`, `due`, `starts`,
`estimate` (minutes), `tags` (array of names), `files` (array of library
relative paths), `parent`, `key`.

- `parent` is either an existing task's numeric id or the `key` of an
  **earlier item in the same array** — that is how a parent and its subtasks
  go in together. `key` is never stored.
- Subtasks are **one level deep**. A subtask cannot be given children.
- An unknown field is a hard error, not a silent no-op.
- All or nothing: a bad item writes none of them and says which one failed.
  Fix it and re-send.

Write the JSON to a file and pass the path. Encode newlines inside JSON
strings as `\n`; literal newlines inside a JSON string are invalid JSON.
`--batch` supplies all per-task properties, so it cannot be mixed with
single-task flags such as `--body-file` or `--file`.

## Related files and shared tags

Discover the real library paths before attaching them:

```bash
oculus files INFO30006 --match brief
oculus task add "Read the brief" -p 7 --file "courses/INFO30006/assignments/brief.pdf" --tags "Reading"
oculus task update 12 --attach "courses/INFO30006/lectures/week1.pdf"
oculus task show 12 --json
oculus task update 12 --detach "courses/INFO30006/lectures/week1.pdf"
```

Use exact `relative_path` values from `oculus files`, not invented paths or
absolute filesystem paths. These are links to existing library files, not
uploads. Repeat `--file` on creation or `--attach` / `--detach` on update for
several files. In a batch, use `files: ["path", ...]`. Duplicate attachment is
a no-op; an unknown or ambiguous path rejects the write, and a bad batch
rolls back every task. `task show` prints full notes and related paths;
`--json` returns `task`, effective `tags`, `files` and `subtasks` for checking.

`--tags "Reading,Assignment"` works on project and task create/update; on
update it replaces the whole set and `--tags ""` clears it. Names match
ignoring case; missing names are registered automatically. Tasks without
explicit tags inherit the project's current tags; clearing task tags resumes
inheritance. `oculus tag list` shows the shared registry; `tag add`, `rename`,
`color` and `rm` manage it (`--help` lists their arguments and colour palette).

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
- `oculus task update` is for the title, notes, dates, estimate, tags and
  related-file links. An **empty
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
