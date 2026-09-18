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
job](#a-configured-job). The player reads them back three ways — see
[Reading them in the player](#reading-them-in-the-player).

The same visual pipeline has a second backend job: `oculus lecture reading`
writes the **reading copy** — the transcript rewritten as text a student can
read, one sentence per line, each pinned to its second, with spoken maths set
as maths. It deliberately has a different density and write lifetime from
chapters; see [The reading copy](#the-reading-copy). It replaced the recap.
In the player it is not a tab of its own but the Transcript tab's second
register, labelled **Enhanced** beside the standard cue list — one artifact
under two names, for the reason [that section](#the-transcript-has-two-registers)
gives.

## Where

| Piece | Location |
| --- | --- |
| Detection: sampling, scoring, thinning, frame grabs | `app/src-tauri/src/chapters.rs` |
| Which of the capture's streams to read | `detect` in `app/src-tauri/src/chapters.rs` |
| The transcript parser and the merged outline | `parse_transcript` / `outline` in `app/src-tauri/src/chapters.rs` |
| The prompt, the reply parser, the validator | `app/src-tauri/src/chapters.rs` |
| Writing the rows and the job's status | `app/src-tauri/src/store.rs` |
| `lecture_chapters` + the three `lectures` columns (migration 29) | `app/src-tauri/src/lib.rs` |
| The job itself — detect, grab, ask, validate, write | `run` in `app/src-tauri/src/chapters.rs` |
| Reading copy: segmentation, windows, prompt, validation, `para`, and the job | `app/src-tauri/src/reading.rs` |
| `lecture_reading` + the three `lectures` columns (migration 34) | `app/src-tauri/src/lib.rs` |
| `oculus lecture candidates`, `oculus lecture chapters`, `oculus lecture reading` | `app/src-tauri/src/bin/oculus.rs` |
| The app's trigger, its two events, and the startup sweep | `chapters::app` in `app/src-tauri/src/chapters.rs` |
| The reading command, progress/finish events, and startup sweep | `reading::app` in `app/src-tauri/src/reading.rs` |
| The phases a run reports, and where each one fires | `Step` in `app/src-tauri/src/chapters.rs` |
| Which agent and model the job runs on | `app/src-tauri/src/harness/jobs.rs`, `app/src/lib/db.ts` |
| The Settings → AI row that picks them | `app/src/pages/settings/AiPage.tsx` |
| The frontend binding, the two event names, the derived ends | `app/src/lib/lectures.ts` |
| The tool verbs the running panel borrows from the timeline | `toolVerb` in `app/src/lib/harness.ts` |
| Reading the rows and the job's state in the app | `getChapters` / `getChapterStatus` in `app/src/lib/db.ts` |
| The player's chapter state, and the event it listens on | `app/src/hooks/useLectureChapters.ts` |
| The Chapters tab — the card list and every state it has | `app/src/components/lectures/ChaptersPanel.tsx` |
| The Transcript tab's Enhanced register, and every state it has | `app/src/components/lectures/ReadingList.tsx` |
| The Standard ↔ Enhanced picker, which is also the enhance job's control | `app/src/components/lectures/TranscriptModePicker.tsx` |
| The reading copy's rows and its job state in the app | `getReading` / `getReadingStatus` in `app/src/lib/db.ts`, `app/src/hooks/useLectureReading.ts` |
| The reading copy's frontend binding and its two event names | `app/src/lib/lectures.ts` |
| The virtualised, playback-following list both transcript registers render through | `app/src/components/lectures/FollowList.tsx` |
| The inline markdown renderer a chapter summary and an enhanced line use | `InlineMd` in `app/src/components/markdown/MdComponents.tsx` |
| The dock's tab strip | `app/src/components/lectures/TranscriptPanel.tsx` |
| The chapter name over the frame, and the scrub-bar ticks | `app/src/components/lectures/LecturePlayer.tsx` |
| The progress line across the playing chapter's card | `app/src/components/lectures/EntryProgress.tsx` |
| Which tab the dock opens on, what order the tabs sit in, and which register the transcript is in | `dockTab` / `dockTabOrder` / `transcriptMode` in `app/src/stores/playerPrefsStore.ts` |
| The one-turn headless run both share | `run_once` in `app/src-tauri/src/harness/mod.rs` |
| ffmpeg lookup (bundled, dev copy, or system) | `app/src-tauri/src/echo360.rs` |
| The frontend's VTT parser, whose timing half is mirrored | `app/src/lib/lectures.ts` |
| A fixture transcript for the parser tests | `app/src-tauri/fixtures/chapters/sample.vtt` |

## The pipeline

Five steps, one ffmpeg process — two on the lecture whose slide capture failed:

0. **Choose the stream.** A capture publishes up to two, and which one holds
   the slides is not fixed; see [Which stream holds the
   slides](#which-stream-holds-the-slides).
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

The naming job adds a sixth step before it asks anything: it writes
`lectures/<uuid>/outline.md`, the transcript and the slide changes merged into
one document in play order. See [The outline is the
document](#the-outline-is-the-document).

## Which stream holds the slides

An Echo360 capture publishes up to two streams — `source1.mp4` and
`source2.mp4` in the lecture folder — and **which of them points at the
projector is not consistent, even inside one subject.** Four MULT20015
lectures put the slides on source 1; a fifth has 44 minutes of black there,
because the projector capture failed and the only thing that recorded the
screen was the second stream. Chaptering that lecture used to find *one*
candidate and hand the agent a menu of one.

`chapters::detect` is the answer, and both jobs go through it:

- Source 1 is decoded first and kept unless it is **dead** — at most two
  candidates in a whole recording. The broken lecture gives 1; a healthy
  capture gives 16 to 23. Nothing has been observed in between, so the bound is
  a which-file decision sitting in an empty middle, not a knob.
- Only a dead source 1 sends it to source 2, so the normal case is still the
  one decode it has always been and the broken case costs ~15 s more.
- Nothing is persisted. Re-detecting is one decode and always reflects what is
  on disk, where a column would be a second place for the same fact to be
  wrong — the same argument as [no candidates
  table](#why-there-is-nothing-to-tune-and-nothing-to-cache).
- `--source 1|2` on `lecture candidates`, `lecture chapters` and `lecture
  reading` overrules all of it, and `findLectureChapters` /
  `writeLectureReading` take the same argument.

Two things it deliberately does not do. **It never picks by candidate count** —
on one measured lecture that chooses the room camera's 20 over the real slide
deck's 16, because a camera saturates the threshold and thinning then lays the
result down as a near-uniform grid. And **it never remembers the answer per
subject**, because MULT20015 alone disagrees with itself.

## Why there is nothing to tune, and nothing to cache

Five measurements on real lectures in this library are the whole design.

- **The picture is violently bimodal, so one threshold covers everything.**
  The stream this reads is a 720p screen capture of a slide deck — no camera,
  no grain, no lighting drift — so a held slide is *dead still*: frame-to-frame
  difference sits at p50 ≈ 0.008 and p90 ≈ 0.08, while a slide change is a
  cliff at p99 ≈ 13 and max ≈ 175. The threshold sits in the empty middle, at 6.
- **That is true of a slide capture and of nothing else.** A room camera never
  holds still: across five lectures the camera stream sits at p50 ≈ 1.4 with a
  maximum of 20, against the slide capture's p50 ≈ 0.01 and maxima past 175.
  No empty middle to put a threshold in, no cliff to find. A camera is outside
  this detector's design rather than a harder case for it — which is why the
  fix for a dead slide capture is [choosing the right
  stream](#which-stream-holds-the-slides) and not a second threshold. The same
  measurement rules out finding whiteboard work: cropped to the board, a camera
  gives p50 = 1.79 at a 1 s lag and p50 = 10.61 at 60 s, saturated at both.
  Writing accumulates where this detector measures a first derivative, so the
  lecturer stepping sideways outweighs ten seconds of ink.
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

- **The CLI mirrors the frontend's VTT timing.** `cue_gaps` handles the same
  two timestamp shapes `parseVtt` does (`HH:MM:SS.mmm` and `MM:SS.mmm`). The
  two parsers are deliberately separate — one is in a React player, the other
  in a headless binary — but the timestamp handling has to agree or a boundary
  would land in a different place in each. `cue_gaps` still throws the *words*
  away, because a pause bonus does not care what was said; `parse_transcript`
  beside it keeps them for the outline, off one shared timestamp reader, so a
  cue start in a reading window and a cue start in an outline are the same
  second. It used to live in `reading.rs` (the recap, then), which was its
  only caller.
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

  Because the name is the second, **a run deletes the grabs it is not about to
  rewrite.** A re-run whose candidate set moved used to leave the old set
  behind, and once the source can change that means frames off a stream this
  run never looked at: the lecture with the dead slide capture kept a grab of
  the room's Crestron splash — the single candidate its black stream produced —
  sitting in the folder. Nothing reads a frame after the turn that asked for
  it, so an orphan's only power is to mislead whoever opens the folder next.
  The sweep touches `.jpg` files directly in the folder and nothing else, so
  the subfolders beside them survive: `live/` is the chat dock's grabs and
  `reading/` is the reading copy's. **They have to be separate folders.**
  Chapters and the reading copy are claimed independently — a reading run can
  start while a chaptering turn is still open on the same lecture — and the
  reading copy thins at 25 s against chapters' 90 s, so in one shared folder
  each job's sweep would take most of the other's frames out from under it. The chat dock's live grab of the
  playhead's moment shares that probing (`grab_frame`, and
  [harness.md](./harness.md)) — a frame the student asked about can land on a
  dropout exactly as easily as a boundary's can.

## Naming them is an agent job

`oculus lecture chapters <ID>` detects the candidates, grabs a frame for each,
writes the outline, and hands the lot to a CLI coding agent through `harness::run_once` — one
prompt, one turn, no thread, nothing in `harness_threads`. It is the first
caller of that function outside `oculus agent` (see
[harness.md](./harness.md)).

**The prompt is small on purpose, and that is the whole argument for driving a
coding agent rather than calling a model API.** It carries the lecture's title
and duration, the path to `outline.md`, the path to `frames/` and how those
files are named — and then stops. The agent opens the five frames it is unsure
about and reads the outline across the boundaries it doubts; a prompt to an API
would have to *carry* fifty frames and 2500 cues to let a model look at five
frames and read four spans. Four things in the prompt are lessons rather than
decoration:

- **A slide change is not a chapter.** Candidate density varies threefold between
  lectures of the same length (two 107-minute recordings in this library give
  15 and 49), so "one chapter per marker" would cut a busy deck every two
  minutes. The agent is asked for the number of things the lecture is actually
  about — usually five to eight, never more than twelve — and each marker
  carries its score to help it choose. The same sentence now says the converse
  too: a topic can turn where no slide changed, and a boundary may be any line
  in the outline. It is also told that a chapter shorter than about
  three minutes is a slide rather than a topic — without that line a 51-minute
  lecture split two adjacent slide titles into two chapters ninety seconds
  apart — and, in the same breath, that a long stretch of housekeeping or a
  worked example *is* a chapter if it lasts, because on its own the minimum
  read as licence to merge and the same lecture came back with its last eleven
  minutes folded into the chapter before them.
- **`grep -n "slide change" outline.md` is named in the prompt.** The markers
  are twenty lines in a file of two and a half thousand, and an agent that has
  not been told how to find them will consider reading all of it.
- **The room's AV splash screen is not a slide.** A dropout spanning the whole
  probe window survives frame selection, so a "connect your laptop" panel does
  reach the agent occasionally. A model looking at the image recognises one
  instantly once it has been told they exist; one that has not been told will
  name a chapter after it.
- **The subject's course folder is named.** An Echo360 title is a room booking
  ("MULT20015_2026_SM2 MO L105"), so without the folder the agent spends
  several turns hunting for the deck — measured on the first run.

### The outline is the document

Before the agent turn, `run` writes `lectures/<uuid>/outline.md`: the
transcript and the detected slide changes as one document in play order, every
line `second  timestamp  text`.

```
      1  00:00:01  Good morning.
    723  00:12:03  --- slide change · score 42.1 · pause ---
    725  00:12:05  An equal superposition.
```

**A file, not prompt text.** A lecture is around 2500 cues, and handing over a
path so the agent reads only the spans it wants is the same argument the frames
make. What merging buys is not brevity but correlation: "the picture changed
here" and "the subject turned here" arrive on adjacent lines instead of in two
documents with timestamp arithmetic in between. It replaces both halves of what
the prompt used to name — the candidate table and `transcript.vtt` — so there is
one document to read and no list to reconcile it against.

Both columns are printed because both are load-bearing: the clock is what a
person reads, and the bare second is what a chapter's `start` has to be
*exactly*. A model asked to convert one to the other will sometimes round, and
a rounded second is not in the allowed set.

A lecture with no transcript still gets an outline — the markers alone — and
the prompt says so, rather than sending the agent looking for words that are
not there.

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
boundary is one of the seconds the outline printed — the detected slide
changes, every transcript cue start, and second 0, which always counts because
the detector's first candidate is typically twenty seconds in.

That set is wider than it used to be, and the widening is the fix for the
lecture this page keeps coming back to. The allowed set was the candidate list
alone, so a recording whose slide capture was black offered a menu of one
however intact its transcript was: the agent understood the lecture perfectly
and had nowhere to cut. **It is still closed**, though — every second on the
menu came off a real file — so the agent cannot invent a timestamp and the
all-or-nothing rollback still means something. Frames are still grabbed at the
slide changes only; a frame per cue would be two and a half thousand JPEGs of
the same slide. A cue start is a place a chapter may *begin*, not a place
anything gets photographed.

`validate` also checks that starts are strictly increasing, that the first is
0, and that there are no more than twelve. A single failure means
nothing is written at all, and the error names the chapter the way
`projects::create_tasks` names a task — `chapter 3 ("…"): …`. The reason is
sharper here than for a task breakdown: drop the third of nine chapters and
there is no gap on the scrub bar to notice, only twenty minutes silently
attributed to the chapter before it.

## What is stored, and what is not

Migration 29: `lecture_chapters` (`lecture_id`, `idx`, `start_seconds`,
`title`, `summary`, cascading with the lecture) plus `chapter_status`,
`chaptered_at` and `chapter_error` on `lectures`.

- **`outline.md` and `frames/` are not stored, they are left.** Both sit in
  `lectures/<uuid>/`, both are overwritten by the next run, and neither is
  anything's source of truth — the same status the frames have always had. The
  chosen source is not recorded anywhere either: re-detecting is one decode,
  and a column would be a second place for it to be wrong.
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

## The reading copy

Chapters answer "what part of the lecture is this?"; the reading copy answers
"what was said, as text". The transcript's real defect is not its length but
that **maths is spoken** — "a naught ket zero plus a one ket one" — and that
speech is filler, restarts and repeats. A paraphrase pass renders
`$a_0|0\rangle + a_1|1\rangle$`, fixes the speech recognition from the slide,
drops the filler and gives one sentence per thought: the lecture as it would
read on the page. It replaced the **recap**, which produced ~150 words of
third-person narration per two-minute segment — 0.45× the transcript, so
neither notes nor a text you could read, under a label column that repeated
the chapter title.

**The unit is a line** (`ReadingLine` in `app/src-tauri/src/reading.rs`):
`start_seconds`, `para`, `text`. `start_seconds` is `floor(cue.start)` of the
first transcript cue the line covers — free and exact, because the prompt
prints every cue with that integer in its first column, and it is what lets
the player find the line from the playhead and the playhead from a line. A
line runs until the next line's start (`chapterEnds` again). A line covers two
to six cues (Echo360 cues are ~3 s), never more than eight; a two-hour lecture
is ~600 lines. There is no label, and no region yet — an attention overlay is
a later, separate pass.

**It shares chapters' evidence pipeline and not its shape.** `reading.rs`
reuses `sample_diffs`, collapse and pause scoring, and the same offset-probed
frame grab that avoids the room's AV splash — into `frames/reading/`, its own
folder for the reason [the sweep](#details-worth-not-rediscovering) describes.
Chapter candidates are thinned at 90 seconds; the reading copy starts from the
same candidates thinned at 25 seconds, merges a segment shorter than roughly
25 seconds into a neighbour, and splits one longer than roughly three minutes
at its longest transcript pause. Second 0 remains the first boundary. Those
segment starts are the **slide changes**: the prompt lists them, the frames
are grabbed at them, and paragraphs are derived from them — but a line does
not have to start on one, only on a cue.

**A run is windowed before it reaches the agent.** Consecutive segments are
grouped into roughly ten-minute windows, snapped to a chapter boundary within
three minutes when a chapter set exists. Windows run in sequence. Each prompt
carries that window's span of the transcript inline — every cue as
`second  timestamp  text`, so the number a line must cite is the first thing
on the row — lists the window's slide changes, and names the frame paths; the
agent opens the frames as images because the slide is what tells it the
notation. None of the transcript is optional reading, which is why the job
refuses a lecture without both its downloaded recording and transcript. A cue
belongs to the window its *start* falls in, so neighbouring windows never both
own the cue that straddles their edge, and a window with no cues at all is
skipped rather than asked about.

The reply is `[{start, text}]`, pulled out of prose, a fence or an envelope
the way `parse_chapters` does. **Validation is per window, and it is the
whole difference between a reading copy and a recap that drifted**, so every
rule names the line and its clock:

1. a non-empty reply, every `text` non-empty;
2. starts strictly increasing;
3. every start the integer second of a cue in this window;
4. the first line on the window's first cue;
5. **coverage** — the cues from a line's start up to the next line's start
   number at most `MAX_CUES_PER_LINE` (8) *and* speak for at most
   `MAX_SPEECH_PER_LINE_SECS` (45 s) between them, summed cue durations so
   silence does not count. A model that folds a minute of speech into one
   sentence has stopped rewriting and started summarising, and the window
   goes back with that line named. Both constants are guesses from cue
   statistics and exist to be tuned after one real run.

A bad window gets one retry with the error appended. Then `para` is set in
Rust — `mark_paragraphs`: true for the window's first line and for the first
line at or after each slide change — and never asked of the model, because
the slide changes are already known to the second and a model asked to mark
them would mark some other set. The model is selected through
`Job::LectureReading` in `app/src-tauri/src/harness/jobs.rs`; the CLI's
`--provider`, `--model` and `--effort` flags replace that selection for one
run.

**Windows are also the commit boundary.** Migration 34 replaces the recap's
table with `lecture_reading(lecture_id, idx, start_seconds, para, text)` plus
`reading_status`, `reading_written_at` and `reading_error` on `lectures`,
dropping `lecture_recap` and its three columns rather than converting them —
nothing in a recap row becomes a line — and rekeying the job's model in the
`job_models` settings row so a picked model survives the rename. A fresh run
atomically claims the lecture and clears the prior set; that shared gate stops
the app and CLI from spending two agent turns on the same lecture. Then
`store::save_reading_window` commits each validated window independently. If
window seven fails, windows one through six from the new run stay visible and
the lecture ends in `error`; this is intentionally different from the
all-or-nothing chapter write. There is no stored window or end second —
windows are job-time batching, and each line ends at the next line's start. A
startup sweep clears a stale `running` left by an interrupted process.

`oculus lecture reading <ID>` is the CLI door. It accepts the same unique
lecture-id prefix and one-run model overrides as chaptering, prints each
window before its agent tool rows, then every line as `hms  text`, and
requires `--force` before replacing an existing reading copy. The app's door
is `lecture_write_reading`, with `lecture-reading-progress` and
`lecture-reading` as its two events. No duration estimate is documented: the
reference lectures have not been measured with this job, so the panel offers
the shape of the cost ("one agent turn per ten minutes of recording") rather
than a number it would be inventing.

**The two jobs are independent, and share code rather than data.** The
reading copy calls `chapters::candidates_with_spacing`, `sample_diffs`,
`cue_gaps` and the same offset-probed `extract_frames` — the expensive,
measured half — but it requires no chapter set to run, and a lecture may have
either, both or neither. Where it *reads* chapters is `store::chapters` at the
top of the windowing step, and they change two things when present: a window
edge snaps to a chapter boundary within three minutes of the ten-minute
target, and each window's prompt names the chapter it sits inside. With no
chapters the snap falls back to ordinary chunking and the prompt simply omits
that line. So chaptering first is better, not required.

## Reading them in the player

Chapters are drawn three times over, because they are three different
questions (`app/src/components/lectures/LecturePlayer.tsx`, and see
[frontend.md](./frontend.md) for the player's own shape): as the dock's
chapter list, as the name over the frame, and as ticks on the scrub bar. The
reading copy is none of them — it is a second *register of the transcript*,
one tab over.

**The dock is Chapters / Transcript / Chat.** What was the transcript panel's
header is a `ViewTabs` strip sitting on the border it already had, with the
`DotsSixVertical` and the drag-to-dock gesture untouched. The tabs stop the
pointerdown from reaching the header: `startDockDrag` captures the pointer on
the element it fires from, which retargets the pointerup onto the header, and
a click needs both ends on one target — without that the tab would never
register one. Everything around the tabs still drags. There is no "In this
video" heading over them; the dock is 220px wide at its narrowest and its
subject is never in doubt. The Transcript tab is only offered when there are
cues, since a tab that could only ever be empty is not a tab; Chat needs
neither a file nor a run and so is always offered, which is what makes the
dock itself unconditional. Chapters is always offered too: it explains its own
empty state and carries the button that fills it.

**These two were briefly one tab, and it is worth saying why they are not.**
*Read* drew the chapters as headings over the reading copy's lines, on the
argument that a chapter is the heading of the text under it. It cost the one
thing the chapter list does well — twelve rows taken in at once, which is a
table of contents, where twelve headings scattered through six hundred lines
is not one — and it bought nothing the transcript's own list could not carry,
because everything the reading copy adds over the transcript is a *row
source*: the lines instead of the cues. So the chapter list came back whole,
and the reading copy moved into the tab it is a rewrite of.

### The chapter list

`app/src/components/lectures/ChaptersPanel.tsx` — five to twelve cards, each a
`<button>` that seeks, carrying the chapter's start, its length rounded to the
minute, its title, and its summary while it is the one playing. The seconds
are dropped from the length because the job will not name anything under about
three minutes a chapter, and `12m 04s` beside a wrapping title is what pushes
the title onto a third line in a 200px dock.

**It is deliberately not the transcript's follow machinery.** That code is
wound around a virtualizer and earns its two-stage handover and countdown ring
on ~2500 rows; twelve fit the panel with room over. So the playing card is
brought into view with `scrollIntoView({ block: "nearest" })` and nothing else
— no pill, no window, no ring. `nearest` scrolls the least that will do, so a
card already on screen does not jump to the middle of the panel under the eye.

A summary goes through `InlineMd`, which flattens paragraphs and lists to
spans: it sits inside the button that seeks, and a `<p>` in a `<button>`
closes the button early in WebKit. The renderer is there because the agents
write maths — `$\log_2 N$` sat in the panel as its own source until it was.

### The transcript has two registers

**Standard and Enhanced are one tab, not two**
(`app/src/components/lectures/TranscriptPanel.tsx`, and
`app/src/components/lectures/ReadingList.tsx` for the second). Standard is the
cue list: every ~3 s fragment as the recogniser heard it. Enhanced is the
reading copy over the same span — the same words with the filler dropped, the
notation fixed off the slide and the spoken maths set as maths. Same order,
same seek on click, same list: switching is a change of register, not a
navigation, which is the whole reason it is a picker in the search row
(`app/src/components/lectures/TranscriptModePicker.tsx`) and not a fourth tab.
It shares that row rather than taking one of its own, because a second row in
a 220px dock is a row of the transcript; the field takes what is left of the
width.

**The picker is also how the enhanced copy gets written**, which is the point
of it being a picker and not a segmented toggle. It is the source switcher's
shape (`SourceControls.tsx`): a row per option with what it is under its name,
and the row for the thing that is not there yet says so and starts the job —
*Enhance* where a downloaded source says *Download*. Picking it runs the job
**and** switches, because the enhanced view is the progress: the job commits
window by window, so its lines arrive into the list behind the panel that
asked for them. A run keeps a spinner on the trigger while you read the other
register, since it is minutes long and the list it has not filled yet is the
only other sign of it. The row carries the job's state as its own second line
— the phase while it runs, the agent's message and *Retry* when a run failed
with nothing to show for it, and the reason it cannot be picked at all when
the recording is not downloaded.

**A stored `enhanced` falls back per lecture** (`modeInFront` in
`TranscriptPanel.tsx`). The register is a habit carried between lectures and
most lectures have no enhanced copy, so opening one would otherwise mean an
empty panel with a Write button parked in it — a control you meet only by
first choosing the empty view. Instead the cues stay in front and the picker
is the one place the job is asked for. A run already in flight keeps the
enhanced view, because watching the windows land is the point. So `ReadingList`
has no "nothing yet" state at all: it is mounted with lines, or with a run
that has not committed its first window.

**The label and the backend noun differ on purpose.** Everything behind the
picker says *reading copy* — `lecture_reading`, `lecture_write_reading`,
`oculus lecture reading`, migration 34. "Enhanced" is what it is *next to
Standard*, where "Reading" would only be a second word for the thing both
registers are. They are one artifact under two names, and renaming the
artifact to match the label would cost a migration for a word.

**Enhanced is coarser than Standard, and that is the design.** A cue is a
three-second fragment and maths spans several of them, so a line covers two to
six cues — the validator's `MAX_CUES_PER_LINE` is what holds that. So the
enhanced list has fewer, longer rows and its highlight moves in ten- to
twenty-second steps rather than three-second ones. That is the granularity of
reading; following along word by word is what the other register is for.

**No chapter headings inside it.** The chapter list is the table of contents
and the name over the frame already says where the playhead is, so a heading
row in the text would be a third answer to a question already answered twice.

**It is one list.** ~600 lines and ~2500 cues are the same order of rows, so
both registers render through `app/src/components/lectures/FollowList.tsx` —
the virtualizer, the snap and band-rule follow-scroll, the nudge / unfollow /
soft-resume handover, the eight-second idle re-sync with its countdown ring on
the *Back to live* pill, the edge fades and the reopen-scroll after the dock
slides. Each register keeps only its own mapping into row space, and each keys
a row by what it *is* — a cue index, a line index — so a measured height
survives a search. They share one `following` flag in the player, since only
one of them is mounted at a time. A search narrows the rows to matching text;
a matched enhanced line is shown as plain text with the hit marked, because
`InlineMd` cannot carry a `<mark>` through it, so a hit's maths reads as its
source until the query is cleared.

**A line's markdown is inline**, for the chapter summary's reason: the row is
a `<button>` that seeks, and for the reading copy the maths is the point. A
`para` line opens a paragraph gap, and the gap is padding on the row's
positioned wrapper rather than a margin on the button — the virtualizer
measures a row's border box, and a margin on an absolutely positioned row
would push it onto the row below by an amount the list never learns of. The
gap is suppressed while searching, where consecutive rows are not consecutive
lines.

**Which register is in front is a player preference**, `transcriptMode` beside
the dock's tab, side and size in `playerPrefsStore` — a habit, not a property
of one recording. It defaults to `standard` for the reason the dock defaults
to Transcript: every downloaded lecture has one, and the enhanced copy has to
be asked for.

**A chapter's end is derived, in the reader.** `chapterEnds` in
`app/src/lib/lectures.ts` is the whole of it — the next chapter's start, or the
lecture's duration for the last. It is given the *player's* duration, which is
the element's where a file is loaded rather than the catalogue's. `spanAt`
beside it answers "which one is the playhead in" over any ordered list of
starts, and the player asks it twice: once over the chapters, once over the
lines.

### How much of this bit is left

`EntryProgress` (`app/src/components/lectures/EntryProgress.tsx`) is a 2px
brand-coloured line across the top edge of the playing chapter's card, filled
by the playhead's position inside *that* chapter's span — not the lecture's,
which the scrub bar already says. Lines do not get one: ten to twenty seconds
is not a span worth measuring, and the highlight moving down the page already
is the progress.

**The fill used to be a hairline above the scrub bar**, beside the chapter's
name inside the controls scrim. It said the same thing over the frame, where it
had to be white-on-black and faded out with the control bar; on the card it
is beside the title it is measuring and it stays. Only the current chapter is
given one: a track on all twelve would read as a ladder rather than as a
playhead, and the highlight already says which card is current.

**The name stayed put.** Which chapter is playing is the one thing the scrub
bar cannot say and the dock only says when it is open, so it is still written
over the frame, in the scrim, fading with the controls.

The playhead reaches it as `atRef`, the same `RefObject<number>` the chat
composer's moment chip reads, and the line writes its own width on a 200 ms
interval rather than through state — the panel is memoised against a player
that re-renders four times a second, and a `currentTime` prop would throw that
memo away on every frame and put the virtualised list back beside a decoding
video.

**The ticks stay on the scrub bar.** Boundaries are notched into the `SeekBar`
track as a 2px cut in the scrim's own black, which reads against the played
fill and the unplayed track alike; a segmented bar was the alternative and
costs the rounded ends and the growing hover height that make it read as one
bar. Second 0 is the left edge, so it is not drawn. Those colours are fixed
rather than semantic on purpose — the bar sits on the frame, where `background`
is whatever the lecturer put on the slide.

**Which tab is in front is a player preference**, `dockTab` beside the dock's
side and size — a habit like the side it is docked to, not a property of one
recording. It defaults to the transcript. A stored `chapters` still means the
chapter list; a stored `recap` or `read`, from the two shapes that came
between, falls to the default through the same tolerant read. **What order
they sit in is the same kind of preference** (`dockTabOrder`), dragged
tab-on-tab in the header; see [frontend.md](./frontend.md) for how a stored
order survives a tab being added, removed or hidden.

### Two jobs, two panels

Both read their state from SQLite (`app/src/hooks/useLectureChapters.ts`,
`app/src/hooks/useLectureReading.ts`) rather than from the `lectures` row the
player was handed: in the side panel that row is a snapshot held by a store
and in a list it is whatever the last `getLectures` returned, and neither is
re-read when a run lands minutes later. Each hook owns its read, listens for
its job's finish event, and re-reads on it; each keeps a module-level map of
when a run this session started and the last step it reported, because the
player unmounts on every tab switch and the job outlives it.

- **Lines appear while the job is still running.** The reading copy commits
  window by window, so the panel shows the list *and* the footer's running
  line together, and the hook re-reads the table on the `writing` phase rather
  than only at the end. This is the visible half of a decision that would
  otherwise live only in the database. Chapters are all-or-nothing in one
  transaction, so there is nothing to show until that run ends.
- **The reading copy can say how far through itself it is; chaptering cannot.**
  It is a countable sequence of agent turns, so its running line carries
  "3/7" — a fact, where a chaptering run's single turn has no denominator and
  is still given none.
- **Both can run at once**, claimed independently in Rust. They no longer share
  a footer to stack their running lines in: each panel shows its own job and
  says nothing about the other's.

### Every state is a real one

Nothing here shows a control that does nothing, so there is no placeholder
among these states. The two jobs put them in different places, though: the
chapter list is its own panel and carries its own, while the enhanced copy's
live in the picker until there is a list to put them beside.

- **Not downloaded.** Both jobs watch the recording. The chapter tab says so;
  the picker's Enhanced row says so as its second line and cannot be picked.
  The download button is already on the frame and on the control bar; neither
  grows a third.
- **No transcript on disk.** The Transcript tab is dropped from the strip
  entirely, which takes the enhanced register with it — and Rust refuses the
  reading job without a transcript anyway, so the register that could not run
  is the one that is not offered. Chapters need only the recording, so that
  tab is untouched and is what the dock falls to (`tabInFront`).
- **Nothing yet.** Chapters offers **Find chapters** and its 8–11 minutes in
  the panel. Enhanced has no panel state at all — `modeInFront` keeps the
  cues in front — so it is the picker's row that offers it, as *Enhance*
  beside the register's name. No duration is quoted for it: the reference
  lectures have not been measured with this job, and the panel says the shape
  of the cost ("one agent turn per ten minutes of recording") rather than a
  number it would be inventing.
- **`running`.** A spinner in `brand` — the accent the app spends on work in
  flight — the phase it is on, the step under it, and a clock counting *up*.
  See [Saying what it is doing](#saying-what-it-is-doing). There is still no
  bar: a fill towards an estimate reaches the end and keeps waiting, which is
  exactly what hung looks like, and an agent turn cannot say how far through
  itself it is. A run already in flight when the app started has no start
  time anywhere — `chaptered_at` and `reading_written_at` are stamped by a
  terminal status only — and is shown without a clock rather than with a
  wrong one.
- **`error`.** The job's own message as the agent left it, and a retry. With
  rows already on screen the message sits in the footer beside them: a
  regenerate that fails leaves what was there (`store::save_chapters` is one
  transaction; the reading copy's windows before the failure are kept), so the
  failure is a line *beside* the rows rather than in place of them. A reading
  run that failed with nothing to show for it has no footer to sit in, so it
  is the picker's Enhanced row that carries the message and the *Retry*.
- **Rows.** The list, with one **Regenerate** in that panel's own footer. The
  menu of two jobs that footer briefly held went with the tab that had both of
  them in it; one panel, one job, one word.

**Nothing is hand-editable.** Chapters and lines are derived data an agent
writes, like `pages` and `parse_status`, so the only affordances are the two
jobs — there is no boundary to drag and no sentence to retype.

**The buttons are disabled while a run is in flight, not apologetic after the
fact.** Rust refuses a second call with "that lecture is already being
chaptered", and an error message is the wrong place to learn that a button was
never going to work.

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
