# Lecture chapters

A two-hour Echo360 recording arrives as one unbroken bar with a transcript
beside it. There is no way to see that minute 34 is where the lecturer stopped
proving things and started on the assignment. Chapters are the shape: the
recording cut at its real topic boundaries, each one named.

Two halves are built: a **detector** that finds the moments worth cutting at,
and a **naming job** that drives a CLI coding agent over those moments and
writes the named chapters to the database. `oculus lecture candidates` is the
first; `oculus lecture chapters` is the whole pipeline, and the app runs that
same job through the `lecture_find_chapters` command. Which agent, model and
reasoning level it runs on is configured in Settings → AI — see [A configured
job](#a-configured-job). The player reads them back three ways — see [Three
readings in the player](#three-readings-in-the-player).

## Where

| Piece | Location |
| --- | --- |
| Detection: sampling, scoring, thinning, frame grabs | `app/src-tauri/src/chapters.rs` |
| The prompt, the reply parser, the validator | `app/src-tauri/src/chapters.rs` |
| Writing the rows and the job's status | `app/src-tauri/src/store.rs` |
| `lecture_chapters` + the three `lectures` columns (migration 29) | `app/src-tauri/src/lib.rs` |
| The job itself — detect, grab, ask, validate, write | `run` in `app/src-tauri/src/chapters.rs` |
| `oculus lecture candidates`, `oculus lecture chapters` | `app/src-tauri/src/bin/oculus.rs` |
| The app's trigger, its two events, and the startup sweep | `chapters::app` in `app/src-tauri/src/chapters.rs` |
| The phases a run reports, and where each one fires | `Step` in `app/src-tauri/src/chapters.rs` |
| Which agent and model the job runs on | `app/src-tauri/src/harness/jobs.rs`, `app/src/lib/db.ts` |
| The Settings → AI row that picks them | `app/src/pages/settings/AiPage.tsx` |
| The frontend binding, the two event names, the derived ends | `app/src/lib/lectures.ts` |
| The tool verbs the running panel borrows from the timeline | `toolVerb` in `app/src/lib/harness.ts` |
| Reading the rows and the job's state in the app | `getChapters` / `getChapterStatus` in `app/src/lib/db.ts` |
| The player's chapter state, and the event it listens on | `app/src/hooks/useLectureChapters.ts` |
| The chapter list, and every state it has | `app/src/components/lectures/ChaptersPanel.tsx` |
| The dock's tab strip | `app/src/components/lectures/TranscriptPanel.tsx` |
| The chapter strip and the scrub-bar ticks | `app/src/components/lectures/LecturePlayer.tsx` |
| Which tab the dock opens on | `dockTab` in `app/src/stores/playerPrefsStore.ts` |
| The one-turn headless run both share | `run_once` in `app/src-tauri/src/harness/mod.rs` |
| ffmpeg lookup (bundled, dev copy, or system) | `app/src-tauri/src/echo360.rs` |
| The frontend's VTT parser, whose timing half is mirrored | `app/src/lib/lectures.ts` |
| A fixture transcript for the parser tests | `app/src-tauri/fixtures/chapters/sample.vtt` |

## The pipeline

Four steps, one ffmpeg process:

1. **Sample.** One `fps=1,scale=160:90,format=gray` decode writes raw frames to
   a pipe. Each frame is diffed against the previous one *as it arrives* — a
   two-hour lecture is 7200 frames ≈ 100 MB, so nothing is collected. The
   result is one mean-absolute-difference per second.
2. **Collapse.** Loud frames within 3 s of each other are one event — a
   dissolve, a build, a scroll — reported at the second it *started*, carrying
   the largest magnitude in the run.
3. **Score.** The magnitude, plus a small bonus when a transcript silence of
   ≥2 s lands within ±8 s.
4. **Thin.** Strongest first, dropping everything within 90 s of something
   already kept, then back into play order. Thinning by strength rather than
   sweeping left to right is what keeps the *important* boundary when two land
   a minute apart.

`--frames` additionally writes one seek-based JPEG per candidate into
`lectures/<uuid>/frames/`, which is how a boundary set gets checked by eye. It
is off by default because detection otherwise writes nothing at all.

## Why there is nothing to tune, and nothing to cache

Four measurements on real lectures in this library are the whole design.

- **The picture is violently bimodal, so one threshold covers everything.**
  These are 720p screen captures of a slide deck — no camera, no grain, no
  lighting drift — so a held slide is *dead still*: frame-to-frame difference
  sits at p50 ≈ 0.008 and p90 ≈ 0.08, while a slide change is a cliff at
  p99 ≈ 13 and max ≈ 175. The threshold sits in the empty middle, at 6.
- **Which is why the threshold barely matters.** On one 42-minute lecture,
  threshold 2 gives 58 collapsed change-points and 19 boundaries after
  thinning; threshold 6 gives 42 and 18, and its first twelve boundaries are
  *identical* to threshold 2's. Only at 12 (23 → 12) does it start dropping
  real changes. A knob whose setting does not change the answer is a knob that
  invites fiddling, so there is none: no flag, no setting, no config surface.
- **Pauses are weak, so they can only ever be a bonus.** Only 25–30 % of slide
  changes have a ≥2 s silence within ±8 s. Gating on one would throw away most
  of the real boundaries, so a nearby pause adds a small amount to the score —
  enough to reorder near-equals during thinning, never enough to promote a
  quiet frame. A lecture with no transcript on disk simply loses the bonus.
- **Detection is seconds, so caching would cost more than it saves.** The
  decode pass saturates every core and decode dominates, which makes the sample
  rate and the 160×90 frame size effectively free: a 42-minute lecture takes
  ~5 s wall for 2534 frames, and a 106-minute lecture ~15–17 s. Nothing is
  written to the database, there is no candidates table, and re-running always
  reflects the file on disk rather than a stale row.

Seek-based frame extraction is likewise instant — ffmpeg jumps to a keyframe
rather than decoding forward — and 768px wide lands around 30 KB with slide
titles and formulas legible, which is the size a model will need.

## Details worth not rediscovering

- **The CLI mirrors the frontend's VTT timing, not its text.** `cue_gaps`
  handles the same two timestamp shapes `parseVtt` does (`HH:MM:SS.mmm` and
  `MM:SS.mmm`); cue text plays no part in detection, so none is parsed. The two
  parsers are deliberately separate — one is in a React player, the other in a
  headless binary — but the timestamp handling has to agree or a boundary would
  land in a different place in each.
- **A lecture id is a UUID, so a unique prefix is accepted.** A prefix matching
  two lectures is reported with the full ids rather than guessed, the same rule
  `one_subject` follows for subject codes. `oculus list -l` prints the first
  eight characters of each, which is what makes the prefix match worth having —
  a command whose only argument cannot be discovered from the CLI is not usable.
- **Three of the lecture folders on a typical machine are empty.** A lecture
  row with no `video_path` has never been downloaded, and the command says so
  and names `oculus run -l --videos` rather than failing on a missing file.
- **The boundary second is the right timestamp and the wrong frame.** The
  loudest changes in a recording are the screen share stopping and starting, so
  a grab taken exactly at a boundary catches the black — and one taken a fixed
  two seconds later catches the room's Crestron "connect your laptop" splash
  instead, which is visually busy, 57 KB, and contains no lecture content at
  all. On the reference lecture that splash appears at three separate
  boundaries with a standard deviation of 81.4 every time, because it is
  literally the same static image; file size cannot see any of this.

  So a grab probes four offsets — +2 s, +6 s, +12 s, then the boundary itself —
  with the *same* seek the grab will use, and takes the earliest frame within
  5 % of the most detailed one. Input seeking lands on a keyframe, so probing
  with one windowed decode would have measured different frames than it wrote.
  The comparison is relative rather than a fixed floor because what counts as
  a detailed frame depends on the deck, while a blank or a splash loses to a
  real slide by a wide margin in any deck. A probe costs ~40 ms, so the whole
  thing adds well under a second per candidate. The file keeps the **boundary**
  second in its name, not the offset one. The chat dock's live grab of the
  playhead's moment shares that probing (`grab_frame`, and
  [harness.md](./harness.md)) — a frame the student asked about can land on a
  dropout exactly as easily as a boundary's can.

## Naming them is an agent job

`oculus lecture chapters <ID>` detects the candidates, grabs a frame for each,
and hands the lot to a CLI coding agent through `harness::run_once` — one
prompt, one turn, no thread, nothing in `harness_threads`. It is the first
caller of that function outside `oculus agent` (see
[harness.md](./harness.md)).

**The prompt is small on purpose, and that is the whole argument for driving a
coding agent rather than calling a model API.** It carries the candidate list,
the lecture's title and duration, the path to `transcript.vtt`, the path to
`frames/` and how those files are named — and then stops. The agent opens the
five frames it is unsure about and reads the transcript across the boundaries
it doubts; a prompt to an API would have to *carry* fifty frames to let a model
look at five. Three things in the prompt are lessons rather than decoration:

- **A candidate is not a chapter.** Candidate density varies threefold between
  lectures of the same length (two 107-minute recordings in this library give
  15 and 49), so "one chapter per candidate" would cut a busy deck every two
  minutes. The agent is asked for the number of things the lecture is actually
  about — usually five to eight, never more than twelve — and the score field
  is there to help it choose. It is also told that a chapter shorter than about
  three minutes is a slide rather than a topic — without that line a 51-minute
  lecture split two adjacent slide titles into two chapters ninety seconds
  apart — and, in the same breath, that a long stretch of housekeeping or a
  worked example *is* a chapter if it lasts, because on its own the minimum
  read as licence to merge and the same lecture came back with its last eleven
  minutes folded into the chapter before them.
- **The room's AV splash screen is not a slide.** A dropout spanning the whole
  probe window survives frame selection, so a "connect your laptop" panel does
  reach the agent occasionally. A model looking at the image recognises one
  instantly once it has been told they exist; one that has not been told will
  name a chapter after it.
- **The subject's course folder is named.** An Echo360 title is a room booking
  ("MULT20015_2026_SM2 MO L105"), so without the folder the agent spends
  several turns hunting for the deck — measured on the first run.

**Rust writes the rows; there is no agent write door.** Unlike `oculus project`
and `oculus task`, which put the *student's own* planning into the database
([projects.md](./projects.md)), chapters are derived data like `pages` and
`parse_status` — regenerable from the recording, and nobody's work. So the
agent replies with JSON and Rust parses it: there is no `oculus chapter add`,
and `oculus.db` stays on the deny list it has always been on.

`parse_chapters` is tolerant of how the reply is packaged — prose around the
array, a fence, an object wrapping it, a float where an integer was asked for
— for the same reason `clean_title` is: the alternative is throwing away a good
answer over its wrapper. What it is *not* tolerant of is the content.

**One bad chapter rolls the whole set back.** `validate` checks that every
boundary came from the candidate list (second 0 always counts — the detector's
first candidate is typically twenty seconds in, so the opening is prepended
before the agent ever sees the list), that starts are strictly increasing, that
the first is 0, and that there are no more than twelve. A single failure means
nothing is written at all, and the error names the chapter the way
`projects::create_tasks` names a task — `chapter 3 ("…"): …`. The reason is
sharper here than for a task breakdown: drop the third of nine chapters and
there is no gap on the scrub bar to notice, only twenty minutes silently
attributed to the chapter before it.

## What is stored, and what is not

Migration 29: `lecture_chapters` (`lecture_id`, `idx`, `start_seconds`,
`title`, `summary`, cascading with the lecture) plus `chapter_status`,
`chaptered_at` and `chapter_error` on `lectures`.

- **There is no `end_seconds`.** A chapter ends where the next one begins, and
  the last at the lecture's duration; the reader derives it. One fact in one
  column — the lesson migration 27 records about `column_id` / `position` /
  `done_at` — because a stored end is a second place for the same fact to be
  wrong.
- `chapter_status` is `NULL | running | ready | error`, mirroring
  `files.parse_status` (migration 3); only a terminal status stamps
  `chaptered_at`. `chapter_error` carries the failure message, because a
  status column cannot and the player has to be able to say what went wrong.
  It is cleared on success. A run killed mid-turn would leave `running` behind
  with no `chaptered_at` — the same stale-status shape the parse pipeline has —
  so `store::reconcile_chapter_status` sweeps it back to NULL at startup, next
  to the sweep `harness::app::reconcile` makes over threads. Without it one
  crash leaves the lecture claiming a job is in flight forever.
- Writing goes through `store::save_chapters`, one transaction that deletes the
  old set and inserts the new one, so a regenerate that fails partway leaves
  the chapters that were already there. `store::set_chapter_status` is a
  sibling of `set_lecture_path` rather than another arm of it: that function's
  allow-list takes a `&str` and so cannot clear a column back to NULL, which is
  what a fresh run needs.
- **Regenerating is deleting and re-running**, the shape `resetFilePipeline` in
  `app/src/lib/db.ts` has for the parse pipeline. `--force` is that door on the
  CLI; without it a lecture that already has chapters is left alone.

The frames stay on disk after a run — ~30 KB each, regenerable in seconds, and
overwritten by the next run. Deleting them would be tidier by a megabyte and
would leave the run with nothing to show for itself afterwards: a chapter's
opening slide sits at `frames/<start_seconds>.jpg`, which is what makes a
boundary set checkable by eye. The player does not draw them — see [what is not
built](#what-is-not-built).

## A configured job

Nothing about which agent runs this is hardcoded any more. The job's provider,
model and reasoning level come from the per-job model registry
([harness.md](./harness.md#per-job-models)) — one `settings` row, one
`ModelPicker` row in Settings → AI — and both doors read the same value:
`oculus lecture chapters` uses it when no flag says otherwise, and
`--provider` / `--model` / `--effort` override it for that one run. Out of the
box it is Codex on `gpt-5.6-luna` at `xhigh`: a long look at fifty slide frames
and an hour of transcript is exactly what a reasoning level is for.

**The app's door is `lecture_find_chapters`.** It resolves the selection, runs
the *same* `chapters::run` the CLI does — there is one job, not two that drift
— and returns as soon as the run is claimed, because the turn takes eight to
eleven minutes and nothing can wait on that. Progress is the `chapter_status`
column, claimed before the ffmpeg pass rather than before the agent turn, since
detection is fifteen seconds a student can see happening. The end arrives as
its own event, `lecture-chapters`, carrying the lecture, `ready` or `error`,
and the message on a failure.

- **A dedicated event, not `lectures-changed`.** That one fires on every
  playback-progress save, so a result eight minutes in the making would be
  indistinguishable from a scrub.
- **Not the harness event stream either.** A headless run reports on thread id
  0, so the hop `useBackendEvents` makes for `oculus project` writes
  ([projects.md](./projects.md)) does not apply: the app started this job, so
  it already knows whose it is and when it ended.
- **One run per lecture.** A second call while `chapter_status` is `running` is
  refused up front — the only check worth making the caller wait for, since two
  runs would spend two subscription turns and race each other's write.

## Saying what it is doing

Nine minutes of spinner is indistinguishable from nine minutes of hung, and the
job is not actually opaque: it decodes a countable number of frames, grabs a
countable number of stills, and then spends most of the run inside an agent
turn that says out loud which file it is opening. All of that was already
flowing and being dropped — `run_once` takes an `on_event` closure, and the app
passed `|_| {}`.

**Two reporters, because there are two kinds of thing to report.** The
pipeline's own phases come out of `chapters::run` as `Step` — `Decoding`,
`Detected`, `Grabbing`, `Asking`, `Writing` — and the agent turn's detail comes
out of the harness as `HarnessEvent`, the same stream the chat timeline draws.
Folding them into one callback would mean `run` inventing a vocabulary for
events that already have one; keeping them apart means the CLI can take the
phases and ignore the rest, which is exactly what it does (its progress is the
agent's printed tool rows).

**One event out, `lecture-chapter-progress`, separate from
`lecture-chapters`.** The two have different lifetimes: a finish is a fact the
panel re-reads SQLite on, a step is a line it paints and forgets. Nothing about
a step is persisted — there is no column for it, and there should not be one,
because the decode would write to the row hundreds of times.

- **The decode reports four times a second, not four hundred.** One sampled
  frame is one second of recording, so a 107-minute lecture would otherwise
  push 6400 events through to move a percentage that has a hundred places to
  be. `sample_diffs` fires per frame and `run` throttles; the callback is where
  the throttle *isn't*, so a future caller that wants every frame can have it.
- **The agent's steps are its tool calls.** `ToolStarted` carries a `ToolKind`
  and a one-line title already — "1386.jpg", a `Grep` pattern, a command — so
  the panel says "Reading 1386.jpg" using `toolVerb`, which moved from the
  timeline's `WorkRow` into `app/src/lib/harness.ts` so both callers share one
  vocabulary for one enum.
- **The first delta of the reply is its own phase.** The reply *is* the chapter
  JSON, so the moment text starts arriving the agent has made up its mind about
  the whole lecture; without that the panel would sit on whichever file
  happened to be read last for a minute or more. It fires once per run, on an
  `AtomicBool`.
- **What is deliberately not streamed is the chapters themselves.** `validate`
  is all-or-nothing on purpose (one bad boundary rolls the set back), and
  drip-feeding half-validated chapters into the panel would fight that
  directly.

## Three readings in the player

Chapters are three different questions, so they are drawn three times over
(`app/src/components/lectures/LecturePlayer.tsx`, and see
[frontend.md](./frontend.md) for the player's own shape).

**Chapters are one of the dock's tabs.** What was the transcript panel's
header is a `ViewTabs` strip — *Chapters*, *Transcript*, *Chat*
([harness.md](./harness.md)) — sitting on the border it already had, with the
`DotsSixVertical` and the drag-to-dock gesture untouched. The
tabs stop the pointerdown from reaching the header: `startDockDrag` captures
the pointer on the element it fires from, which retargets the pointerup onto
the header, and a click needs both ends on one target — without that the tab
would never register one. Everything around the tabs still drags. There is no
"In this video" heading over them; the dock is 200px wide at its narrowest and
its subject is never in doubt. The Transcript tab is only offered when there
are cues, since a tab that could only ever be empty is not a tab; Chat needs
neither a file nor a run and so is always offered, which is what makes the dock
itself unconditional.

**The list follows playback**
(`app/src/components/lectures/ChaptersPanel.tsx`): every chapter collapsed to
its title and length, the current one expanded with its summary, so the prose
beside the video is about what is being said now. Clicking one seeks to its
start. It deliberately does **not** reuse the transcript's follow machinery —
that code is wound around a virtualizer and earns its two-stage handover and
countdown ring on ~2500 rows, where twelve fit the panel with room over. The
current card is brought into view with `scrollIntoView({ block: "nearest" })`
and nothing else: no pill, no window, no ring. Coming back from the Chapters
tab does count as reopening the transcript, though, because that list is
unmounted while chapters are in front and would otherwise return scrolled to
the top with its cue index unchanged.

**A chapter's end is derived, in the reader.** `chapterEnds` in
`app/src/lib/lectures.ts` is the whole of it — the next chapter's start, or the
lecture's duration for the last. It is given the *player's* duration, which is
the element's where a file is loaded rather than the catalogue's.

**The strip and the ticks answer "how much of this bit is left".** Above the
scrub bar, inside the controls scrim, the chapter's name and a hairline that
fills across that chapter's span — not the lecture's, which the scrub bar
already says. It lives in the scrim, so it fades with the control bar.
Boundaries are notched into the `SeekBar` track as a 2px cut in the scrim's own
black, which reads against the played fill and the unplayed track alike; a
segmented bar was the alternative and costs the rounded ends and the growing
hover height that make it read as one bar. Second 0 is the left edge, so it is
not drawn. Those colours are fixed rather than semantic on purpose — the bar
sits on the frame, where `background` is whatever the lecturer put on the
slide.

**Which tab is in front is a player preference**, `dockTab` beside the dock's
side and size in `playerPrefsStore` — a habit like the side it is docked to,
not a property of one recording. It defaults to the transcript: every
downloaded lecture has one, and chapters have to be asked for.

### Every state is a real one

The panel never shows a control that does nothing, so the tab has five states
and no placeholder among them:

- **Not downloaded.** Chaptering watches the recording, so the tab says the
  file has to be there. The download button is already on the frame and on the
  control bar; this state does not grow a third.
- **Nothing yet.** A **Find chapters** button, and what it costs: 8–11
  minutes.
- **`running`.** A spinner in `brand` — the accent the app spends on work in
  flight — the phase it is on, the step under it, and a clock counting *up*.
  See [Saying what it is doing](#saying-what-it-is-doing). There is still no
  bar: a fill towards an estimate reaches the end and keeps waiting, which is
  exactly what hung looks like, and the agent turn — most of the run — cannot
  say how far through itself it is. A run *this session* started has its start
  time in a module-level map, because the player unmounts on every tab switch
  and the job outlives it by eight minutes; a run already in flight when the
  app started has no start time anywhere — `chaptered_at` is stamped by a
  terminal status only — and is shown without a clock rather than with a wrong
  one. Its steps are in a second map for the same reason and with the same
  gap.
- **`error`.** `chapter_error` verbatim, because it is the agent's own
  failure, and a retry.
- **Chapters.** The list, with **Regenerate** in a footer. A regenerate that
  fails leaves the chapters that were already there (`store::save_chapters` is
  one transaction), so that failure is a line *beside* them rather than in
  place of them.

**Nothing is hand-editable.** Chapters are derived data an agent writes, like
`pages` and `parse_status`, so the only two affordances are Find chapters and
Regenerate — there is no boundary to drag and no title to type.

**The button is disabled while a run is in flight, not apologetic after the
fact.** Rust refuses a second call with "that lecture is already being
chaptered", and an error message is the wrong place to learn that a button was
never going to work.

**The job's state is read from SQLite, not from the `lectures` row the player
was handed.** In the side panel that row is a snapshot held by a store and in a
list it is whatever the last `getLectures` returned; neither is re-read when a
run lands eight minutes later. `useLectureChapters` owns the read, listens for
`lecture-chapters`, and re-reads on it.

## What is not built

The stages are done; these are the things deliberately left out.

- **Boundaries are not hand-editable.** See above: a chapter set is
  regenerable derived data, and an edited one would be the only version of it
  nothing could reproduce — plus a `save_chapters` that deletes and re-inserts
  would silently eat the edit on the next run.
- **No thumbnails.** Each chapter's opening frame is already on disk at
  `lectures/<id>/frames/<start_seconds>.jpg`, and the list does not show it. A
  strip of slide images in a 200px dock is a grid of grey rectangles, and the
  frames are a run's leavings rather than a guarantee — a lecture chaptered by
  the CLI on another machine, or one whose folder has been cleaned, has none.
- **Chapters are not retrieval rows.** They are not embedded and not searched:
  `pages` is the index ([retrieval.md](./retrieval.md)), and a chapter title is
  a label on a span of video rather than a passage to match a question
  against.
