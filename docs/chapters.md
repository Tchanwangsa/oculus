# Lecture chapters

A two-hour Echo360 recording arrives as one unbroken bar with a transcript
beside it. There is no way to see that minute 34 is where the lecturer stopped
proving things and started on the assignment. Chapters are the shape: the
recording cut at its real topic boundaries, each one named.

Two halves are built: a **detector** that finds the moments worth cutting at,
and a **naming job** that drives a CLI coding agent over those moments and
writes the named chapters to the database. `oculus lecture candidates` is the
first; `oculus lecture chapters` is the whole pipeline. The app does not
trigger either yet and the player does not draw them — see
[What is not built](#what-is-not-built).

## Where

| Piece | Location |
| --- | --- |
| Detection: sampling, scoring, thinning, frame grabs | `app/src-tauri/src/chapters.rs` |
| The prompt, the reply parser, the validator | `app/src-tauri/src/chapters.rs` |
| Writing the rows and the job's status | `app/src-tauri/src/store.rs` |
| `lecture_chapters` + the three `lectures` columns (migration 29) | `app/src-tauri/src/lib.rs` |
| `oculus lecture candidates`, `oculus lecture chapters` | `app/src-tauri/src/bin/oculus.rs` |
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
  second in its name, not the offset one.

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
  It is cleared on success. A run killed mid-turn leaves `running` behind with
  no `chaptered_at`, which is the same stale-status shape the parse pipeline
  has and the same thing Stage 3 will have to sweep.
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
would throw away the one thing Stage 4 might want a picture of: a chapter's
opening slide is already sitting at `frames/<start_seconds>.jpg`.

## What is not built

Stages 3 and 4 do not exist.

- **No trigger in the app.** Nothing runs the job from the lecture page or
  after a sync; the CLI is the only door.
- **No model configuration.** The command's defaults (`codex`,
  `gpt-5.6-luna`, `xhigh`) are literals in its `clap` struct. Which provider
  runs which job, at what reasoning level, belongs in a settings registry
  shared with every other model-backed job — see [harness.md](./harness.md) —
  and no such registry exists yet.
- **No UI.** The lecture player still shows a transcript and a plain scrub bar
  (`app/src/pages/subject/LecturePage.tsx`, see
  [frontend.md](./frontend.md)). Nothing reads `lecture_chapters`.
