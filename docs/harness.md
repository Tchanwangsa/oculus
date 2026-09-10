# The harness: CLI agents as chat

Chat is a coding agent the student already has — Claude Code or Codex —
run as a subprocess from the library, with its output folded into one
timeline. No API key, no per-token billing: the CLIs carry the student's own
subscription, which is the whole reason for driving them rather than the
model APIs. The shape is bb's (get-bb/bb) with its plugin system taken out:
one bridge per provider, one normalized event stream, a timeline that only
ever sees the stream.

The BYOK layer in [llm.md](./llm.md) is dormant while this is the chat: its
Rust and Settings section are still there, nothing routes to them, and the
old chat page and store are gone.

## Where

| Piece | Location |
| --- | --- |
| Module docs, the manager, Tauri commands, headless `run_once` | `app/src-tauri/src/harness/mod.rs` |
| The normalized event enum and tool classification | `app/src-tauri/src/harness/event.rs` |
| Claude Code bridge (`claude -p`, stream-json) | `app/src-tauri/src/harness/claude.rs` |
| Codex bridge (`codex app-server`, JSON-RPC) | `app/src-tauri/src/harness/codex.rs` |
| Finding the binaries from a GUI app | `app/src-tauri/src/harness/discover.rs` |
| Thread and timeline rows | `app/src-tauri/src/harness/store.rs`, migrations 24–25 in `app/src-tauri/src/lib.rs` |
| Instructions appended to the provider's prompt | `app/src-tauri/templates/HARNESS.template.md` |
| Recorded provider output the bridge tests replay | `app/src-tauri/fixtures/harness/` |
| Frontend types, reads, commands | `app/src/lib/harness.ts` |
| Live state, event folding | `app/src/stores/harnessStore.ts` |
| Page, thread list, timeline, rows, composer | `app/src/pages/ChatPage.tsx`, `app/src/components/harness/` |
| The `@` menu's candidate files | `searchMentionFiles` in `app/src/lib/db.ts` |
| Health in Settings → AI | `app/src/pages/settings/AiPage.tsx` |
| `oculus agent` | `app/src-tauri/src/bin/oculus.rs` |

## How it connects

- **One event stream, two dialects.** Claude's `stream-json` lines
  (Anthropic stream events, a full `assistant` message per block, a `user`
  message per tool result, a `result` per turn) and Codex's JSON-RPC
  notifications (`item/started`, `item/agentMessage/delta`, `item/completed`,
  `turn/completed`, `thread/tokenUsage/updated`…) both become
  `HarnessEvent`: session started, user message, turn started, assistant and
  thinking deltas and their completed blocks, tool started / output / finished,
  usage, rate limits, turn finished, error, exited. Tool calls carry a
  provider-neutral `ToolKind` plus the raw name; `classify` in `event.rs` is
  the one table both bridges share, and it knows an `oculus …` command from
  any other Bash.
- **Claude is a process per thread; Codex is one server for all of them.**
  A Claude thread is one long-lived `claude -p --input-format stream-json`
  process that takes user turns on stdin and is resumed by session id
  (`--resume`) when a new message finds it gone. Codex is one
  `codex app-server` per app, started on first use, with every thread a
  `threadId` inside it — the protocol routes by thread, so a process per
  thread would buy nothing here. Both are handles behind the same `Harness`;
  nothing above it assumes a process per thread.
- **Every thread runs from `agents/`, and that is the containment.** Not the
  library root: measured, a Claude thread rooted there under `acceptEdits`
  wrote straight into `courses/`. From `agents/` the two providers refuse
  writes to `../courses/` in different ways and for the same reason — Codex's
  `workspace-write` sandbox has one writable root, and Claude runs under its
  own sandbox setting with the cwd as the only write root, `--add-dir` for
  reads across the library (without it even `ls ../courses` is refused), and
  `Edit` deny rules on every sibling of `agents/` so `--add-dir` does not
  put the courses back inside `acceptEdits`. `oculus.db` is named in those
  rules; `courses/` heals on the next sync, the database does not. The
  module docs in `claude.rs` say what each part buys and what broke without
  it. Bash writes are refused by both sandboxes at the OS level.
- **The prompt is appended, not replaced.** `HARNESS.template.md`, rendered
  with the real data-dir path and the course folders on disk, goes in as
  `--append-system-prompt` (Claude) or `developerInstructions` (Codex). It
  says where the library is, that `oculus` is on PATH and what it is for,
  that writes stay in `agents/`, and where memory goes. Codex also reads
  `agents/AGENTS.md` on its own, so that file's opener now covers both the
  course folders it is symlinked into and the folder it lives in.
- **The child's environment is edited twice.** `ANTHROPIC_API_KEY` and
  `OPENAI_API_KEY` are stripped, so a key in the shell cannot silently move
  a subscription session onto API billing. And the `oculus` binary's
  directory is put first on PATH — `AGENTS.md` tells the agent to run
  `oculus grep`, and advice that resolves to "command not found" is worse
  than none. Claude's auto-memory is switched off in the same settings
  document: asked to write to `memories/`, it reached for
  `~/.claude/projects/…/memory/` instead, and the library has its own layer.
- **Finding the binaries is the sidecar's problem again.** A Dock-launched
  app has launchd's PATH. `discover.rs` tries `OCULUS_CLAUDE_BIN` /
  `OCULUS_CODEX_BIN`, then PATH, then where the installers put things, then a
  login shell's `command -v`; the answer is cached, failures included, since
  re-asking a login shell on every send would make a missing CLI slow as
  well as absent. Settings → AI shows the result and can recheck.
- **Rust writes the rows; the webview folds the stream.** One consumer
  thread takes every event from every bridge in order, writes it to
  `harness_threads` / `harness_items` (`store.rs`), then emits it on
  `harness-event` with the row id it made. So a tool's finish can never
  overtake its start, and a crash mid-turn leaves a tool row that says so.
  `useBackendEvents` hands the event to `harnessStore`, which keeps only
  what has no row — text and reasoning still streaming, command output still
  arriving — and patches the tool row when its result lands. A page reload
  reads the rows and loses nothing.
- **Approvals and questions have nowhere to go yet.** Claude runs with
  `--permission-prompts none`, so anything the mode does not already allow
  is refused rather than hung; Codex runs with `approvalPolicy: never` and
  native user questions switched off, and the one server request that could
  still arrive is answered with a decline. This is the seam a later stage
  fills — the events and the row kinds are already there.
- **Interrupt is a control message, not a kill.** Claude takes a
  `control_request` of subtype `interrupt` on stdin; Codex takes
  `turn/interrupt` with the active turn id. The process stays up either way,
  and the turn closes with status `interrupted`.
- **Every raw line is kept.** `agents/threads/<id>.ndjson` gets each provider
  line as it arrives (id 0 is the shared Codex server and headless runs). It
  is how a translation bug is diagnosed without re-running an agent, and the
  recordings in `fixtures/harness/` that the bridge tests replay are these
  files. `app-server` is marked experimental; when its shapes drift, a
  recording is the difference between a morning and a week.
- **Codex specifics worth not rediscovering.** `thread/start` takes
  `sandbox` (a mode string) while `turn/start` takes `sandboxPolicy` (an
  object). A resumed thread replays its previous turn's token usage before
  doing anything, which the bridge drops until the next `turn/started`.
  Deltas can arrive before the `item/started` that names them, so a
  completion for an item never opened synthesises the open. Reasoning
  effort is a config key on the thread (`model_reasoning_effort`), not a
  turn parameter. And the server's stderr is every MCP server in the user's
  own `~/.codex/config.toml` failing to sign in; only a tail is kept, for
  the exit message.
- **Claude specifics.** `result.usage` sums every request in the turn — that
  is spend, not context — so context tokens come from the last `assistant`
  message's usage instead. Tool inputs are never taken from the streamed
  `input_json_delta`s; the complete call arrives on the `assistant` line
  right after. A `content_block_start` can carry a prefix of text the
  deltas do not repeat. `CLAUDECODE` and `CLAUDE_CODE_ENTRYPOINT` are
  stripped from the child env because a dev app started from inside a
  Claude Code session inherits them and the CLI refuses to nest.

## The timeline

bb's rows, one level of grouping instead of two. Messages are the spine: the
user's in a bubble on the right (`max-w-[70%]`), the assistant's as
full-width markdown, in a 760px column. The work between two messages —
tool calls, reasoning — is a *step*; a finished step of more than one row
folds into one summary row ("Explored 3 files, ran 2 commands") that opens
to the rows, and finished rows are dimmed. The step still running stays
unfolded at full strength, with a spinner on the open call, streaming text
under it, and "Working…" when nothing is streaming. A tool row is
`[icon] [verb] [title]` with a chevron on hover and a detail card — the
command or the arguments, then the output — behind it; reasoning is a row
titled "Thought" with the text behind it. Command output is the one thing
outside markdown set in monospace, through `CodeText` in
`app/src/components/markdown/MdComponents.tsx`, which is where the font
lives.

The composer is bb's: Enter sends, Shift+Enter breaks a line, the send
button becomes stop while a turn runs, and a footer line carries context
used, spend, and the account's rate-limit windows (Claude's 5-hour and
weekly; Codex's primary and secondary). A thread keeps its provider; the
model can change per send, which on Claude means the next process.

## The model picker

One control does agent, model and reasoning level —
`app/src/components/harness/ModelPicker.tsx`, ported from bb's
`ModelReasoningPicker`. The trigger reads `[mark] Model Name Level ⌄`; the
menu is a strip of provider marks (underlined when active), the models of the
active provider, and a row of levels under a rule. Choosing a model closes
the menu, choosing a level does not — the level is the fine adjustment after
the coarse one. The marks are the two `currentColor` SVGs in
`app/src/components/harness/ProviderMark.tsx`, monochrome like bb's so they
sit in the palette rather than fighting the indigo.

Three things about it are deliberate:

- **Nothing is defaulted out of sight.** There is no "default model" row and
  no "default" level: every turn names both, so what the composer shows is
  what the CLI is told. A fresh composer opens on a real model
  (`defaultSelection` in `app/src/lib/harness.ts`) and that model's own
  preferred level.
- **Models are named, not aliased.** Rows read "Opus 5 (1M)", not "opus".
  `claude --model` takes full names as happily as the moving aliases, so the
  ids in `CLAUDE_MODELS` are the names themselves. Claude Code has no
  model-list call over the stream-json protocol — bb probes it through the
  Agent SDK instead — so that list mirrors bb's catalogue and is the one
  thing here that goes stale by hand. Codex answers `model/list` and needs no
  such list.
- **Levels belong to the model, not the provider.** `claude --effort` takes
  five (`low`…`max`); Codex declares a subset per model. The picker reads
  them off the selected row, so a model that only reasons a little never
  offers a level it would reject.

Both CLIs bind the level when a session starts, not per turn — so changing it
mid-thread respawns the Claude process and restarts the Codex thread, which
`Harness::send` decides by comparing the level a live session was started with
against the one being asked for.

## Subject scope and `@`

The composer carries two more controls than a bare prompt box, both in
`app/src/components/harness/Composer.tsx`.

**A subject, or General.** `SubjectSelect.tsx` scopes the thread. It is not a
sandbox — every thread runs from `agents/` and reads all of `../courses/`
either way — it says which subject the questions are about, so "what's due
this week" has an answer. Picking one appends a short section to the
instructions naming that course folder and its memory bucket
(`instructions()` in `app/src-tauri/src/harness/mod.rs`); General appends
nothing and gets the library-wide brief. The scope is stored on the thread
(`harness_threads.subject_id`) and locks once the thread exists, for the same
reason the provider does: both CLIs bind the appended instructions at session
start, so a re-scope would be a lie until the process was restarted. Rust
reads it back off the row rather than trusting the payload, and joins
`subjects` for the folder name so a renamed subject cannot leave a thread
pointing at a folder that is gone.

**`@` picks a file.** The menu lists files narrowed to the thread's subject,
ordered by prefix match then by what was opened recently, and only ones the
agent can actually read — markdown as written, everything else once the
sidecar has parsed it, the same predicate retrieval uses. Choosing one writes
its **library path** into the message and nothing else. No content is
attached and nothing is retrieved here: the agent already has the library in
front of it and its own tools for opening a file, and a path is what it was
missing. That path is also exactly what `oculus read` takes, which
`HARNESS.template.md` tells the agent.

The earlier BYOK chat agent did the opposite — it read the file, embedded the
query and packed the result into the request — because its model could only
see what the prompt carried. A CLI agent can open the file itself, so that
whole path was dropped rather than ported.

## Stages

Built: the two bridges with recorded fixtures and replay tests, `oculus
agent` as the headless proof, tables and lifecycle, the page, subject scope
and the `@` file menu. Not yet:
approvals and native questions routed to the UI, steering mid-turn (bb's
`turn/steer` and a second stdin line), the plan/todo card, forking, and a
third bridge for the API path when BYOK returns.
