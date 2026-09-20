# The `oculus` CLI

A second binary in `app/src-tauri` that drives the same scrape engine as the
app, with no window involved. Useful for terminal syncs, cron jobs, and
debugging. `app/README.md` carries the full command reference; this page is
how it fits the architecture.

Three kinds of command, and the difference between two of them matters.
`run`, `index` and `auth` **write the library**: they are the app's engine
without the window, and everything they write is a copy of something the
university already has, so a bad run is repaired by running it again.
`search`, `grep`, `read`, `files` and `calendar` only **read**, and exist so a
coding agent, or a person at a prompt, can query the library without the UI and
without a protocol in between: one binary, one `--json` flag, no per-agent
registration and no tool schemas resident in a context window. `project` and
`task` **write the user's own content** — the projects, boards and tasks the
app draws ([projects.md](./projects.md)) — which is a different kind of write
from a scrape: nothing upstream has a copy, and `oculus task rm` is gone for
good where a deleted course file comes back on the next sync. That is why
those commands refuse an unknown column instead of guessing, why a whole
breakdown goes in as one transaction, and why the destructive one says in its
own `--help` that there is no undo.

`lecture` reads no upstream copy at all: it decodes a recording already on disk
(see [chapters.md](./chapters.md)), which is why it has no cache to invalidate
and re-running it is the whole story. `lecture candidates` writes nothing at
all. `lecture chapters` and `lecture reading` write **derived** rows — not a
copy of anything upstream and not the user's own work either, but something
regenerable from the recording — so `--force` is the door for replacing them.
These are also the commands that spend a model's quota, which is why each
leaves an existing result alone unless asked. A reading copy additionally
requires the transcript on disk: it *is* the transcript, rewritten.

Two commands stand outside that split. `docs` documents the whole agent-facing
surface to the agents that use it, and `agent` runs one of those agents for a
single turn through the app's own bridges — the headless proof that a bridge works, with nothing recorded
(see [harness.md](./harness.md)). Its `--subject` takes a course code and
appends the same scope the app's picker does, which is how that section of
the instructions gets read without opening the window.

## Where

| Piece | Location |
| --- | --- |
| The binary | `app/src-tauri/src/bin/oculus.rs` |
| Ranking behind `search` | `app/src-tauri/src/retrieval.rs` |
| Shared path resolution | `app/src-tauri/src/paths.rs` |
| Agent docs: templates, stubs, linking | `app/src-tauri/src/agents.rs` |
| `agent`: the bridges it drives | `app/src-tauri/src/harness/mod.rs` |
| Headless DB writes (scrape tables) | `app/src-tauri/src/store.rs` |
| Headless DB writes (projects and tasks) | `app/src-tauri/src/projects.rs` |
| Repo copy of the reference, regenerated at bundle time | `app/scripts/gen-cli-docs.mjs` |
| Build scripts (`cli`, `cli:install`, `docs:cli`) | `app/package.json` |

## How it connects

- **`--json` is the only global flag.** `--memory-cap` went with the Python
  sidecar it bounded, and did not come back with the local parse engine. A
  MinerU the user installed and started is not this app's child process: its
  memory is its own to manage, and a flag here could not bound it if it tried.
  What a parse costs is the selected engine's business — MinerU's allowance on
  the cloud, this machine's RAM on a local server somebody else is running.
- The CLI reads the same session cookie and writes the same `oculus.db` the
  app uses — a CLI sync shows up in the app and vice versa. But it **never
  creates the database** (schema stays with the app's migrations), so a
  fresh machine must open the app once first; until then the CLI scrapes to
  disk and says so.
- `oculus auth login` launches the app for the SAML browser step and polls
  for the cookie the app saves — that path needs the app because a push or
  biometric challenge needs a human.
- `oculus auth setup` stores the username, password and TOTP setup key that
  let `oculus auth auto` do the whole Okta sign-in headlessly, no app and no
  browser. `oculus auth forget` clears them. See [auth.md](./auth.md) for
  what the setup key is and why it cannot be derived from codes.
- `oculus auth tick` is one keep-alive cycle — roll the session forward, and
  rebuild it headlessly if Canvas has rejected it. It is what the macOS
  LaunchAgent runs on a schedule, so it prints nothing, writes to
  `session-keepalive.log` in the data dir, and always exits 0: launchd has no
  console, and a non-zero exit only reads as a crashed job. Safe to run by
  hand when you want to know whether the agent's path still works.
- `run -s` scrapes, then parses and embeds each written PDF **one file at a
  time**. Both halves are idempotent; re-running is cheap.
- **A parse finishes before the command moves on, and that is minutes per
  file.** It used to return as soon as the old *fast* pass had markdown
  and leave the real parse running in another process, so the good text only
  appeared on some later `oculus index`. Parsing is in this process now
  (`app/src-tauri/src/parse/mod.rs`), so the wait is the whole cloud round
  trip — the page count rewrites itself on the line as pages arrive, which is
  how you tell a long parse from a hung one.
- **The embed half is slower than the parse half, and on a Voyage account with
  no payment method it is dramatically slower**: 10K tokens a minute against
  ~3,571 tokens for a 200-DPI page is about 2.8 pages a minute, so a large deck
  is genuinely an hour or more. It gets the same in-place counter for the same
  reason, and no timeout is imposed on it — the client paces itself against the
  tier it detected and a 429 is routine rather than a failure. See
  [retrieval.md](./retrieval.md).
- **`index` re-embeds a file whose vectors came from a retired model.** There
  is no migration between embedding spaces and there is not meant to be:
  `embed::is_embedded` compares model, dim and page coverage, so the files the
  local Qwen embedder wrote read as unfinished and rebuild on the next run.
- **`index` obeys the spend guard the app sets**, because both processes read
  the same `voyage-usage.json`. Past the configured percentage of Voyage's free
  pixel grant the client refuses with `BudgetReached`, which names the setting
  rather than blaming the account — it is not a rate limit and waiting will not
  clear it. See [retrieval.md](./retrieval.md).
- `oculus status` reports the **parser**, not a local process: the backend
  name, whether it is usable, and its `parser_version` — the handshake that
  decides whether artifacts written elsewhere can be read as this app's. It
  goes through `preflight`, so it never calls MinerU's cloud and costs no
  metered quota. On the local engine it is not quite free: `health()` probes
  the configured loopback address, which answers at once or times out in three
  seconds. See [parsing.md](./parsing.md).
- `run -s` also refreshes each subject's Canvas calendar (class times and due
  dates) into `calendar_events` after the scrape — the CLI has no sync options
  to gate it with, so it always runs. See [calendar.md](./calendar.md).
- Subject codes match on prefix (`MULT20015` finds `MULT20015_2026_SM2`).
- Lecture ids match on a unique prefix, as printed by `oculus list -l`.
  `lecture reading` resolves that prefix, then runs the configured
  `Job::LectureReading`; `--provider`, `--model` and `--effort` override one
  run without changing the registry. Its roughly ten-minute windows run in
  sequence and commit independently, so an error can leave the completed
  windows from this run visible. See
  [chapters.md](./chapters.md#the-reading-copy).

## The query half

- **`search` fails loudly rather than returning zero hits.** The query is
  embedded by the same cloud model that embedded the page images — there is no
  text index to fall back on — so it needs a Voyage key in the keychain and a
  network. An empty index **fails with exit 1 and names `oculus index`**. That
  is deliberate: a caller handed an empty result concludes the library has no
  answer and stops; a caller told why it is empty tries the other door, which
  is `oculus grep`.
- **"Indexed" and "searchable" are different numbers, and `search` says which
  one is zero.** Only vectors from the model that embedded the query are
  scanned, because a dot product across two embedding spaces is meaningless and
  still sorts. A library full of vectors from a retired model therefore fails
  with a message naming that model and `oculus index`, not with "nothing is
  indexed". `oculus status` prints the same split: an `index` line for what can
  be searched now and a `stale` line for what needs re-embedding.
- `search` takes a *set* of subject ids, not one — a prefix code legitimately
  matches the same subject in two terms. `retrieval::search_in` is the
  multi-subject entry point; `retrieval::search` is the one-subject wrapper
  the Tauri command still calls. Both embed the query once.
- **`grep` covers both halves of the library, which is why it is not just
  ripgrep.** Markdown (Canvas pages, announcements, Ed threads) is on disk;
  PDF page text is only in the `pages` table. A caller reaching for ripgrep
  over `courses/` silently misses every slide deck. `grep` scans in subject
  then path order and stops at its limit, so the two sources interleave
  instead of the database half crowding out the disk half. That ordering is
  also why it takes `-c`: a truncated result is biased rather than sampled,
  and narrowing to a category cuts the haystack *before* the limit applies.
- **`-c/--category` is one filter behind two commands.** `grep` and `files`
  share `filter_categories`, so the flag spelled the same way on sibling
  commands cannot disagree about what it accepts, and the help both print is
  built from the list it validates against. That list is
  `paths::CATEGORIES`, beside the `category_from_path` that produces the
  values, with a test tying the two together — a validator holding its own
  copy goes stale the first time a scraper grows a folder.
  A word that is **not** a category is refused, naming the real ones, for the
  same reason an unknown board column is: an empty result from a typo reads
  exactly like an empty library. A real category the selected subjects happen
  not to have is a well-formed question and returns no matches — which is why
  validity is the canonical list and not the categories the matched rows
  carry. Reading it off the rows conflates the two and refuses
  `-s INFO30006 -c quiz` for a subject that simply has no quizzes.
- `read` addresses PDFs by the **same page numbers** `search` reports and the
  app's viewer shows, because all three read `pages.markdown` keyed on
  `(file_id, page_no)`. For an Office document that means the derived sibling
  PDF's pages, via `paths::doc_pdf_rel`.
- File lookup is tiered, not fuzzy: exact path, then exact filename, then
  case-insensitive filename, then path substring — and only the best tier
  that matched anything is considered. A tie inside a tier is reported, never
  guessed.
- `--json` is global and shaped for a caller that will not read prose: one
  document on stdout, and on failure `{"error": "..."}` on **stderr** with
  exit 1. `status --json` carries the index stats too, so a caller can find
  out whether `search` will work before trying it.
- These commands never scrape and start nothing. A read command on a machine
  where the app has never run
  reports what is missing and stops. `search` is the one exception to "reads
  cost nothing": it embeds one query, which spends a few tokens of the Voyage
  allowance.

## The planning half

- **`project` and `task` are the agent's write surface**, and the database is
  the only door: the app's board reads these same rows live, which is why
  nothing here asks for `oculus.db` to be opened directly. Ten subcommands —
  `project list|show|create|update` and `task list|add|update|move|refile|rm`
  — all honouring `--json`. The rules they enforce, and why, are in
  [projects.md](./projects.md); what is CLI-shaped about them is below.
- **A task does not need a project.** `oculus task add` with no `-p` writes an
  **unfiled** task — one that belongs to no project at all, which is the
  absence of a project rather than a project called Inbox
  ([projects.md](./projects.md)). It is the CLI's half of what the app's Tasks
  page does by default, and it exists because "write this down, I have not
  decided where it goes" is most of what gets said to an agent mid-conversation;
  demanding a project id first turns a note into a planning session. Its board
  is the app's default one, so filing it somewhere later needs no translation.
- **`oculus task list` with no `-p` spans the library** — one board per
  project under its name, the unfiled pile first, and `--unfiled` for that pile
  alone. `--column` still requires `-p`: a column id only means something
  against one board, so there is nothing across projects for it to filter.
- **`oculus task refile` is the only thing that changes which project a task
  is on**, and it takes the task's **subtasks with it** — a subtask sits in its
  parent's project, so a lone subtask is refused and told to refile its parent.
  The column maps across by *kind*, into the first column of that kind on the
  destination's board, and a destination with no column of that kind is refused
  rather than given the nearest one. It appends at the end of that column,
  because `position` is an order inside one project's column and means nothing
  across two; `task move` is how it is then placed.
- **A breakdown goes in as one `--batch`, not a command per task.** `oculus
  task add -p <ID> --batch -` reads a JSON array from stdin (or a file) and
  writes it in a single transaction, so a rejected item rolls the whole thing
  back rather than leaving half a plan on the board. An item names its parent
  either by an existing task id or by the `key` of an **earlier item in the
  same array**, which is how a parent and its subtasks go in together; `key` is
  never stored. That contract is the one thing here an agent writes by hand, so
  it is spelled out in `oculus task add --help` and asserted by a test that
  parses the documented JSON.
- **An unknown column is refused, with the ids the board does have.** `--column`
  is free text an agent typed, and a task filed under a column the project
  lacks is not misfiled but invisible — every view renders columns. Naming the
  real ids in the error means the next attempt does not cost a second command
  to find out what the board is called.
- **`task move` is the only command that changes where a task sits** — its
  column, its order and whether it is finished are one fact, and `task update`
  deliberately cannot touch them. Finishing something is moving it into a
  `done` column, not a flag.
- **A project points at exactly one subject, so `-s` breaks ties differently
  here.** Prefix matching is the same as `run` and `calendar`, but a bare code
  legitimately matches the same course in two terms; `one_subject` resolves
  that in favour of the **current** term, and a tie that survives that is
  reported with the full codes rather than guessed. Omitting `-s` makes a
  personal project rather than an ambiguous one.
- **Everything this binary writes is marked `source: agent`**, so the board can
  show which rows it did not write itself.

## Docs for the agents that use it

`oculus docs` fills `agents/` in the data directory, so a coding agent pointed
at a course folder can work without anyone explaining Oculus to it. Sources
live in `app/src-tauri/templates/`; everything except the CLI reference is
written by `app/src-tauri/src/agents.rs`, which **the sync path calls too** —
see the bullet on scaffolding in [sync.md](./sync.md).

```
<data>/agents/
  AGENTS.md       one copy, symlinked into every course folder
  OCULUS-CLI.md   rendered from this binary's own help
  OCULUS.md       stub
  TASTE.md        stub — standing preferences
  memories/       cross-subject; MEMORY.md index stubbed beside them
  memories/<code>/  one subject's, with its own MEMORY.md index
  skills/<name>/SKILL.md    one procedure, generated
  .claude/skills/<name> → ../../skills/<name>
  .agents/skills/<name>  → ../../skills/<name>
<data>/courses/<code>/
  AGENTS.md → ../../agents/AGENTS.md
  agents/INSTRUCTIONS.md    hand-written, for this subject alone
  agents/memories → ../../../agents/memories/<code>
```

**The skills are one directory reached three ways**, because the three CLIs
disagree about where a skill lives. Two of the three disagree the same way:
Claude Code scans `<cwd>/.claude/skills` and Codex scans
`<cwd>/.agents/skills`, both walking up from the working directory, so both
get a relative link beside the one copy. opencode takes a `skills.paths` key
in its config and is linked for not at all. Every path is inside the library —
nothing here writes to a home directory. See [harness.md](./harness.md).

**Both memory buckets live under `agents/`, and that is containment rather
than filing.** `agents/` is the only folder an in-app chat thread can write to
(see [harness.md](./harness.md)), so a subject bucket in the course folder was
a path every template named and no thread could use. The course folder keeps a
symlink to it, and `agents.rs` moves any files it finds on the old path into
the bucket the first time it runs — once, since what it leaves behind is a
link.

Three different lifetimes, which is the whole design:

- **Generated, always overwritten.** `OCULUS-CLI.md` walks clap's command tree
  rather than any list, so a new subcommand or flag appears the moment it
  exists and a test asserts the coverage. Nothing is hand-written, because an
  agent trusts a file over `--help` — a stale reference is worse than none.
  `AGENTS.md` is overwritten too: it is one universal file, which is what
  removes any per-course copy to keep in sync. So are the `skills/`, for a
  sharper version of the same reason: a skill is read as a procedure rather
  than as background, so one naming a flag this binary no longer has is
  followed instead of weighed. There is exactly one copy of each and every
  run rewrites it.
- **Stubbed once, then the user's.** `OCULUS.md`, `TASTE.md` and the
  `MEMORY.md` index in each memory folder are written only when absent. They are the one thing in `agents/` a human authors, and
  overwriting them would be the only unrecoverable thing this command could
  do. So is `agents/INSTRUCTIONS.md` in a course folder, which nothing writes
  at all — it is where per-subject instruction lives now that `AGENTS.md` is
  universal.
- **Linked, never clobbered.** The course-folder `AGENTS.md` is a *relative*
  symlink, so the library stays movable. A wrong target is relinked; a real
  file is left alone with a warning, because it is somebody's work. The two
  skill links follow the same rule — Claude's relative, Codex's absolute
  because its home is a different tree — so a student's own `oculus-plan` in
  `~/.codex/skills` survives a sync.

Details worth not rediscovering:

- Rendering is pinned to 88 columns with colour off, so the output depends on
  the binary and not the terminal that ran it. Wrapping needs clap's
  `wrap_help` feature, which is why it is enabled in `Cargo.toml`.
- Global `--json` is hidden below the root: clap would otherwise repeat its
  full text under all sixteen subcommands, which was a third of the file.
- At ~12 KB `OCULUS-CLI.md` is a *pull* document. `AGENTS.md` says when to
  open it rather than pasting it into every context — the same reason
  `AGENTS.md` itself stays short.
- `bun run cli:install` runs `oculus docs`, so the reference always describes
  the binary actually on `PATH`. `bun run cli` deletes the old binary first:
  cargo will otherwise report success while leaving a stale one in place,
  which would document the wrong build. `bun run cli:dev` is the same build in
  the debug profile — the one the dev app's agents actually run, see
  [development.md](./development.md#the-dev-cli).
- The same rendering also lands in the repo as
  [cli-reference.md](./cli-reference.md), written by
  `app/scripts/gen-cli-docs.mjs` from `oculus docs --stdout`. It runs from
  `beforeBuildCommand`, straight after `stage-cli` has built the release
  binary — the one moment in the toolchain where a current binary is
  guaranteed to exist, so a bundle cannot ship a CLI its reference does not
  describe. It also runs from the dev preflight
  (`app/scripts/predev.mjs`), against the debug binary that preflight has just
  built: that used to be impossible because `tauri dev` never built the CLI at
  all, and it is one process launch now that it does. Either way the generator
  rewrites the file only when the help actually changed, so a clean tree stays
  clean and a changed CLI is dirty in the same commit as its reference.
- Why the repo needs a copy at all: `OCULUS-CLI.md` only exists on a machine
  where the CLI has been installed. The repo copy is for a reader — or an
  agent working on Oculus rather than on a library — with no built binary.
  This page stays prose; the generated file carries the flags.
- **A sync links new course folders on its own**, so `oculus docs` is for
  refreshing the reference after a rebuild, not for catching up on enrolment.
  It is still the only thing that writes `OCULUS-CLI.md`, and its `link_all`
  sweep still covers folders no sync touched — an old term, or a stray
  directory an earlier sync left behind. `link_course` is narrower by design:
  it takes a subject code, so a sync annotates only what it actually scraped.
