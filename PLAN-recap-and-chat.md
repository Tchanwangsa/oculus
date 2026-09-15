# Plan: lecture Recap and the player's Chat tab

Untracked working plan. Not a doc page — `docs/` describes what is built, this
describes what is about to be. Delete it when the work lands.

Two features for the lecture player's dock, beside Chapters and Transcript:

1. **Recap** — one note per slide, read after zoning out. A CLI-agent job like
   chapters, with its own table and tab.
2. **Chat** — a lecture-scoped conversation in the dock, each message carrying
   the moment the playhead is at. Multiple threads per lecture, new + history
   like the Claude Code side panel.

Decisions taken so far (Tanat, 2026-09-15):

- Recap is **not** part of Chapters. Different density, different reading mode,
  shared pipeline.
- Recap bodies are **markdown with LaTeX**, rendered the way the chat timeline
  renders replies.
- Chat is **not** one persistent thread per lecture. New thread from the
  header, history to go back to an earlier one.
- Recap segments follow **slide changes**, not fixed minutes (recommended;
  stands unless overruled).

---

## Part 1 — Recap

### What it is

Forty to a hundred short notes per lecture, one per visual change, each saying
what is on that slide and what was said over it. The reader is someone who
looked away for thirty seconds and wants the last note or two, not the
transcript.

### Job (Rust, `app/src-tauri/src/chapters.rs` or a sibling `recap.rs`)

Reuses `sample_diffs`, `collapse`, `extract_frames`, `harness::run_once`, and
the per-job model registry (`harness/jobs.rs`).

1. **Segment.** Same decode as chapters. `candidates` takes the thinning
   radius as an argument (caller's parameter, not a user knob): ~25 s for
   recap vs 90 s for chapters. Then:
   - floor: a segment under ~25 s merges into its neighbour;
   - ceiling: a segment over ~3 min is split at the longest transcript pause
     inside it, so a lecturer who talks over one slide for six minutes still
     gets two notes.
   - Second 0 is always the first segment.
2. **Grab a frame per segment** with the existing offset probing (the
   splash-screen defence). Into `lectures/<id>/frames/` as now — same naming,
   same overwrite rule. Chapter and recap frames can share the folder since
   both are keyed by boundary second.
3. **Chunk into windows** of about ten minutes, snapped to a chapter boundary
   where chapters exist. One agent turn per window, run **in sequence**. A
   107-minute lecture is ~100 frames plus ~20k words; that is not one turn.
4. **Prompt per window** (see below). The window's transcript span is
   **inlined**, frames are given as paths. Unlike chapters, nothing here is
   optional reading, so making the agent fetch the transcript costs tool calls
   for no gain.
5. **Validate per window**: every `start` is a segment start in this window,
   strictly increasing, the window's first segment is present, at most one
   note per segment (the agent may merge adjacent segments, never split one).
   A bad window fails that window with the chapter-style message
   `note 3 (00:14:22): …`; retried once, then the job errors.
6. **Write per window** (differs from chapters, deliberately). A window is
   validated on its own, and a fifteen-minute job that shows nothing until the
   end looks hung. `store::save_recap_window` inserts one window's rows in a
   transaction; a fresh run deletes the old set first, so a regenerate that
   fails at window 7 leaves windows 1–6 of the *new* set and says so.
   Reconsider if the partial set turns out to be confusing in practice.

Status flow mirrors chapters exactly: `recap_status` claimed before the decode,
`recapped_at` stamped by a terminal status, `recap_error` cleared on success,
`store::reconcile_recap_status` at startup beside the chapter sweep.

**Needs a transcript on disk.** Chapters treat a missing transcript as losing
the pause bonus; recap is *about* what was said, so a lecture with no
`transcript.vtt` is refused up front with a message naming the download.

### Prompt (per window)

Carries: lecture title, the chapter title if the window sits in one, the
window's span as `HH:MM:SS`, the segment list (second, timestamp), the
transcript span inline as plain cues, the frames path and naming, the course
folder. Rules:

- One note per segment unless two adjacent segments are genuinely one thought.
- **Describe this moment, not the chapter.** What the slide shows, what the
  lecturer is arguing, any equation or definition written, any question asked.
  Two to four sentences. Present tense.
- Lecturer's own vocabulary and notation. Maths in `$…$` / `$$…$$`, which the
  app renders (KaTeX). Code in fences.
- Say when a segment is a worked example, a student question, an aside, or
  housekeeping.
- A short label, two to six words, may be empty when the body is the whole
  point.
- The AV splash screen is not a slide (same paragraph as chapters).

Reply: JSON array `[{"start": 742, "label": "…", "body": "…"}]`, parsed by the
same tolerant `json_candidates` path as chapters.

### Storage — migration 31

```
lecture_recap (lecture_id, idx, start_seconds, label, body)   cascading
lectures: recap_status, recapped_at, recap_error
```

No `end_seconds` (derived, same as chapters). No `window` column: windows are
a job-time detail, and a stored one would be a second place for a boundary to
be wrong.

### CLI

`oculus lecture recap <ID>` beside `lecture chapters`: `--force`, `--provider`
/ `--model` / `--effort`, the same prefix-id rule, the same "not downloaded"
message. Progress: one line per window ("window 3 of 9 — 00:20:14–00:31:02"),
the agent's tool rows under it.

### Model registry

`Job::LectureRecap` in `jobs.rs`, entry in `JOBS` / `DEFAULT_JOB_MODELS` in
`app/src/lib/db.ts`, a row in Settings → AI. Default cheaper than the chapter
job: this is paraphrase, not judgment. Claude Sonnet 5 at `medium` or Codex
at `medium`; pick after measuring.

**Measure before promising a number.** Run the 42-minute and 107-minute
reference lectures, record wall time per window and total, put the totals in
the doc the way chapters did. The panel's "Takes N minutes" line comes from
that, not from a guess.

### Tab

Third `DockTab` value `"recap"` in `playerPrefsStore`, third entry in
`TranscriptPanel`'s `TABS`, a `RecapPanel` beside `ChaptersPanel`, a
`useLectureRecap` hook shaped like `useLectureChapters` (same two events:
`lecture-recap` for a finish, `lecture-recap-progress` for a step, with
`window: {done, total}` on the countable phase).

- **All bodies visible**, not collapsed to the current one: the previous note
  or two are what "I zoned out" needs. Current note gets the `bg-brand/12`
  treatment chapters use; `scrollIntoView({ block: "nearest" })` on change.
  Do not scroll while the pointer is over the panel or the reader is
  mid-scroll — a note being read must not move under the eye. Keep it that
  simple; no pill, no countdown ring (those earn their keep on 2500 rows).
- Row: timestamp, label (if any), body. Click seeks.
- Bodies render through `react-markdown` with `remarkMath` + `rehypeKatex` and
  `MD_COMPONENTS` — the same stack `Timeline.tsx` uses, wrapped in the same
  prose class so display maths and code blocks look like a reply. Size it
  down for the dock (`text-[11px]`).
- Under a hundred rows of prose is fine without a virtualiser.
- Same five states as chapters: not downloaded / no transcript / nothing yet
  (**Write recap** + measured cost) / running (spinner, phase, "Window 3 of
  9", elapsed clock) / error (verbatim + retry) / list with **Regenerate** in
  a footer. Because windows write as they land, "running" with a partial list
  is a real state: the list above, the running line in the footer.
- Nothing is hand-editable. Derived data, same argument as chapters.

Recap does not touch the scrub bar or the chapter strip.

---

## Part 2 — Chat tab

### What it is

The Claude Code side-panel shape, in the dock: a header with the thread's
title, a **history** button listing this lecture's threads, a **new thread**
button; the timeline; a compact composer. Every message can carry the moment
the playhead is at.

### Rust

1. **Lecture scope on a thread.** Migration: `harness_threads.lecture_id TEXT
   REFERENCES lectures(id) ON DELETE SET NULL` beside `subject_id`. Set at
   creation only (both CLIs bind instructions at session start). The thread's
   `subject_id` is the lecture's subject, filled by Rust from the lecture row,
   not trusted from the payload.
2. **Instructions.** `instructions()` in `harness/mod.rs` gains a lecture
   section after the subject one: the recording folder, `transcript.vtt`, the
   course folder, and the chapter list inline (small, and it saves a turn).
   "The student is watching this lecture; a message may carry the moment it
   was sent at — a timestamp, the last minute of transcript, and a frame."
3. **`SendOptions.context: Option<String>`.** Appended to the prompt the CLI
   receives, after the student's text, under a `---` and a heading. **Not**
   stored as the user row's content; the row's `meta` carries `{"at": 220}`
   so the timeline can show "at 3:40" on the bubble. `harness_edit_resend`
   and rewind treat it like any other option.
4. **`lecture_grab_frame(id, seconds) -> path`.** One JPEG via the existing
   probe-and-grab, into `lectures/<id>/frames/live/<seconds>.jpg` so it never
   collides with a job's frames. ~40 ms. Overwritten freely.
5. **Thread listing by lecture.** `getHarnessThreads` already returns
   `subject_id`; add `lecture_id` to the row type and a `harness_threads_for`
   filter, or filter client-side — the list is small.

### The moment, built by the player at send time

- `at`: the playhead second.
- The transcript cues from `at − 60` to `at`, inline as plain text.
- The chapter and the recap note the playhead is in, if either exists.
- The frame path from `lecture_grab_frame`.

A toggle in the composer turns the attachment off for a message that is not
about the moment. The chip shows the timestamp it will send; it tracks the
playhead until send, then freezes on the row.

### Frontend

**The store.** `harnessStore` holds one `activeId` and one `items` array, and
the side panel can sit beside the Chat page, so two threads can be open at
once. Change `items` to `Record<number, HarnessItem[]>` keyed by thread, let
`open(id)` load into that map, and have each view own its thread id: the Chat
page keeps `activeId`, the dock keeps a `threadId` in the player. `live`,
`queued` and `contextDrift` are already per-thread, and `apply` already
routes by thread id, so the fold needs no change. This is the one real risk
in the feature; do it first and run the existing chat page against it before
the dock exists.

**`LectureChatPanel`** in `app/src/components/lectures/`:

- Header row: title (thread title or "New thread"), a history button that
  opens a popover listing this lecture's threads newest first (title, model
  mark, relative time — reuse `ProviderMark`, not `ThreadList`, which is built
  for the page's column), and a new-thread button. That is the Claude Code
  panel's top-right pair.
- Body: `Timeline` unchanged, with `questions` and `pending` wired the way
  `ChatPage` wires them, and the stick-to-bottom hook lifted out of
  `ChatPage.tsx` into a shared hook.
- Composer: a compact sibling of `Composer`, not the same component. Textarea,
  send/stop, `ModelPicker` (every send names an explicit model and level —
  no default selection), the moment toggle + chip. No subject select (fixed by
  the lecture), no `@` menu (the agent already has the lecture named; a file
  can be typed as a path).
- Empty state: the provider mark and one line. No suggestion chips in a
  200px dock.

**Threads for the lecture also appear on the Chat page** under their subject,
with a small lecture marker on the row, so nothing lives only in the dock.
Opening one there works; it just has no moment to attach.

**Width.** `Timeline` rows and `WorkRow` are `min-w-0` and wrap, but a
200px dock is tight for a reply with a code block. Try it at the dock's
minimum; if it reads badly, raise the dock's minimum width while the Chat tab
is in front rather than restyling the rows.

Sending does not pause playback.

---

## Build order

Each stage is a commit with its doc change. Subagents implement, code is
reviewed here, UI is verified by Tanat in the running app.

1. **Recap job.** Segmenting, windows, prompt, parse/validate, per-window
   writes, migration 31, status columns + startup sweep, `Job::LectureRecap`,
   `oculus lecture recap`. Measure the two reference lectures.
2. **Recap tab.** Store enum, hook, `RecapPanel`, markdown + KaTeX rendering,
   the five states, the Settings → AI row.
3. **Lecture scope in the harness.** ✅ Done (migration 30). Migration for `lecture_id`,
   `instructions()` section, `SendOptions.context`, the `at` meta on the row,
   `lecture_grab_frame`.
4. **Per-thread `items` in the store.** ✅ Done. Refactor, then the Chat page runs
   unchanged against it.
5. **Chat tab.** ✅ Done. `LectureChatPanel`, compact composer, history + new thread,
   the moment attachment, lecture marker on the Chat page's rows.
6. **Docs.** The chat half shipped with its stages; the recap half is what
   is left. A "Recap" section in `docs/chapters.md` (shared pipeline, shared
   measurements), lecture scope + the moment attachment + per-thread items in
   `docs/harness.md`, the dock's tabs in `docs/frontend.md`, both new commands
   in `docs/cli.md`.

## Open

- Tab label: **Recap** (alternatives: Notes, Guide). Notes collides with the
  student's own notes.
- Whether the moment's frame should come from the playing element instead of
  ffmpeg. Canvas capture of a video served from the localhost media server
  may be tainted cross-origin; ffmpeg is certain and cheap, so start there.
- Whether per-window writes should be rolled back on a job error. Try
  partial-visible first.
