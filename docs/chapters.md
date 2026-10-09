# Lecture chapters

An agent job over a visual pipeline for Echo360 recordings: **chapters** cut
a lecture at its real topic boundaries and name each span. It runs from the
CLI (`oculus lecture chapters`) and from the player, through the same Rust
`run`. A second, much lighter job finds [where the lecture
ends](#where-the-lecture-ends) before the Q&A (`oculus lecture end`).

## Where

| Piece | Location |
| --- | --- |
| Detection, stream choice, frame grabs, outline, prompt, reply parser, validator, the job, `Step` | `app/src-tauri/src/chapters.rs` |
| The app commands and job-specific events | `chapters::app` in `app/src-tauri/src/chapters.rs` |
| Shared run inputs, recording checks, app progress and thread orchestration | `app/src-tauri/src/lecture_jobs.rs` |
| Row writes, status, claims, reconcile; the tables (migrations 29, 42) | `app/src-tauri/src/store.rs`; `app/src-tauri/src/migrations.rs` |
| Where the lecture ends: window, picture hint, brief, prompt, reply parser, validator, app command | `app/src-tauri/src/lecture_end.rs` |
| `oculus lecture candidates` / `chapters` / `end` | `app/src-tauri/src/bin/oculus/lecture.rs` |
| Which agent, model and effort each job runs on | `app/src-tauri/src/harness/jobs.rs`, `app/src/pages/settings/JobsPage.tsx` |
| Transcript search and stable source indexes | `app/src/hooks/useTranscriptSearch.ts` |
| The one-turn headless run chapters use; ffmpeg lookup | `run_once` in `app/src-tauri/src/harness/mod.rs`; `app/src-tauri/src/echo360.rs` |
| Frontend bindings, event names, `chapterEnds`; `spanAt` and the VTT parser | `app/src/lib/lectures.ts`, `app/src/lib/media.ts` |
| The hidden opencode agent the end job runs as | `app/src-tauri/templates/OPENCODE.template.json` |
| The end in the player: first-open trigger, landed ends, `lectureEnd` | `app/src/lib/lectureEnd.ts`, `app/src/hooks/useLectureEnd.ts` |
| Rows and job state; the job hooks | `app/src/lib/db.ts`; `app/src/hooks/useLectureJob.ts`, `app/src/hooks/useLectureChapters.ts` |
| The dock's tab strip | `app/src/components/lectures/TranscriptPanel.tsx` |
| Chapter list and its run status | `app/src/components/lectures/ChaptersPanel.tsx`, `app/src/components/lectures/RunStatus.tsx` |
| Chapter name over the frame, scrub ticks, card progress line | `app/src/components/lectures/LecturePlayer.tsx`, `app/src/components/lectures/EntryProgress.tsx` |
| Dock tab and tab order | `app/src/stores/playerPrefsStore.ts` |
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
holds the slides varies even within one subject. `chapters::detect` keeps
source 1 unless it is **dead** — at most `DEAD_SOURCE` (2)
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
- Chapters write `frames/`, the chat dock `frames/live/`; Up Next's thumbnail
  is `thumb.jpg` beside them
  ([viewers.md](./viewers.md#up-next-offers-the-subjects-next-lecture)). They
  must stay separate: a chapters run's sweep would delete the dock's grabs.
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
override it for one run. The default is Codex `gpt-5.6-luna` at `xhigh`.

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

**The jobs share code, not data.** `lecture_jobs` owns the run inputs,
recording checks and app orchestration of chapters and the lecture end. Each
job owns when it claims the lecture and commits its output.

## Where the lecture ends

Recordings run on after the lecturer signs off — students at the lectern,
packing up, a black projector — so "watched to the last 30 s" may never
happen. `lecture_end` finds the line the lecturer finishes on and stores
`lectures.content_end_seconds`. Whether a line is the lecturer wrapping up or
a student saying thanks mid-Q&A is a language judgement, so a model reads the
transcript's tail; a phrase list both missed wordings and matched inside the
Q&A.

**It is one tool-less turn, not an agent session.** It runs through
`Harness::one_turn` ([harness.md](./harness.md#one-off-turns)) on the
`lectureEnd` job's model — no skills, no `AGENTS.md`, no files to open — with
the brief in `lecture_end::INSTRUCTIONS` (opencode's `oculus-lecture-end`
agent). The app passes its own harness; the CLI makes one per command. The
default is Claude `claude-haiku-4-5-20251001` with no effort level. On 30
hand-labelled recordings it and Codex `gpt-5.6-luna` at `low` each place 28
ends within 15 s (a phrase list, 26), on a prompt of ~4k tokens.

**The prompt carries the tail inline.** The window is every cue starting in
the last `TAIL_SECS` (900 s) of the recording, whose length is the row's
duration or the last cue's end, whichever is later. Each line is `second
clock  Speaker N: text`, both columns for the reason the outline has them;
the speaker comes from the cue's `<v Speaker N>` tag
(`chapters::parse_transcript_voiced`) and is printed only where the voice
changes — one label per cue doubled the prompt. Above the lines: title, course code and length.

**The picture adds one hint, never a verdict.** With the recording on disk,
`chapters::tail_luma` decodes only its last 900 s (`-sseof`) and `black_tail`
looks for a run under luma 10 of at least 60 s that lasts to the end of the
file, after at least 60 s of picture. Less picture than that is a dead
capture, so source 2 is read instead, if downloaded. A run found becomes one
prompt line, "The projector goes black from 48:45 (2925) to the end of the
recording." Frames are aligned to the file's own length from ffmpeg's log,
which can run seconds past the row's duration. A still picture is not a hint:
the last slide is often held while the lecturer talks on. No video, no hint;
the job needs only the transcript.

**The reply cites a line and quotes it.** `{"ends_at": <second>, "quote":
"<3–12 words>"}`, or both null for a recording cut off mid-lecture.
`parse_reply` tolerates the wrapper as chapters' does (prose, fences, an
envelope, a float, or a string such as `"1911 31:51"` whose first word is
the second). `validate` does not tolerate the content:

- `ends_at` must lie inside the transcript shown;
- the quote — lowercased, punctuation to spaces, whitespace collapsed, matched
  on word boundaries — must lie in a line starting within `NEAR_SECS` (30 s)
  of it, or run across into the next line, since a sign-off often spans two
  cues. The nearest such line wins: small models cite the line before the
  one they quote, so the quote is the evidence and the second only says where
  to look;
- a quote found only further away is an error naming the line it is in; a
  second with no quote, or a quote with no second, is an error.

A rejected reply is asked again once with the reason appended; a second
rejection is the run's error. A provider failure is not retried. The stored
end is the **end** of the line the quote finishes in, not the cited start.

**What is stored** (migration 42): `content_end_seconds`, `content_end_quote`,
`content_end_status` (`NULL | running | ready | none | error`) and
`content_end_error`. `store::claim_content_end` claims atomically — refused
while `running`, and over `ready` or `none` unless forced; an `error` re-runs.
`store::save_content_end` writes the result and, in the same transaction,
marks the lecture Done when `progress_seconds` is already within 10 s of the
end. A failed run keeps the previous end, as a failed regenerate keeps old
chapters; `store::reconcile_content_end_status` sweeps a stale `running` at
startup.

**The app door** is `lecture_find_end`: it checks the transcript, claims,
returns, and emits `lecture-end` (`{lectureId, status, seconds, quote,
error}`) when the turn is done. No progress events — there is one turn and no
tools.

**The player runs it on first open.** `useLectureEnd` calls `openLectureEnd`
(`app/src/lib/lectureEnd.ts`), which reads the columns from SQLite rather
than the player's row and, when `content_end_status` is `NULL` and a
transcript is on record, starts one run — once per lecture per session, a
module-level set surviving remounts and StrictMode. Never on `running`,
`ready`, `none` or `error`; a lecture without a transcript runs once one
arrives. It shows nothing while running. The module keeps each lecture's
latest state, from SQLite or the event, outside React, so Done and Up Next
use an end that lands mid-viewing ([viewers.md](./viewers.md)).

**A list runs it for lectures already started.** The subject's Lectures page
and Home's Lectures card pass their rows to `findStartedLectureEnds`, which
starts the same run for each one with a position past 5 s, not Done, a
transcript and a `NULL` status — sharing the once-per-session set. A position
already past the end it finds then marks the lecture Done with no reopening.

**A failed run shows in the Chapters tab**, the dock's home for lecture jobs:
one line under the list, "Couldn't find where this lecture ends — <error>",
with Retry. Retry passes no `force` — the claim re-runs an `error`, and
refuses rather than replace an end found meanwhile. An invoke that never
claims (the transcript gone from disk) shows the same line.

## Reading them in the player

Chapters appear three ways — the dock's chapter list, the name over the frame,
and ticks on the scrub bar (2px cuts in the scrim's own black, fixed colours
because the bar sits over the slide). A found lecture end is a white flag on
the same bar, standing proud of the track, with the stretch after it dimmed
(`endAt` on `MediaPlayer`, from `contentEnd`).

**The dock is Chapters / Transcript / Chat.** Transcript is offered only
when there are cues (`tabInFront` falls back to Chapters). The tabs stop
`pointerdown` reaching the header, whose `startDockDrag` captures the pointer
and would retarget the `pointerup`, killing the click. Tab and tab order are
player preferences; an unknown stored value falls to the default.

**The chapter list** is up to twelve seeking `<button>` cards; the playing
one is brought into view with `scrollIntoView({ block: "nearest" })` rather
than `FollowList` — twelve rows need no virtualiser.

**Summaries render through `InlineMd`**: a `<p>` inside the seeking
`<button>` closes it early in WebKit.

**`EntryProgress`** is a 2px line across the playing chapter's card, filled by
the playhead's position within that chapter. It reads the playhead from
`atRef` on a 200 ms interval rather than a prop, which would break the panel's
memo against a player re-rendering four times a second.

**Job state is read from SQLite**, not the `lectures` row the player holds,
which nothing re-reads when a run lands. `useLectureJob` keeps module-level
maps of each run's start and last step, because the player unmounts on every
tab switch and the job outlives it. A running state has a spinner and a
clock counting up but no bar — an agent turn has no denominator — and a run
found in flight at app start shows no clock, since `chaptered_at` stamps
terminal states only. Buttons are disabled while a run is in flight.

## Gotchas

- Nothing is hand-editable — a regenerate's delete-and-insert would silently eat an edit.
- Keep both `second` and `timestamp` in the outline and the end prompt; a model converting between them rounds and fails validation.
- Keep the startup reconciles, or a crashed run leaves the lecture `running` and every retry refused.
- `oculus lecture end --dry-run` and `--print-prompt` must not touch the `content_end_*` columns: they run against databases without migration 42.
- Don't listen on `lectures-changed` for a job's end; it fires on every progress save.
- `cue_gaps`/`parse_transcript` must read timestamps the way `parseVtt` in `app/src/lib/media.ts` does (`HH:MM:SS.mmm` and `MM:SS.mmm`), or a boundary lands on a different second in each.
- Don't use a `currentTime` prop in the dock panels; it breaks their memo — read `atRef`.
