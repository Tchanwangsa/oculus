# Calendar

Class times, deadlines, lecture recordings, your own notes and dated tasks on
one grid. Canvas rows are fetched during a sync; the page only reads the
database, so it works offline.

## Where

| Piece | Location |
| --- | --- |
| Canvas calendar fetch + Tauri command | `app/src-tauri/src/calendar.rs` |
| `calendar_events` (migration 19), `local_events` (22), `projects.event_id` (33) | `app/src-tauri/src/migrations.rs` |
| Headless writes (CLI) | `app/src-tauri/src/store.rs` |
| Frontend reads + writes | `app/src/lib/db.ts` |
| Event model, `loadCalendar`, colours, date maths | `app/src/lib/calendar.ts` |
| The task layer's read | `getAllOpenTasks` in `app/src/lib/projects.ts` |
| New/edit event dialog | `app/src/components/calendar/EventDialog.tsx`, `app/src/stores/eventEditorStore.ts` |
| Pinning a project to an event | `app/src/components/projects/EventLink.tsx` |
| Page + views (week, month, agenda, card, mark, row) | `app/src/pages/CalendarPage.tsx`, `app/src/components/calendar/` |
| Minute clock for "now" | `app/src/hooks/useNow.ts` |
| Post-sync refresh (`CALENDAR_UPDATED_EVENT`) | `app/src/hooks/useBackendEvents.ts` |

## Five layers, three owners

| Layer | Source | Why it exists |
| --- | --- | --- |
| `class` | Canvas `calendar_events?type=event` | The published timetable, when staff publish one |
| `due` | Canvas `calendar_events?type=assignment` | Deadlines, including quizzes (a quiz has an assignment shell) |
| `lecture` | The `lectures` table synced from Echo360 | The fallback timetable for a subject Canvas is silent about |
| `note` | `local_events` | Rows the user writes in Oculus |
| `task` | `project_tasks`, read live | A dated, unfinished task, on a board or unfiled ([projects.md](./projects.md)) |

- **Canvas rows are neither editable nor deletable** — a sync writes them
  straight back. **Notes are both**, because nothing else will clean them up.
  **Tasks are neither** here: the calendar only reads `project_tasks`, so the
  card links to the task's project, and an unfiled task links nowhere.
- `EventDialog` writes notes with `source = 'manual'`; older rows may say
  `automation`, and the card shows which. An edit leaves `source` alone.
- The dialog is mounted once in `app/src/layouts/AppLayout.tsx` and raised
  through `eventEditorStore`, because its two openers — the page header and
  `EventPopover`, which Home's Today list also renders — share no subtree.

## Canvas rows are replaced per subject after a sync

- Not a scrape phase: nothing is written under `courses/`. After
  `scrape-complete` the frontend calls `calendar_sync_events` per subject
  (`syncCalendar`), gated by the `calendar` sync option; the CLI calls
  `calendar::fetch` then `store::replace_calendar_events` and always runs.
- Both writers delete the subject's rows and re-insert, so a moved or
  cancelled class disappears. That is safe only because the fetch is always a
  whole course's calendar (`all_events=true`) — never add a date window.
- Canvas materialises repeating classes server-side; no recurrence rule is
  stored or interpreted.
- A sectioned class comes back as a parent with `child_events`. Children
  replace the parent, filtered to the user's own sections
  (`include[]=sections` returns the caller's); when none match, every child is
  kept. The query asks for the course and its sections, then dedupes by id,
  because Canvas nests section occurrences inconsistently.
- A subject with any `class` rows suppresses its `lecture` layer, or a class
  and its recording draw twice. Most UniMelb subjects publish nothing, so the
  recordings are the timetable.

## What Oculus owns lives outside the Canvas table

- `local_events` is never touched by a sync and is merged by `loadCalendar`
  *after* Canvas and lectures, so a local `class` cannot suppress recordings.
  Its `subject_id` is nullable (`ON DELETE SET NULL`); subject-less rows file
  under a "Personal" key kept out of the subject palette.
- Tasks are read live through `getAllOpenTasks` (undated and finished dropped
  in SQL, project joined LEFT so unfiled tasks survive), never copied into
  `local_events`: a copy would outlive the task being re-dated, finished or
  deleted. The page refreshes on `PROJECTS_UPDATED_EVENT` as well as
  `CALENDAR_UPDATED_EVENT`.
- `projects.event_id` holds a `CalEvent.id` (a Canvas id, or `local_<n>`) and
  is resolved live against `loadCalendar()`. A pin that fails to resolve
  says so and offers to clear itself. `task` events are excluded from the
  picker, since pinning a project to its own task is a loop.

## Deadlines, notes and tasks are instants

`isInstant` and `isSelfImposed` in `app/src/lib/calendar.ts` are the one
model: a deadline, note or task is a point in time, and a note or task is one
*you* set, drawn in a quieter register. `app/src/components/calendar/EventMark.tsx`
is the shared vocabulary — filled flag for a Canvas deadline, filled pin for a
note, outline checkbox for a task, dot for anything with duration — so a
task's date never reads as a submission cutoff.

## The week grid fits its hours and hides nothing

In `app/src/components/calendar/WeekView.tsx`:

- `hourRange` covers the timed events in view plus an hour either side, a
  floor of 8am–6pm, and on the current week the current hour. An event past
  midnight counts as ending at 24:00.
- The split is exhaustive: a timed event is inside `hourRange` by
  construction; an instant or all-day event sits on the grid when the hours
  reach it and in a strip above it when they do not — never both. Instants are
  kept out of `hourRange`, or one 11:59pm cutoff pins every week to midnight.
- An instant's marker is centred on its minute and nudged back inside the grid
  by at most half its height, never snapped to an hour. A zero-length `class`
  is clamped the same way; a class with real length is shortened, not moved.
- The strip is headed by its loudest layer ("Due", "Tasks" or "Notes").
- A `24h` toggle opens the whole day and is remembered in `localStorage`.
- Below `MIN_GRID_PX` the week scrolls sideways. One scroller carries both
  axes with headers, strip and hour gutter sticky inside it, because `sticky`
  resolves against the nearest scrollport.
- Past hours carry a wash mixed from `muted-foreground`, finished events turn
  `chart-other`, and today gets a time line — all from `useNow`.

## Gotchas

- Never add a date window to the Canvas fetch — the write deletes the subject's rows, so a window would erase everything outside it.
- Never make `projects.event_id` a foreign key — the sync's delete-and-reinsert would cascade every pin to NULL even though the ids come back identical.
- Never copy tasks into `local_events` — nothing cleans that table up.
- `EventRow` takes `now` as a prop; a row calling `useNow()` puts a timer behind every line of Agenda and Home's Today list.
- `MonthView` measures its chip capacity from the grid height; a constant clips the last chip with no "+N more".
- The header's control group wraps rather than clipping under the page's `overflow-hidden`.
- A wash mixed from `surface` is invisible — it is within a few percent of the background.
- Times are stored as each source gives them (Canvas UTC ISO, Echo360 zone-less wall clock, task `due_at` in either ISO or SQLite format); read task times with `sqliteUtcToMs` in `app/src/lib/format.ts`, and never normalise on the way in.
