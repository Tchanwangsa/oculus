# Projects

A project is a piece of work you are doing — usually an assignment — scoped to
one subject or to none, broken into tasks and one level of subtask. It opens on
an **Overview** — what it is, the facts pinned to it, what is next — and its
tasks are read three ways behind a **Tasks** tab: a board, a table or a
timeline. Each task is a page of its own as well. There is also a view over
*every* project at once, at `/tasks`, which is where the tasks that belong to
**no** project live — see [the universal view](#the-universal-view) below.

The two are **one section** in the shell, with **one sidebar row, labelled
Tasks**, leading to `/projects`: they were always the same rows read two ways,
and the split was noise. Projects is the landing tab — today's index, where a
project and a task are created — and Tasks is the second. The strip between
them is `app/src/components/projects/SectionHeader.tsx`, and it *navigates*
rather than swapping state, so both URLs survive with their history, their
⌘-click and their restored tabs intact.
Nothing here is
scraped: every row is the student's own planning, or a plan the chat agent
wrote for them through the `oculus` CLI. That is what shapes most of the
decisions below — nothing upstream has a copy, so nothing can be repaired by
syncing again.

## Where

| Piece | Location |
| --- | --- |
| `projects` + `project_tasks` tables (migration 27) | `app/src-tauri/src/lib.rs` |
| `projects.tags` + `projects.event_id` (migration 33) | `app/src-tauri/src/lib.rs` |
| Nullable `project_tasks.project_id` (migration 37) | `app/src-tauri/src/lib.rs`, `UNFILED_TASKS_SQL` in `app/src-tauri/src/projects.rs` |
| Frontend reads and writes | `app/src/lib/projects.ts` |
| What is on screen | `app/src/stores/projectsStore.ts` |
| Headless writes (CLI, and so the agent) | `app/src-tauri/src/projects.rs` |
| `oculus project` / `oculus task` | `app/src-tauri/src/bin/oculus.rs` |
| Index, one project, one task | `app/src/pages/ProjectsIndexPage.tsx`, `app/src/pages/ProjectPage.tsx`, `app/src/pages/TaskPage.tsx` |
| Every task at once, filed or not | `app/src/pages/TasksPage.tsx`, `app/src/hooks/useTaskList.ts` |
| The section's two tabs, and the row that leads to them | `app/src/components/projects/SectionHeader.tsx`, `app/src/components/sidebar/Sidebar.tsx`, `app/src/components/sidebar/NavItem.tsx` |
| The universal view's four filters | `app/src/components/projects/TaskFilters.tsx` |
| Its two views, and the rules they share | `app/src/components/projects/TasksBoard.tsx`, `app/src/components/projects/TasksTable.tsx`, `app/src/components/projects/universalTasks.ts` |
| Where a task is filed: the picker, and the composer over it | `app/src/components/projects/ProjectPicker.tsx`, `app/src/components/projects/NewTaskButton.tsx` |
| The drag every board and table shares | `app/src/hooks/useCardDrag.ts` |
| A board card's title: the fold, and the click into the task | `app/src/components/projects/CardTitle.tsx` |
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
- **`project_tasks.project_id` is nullable** (migration 37), and NULL is a task
  that belongs to no project *at all* — not an "Inbox" project, which would be
  a row the user could rename, archive or delete out from under the feature.
  `column_id` stays NOT NULL, so such a task still names a column something can
  draw: the ids of the **default board**, which are also the ids a new project
  is born with, so filing it into one later needs no translation. Every reader
  and writer reaches a board through one helper — `boardOf` in
  `app/src/lib/projects.ts`, `board_of` in `app/src-tauri/src/projects.rs` —
  which hands back the project's columns or those defaults, so "no project" is
  a *checked* case rather than a waiver.

  Making it nullable meant rebuilding the table (SQLite cannot ALTER a NOT NULL
  away), and the rebuild has a trap worth knowing: the new table's `parent_id`
  must self-reference **the new table**, not the `project_tasks` it is about to
  replace. Pointed at the old one, the recipe's `DROP TABLE` fires that
  clause's `ON DELETE CASCADE` and empties the copy it has just made — a drop
  performs an implicit delete of every row while foreign keys are on, and
  `defer_foreign_keys` defers the *check*, not the action. The SQL lives in
  `UNFILED_TASKS_SQL` in `app/src-tauri/src/projects.rs` rather than inline in
  the migration, so the test that catches this runs the exact string the app
  runs.

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
- **The body is markdown, and it can name a course file.** It was a plain
  textarea on the argument that a task body is a paragraph and two reminders;
  it is not — it is where the plan for one piece of work is written, so it
  holds a checklist, a formula and the files the work is *about*. So the page
  renders it through `CompactMd`, the chat timeline's own renderer
  (`app/src/components/markdown/MdComponents.tsx`), and clicking it opens an
  editor, the grammar the title one line above already uses. **Nothing was
  written to render a mention**: the renderer already draws a backticked
  library path as a clickable `FileChip` and a library-relative image as the
  picture, so `@` needed only an *editor* — the composer's own box and the
  shared `@` menu (`app/src/components/harness/MentionInput.tsx`,
  `useMentionMenu.ts`, `docs/harness.md`), scoped to the project's subject
  where there is one and to the whole library otherwise, since an unfiled task
  has no project at all. Pasting or dropping a picture embeds it, which is the
  one place a picture is written before a send because there is no send
  (`docs/harness.md`). Saving is the textarea's rules unchanged: ⌘↵ writes
  without leaving, blur writes, an unchanged body writes nothing, an empty one
  writes `null` — and the body is `updateTask`, never `moveTask`.
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
  nothing in the webview's pool notices. That subprocess is sandboxed, and
  until its sandbox was given the database's three files by name it could not
  write at all: SQLite answered every batch with "attempt to write a readonly
  database" because `oculus.db-wal` was out of reach. See the containment
  bullets in [harness.md](./harness.md). So `app/src/hooks/useBackendEvents.ts`
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
  spacing they had. The segments are buttons carrying a
  `data-tab-href` (`app/src/lib/newTabClicks.ts`), not `Link`s — a plain click
  follows the shell's departure rules at `navigateActive`
  (`app/src/lib/tabRouters.ts`), and the attribute is what gives ⌘-click the
  crumb in its own tab; the task page's project crumb is built the same way.
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
- **The board's drag is a pointer gesture, not HTML5 drag-and-drop, and the
  reason is worth keeping.** It was HTML5 DnD, and it worked *sometimes*: a
  press that landed on a card's due chip or subtask counter — both `<span>`,
  both handed `user-select: text` back inside `app/src/index.css`'s
  `body { user-select: none }` — started a **text selection**, and in WebKit a
  selection pre-empts the element drag, so `dragstart` never fired at all. From
  the outside the card moved when you grabbed dead space and did nothing when
  you grabbed a chip. Rather than patch the selection rule, the board, the
  project table and the universal board all moved to the tab strip's gesture:
  `useCardDrag` in `app/src/hooks/useCardDrag.ts` — a 4px threshold,
  `setPointerCapture`, rects measured once at the lift, the card riding
  `translate` under the pointer, neighbours transitioning out of its way, and
  the store written once on drop. It is what the user asked the drag to look
  like, and it is immune to the selection problem by construction.

  Two edges of that gesture are load-bearing. The press must **not** be
  cancelled: in WebKit a `click` is raised from a `mousedown`/`mouseup` pair,
  so a `preventDefault()` on `pointerdown` — the obvious way to stop the
  selection — also removes the only way into a task's page. The selection is
  held off by CSS on the drag surface instead, and a `preventDefault()` on
  `pointermove`, which suppresses the compatibility `mousemove` a selection is
  *extended* by while leaving the click alone. The one HTML5 drag left in the
  app is the dock's tab reorder (`app/src/components/ui/ViewTabs.tsx`), which
  still needs its `dataTransfer.setData()` — see the root `CLAUDE.md`.

  **A drop is not the end of the move, and treating it as one played the move
  backwards.** The write goes to SQLite and the new order comes back as a
  re-read tens of milliseconds later, so a gesture that ended on the release
  took every transform off while the list was still in its old order: the item
  snapped back into the slot it had been lifted out of, sat there, and then
  jumped to its new one when the re-read landed — with the table's row numbers
  all renumbering in that same frame, which is what made it read as a flicker
  rather than as a move. So the release opens a third phase, the **settle**:
  the item glides to the offset its neighbours have been holding open, the
  view keeps drawing the list it had when the pointer went up
  (`useSettledList`), and the two are swapped only once the glide has run *and*
  the new order has arrived — at which point they are the same picture and the
  swap cannot be seen. That needs two things from a caller: the list identity
  to watch (`settleOn`), and an honest answer from its drop handler about
  whether it wrote anything, since a refused drop has no new order coming and
  must spring back instead of hanging over a slot nothing will fill. The row
  numbers fade for the length of the gesture and fade back in already
  renumbered, because a position in a list being rearranged is not a fact worth
  showing mid-flight.
- **A card is fixed-size furniture, and the whole of it opens the task.** A
  title is one string the user pasted, so it is capable of being a single
  unbreakable token — a tracking URL — which painted straight out through the
  card's right edge and across the column beside it, or of being long enough to
  make one card taller than the column. `app/src/components/projects/CardTitle.tsx`
  is the one definition both boards draw it with: `break-words` inside a
  `min-w-0` flex child so an unbreakable token wraps, a three-line clamp, and a
  **Show more** toggle offered only when the clamp is *measured* to have hidden
  something — a `ResizeObserver` against the element's own computed line
  height, so the column's width is what decides rather than a character count
  (the pattern `useOverflows` in `app/src/components/harness/Timeline.tsx`
  set).

  Clicking anywhere on the card that is not a control now opens the task; the
  title alone used to be the way in, which is a link-sized target on a
  card-sized affordance. The card stays a plain `<article>` that navigates on
  click rather than becoming an anchor or a button, because it *contains*
  controls — the title's anchor, the toggle — and interactive content nested in
  either is invalid markup WebKit repairs by closing the outer control early.
  It carries `data-tab-href` so ⌘-click gets the task in its own tab from
  anywhere on the card (`app/src/lib/newTabClicks.ts`), the toggle carries
  `data-tab-skip` and stops the click it does not want, and neither of them
  cancels `pointerdown` — cancelling the press is what once removed every
  card's click, per the drag bullet above.
- The timeline packs
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

## The universal view

`/tasks` is every task there is — across every project, plus the ones filed
nowhere — read as a board of four columns or as a flat table
(`app/src/pages/TasksPage.tsx`). It is the **second tab of the Projects/Tasks
section**, under the strip both pages share, so neither page carries a heading
of its own: the strip names the section, and a title on the same rule would be
a second one. A project's board is where *one* plan is
arranged; this answers the question a board cannot, which is what there is to
do at all, and it is where a task with nowhere to go gets written down.

- **It is not `projectsStore`.** That store holds one open project and that
  project's tasks, which is exactly the wrong shape here: a view spanning every
  project has no `activeId` to be, and an unfiled task would have to pretend to
  belong to whatever was open last. `app/src/hooks/useTaskList.ts` is a plain
  hook over `getAllTasks` / `getUnfiledTasks` plus the project list, refreshing
  on the same `PROJECTS_UPDATED_EVENT` everything else does — so a click here
  and a write the chat agent made through `oculus task` still arrive by one
  door. The projects come along because a task carries its project's *name*,
  not its board, and a column id only means something against a board. This
  page asks for `"all"`; `getUnfiledTasks` is still what resolves an **unfiled
  task's own page** (`app/src/pages/TaskPage.tsx`), which is why the scope
  outlived the strip that was named after it.
- **Its columns are `DEFAULT_COLUMNS`** — Backlog / Todo / In progress / Done —
  aliased as `UNIVERSAL_COLUMNS` in
  `app/src/components/projects/universalTasks.ts` rather than written out a
  second time, because the board every project is *born* with is the closest
  thing to a shared vocabulary there is. They were column *kinds* until
  2026-09-19, which collapsed Todo and In progress into one column while the
  table's `StatusPill` went on naming them apart — one page, two answers.
  - A card is placed **id first, kind second** (`universalColumnOf`): its own
    board's column, if that column still carries one of the four default ids;
    otherwise the kind's home column — `active` lands on In progress, since a
    renamed or added active column has no way to claim Todo specifically. A
    column its own board no longer has falls to Backlog, and the table's
    `StatusPill` is what says so.
  - A drop onto universal column X writes, on the task's **own** board, the
    column with id X if it has one, else the first column of X's *kind*, else
    nothing at all (`columnForUniversal`). So a board that renamed `todo` to
    "Next" still receives a drop onto Todo, and a board with no Done column
    still honestly refuses one. A drop that resolves to the column the card is
    already in writes nothing either — two universal columns can resolve to one
    column on a board that has only one active column.
- **Four filters, opening on Todo** — status, project, subject, due
  (`app/src/components/projects/TaskFilters.tsx`, state in the page). They
  replaced an `All tasks · Unfiled` strip: **Unfiled is a value of the project
  filter** now, which is strictly more useful, since there was previously no
  way to ask for one project's tasks in this view at all. The filter is a
  **read** — one `getAllTasks` narrowed by a predicate over the list the page
  already has, not a query variant per question, which is how the old two-query
  strip came to say two different things about the same rows.
  - **The status set decides which columns the board has.** A filtered-out
    status simply has no column, so filtering to Todo alone leaves one column
    and nowhere to drag — deliberate, because a board that ignored a filter the
    toolbar says is on would be worse. A status is changed from the table's
    `StatusPill`, or by widening the filter. The set is never empty: the
    control keeps the last checked status checked.
  - **Due is asked through `sqliteUtcToMs`**, never by comparing the strings —
    `due_at` is an ISO stamp from the UI and SQLite's `YYYY-MM-DD HH:MM:SS` from
    the CLI, and the `T` beats the space and reorders a day. *This week* is the
    calendar's own Monday-first week (`startOfWeek`), so it names the days the
    calendar page draws.
  - **Status and view persist** (`oculus-tasks-status`, `oculus-tasks-view`) —
    they are how you like to work. Project, subject and due are per-question and
    start at Any every time; carrying them would want the URL, which is also
    what would make ⌘-click and a restored tab carry a filter, and is not worth
    building until it is asked for. `oculus-tasks-scope` is the strip's dead
    key: nothing reads it any more and nothing cleans it up, as with
    `oculus-project-view`.
  - The `done/total` counter **stops being one under a filter** — defaulting to
    Todo it would read `0/12 done`, true of the rows on screen and nonsense as
    a summary — and says `N shown` instead. An empty list likewise names the
    filter rather than claiming you have no tasks.
- **There is no manual order in this view, and there cannot be.** `position` is
  a fractional slot *inside one project's column*; two projects' positions are
  two unrelated number lines, so a midpoint between them is arithmetic on
  unrelated units, and a hand-set order would be scrambled the next time either
  project renumbered. So a universal column is **sorted, not arranged** — due
  date (nulls last), then project (unfiled first), then `position`, which is
  `UNIVERSAL_ORDER` in `app/src/lib/projects.ts` — a same-column drag is a
  no-op, and the table sorts by header instead of dragging by grip. A move into
  another column therefore **appends**: `appendNeighbour` finds the last task
  *of that task's own project* in the destination, because that is the only run
  of cards its `position` is comparable with.
- **A task's route depends on whether it has a project.** `taskHref`
  (`app/src/components/projects/taskHref.ts`) writes
  `/projects/:projectId/tasks/:taskId` for a filed task and `/tasks/:taskId`
  for an unfiled one, and `app/src/pages/TaskPage.tsx` serves both: the project
  is `null`, the board is `boardOf`'s default four, and the rows come from
  `useTaskList` instead of the store. One page, because everything a task page
  does is the same either way.
- **⌘K finds an unfiled task, and that took two fixes.** `searchTasks` joins
  the project **LEFT** — an inner join dropped exactly the tasks with no board
  to find them on — and `COALESCE`s the project name into the haystack, because
  `||` in SQLite yields NULL if either side is, so an unfiled task's haystack
  was NULL and matched nothing even once the join was right. A hit with no
  project reads **Unfiled**, the word the rest of the feature uses.
- **Filing one is `refileTask`** (`app/src/lib/projects.ts`, `refile_task` in
  `app/src-tauri/src/projects.rs`, `oculus task refile`), offered by the
  Project cell in the table and the Project row on a task's page, both through
  `app/src/components/projects/ProjectPicker.tsx`. Three rules make it the only
  writer of `project_id` after a create:
  - It **maps the column across by kind**, into the first column of that kind
    on the destination's board — the fallback half of the rule
    `columnForUniversal` applies to a drag — and refuses a destination with no
    column of that kind rather than picking the nearest, because there is no
    nearest kind. Kind alone, with no id step in front of it: a refile crosses
    two boards that share nothing but what a column means, where a drag stays
    on one board and can honestly aim at an id. `done_at` still follows the
    column it lands in.
  - It **carries the task's subtasks with it**, each by its own column's kind.
    A subtask sits in its parent's project — `assertCanParent` /
    `assert_can_parent` refuse both directions of the alternative — so this is
    the one operation allowed to move both sides at once, and a subtask on its
    own is refused and told to refile its parent.
  - It **appends** at the destination column's end, for the reason above: there
    is no slot across projects to aim at. `moveTask` is then how it is placed.
