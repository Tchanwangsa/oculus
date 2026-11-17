# Calendar

Class times, deadlines, lecture recordings, your own notes and dated tasks on
one grid. Canvas rows are fetched during a sync; the page only reads the
database, so it works offline.

## Where

| Piece | Location |
| --- | --- |
| Canvas calendar fetch + Tauri command | `app/src-tauri/src/sources/calendar/` |
| `calendar_events` (migration 19), `local_events` (22), `projects.event_id` (33) | `app/src-tauri/src/db/migrations.rs` |
| Headless writes (CLI) | `app/src-tauri/src/db/store/calendar.rs` |
| Frontend reads + writes | `app/src/lib/db/calendar.ts`, `app/src/lib/db/localEvents.ts` |
| Accepted chat proposals and reversible local-event writes | `app/src-tauri/src/db/proposals/` |
| Event model, `loadCalendar`, colours, date maths | `app/src/lib/planning/calendar/` |
| The task layer's read | `getAllOpenTasks` in `app/src/lib/planning/projects/reads.ts` |
| New/edit event dialog | `app/src/components/calendar/EventDialog.tsx`, `app/src/stores/planning/eventEditorStore.ts` |
| Pinning a project to an event | `app/src/components/projects/page/EventLink.tsx` |
| Page + views (week, month, agenda, card, mark, row) | `app/src/pages/tools/CalendarPage.tsx`, `app/src/components/calendar/` |
| Minute clock for "now" | `app/src/hooks/ui/useNow.ts` |
| Pure term teaching weeks, date assignment and recording status | `app/src/lib/weeks/` |
| Subject week loader and refresh | `app/src/hooks/data/useSubjectWeeks.ts` |
| Lectures tab's week groups and rows | `app/src/lib/lectures/weeks.ts`, `app/src/pages/subject/content/LecturesPage.tsx` |
| Post-sync refresh (`CALENDAR_UPDATED_EVENT`) | `app/src/hooks/backend/useBackendEvents.ts` |

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
- `EventDialog` writes notes with `source = 'manual'`; accepted chat cards
  write `source = 'agent'`. An edit leaves `source` alone.
- The dialog is mounted once in `app/src/layouts/AppLayout.tsx` and raised
  through `eventEditorStore`, because its two openers — the page header and
  `EventPopover`, which Home's Upcoming card also renders — share no subtree.
  Its date picker loads the first time a `DateTimeField` popover opens. Each
  opening has its own session, so a pending save refreshes calendars when it
  finishes but only closes its own form.

## Canvas rows are replaced per subject after a sync

- The app fetches through `runtime::blocking::run`, keeping synchronous Canvas HTTP
  off the async runtime.
- Not a scrape phase: nothing is written under `courses/`. After
  `scrape-complete` the frontend calls `calendar_sync_events` per subject
  (`syncCalendar`), gated by the `calendar` sync option; the CLI calls
  `sources::calendar::fetch` then `db::store::replace_calendar_events` and always runs.
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
  and its recording draw twice. Many subjects (most at UniMelb) publish
  nothing, so the recordings are the timetable.

## Teaching weeks are shared across a term

`subjectWeeks` derives a subject's strip from subjects with the same
`term_name`. Only timed, valid Canvas `class` rows count; deadlines and
all-day rows cannot establish a timetable. Each subject prefers rows whose
titles contain the word lecture or lectures, falling back to all its classes
when none are named lectures. A week is teaching when at least
`ceil(timetabled subjects × TEACHING_WEEK_THRESHOLD)` distinct subjects have
one of these classes. The threshold is 0.5; a two-subject term needs one.
Repeated classes for one subject never increase its vote.

Week 1 is the term's first qualifying week. Teaching weeks number 1…N;
non-qualifying weeks between them are unnumbered Breaks. A missing class for
one subject does not by itself make a break. A lone late class below the
threshold cannot extend the term. A subject ends at its own last preferred
class week that qualifies as teaching, followed by exactly +1, +2 and +3.
A subject with no qualifying class inherits the term's span, including future
weeks even when it only has recordings so far.

Only a term with no usable classes falls back to the subject's recordings:
every week from its first to last valid recording is numbered, including
gaps, then +1…+3. Without dated recordings, or when a term has classes but no
week reaches the threshold, the strip is empty and its current index is -1.
The Lectures tab keeps recordings visible even without a strip and lists
invalid dates under Undated.

Week boundaries are local civil Mondays with exclusive next-Monday ends.
Keys encode the civil Monday in UTC, so DST does not change their spacing.
`weekOf` returns a week key, folding dates before week 1 into week 1 and
dates after +3 into +3; an empty strip or invalid date returns null. The
current week uses the same clamping. Neither late recordings nor other
activity grows the strip.

`recordingStatus` compares a recording with all its subject's timed classes,
independently of the title preference used for week numbering. A match is a
class on the same local day starting from 15 minutes before to 30 minutes
after the recording, inclusive. A subject with no usable classes has unknown
status. Otherwise an unmatched recording is untimetabled: the Lectures tab
greys its row but keeps playback and actions available. Grey means outside
the student's timetable, not a break; another stream or a holiday capture
can be grey inside a numbered week.

`useSubjectWeeks` loads subjects, calendar rows and recordings, exposes only
the subject's term, and refreshes on calendar updates and lecture changes.
It uses the minute clock for the current index and discards stale reads after
a subject change or unmount. The Lectures tab groups through this model;
its download and progress events also reload the rows.

## What Oculus owns lives outside the Canvas table

- `local_events` is never touched by a sync and is merged by `loadCalendar`
  *after* Canvas and lectures, so a local `class` cannot suppress recordings.
  Its `subject_id` is nullable (`ON DELETE SET NULL`); subject-less rows file
  under a "Personal" key kept out of the subject palette.
- Chat cards address these rows as `local_<id>`, the same ids printed by
  `oculus calendar` and `oculus project events`. Rust refuses other event ids
  for changes or deletes. A date-only start is all day; only `class` events
  accept an end. Apply and Undo share the proposal transaction and stale-row
  checks described in [projects.md](./projects.md#two-writers-enforce-the-same-rules).
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

`isInstant` and `isSelfImposed` in `app/src/lib/planning/calendar/model.ts` are the one
model: a deadline, note or task is a point in time, and a note or task is one
*you* set, drawn in a quieter register. `app/src/components/calendar/EventMark.tsx`
is the shared vocabulary — filled flag for a Canvas deadline, filled pin for a
note, outline checkbox for a task, dot for anything with duration — so a
task's date never reads as a submission cutoff.

## The week grid fits its hours and hides nothing

In `app/src/components/calendar/WeekView.tsx`, with `hourRange` in `app/src/lib/planning/calendar/query.ts`:

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
- Week overlap lanes are cached until the events, week or grid start changes;
  minute ticks update time styling. Month overflow lists share the grid clock.
- Past hours carry a wash mixed from `muted-foreground`, finished events turn
  `chart-other`, and today gets a time line — all from `useNow`.

## Dates follow the system locale; weeks and clocks do not

Every date and time is formatted in `displayLocale`
(`app/src/lib/format/locale.ts`): the system locale, so word order and month
and day names are the student's, unless a test pins one with
`pinDisplayLocale` so its expected strings hold anywhere. Two things are the
same for everyone: the calendar is Gregorian (`-u-ca-gregory`; a Thai region,
en-TH or th-TH, would otherwise print "October 2569 BE"), and the clock is
12-hour (`-u-hc-h12`). Formats name their fields rather than using
`dateStyle`/`timeStyle`, whose patterns can add an era or pad the hour. Weeks
start on Monday whatever the locale: `WEEK_START` in
`app/src/lib/planning/calendar/dates.ts`, which `startOfWeek`, `monthGrid` and
the month view's day names follow.

## Gotchas

- Never add a date window to the Canvas fetch — the write deletes the subject's rows, so a window would erase everything outside it.
- Never make `projects.event_id` a foreign key — the sync's delete-and-reinsert would cascade every pin to NULL even though the ids come back identical.
- Never copy tasks into `local_events` — nothing cleans that table up.
- `EventRow` takes `now` as a prop; a row calling `useNow()` puts a timer behind every line of Agenda and Home's Upcoming card.
- `MonthView` measures its chip capacity from the grid height; a constant clips the last chip with no "+N more".
- The header's control group wraps rather than clipping under the page's `overflow-hidden`.
- A wash mixed from `surface` is invisible — it is within a few percent of the background.
- Times are stored as each source gives them (Canvas UTC ISO, Echo360 zone-less wall clock, task `due_at` in either ISO or SQLite format); read task times with `sqliteUtcToMs` in `app/src/lib/format/format.ts`, and never normalise on the way in.
