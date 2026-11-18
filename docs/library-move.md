# The library move into the Drive tree

A library written before the Drive tree (`LEGACY` in
`app/src-tauri/src/library/layout/shape.rs`: synced folders at a subject's
root, one folder per own kind, recordings in the data dir's
`lectures/<media id>/`) is moved into the current one
([architecture.md](./architecture.md#one-module-owns-every-path-shape-in-the-library))
once, by `app/src-tauri/src/library/migrate/`. Nothing in the app reads the
old layout afterwards; old paths stay readable through `path_moves`.

## It runs before anything touches the library

`library::migrate::at_startup` is the first thing after Start over's cleanup
in both processes, since they share `oculus.db` and the folders:

- **The app** calls it in `run()` (`app/src-tauri/src/lib.rs`), before the
  Tauri builder, so no window, job queue, watcher or media server exists yet.
  A failure is kept until `setup`, which destroys the window before its page
  runs, says why in a dialog (`refuse_to_start`) and exits; the next launch
  resumes.
- **The CLI** calls it in `main` before dispatching any command that reads
  or writes the library. Not before `library migrate` (which runs or
  previews it itself), `keyd …` (no library, and `tauri dev`'s preflight runs
  `keyd install`, where a move's failure would read as keyd's) or `docs …`
  (only the agents' folder). A failure exits 1 with the message.

A marker, `<data dir>/.library-move/done` holding `drive`, makes every later
call a no-op without opening anything. A data dir with no old folder (a new
library, or one written in the new tree) only gets the marker. An exclusive
lock, `.library-move/lock` (keyd-core's `flock`), keeps two processes from
moving at once: the second waits, then finds the marker.

## Plan, move, rewrite

1. **Plan** (`plan/`). Every step is decided before the first rename, from a
   walk of the folders and the database's rows, and written to
   `.library-move/journal.json`. Each step's source is where the thing is
   when that step runs. Names are chosen against the disk plus what earlier
   steps put there, compared as APFS and NTFS compare them: one Unicode form
   (NFC) and no case.
   - **Reserved names** first: anything at a subject's root named `Course` or
     `.oculus` in any case (the student's) becomes `Course (mine)` before
     anything moves in under that name. It gets no `path_moves` row, since
     its old path is the new tree's.
   - **Debris** first: `.claude/` agent scratch and the hidden
     `-staging-`/`-quality-` folders an interrupted parse left, under
     `courses/` and `lectures/`, deleted when they hold no file.
   - **Synced** folders into `Course/` (`files` → `Course/Files`, `ed` →
     `Course/Discussion`, …; `layout::legacy_folder_moves`), and `home.md`,
     `syllabus.md`. Videos the database knows in the old `files/` (module
     videos, page-embed downloads) go to `Course/Media/` with their
     `<stem>.vtt` and `<name>.vtt` captions.
   - **Own** folders (`uploads`, `documents`, `canvases`, `notebooks`)
     flatten into the subject's root, keeping everything below them. A file
     travels with what its parse made of it, so a clash renames them together
     (`a.pdf` → `a (2).pdf` with `a (2).md`, `a (2)_images/` and its records).
     An entry named `Course` or `.oculus` becomes `Course (mine)`. The notes'
     `assets/` becomes `.assets/` at every depth.
   - **Parse records** (`<stem>.pages.json`, `<stem>.emb.json`) go to
     `.oculus/derived/<the PDF's folder>/` (`layout::derived`); the parsed
     `.md`, `<stem>_images/` and a converted `<name>.docx.pdf` stay beside
     their source.
   - **The agent folder**: the `memories` link is dropped and remade at its
     new depth, `INSTRUCTIONS.md` and dot-entries go to `.oculus/agents/`,
     anything an agent wrote there for the student goes to the subject's
     `Agent notes/`, where it stays in sight.
   - **Recordings**: each `lectures/<id>/` with something in it becomes
     `Course/Recordings/<date>-<slug>/`, named by `layout::new_recording_rel`
     as a new download would be. `raw*.mp4` partials are deleted; a folder
     left empty is removed and its lecture keeps no `folder_rel`.
   - Loose files at `courses/` itself and folders with no lecture row stay,
     logged once.
   - **Rows that would meet**: a `files` row whose file is gone can map onto
     the path a file from another folder lands on (a stale `documents/a.pdf`
     and a real `uploads/a.pdf` both flatten to `a.pdf`). The row with its
     file keeps the path; the stale one gets `a (2).pdf`. Should a clash still
     reach the database, that row keeps its old path and is logged, rather
     than fail the transaction on every launch.
   - A `lectures/` that is a symlink is refused with a message: the student
     puts the folder back in the data dir, since the move only renames inside
     it.
2. **Move** (`apply.rs`) with `std::fs::rename` only. Each step's index is
   appended to `.library-move/done.log` as it lands; a crash resumes from the
   journal, and a rename whose source is gone and target present counts as
   landed. A rename across volumes stops the move with a message: nothing is
   ever copied.
3. **Rewrite the database** (`db/`) in one transaction, after the schema is
   brought up with the app's own migration list (the plugin's conversion, so
   the frontend's `Database.load` finds nothing to do). Rows are UPDATEd,
   never deleted and re-inserted, so `pages`, versions and task links stay on
   their `files` row: `files.relative_path`, `category` (`category_of`; a
   drawing's `canvas` stays for the whiteboard rename, which reads it to
   find a drawing whose file is gone) and a renamed `filename`; `sync_run_files`, `echo_embeds`, `inbox_item_entries`,
   `revision_notes.source_ref`; every path string inside `chat_proposals`
   and `agent_changes` JSON; `lectures.folder_rel` and its media paths; the
   links in a moved note's `document_versions`. The `library_layout` setting
   is written in the same transaction, so a resumed run never applies it
   twice.
4. **Rewrite files** (`contents.rs`, `links.rs`): every moved markdown file's
   relative links, the `asset://` picture links notes hold, a renamed PDF's
   records, the notebooks' `pages.json`
   (under `canvas-pages/` or `whiteboard-pages/`), the subjects' memories links, and `agents/opencode.json` when
   opencode has been used. Each is a no-op the second time.

## Stored paths are recognised, never followed

`lectures.video_path`, `video2_path` and `transcript_path` are absolute
paths into wherever the library was when they were written. The move
computes every lecture move from `<data dir>/lectures/<id>/` and rewrites a
stored path by its `…/lectures/<id>/<file>` tail, joined to this data dir
(`db/rewrite.rs`, `lecture_path`). An `asset://` link in a note is read the
same way, by its `courses/…` tail.

## Links follow what they named before the move

A relative link is read from the file's old place and followed through the
rules; when that target is not what the link names from the file's new place
(in exact case: a case-insensitive volume would find `files/` for `Files/`),
it is rewritten with `link_between`, the spelling the sync now writes. So a
note's link to a sibling renamed on a clash (`brief.pdf` → `brief (2).pdf`)
follows it, even though an uploaded `brief.pdf` now sits beside the note. A
target not on disk is followed only when a rule moved its folder (a page or a
video not fetched yet; a video lands in `Media/`). A re-pointed link names
nothing from the old place, so a second pass leaves it.

## Old paths resolve through `path_moves`

Migration 64 adds `path_moves(old_prefix, new_prefix)`, one row per rule the
plan made: a folder per old folder of each subject, one per recording, and one
per file that left its folder on its own (videos, parse records, renamed
files). The longest prefix a path starts with at a boundary (`/`, `#page=`,
`:L15`, `?`) wins and the rest carries over. `library::migrate::resolve_moved`
is the Rust reader; `app/src/lib/files/pathMoves.ts` the frontend's, used by
`openCitation` when a cited path has no row. Chat history (`harness_items`,
`agents/threads/`), undo snapshots and saved tabs keep their old text and are
resolved when read.

## Drawings named `.canvas`

Right after the Drive move and under its lock,
`library::migrate::whiteboard` renames every `.canvas` drawing among the
student's own files to `.whiteboard`, `canvas-pages/` and
`recovery/canvases/` to their new names, and the rows, `path_moves`, page
manifests and the notes that link to a renamed drawing with them, with its own journal and marker (`.library-move/whiteboard`).
A rename's rules go through `record_moves` (`library/migrate/resolve.rs`):
the drawing's own rule, plus every old path the rules already kept resolve
to the drawing's old path, mapped straight to its new one, since a lookup
takes one step. An old path is derived only when the kept rules really send
it there, so a file renamed on a clash (`uploads/a.canvas` → `a (2).canvas`)
keeps its own rule rather than taking its folder's (`uploads` → the subject
root) to another drawing named `a`. Drive's reconcile records its renames
the same way. A drawing's row is found as the volume finds the file
(`name_key`: one Unicode form, no case), and a row spelled other than the
disk gets a rule for its own spelling too. A
`.canvas` drawing that reaches the library later (a backup, another Mac, an
agent) is renamed by Drive's reconcile and imports
([frontend.md](./frontend.md#own-files-written-outside-the-app-get-rows)), with
the same test and naming (`library/files/whiteboards/legacy.rs`).

## Running it by hand

`oculus library migrate --dry-run` prints the plan, or the journal a cut-short
run left, without locking or writing anything; `oculus library migrate` runs
it. A debug build stops after N renames when
`OCULUS_LIBRARY_MOVE_STOP_AFTER=N` is set, for checking a resume on a copy of
a library. Run either against a copy only, with `HOME` pointed at it
(`dirs::data_dir` reads `$HOME`): the move changes the live library in place.
