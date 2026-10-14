# Projects

A project is a piece of work — usually an assignment — scoped to one subject
or none, broken into tasks with one level of subtask. It opens on an
**Overview**, reads its tasks as a board, table or timeline, and gives each
task a page. `/tasks` is every task at once, including those filed under no
project. Nothing here is scraped: every row is the student's own planning or a
plan the chat agent wrote through the `oculus` CLI, so nothing upstream can
repair a bad row.

## Where

| Piece | Location |
| --- | --- |
| Tables: `projects`, `project_tasks` (27), tags + `event_id` (33), nullable `project_id` (37), `task_tags` + `task_tag_links` + `task_files` (47) | `app/src-tauri/src/db/migrations.rs`, `UNFILED_TASKS_SQL` in `app/src-tauri/src/db/projects/migration37.rs` |
| Frontend reads and writes (direct SQL) | `app/src/lib/planning/projects/` (`reads.ts`, `taskWrites.ts`, `projectWrites.ts`, `columns.ts`, `rows.ts`, `events.ts`, `tags.ts`, `taskFiles.ts`, `touch.ts`) |
| Headless writes (CLI, and so the agent) | `app/src-tauri/src/db/projects/` (`tasks/`, `tags.rs`, `tasks/links.rs`, `project_writes.rs`, `columns.rs`, `rows.rs`) |
| `oculus project` / `oculus task` / `oculus tag` | `app/src-tauri/src/bin/oculus/commands/planning/`, `app/src-tauri/src/bin/oculus/args/planning.rs` |
| One open project and its tasks | `app/src/stores/planning/projectsStore.ts` |
| Index, one project, one task, every task | `app/src/pages/work/ProjectsIndexPage.tsx`, `app/src/pages/work/ProjectPage.tsx`, `app/src/pages/work/TaskPage.tsx`, `app/src/pages/work/TasksPage.tsx` |
| A task page's parts | `app/src/components/projects/task/` (header, property pills, tags, related files, footer), `app/src/pages/work/task/` (title, body, checklist, mutations, data) |
| Index subject groups and collapsed preferences (shared with chat) | `app/src/lib/ui/subjectGroups.ts`, `app/src/hooks/shell/useCollapsedGroups.ts` |
| A subject's Projects tab | `app/src/pages/work/ProjectsPage.tsx` |
| The section's nav column and its fold | `app/src/layouts/TasksLayout.tsx`, `app/src/components/projects/nav/TasksNav.tsx`, `app/src/components/projects/nav/TagsGroup.tsx`, `app/src/components/projects/nav/panel.ts` |
| The page header row; the rail item | `app/src/components/projects/page/SectionHeader.tsx`, `app/src/components/sidebar/Sidebar.tsx` |
| Overview, project tags, pinned calendar event | `app/src/components/projects/page/ProjectOverview.tsx`, `app/src/components/projects/fields/TagEditor.tsx`, `app/src/components/projects/page/EventLink.tsx` |
| Task tags: registry hook, chips, picker, palette | `app/src/hooks/data/useTaskTags.ts`, `app/src/components/projects/tags/`, `--color-tag-*` in `app/src/styles/colors.css` |
| Board, table, timeline, and the shaping behind them | `app/src/components/projects/board/ProjectBoard.tsx`, `app/src/components/projects/lists/ProjectTable.tsx`, `app/src/components/projects/timeline/ProjectTimeline.tsx`, `app/src/components/projects/tasks/taskTree.ts` |
| The universal view: hook, views, filters, sort, groups, shared rules | `app/src/hooks/data/useTaskList.ts`, `app/src/components/projects/tasks/` (`taskViews.ts`, `filterRules.ts`, `taskSort.ts`, `taskGroups.ts`, `universalTasks.ts`, `TaskFilters.tsx`) |
| The universal table, its columns and the Display menu | `app/src/components/projects/lists/` (`TasksTable.tsx`, `TaskRow.tsx`, `TaskCells.tsx`, `tasksColumns.ts`), `app/src/components/projects/display/`, `app/src/components/projects/board/TasksBoard.tsx` |
| Card drag and board chrome | `app/src/hooks/gestures/useCardDrag.ts` (on `app/src/hooks/gestures/usePointerDrag.ts`), `app/src/components/projects/board/BoardParts.tsx` |
| Subject chips, subtask expansion, status and progress marks | `app/src/components/projects/tasks/TaskMarks.tsx` |
| A card's title | `app/src/components/projects/board/CardTitle.tsx` |
| Filing a task; creating one | `app/src/components/projects/nav/ProjectPicker.tsx`, `app/src/components/projects/nav/NewTaskButton.tsx` |
| Rename / archive / delete | `app/src/components/projects/lists/ProjectMenu.tsx`, `app/src/components/projects/lists/useProjectActions.ts` |
| Routes and the breadcrumb trail | `app/src/components/projects/nav/projectHref.ts`, `app/src/components/projects/nav/taskHref.ts`, `app/src/components/projects/page/ProjectCrumbs.tsx` |
| Date and time picking | `app/src/components/projects/fields/DateTimeField.tsx` |
| Overlap packing, shared with the calendar | `app/src/lib/planning/lanes.ts` |
| The agent's instructions | `app/src-tauri/templates/AGENTS.template.md`, `app/src-tauri/templates/HARNESS.template.md` |

## The schema assumes the student owns every row

- **`subject_id` is nullable and clears rather than cascades** (NULL is
  "Personal"): dropping a course must not take the student's planning with
  it. Project → tasks and task → subtask *do* cascade.
- **A board's columns are JSON on the project.** They are renamed per project,
  so the app reasons only about a column's `kind` (`backlog` | `active` |
  `done`), never its name or id.
- **`position` is `REAL`**, so a drop writes one row at the midpoint of its
  neighbours; both writers renumber the column to whole numbers when the gap
  underflows.
- **`source` is `'manual'` or `'agent'`**; everything the CLI writes is `agent`.
- **A project's tags are JSON on the project** (`NOT NULL DEFAULT '[]'`),
  normalised on the way in by both writers (`normaliseTags` / `normalise_tags`:
  trim, collapse whitespace, dedupe case-insensitively, cap 24), so "used
  before?" is a plain string compare everywhere else.
- **A task's tags are a registry, not JSON**: `task_tags` (name unique ignoring
  case, a palette *key* for colour — never a hex value, so the palette can be
  retuned) and `task_tag_links` (cascades from both sides). Renaming,
  recolouring or deleting a tag is one row and reaches every task, which a name
  copied into each task cannot do. The palette (`slate`, `indigo`, `green`,
  `amber`, `rose`, `purple`, `teal`, `orange`) is validated by both writers;
  the tag tokens are in `colors.css` and are not theme-derived. Migration 47
  seeds Assignment, Exam, Reading, Study and Admin.
  - **Creating by name is idempotent**: a name matching a tag ignoring case
    returns that tag. The CLI makes unknown names on the fly (`slate`); a
    picker's "Create" never fails on a tag typed twice.
  - **A tag set is replaced, not patched**: new links go in before the rest come
    out, so a failure never leaves fewer tags (the TS side has no transaction).
    Every read returns `tag_ids` through one column expression and one mapper
    (`TASK_TAG_IDS_SQL`, `toTask`), in registry order.
- **A task's related files are `task_files`**, keyed by `files.id`. That id
  survives a re-sync (the upsert keeps it), so the cascade FK is safe, unlike
  `event_id`. The CLI names a file by `relative_path` and refuses a path that
  matches no file, or several subjects' files.
- **Tag and file link writes bump the task's and its project's `updated_at`**
  (`touchTask` / `touch_task`); the Tasks table's Updated column reads it.
- **`project_tasks.project_id` is nullable**: NULL is a task filed under no
  project — not an "Inbox" project the user could rename or delete.
  `column_id` stays NOT NULL and holds a **default board** id; every reader
  and writer reaches a board through `boardOf` / `board_of`, which returns the
  project's columns or the defaults.
- **`event_id` is a pointer, not a foreign key.** It holds a `CalEvent.id` as
  `app/src/lib/planning/calendar/` mints it, so one column addresses three tables. A
  sync deletes and re-inserts a subject's `calendar_events` with the same ids,
  so an `ON DELETE SET NULL` would clear every pin on each sync. `EventLink`
  resolves it live, offers to clear a pin that stops resolving, and keeps
  `task` events out of the picker (pinning a project to its own task is a
  loop).

**The migration-37 rebuild points `parent_id` at the new table.** SQLite cannot
ALTER a NOT NULL away, so the table is rebuilt. With foreign keys on, `DROP
TABLE` deletes every row first, so a `parent_id` still referencing the old
`project_tasks` would cascade-empty the copy just made; `defer_foreign_keys`
defers the check, not the action. The SQL is `UNFILED_TASKS_SQL` rather than
inline in the migration so its test runs the exact string the app runs.

## Two writers enforce the same rules

The frontend writes over `getDb()` in `app/src/lib/planning/projects/` (no Tauri
command — nothing here needs the network or a subprocess); headless,
`app/src-tauri/src/db/projects/` writes the same rows with the same rules. Change
a table and both move with the migration. Neither creates the database.

- **`moveTask` / `move_task` is the only writer of `column_id`, `position` and
  `done_at`** — one fact in three columns. `done_at` is derived from the
  destination column's `kind`, including on create. `updateTask` cannot touch
  them. A second writer would let `done_at` disagree with the column, and the
  calendar, which filters on `done_at`, would keep drawing a finished task.
- **A column id is checked against the task's board** and an unknown one is
  refused with the ids the board has: a task in a column no view renders is
  invisible, and `--column` is free text an agent typed.
- **Subtasks are one level deep, enforced in code** in both directions
  (`assertCanParent` / `assert_can_parent`): views draw a task and its
  children, so a grandchild would never be drawn.
- **A breakdown is one transaction.** `oculus task add -p <ID> --batch -`
  takes a JSON array whose items may name a parent by task id or by the `key`
  of an earlier item (never stored). One rejected item rolls back the batch;
  the single-task path is the same function (`create_tasks`).
- **`refileTask` / `refile_task` is the only writer of `project_id` after
  create** — see [the universal view](#the-universal-view).

## Every write refreshes through one window event

Each write in `app/src/lib/planning/projects/` fires `PROJECTS_UPDATED_EVENT`, and
components showing project data reload on it. The store's write wrappers
deliberately do not re-read: a second refresh path doubled a drag's reads and
could land out of order.

The agent writes from a subprocess nothing in the webview notices, so
`app/src/hooks/backend/useBackendEvents.ts` watches the harness stream and fires the
same event when a tool call whose **command text** contains `oculus project` /
`oculus task` finishes. It matches text rather than the tool's kind because
`is_oculus_cli` in `app/src-tauri/src/harness/event/classify.rs` only checks the first
words, and `cd … && oculus task add` classifies as plain Bash. A false
positive costs one re-read; a false negative is a silently wrong board.

## `position` is the only order a query applies

`getTasks` returns a project's parents and subtasks in one query ordered by
`position` alone — a `column_id` in the ORDER BY would sort the columns
alphabetically, which is not the board's order. The board's order is the
project's `columns` array. `taskTree.ts` shapes every view and promotes a row
whose parent is missing rather than dropping it.

## Pages and routes carry what a tab title needs

- **A task is a page**, at `/projects/:projectId/tasks/:taskId` — nested
  because a status is a column on *that* board — or `/tasks/:taskId` when
  unfiled. `taskHref` picks the route; `TaskPage.tsx` serves both, with
  `boardOf`'s default board and `useTaskList` rows for an unfiled task.
- **The task page is a header, a pill row and four blocks.** The header holds
  the nav fold toggle (only inside `TasksLayout`; `useTasksPanel()` is null in
  the side panel), back, the trail `Tasks › title` and a `…` menu with Delete.
  The pills are Status, Project (read-only "follows parent" on a subtask), Due,
  Starts, Estimate (whole minutes in a popover; 0 clears), the task's tags with
  a `+` (`TagPicker`, writing `updateTask({ tagIds })`) and, on a parent, the
  checklist progress. Below: the description, the subtasks as a checklist (a tick is
  `moveTask`; a subtask has none), **Related files** and a created/updated
  footer. There is no comment box.
- **Related files open through `openLibraryPath`**, the opener every citation
  takes, so a PDF or note lands in the side panel and a binary in the system
  viewer; ⌘-click opens a tab. A chip is a container with two sibling buttons
  (open, ×), never nested ([ui.md](./ui.md#gotchas)). The attach picker is
  `searchAttachableFiles`, ranked like ⌘K, with recents on an empty query.
- **Names ride in the query** (`?n=`, via `projectHref` / `taskHref`), because
  `tabInfo` titles a tab from the path alone. Renaming re-navigates to the new
  href so the tab re-titles.
- **The task body is markdown in the note editor**, through `NoteField`
  (`app/src/components/documents/NoteField.tsx`; see
  [editor.md](./editor.md#notefield-is-the-note-editor-as-a-form-field)):
  always Live mode, so the preview is the read view, with the note's maths
  tools, tables and shortcuts, and its formatting toolbar under the text
  while it is edited. `@` searches the project's subject, or the whole
  library for a task without one. A picture pasted, dropped or picked is
  written to `agents/attachments/` on arrival (`writeAttachment`) and linked
  as `![name](agents/attachments/…)`. Blur or ⌘↵ writes through
  `updateTask`; `taskBodyEdit` makes an unchanged body write nothing and an
  empty one write `null`. The page keys the body by task, so switching tasks
  saves to the task left. A write from elsewhere (the agent through the CLI)
  replaces the text while the field is at rest; while it is edited, the
  user's text stands.
- **Dates go through `DateTimeField`**, never a native input
  ([ui.md](./ui.md#gotchas)). A `Date` built from local parts is
  the instant meant and `toISOString()` is the only zone conversion. Picking a
  day keeps the set time, else defaults to 23:59 for a due date and the
  morning for a start. `variant="pill"` draws it as a property pill.
- **The project page's strip is `Overview | Tasks`**, with Board / Table /
  Timeline as a quieter third row below the toolbar. `isView` in
  `ProjectPage.tsx` sends an unknown stored `oculus-project-view` to the
  board.
- **`TasksLayout` wraps every route in the section** with a resizable, foldable
  `SideNav` column (`TasksNav`; width and fold under `oculus-tasks-nav-width`,
  ⌘⌥B). Its rows navigate rather than swap state, so both pages keep history,
  ⌘-click and tab restore. The rail has one item, **Tasks**, opening `/tasks`
  and lit on `/projects*` too. `SectionHeader` is just the page's header row,
  with the fold toggle at its start.
- **`ProjectCrumbs`** starts at Projects and links the subject to its Projects
  tab. Segments are buttons with `data-tab-href` (`app/src/lib/shell/newTabClicks.ts`)
  so ⌘-click opens a tab; it is a Fragment so it inherits each page's gap.
- **The index** draws a subject group only once it has a project, plus
  Personal always; `NewProjectButton` is the door for a subject's first
  project. Its picker folds past-term subjects behind `Past subjects (n)`,
  forced open when the selection is inside. Archived projects sit in a
  collapsed section at the bottom; a subject's Projects tab shows active only.
- **Rename has two doors** — `ProjectMenu`'s dialog (a list row is a `Link`)
  and the header's in-place edit — both through `useProjectActions`.
- **The timeline** packs overlapping bars with `packLanes`, shared with
  `app/src/components/calendar/WeekView.tsx` and generic over how a span is
  measured. Range and axis geometry follow the calendar day rather than each
  minute; row lane layouts are cached by their placed items, while the current
  time line, elapsed wash and overdue colours still refresh each minute.

## Cards drag on pointer events and settle before swapping

Boards and the project table drag with `useCardDrag` on the shared
`usePointerDrag` gesture, not HTML5 drag-and-drop: in WebKit a text selection
starting on a card's `<span>` pre-empts the element drag. The WebKit
rules are in [ui.md](./ui.md#gotchas) — never cancel `pointerdown` (it kills the
click into the task), cancel `pointermove` instead.

**A drop is not the end of the move.** The new order arrives from a SQLite
re-read a moment later, so releasing opens a **settle** phase: the card glides
to its gap, the view keeps drawing the list it had at release
(`useSettledList`), and the two swap only when the glide has run *and* the new
list (`settleOn`) has arrived. A drop handler must report whether it wrote;
a refused drop springs back instead of waiting for an order that never comes.
Row numbers fade for the gesture.

**A card is fixed-size and the whole of it opens the task.** `CardTitle`
wraps unbreakable tokens (`break-words` in a `min-w-0` child), clamps to three
lines, and offers **Show more** only when a `ResizeObserver` measures the
clamp hiding something. The card is a plain `<article>` that navigates on
click, not an anchor or button, because it contains controls; it carries
`data-tab-href` for ⌘-click, and the toggle carries `data-tab-skip`.

## The universal view

`/tasks` shows every task, filed or not, as a flat table or a four-column board
(`app/src/pages/work/TasksPage.tsx`), beside `TasksNav`. What it shows is three
independent layers, all predicates and orderings over one `getAllTasks`.

- **The nav column picks a view and a tag, as URL state** —
  `/tasks?view=all|agent&tag=<id>` (`taskViews.ts`). The two compose
  and each link keeps the other; an unknown value, or a tag id missing from
  the registry, means none. They are real links so ⌘-click and restored tabs carry
  both. Off `/tasks` neither is selected.
- **Search and Filter narrow further.** Search matches title, project name and
  tag names. The Filter popover holds status, project, subject, due and tags
  (`filterRules.ts`); it starts empty and nothing in it persists.
- **Display shapes the result** (`components/projects/display/`): view, grouping
  (none, status, project, tag, due), sort, show completed and which table
  properties show, in what order — one JSON object under `oculus-tasks-display-v2`,
  validated field by field on read, so a property added later starts hidden and
  an unknown stored one is dropped. The default is due date ascending with
  finished tasks hidden; a stored object overrides every default, so changing
  one means bumping the key. Grouping and properties are table-only; a
  task with several tags appears under each tag group. Name is always first and
  cannot be hidden. A column header sets the same sort, so there is one source
  of truth.

- **It does not use `projectsStore`**, which holds one open project.
  `useTaskList` reads `getAllTasks` plus the project list (a column id means
  nothing without its board) and reloads on `PROJECTS_UPDATED_EVENT`.
  `getUnfiledTasks` resolves an unfiled task's page.
- **Its columns are `DEFAULT_COLUMNS`** (Backlog / Todo / In progress / Done),
  aliased as `UNIVERSAL_COLUMNS`.
  - `universalColumnOf` places a card **id first, kind second**: its own
    column if that is a default id, else the kind's home (`active` → In
    progress); a column missing from its board falls to Backlog.
  - `columnForUniversal` maps a drop onto column X to the task's own board:
    the column with id X, else the first of X's kind, else nothing. A drop
    resolving to the current column writes nothing.
- **A column's look is its variant** (`columnVariant.ts`: backlog, todo, doing,
  done): backlog and done by kind, a default `todo` / `doing` id by id, any
  other active column by rank (the last is in flight, earlier ones queued).
  `TaskGlyph` (dashed circle, empty, half-filled brand, check) and `StatusPill`'s
  tint, including the picker's row glyphs, both read it, so they cannot disagree.
- **Unfiled is a value of the project filter**, and filtering is a predicate
  over one `getAllTasks`, not a query per question.
  - The status set decides which columns the board draws, and is never empty.
  - Due compares through `sqliteUtcToMs`: `due_at` is ISO from the UI and
    `YYYY-MM-DD HH:MM:SS` from the CLI, and string order differs. *This week*
    is the calendar's `startOfWeek` ([calendar.md](./calendar.md)); *Overdue*
    is unfinished and past due, the same as the red on `DueChip`.
- **The table's grid is built at run time** (`tasksColumns.ts`) and handed to
  `GridTable` as `colsStyle`, because Tailwind cannot scan class names made from
  the shown properties. Cells edit inline — `StatusPill`, `TagPicker`,
  `ProjectPicker` (a subtask's Project stays text) — and it has no pagination.
  Updated and Created read SQLite's naive UTC through `sqliteUtcToMs`.
- **There is no manual order here.** `position` is a slot inside one project's
  column, so positions across projects are unrelated numbers. Rows and cards
  sort by the Display sort, whose default (due, nulls last; then project, unfiled
  first; then `position`) mirrors `UNIVERSAL_ORDER`; a same-column drag is a
  no-op, and a move to another column appends after the last card of *that
  task's own project* (`appendNeighbour`).
- **⌘K finds unfiled tasks** because `searchTasks` LEFT JOINs the project and
  `COALESCE`s its name — SQLite's `||` yields NULL if either side is NULL.
- **Refiling** (`refileTask` / `refile_task` / `oculus task refile`, from
  `ProjectPicker` in the table and on the task page):
  - maps the column by **kind** into the destination's first column of that
    kind, refusing a board with none — two boards share only what a column
    means;
  - carries the task's subtasks, each by its own kind; refiling a subtask
    alone is refused;
  - appends at the destination column's end, then places via `moveTask`.

## Boards share rendering while each view owns placement

`BoardView` in `app/src/components/projects/board/BoardParts.tsx` composes columns,
draggable cards and the lifted copy. Project and universal boards supply the
card body, ordered rows and drop policy; parent/sibling placement stays in
`app/src/components/projects/tasks/taskTree.ts`. Tables and timelines share the
subtask disclosure control in `app/src/components/projects/tasks/TaskMarks.tsx`.

Project and task patches bind values through `app/src/lib/planning/sqlPatch.ts`.
`undefined` leaves a field alone, `null` clears it, and an empty patch neither
updates timestamps nor announces a write. Domain validation and refresh events
remain in `app/src/lib/planning/projects/`.

## Gotchas

- A table rebuild with a self-referencing FK must reference the *new* table, or `DROP TABLE` cascade-empties the copy — see `UNFILED_TASKS_SQL`.
- Only `moveTask` / `move_task` writes `column_id`, `position` or `done_at`; any other writer lets the calendar show finished tasks.
- Never add `column_id` to `getTasks`' ORDER BY; it sorts columns alphabetically.
- Never make `event_id` a foreign key; every sync would clear every pin.
- Never compare `due_at` strings; the UI's ISO `T` and the CLI's space sort differently.
- `toISOString().slice(0, 16)` renders UTC into a local field and shifts the date.
- A card's `pointerdown` must not be cancelled, or no card opens its task.
- Never order across projects by `position`; the numbers are per-column.
- Keep the harness hook matching command text (`oculus project|task|tag`), not tool kind, or chained `cd … && oculus task` writes never refresh the board.
- Store a tag's colour as a palette key, never a hex value; both writers refuse an unknown key.
- Never nest a removable `TagChip` (it renders a button) inside a button or a row that is one.
- Never build the universal table's grid from class names assembled at run time; pass `colsStyle`.
