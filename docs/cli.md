# The `oculus` CLI

A second binary in `app/src-tauri` that drives the same engine as the app with
no window: terminal syncs, the keep-alive job, and the tool surface a coding
agent queries and plans through. Flags are in
[cli-reference.md](./cli-reference.md); this page is how the CLI fits.

## Where

| Piece | Location |
| --- | --- |
| The binary (`main.rs` dispatches, `args.rs` is the clap tree) | `app/src-tauri/src/bin/oculus/` |
| Query commands, category filter, file lookup | `app/src-tauri/src/bin/oculus/query.rs` |
| `project` and `task` | `app/src-tauri/src/bin/oculus/planning.rs`, `app/src-tauri/src/projects.rs` |
| `lecture` | `app/src-tauri/src/bin/oculus/lecture.rs` |
| `transcribe` | `app/src-tauri/src/bin/oculus/transcribe.rs`, `app/src-tauri/src/transcribe/` |
| `docs`: help rendering; agent docs, stubs and links | `app/src-tauri/src/bin/oculus/docs.rs`, `app/src-tauri/src/agents.rs` |
| The memory store | `app/src-tauri/src/memory.rs` |
| `keyd` and the app's startup install | `app/src-tauri/src/bin/oculus/keyd.rs`, `app/src-tauri/src/keyd.rs` |
| Headless writes to the scrape tables | `app/src-tauri/src/store.rs` |
| Repo copy of the reference | `app/scripts/gen-cli-docs.mjs` |

## What a command writes decides how careful it is

| Commands | Write | Consequence |
| --- | --- | --- |
| `run`, `index`, `auth` | Copies of what the university has | A bad run is repaired by running it again |
| `search`, `grep`, `read`, `files`, `calendar`, `list`, `status` | Nothing | Safe for any caller |
| `project`, `task` | The student's own plans | No upstream copy: unknown columns are refused, batches are one transaction, `task rm` says in `--help` there is no undo |
| `memory` | Agents' notes in `agents/memories/` | Files, not rows, because the thread writing them cannot open the database ([harness.md](./harness.md)) |
| `lecture chapters`, `lecture reading`, `lecture end` | Derived rows, regenerable from the recording | They spend model quota, so an existing result is kept unless `--force` |
| `transcribe` | `<video>.vtt` beside a library video, nothing in the database | Tries the engines in the order set in Settings → Transcription (Groq, local Whisper, on-device speech by default); Groq spends the free-tier audio allowance, the other two run on this Mac. An existing `.vtt` is kept unless `--force` ([viewers.md](./viewers.md#videos-without-captions-are-transcribed)) |
| `docs` | The library's `agents/` folder | [Below](#oculus-docs-writes-the-agents-folder) |
| `keyd install`, `keyd uninstall` | The LaunchAgent `com.tchan.oculus.keyd`, `bin/oculus-keyd` and its stamp in the data dir; install also (re)loads the agent | Never touches `vault.bin` or the keychain, so uninstalling loses no key ([below](#keyd-install-never-points-the-agent-at-a-build-tree)) |
| `keyd status` | Nothing | Pings keyd, which starts it; prints no secret |
| `agent` | Nothing recorded | One turn through the app's own bridges; `--subject` appends the picker's scope ([harness.md](./harness.md)) |

`lecture candidates` only decodes a recording on disk, so re-running it is the
whole story ([chapters.md](./chapters.md)). `--json` is the only global flag.

## The CLI shares the app's cookie and database, but never creates the database

A CLI sync shows up in the app and vice versa. Schema belongs to the app's
migrations, so on a fresh machine the app must open once first; until then
`run` scrapes to disk and says so.

- `auth login` launches the app for the SAML browser step, because a push or
  biometric challenge needs a human. `auth setup` stores what `auth auto` needs
  to sign in headlessly; `auth forget` clears it ([auth.md](./auth.md)).
  `auth auto` is a manual sign-in, so it skips the
  [attempt guard](./auth.md#every-sign-in-attempt-goes-through-one-guard)'s
  wait and lifts its pause.
- `auth tick` is one keep-alive cycle, run by the LaunchAgent. Its sign-in is
  automatic, so the guard can skip it. It prints nothing, logs to
  `session-keepalive.log`, and always exits 0, because launchd reads a
  non-zero exit as a crashed job.
- `run -s` scrapes, then parses and embeds each written PDF one file at a time,
  then replaces each subject's `calendar_events` — always, since there are no
  sync options to gate it ([calendar.md](./calendar.md)).
- Subject codes match on prefix (`MULT20015` finds `MULT20015_2026_SM2`);
  lecture ids match on a unique prefix as `oculus list -l` prints them.

## `keyd install` never points the agent at a build tree

The LaunchAgent's program is fixed: a keyd inside an app bundle is registered
in place, since its caller check needs its bundle, and any other is copied to
`bin/oculus-keyd` in the data dir through a temp file and a rename. Without
`--from` it takes the keyd beside the CLI in the bundle, or in a debug CLI the
output of `bun run keyd` in the checkout it was built from
([development.md](./development.md#oculus-keyd-is-built-apart-so-its-signature-only-changes-with-its-source)).

- `launchctl bootout` returns before launchd lets go of the job, and a
  `bootstrap` too soon after fails and leaves keyd unloaded. Install and
  uninstall wait until `launchctl print` stops finding the label.
- The stamp is written last, so a failed load is retried by the next
  preflight.
- `status` compares the installed stamp, the stamp of the keyd this CLI would
  install, and the source hash the running keyd reports.

## `run` and `index` take minutes per file, and that is not a hang

Parse and embed run in-process with no timeout from above
([parsing.md](./parsing.md), [retrieval.md](./retrieval.md)); a page counter
rewriting itself in place is how a long run is told from a stuck one.

- Embedding is the slower half: without a payment method Voyage allows under
  three pages a minute, so a large deck takes an hour.
- `index` re-embeds any file whose vectors came from another model, dim or
  instruction, or cover too few pages (`embed::is_embedded`). There is no
  migration between embedding spaces.
- `index` obeys the spend guard Settings sets, because both processes read
  `voyage-usage.json`. Past it the client returns `BudgetReached`, which names
  the setting; waiting will not clear it.
- `status` reports the selected parser through `preflight`, so it never spends
  cloud quota; on the local engine it probes the loopback address with a
  three-second timeout.
- `lecture reading` runs its roughly ten-minute windows in sequence and commits
  each one, so an error can leave this run's finished windows visible.
  `--provider`, `--model` and `--effort` override one run without changing the
  configured job ([chapters.md](./chapters.md)).
- `lecture end` takes several ids or `--all` (lectures with a transcript whose
  end was never looked for; every one with `--force`) and prints a line, or a
  JSON object per line with `--json`, per lecture. `--dry-run` asks the model
  but writes nothing and never reads the columns migration 42 adds, so it runs
  against a database without them; the hidden `--print-prompt` prints the
  brief and the prompt and asks nothing
  ([chapters.md](./chapters.md#where-the-lecture-ends)).

## Read commands fail loudly rather than return nothing

- `search` embeds the query with the same cloud model as the pages, so it
  needs a Voyage key and a network, and spends a few tokens. With no vectors in
  the current model's space it exits 1 and names `oculus index` — separately
  for "nothing indexed" and "indexed by a retired model" — because a caller
  given an empty result concludes the library has no answer. `status` prints
  the same split as `index` and `stale` lines. It takes a *set* of subject ids,
  since a prefix can match one course in two terms (`retrieval::search_in`).
- `grep` covers both halves of the library: markdown on disk, and PDF page
  text that exists only in `pages`. Ripgrep over `courses/` misses every slide
  deck. It scans in subject-then-path order and stops at its limit, so a
  truncated result is biased; `-c` narrows before the limit applies.
- `-c/--category` is one filter (`filter_categories`) behind `grep` and
  `files`, validated against `paths::CATEGORIES`. An unknown word is refused
  with the real list; a real category a subject lacks returns nothing.
- `read` uses the page numbers `search` reports and the viewer shows — all
  three key `pages.markdown` on `(file_id, page_no)`. An Office document's
  pages are its derived PDF's (`paths::doc_pdf_rel`); a spreadsheet's are its
  sheets, and with no rows yet `read` prints its `.md` from disk.
- Read commands share subject-ID resolution in `query.rs`: an omitted scope
  means every subject, while a prefix keeps every matching term and an unknown
  code fails before querying files.
- File lookup is tiered, not fuzzy: exact path, exact filename,
  case-insensitive filename, path substring. Only the best tier that matched
  counts, and a tie in it is reported, never guessed.
- With `--json`, success is one document on stdout and failure is
  `{"error": "..."}` on stderr with exit 1.

## `project` and `task` are the agent's only door to the board

The app's board reads these rows live, so no agent opens `oculus.db`. The rules
are in [projects.md](./projects.md); the CLI-shaped parts:

- `task add` with no `-p` writes an **unfiled** task, belonging to no project,
  so "write this down" does not first need a project id. `task list` with no
  `-p` spans every board, unfiled first; `--unfiled` shows only those;
  `--column` requires `-p`.
- `task add --batch` reads a JSON array and writes it in one transaction. An
  item names its parent by task id or by the `key` of an earlier item in the
  same array; `key` is never stored. The contract is spelled out in
  `task add --help`, and a test parses that documented JSON.
- An unknown `--column` is refused with the board's real ids: a task in a
  column the board lacks is invisible, not misfiled.
- `task move` alone changes a task's column, order and done-ness — they are one
  fact. `task refile` alone changes its project, carrying subtasks with it and
  mapping the column by kind; a lone subtask is refused.
- `-s` resolves a code matching two terms to the current term, and reports a
  tie that survives that. Omitting `-s` makes a personal project.
- Batch task input and memory bodies share the UTF-8 file/stdin reader in
  `main.rs`; `-` is stdin and each command retains its own input validation.
- Every project and task it writes is marked `source: agent`.

## `oculus docs` writes the agents folder

It fills `agents/` in the data directory so an agent in a course folder needs
no explanation of Oculus. Everything but the CLI reference is written by
`app/src-tauri/src/agents.rs`, which every sync also calls ([sync.md](./sync.md)).

```
<data>/agents/
  AGENTS.md  OCULUS-CLI.md  OCULUS.md  TASTE.md
  memories/MEMORY.md             cross-subject bucket
  memories/<code>/MEMORY.md      one subject's bucket
  skills/<name>/SKILL.md
  .claude/skills/<name> → ../../skills/<name>
  .agents/skills/<name> → ../../skills/<name>
<data>/courses/<code>/
  AGENTS.md → ../../agents/AGENTS.md
  agents/INSTRUCTIONS.md         hand-written; nothing writes it
  agents/memories → ../../../agents/memories/<code>
```

Each file has one of four lifetimes:

- **Generated, always overwritten**: `OCULUS-CLI.md` (walked from clap's tree,
  with a test asserting coverage), `AGENTS.md` and `skills/`. An agent trusts
  these over `--help` and follows a skill as a procedure, so a stale one is
  worse than none.
- **Stubbed once**: `OCULUS.md`, which a human authors.
- **Merged**: `TASTE.md`'s guidance is re-rendered with the user's bullets
  carried under their headings, and each `MEMORY.md` is rebuilt from the
  memories beside it. Prose under a heading, or missing headings, is reported
  and left alone rather than guessed at.
- **Linked, never clobbered**: every link is relative, so the library can
  move. A wrong target is relinked; a real file is left with a warning. A real
  `agents/memories/` folder in a course has its files moved into the bucket
  before it becomes a link.

Skills are one directory reached three ways: Claude Code scans
`.claude/skills` and Codex `.agents/skills`, both walking up from the working
directory, and opencode reads a `skills.paths` config key. Nothing is written
to a home directory. Both memory buckets sit under `agents/` because it is the
only folder a chat thread may write.

A sync links only the course folders it scraped (`link_course`); `oculus docs`
sweeps them all (`link_all`) and is the only writer of `OCULUS-CLI.md`. Help is rendered at 88 columns with colour off (clap's `wrap_help`), so output
depends on the binary, not the terminal; the global `--json` is hidden below
the root so it is not repeated under every subcommand. `OCULUS-CLI.md` is a
pull document that `AGENTS.md` says when to open.
[cli-reference.md](./cli-reference.md) is the same rendering, written by
`app/scripts/gen-cli-docs.mjs` only when the help changed
([development.md](./development.md#predev-is-the-whole-preflight-and-it-is-idempotent)).

## Gotchas

- An empty `search` result would read as "no answer" — keep failing loudly with the next command to run.
- Ripgrep over `courses/` misses PDF text — use `oculus grep`.
- `auth tick` exiting non-zero reads as a crash to launchd.
- A stale `oculus` documents and runs the wrong build — `bun run cli` deletes the binary before building ([development.md](./development.md#the-dev-cli-is-built-by-the-preflight-not-by-tauri-dev)).
