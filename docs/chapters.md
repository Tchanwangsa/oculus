# Lecture chapters

Two agent jobs over one visual pipeline for Echo360 recordings: **chapters**
cut a lecture at its real topic boundaries and name each span, and the
**reading copy** rewrites the transcript as readable text, one sentence per
line, each pinned to its second. Both run from the CLI (`oculus lecture
chapters`, `oculus lecture reading`) and from the player, through the same
Rust `run`.

## Where

| Piece | Location |
| --- | --- |
| Detection, stream choice, frame grabs, outline, prompt, reply parser, validator, the job, `Step` | `app/src-tauri/src/chapters.rs` |
| The app commands and job-specific events | `chapters::app` in `app/src-tauri/src/chapters.rs`, `reading::app` in `app/src-tauri/src/reading.rs` |
| Shared run inputs, recording checks, app progress and thread orchestration | `app/src-tauri/src/lecture_jobs.rs` |
| Reading copy: segmentation, windows, prompt, validation, `para`, the job | `app/src-tauri/src/reading.rs` |
| Row writes, status, claims, reconcile; the tables (migrations 29, 34) | `app/src-tauri/src/store.rs`; `app/src-tauri/src/migrations.rs` |
| `oculus lecture candidates` / `chapters` / `reading` | `app/src-tauri/src/bin/oculus/lecture.rs` |
| Which agent, model and effort each job runs on | `app/src-tauri/src/harness/jobs.rs`, `app/src/pages/settings/AiPage.tsx` |
| Transcript search and stable source indexes | `app/src/hooks/useTranscriptSearch.ts` |
| The one-turn headless run both use; ffmpeg lookup | `run_once` in `app/src-tauri/src/harness/mod.rs`; `app/src-tauri/src/echo360.rs` |
| Frontend bindings, event names, `chapterEnds` / `spanAt`, the VTT parser | `app/src/lib/lectures.ts` |
| Rows and job state; the job hooks | `app/src/lib/db.ts`; `app/src/hooks/useLectureJob.ts`, `app/src/hooks/useLectureChapters.ts`, `app/src/hooks/useLectureReading.ts` |
| The dock's tab strip and register picker | `app/src/components/lectures/TranscriptPanel.tsx`, `app/src/components/lectures/TranscriptModePicker.tsx` |
| Chapter list, enhanced list, shared run status, follow list | `app/src/components/lectures/ChaptersPanel.tsx`, `app/src/components/lectures/ReadingList.tsx`, `app/src/components/lectures/RunStatus.tsx`, `app/src/components/lectures/FollowList.tsx` |
| Chapter name over the frame, scrub ticks, card progress line | `app/src/components/lectures/LecturePlayer.tsx`, `app/src/components/lectures/EntryProgress.tsx` |
| Dock tab, tab order, transcript register | `app/src/stores/playerPrefsStore.ts` |
| Tool verbs shared with the chat timeline; parser fixture | `toolVerb` in `app/src/lib/harness.ts`; `app/src-tauri/fixtures/chapters/sample.vtt` |

## Detection is one decode against one fixed threshold

1. **Choose the stream** ([below](#detect-picks-the-stream-and-never-by-candidate-count)).
2. **Sample** — one `fps=1,scale=160:90,format=gray` decode to a pipe, each
   frame diffed against the last as it arrives; nothing is collected.
3. **Collapse** loud frames within 3 s into one event, at its first second.
4. **Score** — magnitude, plus a small bonus for a ≥2 s pause within ±8 s.
5. **Thin** strongest-first at 90 s spacing, so the important one of two
   nearby boundaries survives, then back into play order.

The constants in `chapters.rs` rest on measurements of real lectures:

- A slide capture is bimodal — held slide p50 ≈ 0.008, change p99 ≈ 13, max ≈
  175 — so the threshold (6) sits in the empty middle. Threshold 2 and 6 give
  the same first twelve boundaries, so there is no knob, flag or setting.
- A room camera is never still (p50 ≈ 1.4, max ≈ 20): it has no gap to put a
  threshold in, so it is outside this detector's design, not a harder case.
- Only 25–30 % of slide changes have a nearby ≥2 s pause, so a pause can only
  be a score bonus, never a gate. No transcript just loses the bonus.
- A 42-minute lecture decodes in ~5 s, so nothing is cached and detection
  writes nothing (`oculus lecture candidates --frames` adds JPEGs to check by eye).

## `detect` picks the stream, and never by candidate count

A capture has up to two streams (`source1.mp4`, `source2.mp4`) and which one
holds the slides varies even within one subject. `chapters::detect`, used by
both jobs, keeps source 1 unless it is **dead** — at most `DEAD_SOURCE` (2)
candidates; a failed capture gives ~1, a healthy one 16–23 — and only then
decodes source 2.

Picking the stream with more candidates is wrong: a room camera saturates
the threshold and out-scores a real deck (20 vs 16 on one lecture). The choice
is never persisted or remembered per subject. `--source 1|2` (CLI) and the app
commands' `source` argument override it.

## Frame grabs probe past the boundary

The boundary second often shows black (screen share restarting) and a fixed
+2 s often shows the room's "connect your laptop" splash. `grab_frame` probes
`GRAB_OFFSETS` (+2, +6, +12, then 0) with the same keyframe seek the grab
uses, and takes the earliest within 5 % of the most detailed — relative,
because "detailed" depends on the deck. The file is named by the **boundary**
second.

- `extract_frames` deletes `.jpg`s in its folder that this run will not
  rewrite, so a folder never mixes candidate sets or streams.
- Chapters write `frames/`, the reading copy `frames/reading/`, the chat dock
  `frames/live/`. They must stay separate: the jobs are claimed independently
  and thin at different spacings, so a shared folder's sweep would delete the
  other job's frames mid-run.
- `GRAB_WIDTH` (768 px, ~30 KB) keeps formulas legible across dozens of
  frames; the dock's single grab uses `LIVE_GRAB_WIDTH`.

## Naming is one agent turn over files, not a prompt carrying them

`chapters::run` detects, writes `lectures/<uuid>/outline.md`, grabs frames,
and hands one prompt to `harness::run_once` — one turn, no thread. The prompt
carries paths: the agent opens only the frames and spans it doubts, where an
API call would have to carry fifty frames and ~2500 cues.

**The outline** is the transcript and the slide-change markers merged in play
order, every line `second  timestamp  text`. Both columns are printed because
a model converting a clock to seconds sometimes rounds, and a rounded second
fails validation. A lecture with no transcript gets markers only, and the
prompt says so.

Prompt lines that exist because a run went wrong without them:

- A slide change is a place you *may* cut; candidate density varies 3× between
  same-length lectures (15 vs 49), so the agent is asked for 5–8 chapters,
  never more than `MAX_CHAPTERS` (12). A topic may also turn at any cue.
- Under ~3 minutes is a slide, not a chapter — but a long housekeeping stretch
  *is* one (without that clause the minimum reads as licence to merge).
- `grep -n "slide change" outline.md` finds the markers in a 2500-line file.
- The room's AV splash is not a slide; never name a chapter after one.
- The course folder is named, since an Echo360 title is a room booking.

**Rust writes the rows.** Chapters are derived data like `pages`, so the agent
replies with JSON and there is no `oculus chapter` write command.
`parse_chapters` tolerates the wrapper (prose, fences, an envelope, floats);
`validate` does not tolerate the content:

- every start is in a **closed set**: second 0, the slide changes, and every
  cue start — so a lecture whose slide capture is black can still be chaptered
  from speech, but no timestamp can be invented;
- the first start is 0, starts strictly increase, at most 12 chapters;
- one bad chapter rejects the whole set, naming it (`chapter 3 ("…"): …`);
  dropping one would silently hand its span to the chapter before.

## What is stored

- `lecture_chapters` cascades with the lecture; `lectures` carries
  `chapter_status` (`NULL | running | ready | error`), `chaptered_at`
  (terminal states only) and `chapter_error`.
- **No end column**: `chapterEnds` derives it from the next start or the
  *player's* duration.
- `store::save_chapters` deletes and inserts in one transaction, so a failed
  regenerate keeps the old set; replacing one needs `--force` or Regenerate.
- `store::reconcile_chapter_status` sweeps a stale `running` to NULL at startup.
- `outline.md` and `frames/` are left on disk, overwritten by the next run,
  and are nobody's source of truth.

## The app runs the same job and streams its phases

Each job's provider, model and effort come from the per-job registry
([harness.md](./harness.md)); CLI `--provider` / `--model` / `--effort`
override it for one run. Defaults: Codex `gpt-5.6-luna` at `xhigh` for
chapters, `medium` for the reading copy.

`lecture_find_chapters` runs the same `chapters::run`, claiming status before
the decode, and returns once claimed. `check_start` refuses a second call
while `chapter_status` is `running`.

- **Finish** is `lecture-chapters` (lecture, `ready`/`error`, message). Not
  `lectures-changed`, which fires on every playback-progress save; not the
  harness stream, since a headless run reports on thread 0.
- **Progress** is `lecture-chapter-progress`, never persisted, fed by the
  pipeline's `Step`s and the agent's `HarnessEvent`s, kept apart (the CLI
  prints only tool rows and the `Detected` line). Phases: `decoding` (throttled
  to 250 ms in `run`) → `frames` → `agent` (each `ToolStarted` via `toolVerb`)
  → `naming` (the reply's first delta, once — the reply *is* the JSON) →
  `writing`. Chapters themselves are never streamed.

## The reading copy

A rewrite, not a summary: spoken maths set as maths, recognition errors fixed
from the slide, filler dropped. **A line** (`ReadingLine`) is `start_seconds`
(`floor` of its first cue's start), `para` and `text`, ending at the next
line's start; it covers two to six cues, ~600 lines for two hours.

**Segments are denser than chapters.** The same candidates are thinned at 25 s
instead of 90, short edge segments merged, and any segment over 3 minutes
split at its longest pause. Segment starts are the slide changes: the prompt
lists them, frames are grabbed at them, paragraphs break at them. Lines start
on cues, not on segments.

**The job is windowed.** Segments group into ~10-minute windows, snapped to a
chapter boundary within 3 minutes when chapters exist, run in sequence. Each
prompt carries its window's cues inline as `second  timestamp  text`, lists
its slide changes, and names its frames. A cue belongs to the window its start
falls in; a window with no cues is skipped. The job refuses a lecture without
a downloaded recording and transcript.

**Validation is per window**, and every error names the line and its clock:
non-empty text; strictly increasing starts; each start a cue second in this
window; the first on the window's first cue; and **coverage** — at most
`MAX_CUES_PER_LINE` (8) cues and `MAX_SPEECH_PER_LINE_SECS` (45 s of summed
speech) per line, which is what stops a rewrite becoming a summary. A rejected
window is retried once with the error appended. `mark_paragraphs` then sets
`para` in Rust (window's first line, and the first line at or after each
slide change) — never the model, which would mark a different set.

**Windows are the commit boundary.** `store::claim_reading` atomically claims
the lecture and clears the old copy, so the app and CLI cannot both spend
turns on it; `store::save_reading_window` commits each validated window. A
failure in window seven keeps windows one to six visible and ends in `error`
— deliberately unlike chapters' all-or-nothing write. No window or end second
is stored; `reading_status` / `reading_written_at` / `reading_error` and a
startup reconcile mirror chapters. The app door is `lecture_write_reading`,
with `lecture-reading-progress` and `lecture-reading`.

**The jobs share code, not data.** `lecture_jobs` owns their run inputs, source
loading, recording checks and app orchestration. Each job owns when it claims
the lecture and commits its output. The reading copy needs no chapter set; when
one exists it reads `store::chapters` only to snap window edges and to name
the enclosing chapter in each prompt.

## Reading them in the player

Chapters appear three ways — the dock's chapter list, the name over the frame,
and ticks on the scrub bar (2px cuts in the scrim's own black, fixed colours
because the bar sits over the slide). The reading copy is the Transcript tab's
**Enhanced** register beside Standard.

**The dock is Chapters / Transcript / Chat.** Transcript is offered only
when there are cues (`tabInFront` falls back to Chapters). The tabs stop
`pointerdown` reaching the header, whose `startDockDrag` captures the pointer
and would retarget the `pointerup`, killing the click. Tab, tab order and
register are player preferences; an unknown stored value falls to the default.

**The chapter list** is up to twelve seeking `<button>` cards; the playing
one is brought into view with `scrollIntoView({ block: "nearest" })` rather
than `FollowList` — twelve rows need no virtualiser.

**Standard and Enhanced are one list.** Both render through `FollowList`,
keying rows by cue or line index so measured heights survive a search.
"Enhanced" is the UI label; everything behind it says *reading copy*.

- The picker (`TranscriptModePicker.tsx`) is the enhance job's control:
  picking Enhanced with no copy runs the job *and* switches, since lines
  arrive window by window. Its row carries the phase, error and Retry, or why
  it cannot run.
- `modeInFront` shows Standard when the stored register is `enhanced` but this
  lecture has neither a copy nor a run — so `ReadingList` has no empty state.

**Summaries and enhanced lines render through `InlineMd`**: a `<p>` inside
the seeking `<button>` closes it early in WebKit. An Enhanced search hit shows
plain text, since `InlineMd` cannot carry a `<mark>`. A `para` gap is padding
on the positioned wrapper, not a margin the virtualiser cannot measure.

**`EntryProgress`** is a 2px line across the playing chapter's card, filled by
the playhead's position within that chapter. It reads the playhead from
`atRef` on a 200 ms interval rather than a prop, which would break the panel's
memo against a player re-rendering four times a second.

**Job state is read from SQLite**, not the `lectures` row the player holds,
which nothing re-reads when a run lands. `useLectureJob` keeps module-level
maps of each run's start and last step, because the player unmounts on every
tab switch and the job outlives it. The reading hook re-reads on `writing`
so lines appear mid-run, and shows `3/7` windows. A running state has a
spinner and a clock counting up but no bar — an agent turn has no
denominator — and a run found in flight at app start shows no clock, since
the `*_at` columns stamp terminal states only. Buttons are disabled while a
run is in flight.

## Gotchas

- Nothing is hand-editable — a regenerate's delete-and-insert would silently eat an edit.
- Keep both `second` and `timestamp` in the outline and the reading prompt; a model converting between them rounds and fails validation.
- Don't let the model set `para`; `mark_paragraphs` knows the slide changes exactly.
- Keep the startup reconciles, or a crashed run leaves the lecture `running` and every retry refused.
- Don't listen on `lectures-changed` for a job's end; it fires on every progress save.
- `cue_gaps`/`parse_transcript` must read timestamps the way `parseVtt` in `app/src/lib/lectures.ts` does (`HH:MM:SS.mmm` and `MM:SS.mmm`), or a boundary lands on a different second in each.
- Don't use a `currentTime` prop in the dock panels; it breaks their memo — read `atRef`.
