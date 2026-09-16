# Projects

A project is a piece of work you are doing — usually an assignment — scoped to
one subject or to none, broken into tasks and one level of subtask. It opens on
an **Overview** — what it is, the facts pinned to it, what is next — and its
tasks are read three ways behind a **Tasks** tab: a board, a table or a
timeline. Each task is a page of its own as well. Nothing here is
scraped: every row is the student's own planning, or a plan the chat agent
wrote for them through the `oculus` CLI. That is what shapes most of the
decisions below — nothing upstream has a copy, so nothing can be repaired by
syncing again.

## Where

| Piece | Location |
| --- | --- |
| `projects` + `project_tasks` tables (migration 27) | `app/src-tauri/src/lib.rs` |
| `projects.tags` + `projects.event_id` (migration 33) | `app/src-tauri/src/lib.rs` |
| Frontend reads and writes | `app/src/lib/projects.ts` |
| What is on screen | `app/src/stores/projectsStore.ts` |
| Headless writes (CLI, and so the agent) | `app/src-tauri/src/projects.rs` |
| `oculus project` / `oculus task` | `app/src-tauri/src/bin/oculus.rs` |
| Index, one project, one task | `app/src/pages/ProjectsIndexPage.tsx`, `app/src/pages/ProjectPage.tsx`, `app/src/pages/TaskPage.tsx` |
| A subject's Projects tab | `app/src/pages/subject/ProjectsPage.tsx` |
| The Overview: About, properties, upcoming | `app/src/components/projects/ProjectOverview.tsx` |
| Tags, and the pinned calendar event | `app/src/components/projects/TagEditor.tsx`, `app/src/components/projects/EventLink.tsx` |
| Rename / archive / unarchive / delete | `app/src/components/projects/ProjectMenu.tsx`, `app/src/components/projects/useProjectActions.ts` |
| Picking a date and time | `app/src/components/projects/DateTimeField.tsx`, `app/src/components/ui/calendar.tsx` |
| Editing a number in place | `app/src/components/projects/DraftField.tsx` |
| The three task views, and the shaping behind them | `app/src/components/projects/`, `app/src/components/projects/taskTree.ts` |
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
- **Tags are JSON on the project too** (migration 33), for the same reason the
  columns are: nothing queries them in SQL, and a student has a handful of
  projects, so "every tag I have used" is a scan over tens of rows in the page
  rather than a `GROUP BY`. The day a tag needs a colour, a description, or a
  rename that fans out across projects is the day it earns a `tags` +
  `project_tags` pair. `NOT NULL DEFAULT '[]'` so every reader parses the same
  shape. Normalisation — trim, collapse whitespace, deduplicate
  case-insensitively, cap at 24 — happens on the way *in*, in both writers
  (`normaliseTags`, `normalise_tags`), which is what makes "have I used this
  before" a plain string compare everywhere else.
- **`event_id` is a pointer, not a foreign key**, and that is the interesting
  part — see the calendar bullet below.

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
- **A task is a page, at `/projects/:projectId/tasks/:taskId`.** The board and
  the table are where a plan is *arranged*; the page is where one piece of it
  is thought about — a description, its dates, its estimate, its subtasks — so
  it is a full page rather than a dialog over the board it came from. It is
  nested under the project rather than living at `/tasks/:id` because it cannot
  draw anything without the project: a status is a column on *that* board, and
  `moveTask` has to be handed an id the board actually has. Its title rides in
  `?n=` like a project's name, and committing a rename re-navigates to the new
  href so the tab you are looking at re-titles itself
  (`app/src/components/projects/taskHref.ts`).
- **Until that page there was no way to set a due date in the app at all** —
  only `oculus task update --due`. Both editors of one go through
  `app/src/components/projects/DateTimeField.tsx`: a button showing what is
  set, over a popover holding shadcn's `Calendar` and one `type="time"` input.
  It was an `<input type="datetime-local">` first, which is the obvious answer
  and the wrong one — the root `CLAUDE.md`'s UI conventions record why native
  date inputs are not usable here. What survives from that version is the
  arithmetic: a `Date` built from local parts *is* the instant the user meant,
  and `toISOString()` is the only place a zone is applied, because the tempting
  `toISOString().slice(0, 16)` renders *UTC* into a local field — an 11pm
  Melbourne deadline reads back as midday and saving it moves the date. Picking
  a day keeps whatever clock is already set and otherwise defaults it: end of
  day for a due date, morning for a start, since picking "the 20th" for a
  deadline means the 20th and not one minute past midnight on it.
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
  the project. `taskTree.ts` does the shaping for every view, so a row
  whose parent is missing is promoted to top level rather than dropped.
- **The top strip is `Overview | Tasks`, and the three task views nest under
  it.** They were four siblings — board, table, backlog, timeline — and that
  strip read as four unrelated buttons, because it was mixing a question
  ("what is this project?") with three answers to a different one ("what is
  left?"). So `ViewTabs` (the Sync page's rule) now carries only the two, and
  Board / Table / Timeline get a third row of their own below the toolbar, as a
  deliberately quieter strip: smaller, no indigo underline, the active one
  marked by a fill. Two identical underline strips stacked would read as two
  peers and fight for the same rule, and sharing the toolbar with the project's
  name — where they started — made them read as two more breadcrumbs that a
  long name could shove along the row. The toolbar's fixed height still holds
  where it matters: the three views all sit under the same two rows, so moving
  between them cannot jolt the work below.
- **The trail is links, not decoration**
  (`app/src/components/projects/ProjectCrumbs.tsx`). The project page and a
  task page both opened with the subject, a slash, and the thing you were
  looking at, and no segment went anywhere — so a project reached from Home or
  from a task had no way back to the list it belongs to except the sidebar. The
  trail now starts at **Projects** and the subject leads to that subject's own
  projects tab, which are the two lists the page could have been opened from.
  Personal stays plain text: its only list is the one `Projects` already points
  at. The component is a Fragment rather than a wrapper so it drops into each
  page's existing crumb row and inherits that row's gap — the project page's
  toolbar is `gap-2.5`, the task page's line is `gap-1.5`, and both keep the
  spacing they had.
- **The Backlog view is gone, and the board's drag is why.** It was the same
  pile read as a list with a promote button per stub, which earned its keep
  only while dragging a card out of the backlog column did not work — see the
  WebKit bullet below. Once it did, the list was a second screen for making a
  move the board already makes by the gesture a kanban board exists for.
  `oculus-project-view` still holds `"backlog"` in the localStorage of anyone
  who used it, so `isView` in `app/src/pages/ProjectPage.tsx` no longer accepts
  that string and the stored value falls through to the board — an
  unrecognised view would otherwise render nothing at all, on the machine of
  whoever used the feature most.
- **A card could not be dragged at all, for a one-line reason worth
  remembering.** WebKit — which is what a Tauri WKWebView is — **aborts a drag
  whose `dragstart` handler sets no data on the `dataTransfer`**: no `dragover`
  and no `drop` ever fire, silently, with every handler correctly attached. So
  `ProjectBoard` and `app/src/components/llm/FallbackList.tsx` each call
  `e.dataTransfer.setData("text/plain", …)` purely to keep the drag alive; the
  payload is never read, because the dragged id is already in React state.
  Both carry a comment saying so, because the call looks like dead code and
  deleting it breaks the feature without breaking a type or a test. The timeline packs
  overlapping bars with `packLanes` in `app/src/lib/lanes.ts` — lifted out of
  `app/src/components/calendar/WeekView.tsx`, which was its only caller until
  this needed the same packing, and made generic over the item because the two
  measure a span differently (a class in epoch milliseconds, a task bar in
  pixels) and the packing never needs to know which.
- **The index draws a group only once it has something in it**, plus Personal,
  which is always offered because it is where a subject-less project goes and
  it cannot be discovered otherwise. That keeps a term's worth of empty
  headings off the page, but it also means a group's own inline composer can
  only ever add to a subject that already has projects — so the page carries a
  second door beside the title
  (`app/src/components/projects/NewProjectButton.tsx`), where the group is a
  field you fill in rather than a heading you have to find first. It is how a
  subject's *first* project gets started from the index at all; the subject's
  own Projects tab is the other way in, and needs no picker.

  That picker's list is **Personal and this term's subjects, with past terms
  folded behind a `Past subjects (n)` disclosure** — the Sync page's picker
  behind the same caret and the same words, so it reads as one idea rather than
  two. Past subjects stay reachable rather than being dropped the way the chat
  scope picker drops them (`SubjectSelect` lists only current subjects plus
  whatever the thread already points at), because a project can outlive the
  term it was set in. The fold is forced open whenever the selection is inside
  it, or creating a project for a past subject and reopening the picker would
  show a tick nowhere. The index itself is deliberately *not* folded this way:
  a past subject with live projects is still work you have on.
- **The calendar reads tasks live rather than copying rows.** See
  [calendar.md](./calendar.md) — a task re-dated, finished or deleted on its
  board would otherwise leave a row on the grid that nothing cleans up, since
  `local_events` has no cleanup pass. That is also why a task is not deletable
  from the calendar: its card links through to the project instead.
- **A project points back at one calendar event, and that pointer is resolved
  live for the same reason.** An assignment's project answers to a deadline
  Canvas already published, so `event_id` holds a `CalEvent.id` as
  `app/src/lib/calendar.ts` mints it — which means one column addresses three
  tables, because what is pinned is the thing on the grid rather than a row in
  any one of them. It is deliberately **not** a foreign key: a sync deletes a
  subject's `calendar_events` rows and re-inserts them with identical ids, so a
  `REFERENCES … ON DELETE SET NULL` would clear every pin in the app halfway
  through the next sync. `EventLink` resolves it against `loadCalendar()`
  instead, loading nothing at all until there is a pin to resolve or an open
  picker — and a pin that stops resolving (the assignment unpublished, the local
  row deleted) says so and offers to clear itself, because a link the user set,
  cannot see and cannot remove is worse than a broken one they can. `task`
  events are kept out of the picker: those *are* this project's own rows read
  back onto the grid, so pinning to one would be a loop.
- **Archiving is reversible and visible; deleting is neither.** The index reads
  `status: "all"` and splits the list itself — one store holds one list and one
  set of counts, so a second query would overwrite the first — with the
  archived rows in a collapsed section at the bottom, drawn only once there is
  something in it. That is the page's standing rule (Personal is the one
  deliberate exception, since it is the only way to discover where a
  subject-less project goes), and it is why archived projects are no longer
  reachable by their link alone. Its collapse key stores the *open* state,
  inverting `COLLAPSED_KEY` beside it, because the wanted default here is shut.
  A subject's Projects tab stays on active only: it is where you work, the
  index is where you keep the record.
- **Rename has two doors on purpose.** `ProjectMenu` carries a dialog, because
  a list row is a `Link` and turning it into a field would stop it being one;
  the project's own header edits its name in place, because that is the thing
  you are already looking at. Both land on `updateProject(id, { name })`
  through `useProjectActions`, which is one hook rather than three copies of
  the same four store calls — the kind of duplication that stays correct right
  up until one page grows a confirmation the others do not have. The header's
  menu navigates back to the index after an archive or a delete, since a board
  whose project the list no longer carries has nothing left to say, and stays
  put after an unarchive.
- **A project's name travels in its route's query** (`?n=`, `projectHref` in
  `app/src/components/projects/projectHref.ts`), because `tabInfo` titles a tab
  from the path alone and has no project list to look one up in — the same
  trade the lecture route makes with `?t=`.
