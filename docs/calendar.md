# Calendar

Class times, deadlines and lecture recordings on one grid. Most rows come from
Canvas's calendar API during a sync; the page only reads them, so it works
offline and pages between months without touching Canvas. A fourth layer holds
Oculus's own rows, which the calendar reads and can delete but nothing
currently creates — see the note under "The four layers".

## Where

| Piece | Location |
| --- | --- |
| Canvas calendar fetch + Tauri command | `app/src-tauri/src/calendar.rs` |
| `calendar_events` table (migration 19) | `app/src-tauri/src/lib.rs` |
| `local_events` table (migration 22) | `app/src-tauri/src/lib.rs` |
| Headless writes (CLI) | `app/src-tauri/src/store.rs` |
| Frontend reads + writes | `app/src/lib/db.ts` |
| Event model, colours, date maths | `app/src/lib/calendar.ts` |
| Minute clock for "now" markers | `app/src/hooks/useNow.ts` |
| Page + views | `app/src/pages/CalendarPage.tsx`, `app/src/components/calendar/` |
| Post-sync refresh | `app/src/hooks/useBackendEvents.ts` |

## The four layers

| Layer | Source | Why it exists |
| --- | --- | --- |
| `class` | Canvas `calendar_events?type=event` | The published timetable, when staff publish one |
| `due` | Canvas `calendar_events?type=assignment` | Deadlines, including quizzes (a quiz has an assignment shell) |
| `lecture` | The `lectures` table already synced from Echo360 | The fallback timetable for a subject Canvas is silent about |
| `note` | The `local_events` table | Anything Oculus wrote itself, rather than read from Canvas or Echo360 |

**Nothing writes the `note` layer today.** Its only writer was the automations
feature (removed — see [index.md](./index.md)), so the layer holds whatever
rows that left behind and is otherwise empty. Reading, rendering and deleting
are all still wired, because those rows are real events on a real grid; the
`local_events` table is where a reminder UI or a returning automation writes
next.

## How it connects

- **Not a scrape phase.** Calendar rows land in the database only — nothing is
  written under `courses/`, so there is no artifact to report and no
  file-manifest skip to honour. Like the Echo360 lecture list, it runs *after*
  a scrape completes: the frontend calls `calendar_sync_events` per subject and
  writes the rows (`app/src/hooks/useBackendEvents.ts`), and the CLI calls
  `calendar::fetch` then `store::replace_calendar_events` itself. Both are
  gated by the `calendar` sync option (`app/src/components/sync/SyncSettings.tsx`);
  the CLI always runs it.
- **What Oculus writes lives in its own table, and that is the whole point.**
  The Canvas write below deletes a subject's rows and re-inserts them, so a
  deadline Oculus derived and put on the grid would survive exactly until the
  next sync. `local_events` (migration 22) is therefore separate, never touched by a
  sync, and merged in by `loadCalendar` after the Canvas rows and the lecture
  layer — after, so a locally written `class` cannot suppress a subject's
  Echo360 recordings. Its `subject_id` is nullable (`ON DELETE SET NULL`): a
  personal reminder need not belong to a subject, and dropping a course must not
  take your own rows with it. Subject-less rows file under a "Personal" key that
  is kept out of the subject colour palette, or one note would recolour
  everything. Because nothing else will ever clean these up, every local row is
  deletable from `EventPopover`; Canvas rows get no such control, since a sync
  would only write them straight back.
- **A note is an instant, not a span** — the same rule deadlines follow, and the
  same machinery (`isInstant` in `app/src/lib/calendar.ts`), drawn with a pin
  and a quieter tint so a reminder does not shout over a real cutoff.
- **The write replaces, it does not accumulate.** A class moved or cancelled in
  Canvas has to disappear, and an upsert into a growing set would leave the old
  occurrence on the grid forever. Both writers delete the subject's rows and
  re-insert, which is safe only because the fetch is always a whole course's
  calendar — keep it that way if the query ever grows a date window.
- **Repeating classes are already expanded.** Canvas materialises a series
  server-side, so a semester of lectures is many rows and there is no
  recurrence rule stored or interpreted anywhere. `all_events=true` fetches the
  whole span in one walk rather than guessing a window that would clip the
  first or last teaching week.
- **A sectioned class is a parent plus children.** When staff schedule one
  event per tutorial section, Canvas returns a parent spanning them all with
  the real occurrences as `child_events`. Rendering the parent would show every
  section's tutorial as if the student attended all of them, so children
  replace their parent, filtered to the sections the user is actually enrolled
  in — `include[]=sections` on the courses API returns the *calling user's*
  sections, which is what makes that filter possible. When no child matches
  (sections unknown, or scoped some other way) every child is kept: a crowded
  calendar beats an empty one.
- **The query asks about the course and its sections, then deduplicates.**
  Canvas has returned a section occurrence nested under its parent in some
  cases and top-level in others; requesting both contexts and deduplicating by
  id makes the result the same either way.
- **Recordings fill in only where Canvas is silent.** A subject with `class`
  rows suppresses its `lecture` layer (`loadCalendar` in
  `app/src/lib/calendar.ts`) — otherwise a published lecture and its Echo360
  recording would draw the same class twice. In practice most UniMelb subjects
  publish nothing to the Canvas calendar, so the recordings *are* the
  timetable; see [sync.md](./sync.md) for how that list is fetched.
- **Past and future are drawn differently, and "now" is live.** Elapsed hours
  carry a grey wash, finished events drop their subject colour for the neutral
  `chart-other`, and today's column gets a time line — all driven by
  `app/src/hooks/useNow.ts`, which re-renders on the minute so a window left
  open all day does not quietly lie. Two consequences worth knowing: the week
  containing today stretches its hour range to keep "now" on the grid (at 10pm
  every class is over and the grid would otherwise stop at 7pm with nothing
  marking where you are), and the wash is mixed from `muted-foreground`, not
  `surface` — `surface` sits within a couple of percent of the page background
  and is invisible as a wash.
- **A deadline is drawn at its hour when the grid covers that hour**, as a slim
  marker laid over the classes rather than a block competing with them — a
  submission is an instant, not a span. The marker is centred on the exact
  minute and then clamped back inside the grid: deadlines cluster at the ends
  of the day (11:59pm above all), and one centred there hangs half off the
  bottom edge. It is nudged by at most half its own height rather than snapped
  to the nearest hour, which would redraw an 11:59pm cutoff at 11pm or at
  midnight — a time it is not due. The ones the grid cannot reach (an
  11:59pm cutoff against a grid that stops at 7pm) collect in the labelled
  "Due" strip above it. Every deadline is in exactly one of the two, never
  both. Deadlines are also kept out of `hourRange`: letting an 11:59pm cutoff
  stretch the grid would pin every week open to midnight for one marker.
- **A deadline never has a class's shape.** Wherever it appears it is a flag on
  a tinted pill; a class is a flat dot-and-time row. Before that they were
  rendered but not *indicated* — a deadline read as just another line in a
  column of classes.
- **The week grid sizes itself to what it must show**: the classes in view, an
  hour of padding either side, a floor of 8am–6pm, and — on the week containing
  today — the current hour, which is why the grid reaches 3am when you open it
  at 4am. An event running past midnight counts as ending at 24:00 rather than
  at its clock time, or 11pm–12:30am would read as "ends at 0:30" and shrink
  the grid below the block it needs to hold.
- **Nothing can hide outside the fitted range, by construction.** The split in
  `app/src/components/calendar/WeekView.tsx` is exhaustive: an event is either
  timed (and `hourRange` is computed *from* that set, so the grid covers it) or
  it is a deadline/all-day (and lands in the grid when the hours reach it, in
  the strip when they do not). There is no third case to leak through. A `24h`
  toggle in the grid's corner still opens the full midnight-to-midnight day and
  remembers the choice — the guarantee is structural, but "these hours do not
  exist" reads as a limitation, and the whole day should be reachable when you
  want to look.
- **Times are stored exactly as each source gives them.** Canvas sends ISO8601
  UTC, Echo360 sends local wall clock with no zone marker, and `new Date` reads
  each correctly. Nothing normalises them on the way in — a room booking is a
  wall-clock fact, and rewriting it to UTC would only add a way to be wrong.
