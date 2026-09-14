# Projects

A project is a piece of work you are doing — usually an assignment — scoped to
one subject or to none, broken into tasks and one level of subtask, and read
back four ways: a board, a table, a backlog or a timeline. Nothing here is
scraped: every row is the student's own planning, or a plan the chat agent
wrote for them through the `oculus` CLI. That is what shapes most of the
decisions below — nothing upstream has a copy, so nothing can be repaired by
syncing again.

## Where

| Piece | Location |
| --- | --- |
| `projects` + `project_tasks` tables (migration 27) | `app/src-tauri/src/lib.rs` |
| Frontend reads and writes | `app/src/lib/projects.ts` |
| What is on screen | `app/src/stores/projectsStore.ts` |
| Headless writes (CLI, and so the agent) | `app/src-tauri/src/projects.rs` |
| `oculus project` / `oculus task` | `app/src-tauri/src/bin/oculus.rs` |
| Index and one project | `app/src/pages/ProjectsIndexPage.tsx`, `app/src/pages/ProjectPage.tsx` |
| A subject's Projects tab | `app/src/pages/subject/ProjectsPage.tsx` |
| The four views, and the shaping behind them | `app/src/components/projects/`, `app/src/components/projects/taskTree.ts` |
| Overlap packing, shared with the calendar | `app/src/lib/lanes.ts` |
| The calendar's task layer | `getAllOpenTasks` in `app/src/lib/projects.ts`, `app/src/lib/calendar.ts` |
| The agent's instructions | `app/src-tauri/templates/AGENTS.template.md`, `app/src-tauri/templates/HARNESS.template.md` |

## The schema, and why it is shaped that way

Migration 27's comment block is the long version; the decisions worth knowing
from outside are these.

- **`subject_id` is nullable and clears rather than cascades.** NULL is
  "Personal". This is the user's own planning, so dropping a course must not
  take it away — the same call `local_events` (migration 22) and
  `harness_threads` (25) made, and the opposite of the Canvas-owned tables,
  whose rows genuinely *are* the course's and cascade with it. Project →
  tasks, on the other hand, **is** `ON DELETE CASCADE`, because a project
  really does own its tasks where a subject merely scopes them; so is
  task → subtask.
- **The board's columns are JSON on the project, not a table.** Columns are
  renamed per project and their shape is still moving — the same call
  `automations.graph` made in migration 18: JSON for the part that keeps
  changing, real columns for the part that gets queried. What the app reasons
  about is a column's `kind` (`backlog` | `active` | `done`), never its name or
  its id, both of which are the user's to change.
- **`position` is `REAL`** so that dropping a card writes one row instead of
  renumbering a column: the new position is the midpoint of its two
  neighbours. Repeated midpoints do eventually exhaust a double, so both
  writers renumber the column to whole numbers when the gap underflows and
  take the midpoint again.
- **`source` is `'manual'` or `'agent'`**, so a board can show which rows it
  did not write itself. Everything the CLI writes is `agent`.

## How it connects

- **Two writers of the same tables, exactly like the scrape tables.** In the
  app the frontend owns these writes outright — direct SQL over `getDb()` in
  `app/src/lib/projects.ts`, no Tauri command in the path, because nothing here
  needs the network, the keychain or a subprocess. Headless,
  `app/src-tauri/src/projects.rs` writes the same rows with the same rules.
  That is the pair `app/src-tauri/src/store.rs` is for the scrape tables (see
  [frontend.md](./frontend.md)): change a table's shape and both writers move
  together, plus the migration. Neither creates the database — a fresh machine
  opens the app once first.
- **`moveTask` is the only writer of `column_id`, `position` and `done_at`.**
  They are one fact in three columns, and the move is the only operation that
  reads the project's board to learn whether the destination is a `kind:
  "done"` column. So `updateTask` cannot touch them, in either writer, and
  every door — an inline status pill, a drag on the board, promoting a backlog
  stub, a CLI `--column` — goes through the one function. A second writer would
  be a second place to leave `done_at` disagreeing with the column the card is
  sitting in, and that bug surfaces on the *calendar*, which filters on
  `done_at`: a task ticked off on its board would go on drawing itself as a
  deadline, far from the code that got it wrong. Creating a task straight into
  a done column follows the same rule, so the two can never disagree whichever
  door the row came through.
- **A column id is checked against the project's own board wherever a task is
  placed**, and an unknown one is refused with the ids the board does have. A
  task filed under a column the project lacks is not merely misfiled: every
  view renders columns, so nothing draws it at all. That is survivable while
  the only writer is a drag on a board that just rendered the column; it stops
  being survivable when `--column` is free text an agent typed.
- **Subtasks are one level deep, enforced in code.** SQLite cannot express
  "the parent has no parent" as a constraint, and the check has to exist
  because the views draw a task and its children, not a tree — a grandchild
  would simply never be drawn. Both directions are checked: a task cannot be
  filed under a subtask, and a task that already has children cannot be given a
  parent.
- **Every write fans out through a `window` event, and that is the only
  refresh path.** Each write in `app/src/lib/projects.ts` fires
  `PROJECTS_UPDATED_EVENT`; the store's write wrappers deliberately do *not*
  re-read, and a component showing project data listens for the event and
  reloads — the way the calendar listens for `CALENDAR_UPDATED_EVENT`.
  Refreshing in the wrappers as well cost a drag two full task reads plus a
  list read, and the two paths could land out of order. The bigger reason is
  that it leaves **one door**: a click in the UI and a write the chat agent
  made from a separate process arrive the same way, so nothing can work for one
  and not the other.
- **The agent's writes get into that door through the harness.** `oculus
  project` / `oculus task` write from a subprocess with its own connection —
  nothing in the webview's pool notices. So `app/src/hooks/useBackendEvents.ts`
  watches the harness event stream, remembers tool calls whose command text
  matches `oculus project`/`oculus task`, and fires the same
  `PROJECTS_UPDATED_EVENT` when one finishes. It matches on the command text
  rather than the tool's classified kind, because `is_oculus_cli` in
  `app/src-tauri/src/harness/event.rs` only word-matches the first few words —
  `cd … && oculus task add` classifies as plain Bash, and a kind gate would
  drop exactly the write the hop exists for. The trade is one-sided on purpose:
  a false positive costs one re-read of a handful of rows, a false negative
  costs a board that is silently wrong. See [harness.md](./harness.md).
- **A breakdown goes in as one transaction, with its own internal
  references.** `oculus task add -p <ID> --batch -` takes a whole breakdown as
  a JSON array; an item may name its parent by an existing task id *or* by the
  `key` of an earlier item in the same array, which is how a parent and its
  subtasks go in from one call. `key` is never stored. Because a breakdown is a shape rather than a
  pile of rows, a single rejected item — an unknown column, a parent that is
  itself a subtask, a date that is not a date — rolls the whole batch back and
  writes nothing: half a breakdown on the board is worse than none. The
  single-task path is the same function with one item, so both doors behave
  identically.
- **The task list is read in one query and grouped in the app.**
  `getTasks` returns a project's parents and subtasks together in `position`
  order — deliberately with no `column_id` in the ORDER BY, which would sort
  the columns alphabetically ("backlog, doing, done, todo") and that is not the
  board's order and never will be. The board's order is the `columns` array on
  the project. `taskTree.ts` does the shaping for all four views, so a row
  whose parent is missing is promoted to top level rather than dropped.
- **The four views are sibling tabs, not a dropdown** (`ViewTabs`, the Sync
  page's rule), with a fixed-height toolbar under them so switching views does
  not jolt the work below. The Backlog view is not made redundant by the board's
  backlog column: it is the same pile read as a list, with a promote button per
  stub, and promoting is a `moveTask` like any other. The timeline packs
  overlapping bars with `packLanes` in `app/src/lib/lanes.ts` — lifted out of
  `app/src/components/calendar/WeekView.tsx`, which was its only caller until
  this needed the same packing, and made generic over the item because the two
  measure a span differently (a class in epoch milliseconds, a task bar in
  pixels) and the packing never needs to know which.
- **The calendar reads tasks live rather than copying rows.** See
  [calendar.md](./calendar.md) — a task re-dated, finished or deleted on its
  board would otherwise leave a row on the grid that nothing cleans up, since
  `local_events` has no cleanup pass. That is also why a task is not deletable
  from the calendar: its card links through to the project instead.
- **A project's name travels in its route's query** (`?n=`, `projectHref` in
  `app/src/components/projects/projectHref.ts`), because `tabInfo` titles a tab
  from the path alone and has no project list to look one up in — the same
  trade the lecture route makes with `?t=`.
