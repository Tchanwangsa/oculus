# The harness: CLI agents as chat

Chat is a coding agent the student already has — Claude Code, Codex or
opencode — run as a subprocess from the library, with its output folded into
one timeline. Claude Code and Codex carry the student's own subscription,
which is the whole reason for driving them rather than the model APIs;
opencode is the odd one out and carries whatever provider key `opencode auth`
holds, so it is the one agent here that does spend per token — it earns its
place by reaching every provider at once rather than by being free. The shape
is bb's (get-bb/bb) with its plugin system taken out: one bridge per provider,
one normalized event stream, a timeline that only ever sees the stream.

The BYOK API layer this replaced is gone: its Rust, its Settings sections and
its chat page and store were deleted once nothing routed to them. Migrations
16 and 17 stay, so `llm_usage`, `chats` and `chat_messages` are still there
and still empty of readers (see `app/src-tauri/src/lib.rs`).

## Where

| Piece | Location |
| --- | --- |
| Module docs, the manager, Tauri commands, headless `run_once` | `app/src-tauri/src/harness/mod.rs` |
| The normalized event enum and tool classification | `app/src-tauri/src/harness/event.rs` |
| Claude Code bridge (`claude -p`, stream-json) | `app/src-tauri/src/harness/claude.rs` |
| Codex bridge (`codex app-server`, JSON-RPC) | `app/src-tauri/src/harness/codex.rs` |
| opencode bridge (`opencode serve`, HTTP + SSE) | `app/src-tauri/src/harness/opencode.rs` |
| opencode's containment ruleset and system prompt | `app/src-tauri/templates/OPENCODE.template.json` |
| Finding the binaries from a GUI app | `app/src-tauri/src/harness/discover.rs` |
| Installing a missing one from Settings → AI | `app/src-tauri/src/harness/install.rs`, `app/src/components/settings/InstallAgentDialog.tsx` |
| Signing an installed one back in, and classifying an auth failure | `app/src-tauri/src/harness/signin.rs`, `app/src/components/harness/SignInDialog.tsx` |
| Whether each CLI has credentials, shared by the three surfaces that ask | `app/src/hooks/useSignInStatus.ts` |
| Thread and timeline rows | `app/src-tauri/src/harness/store.rs`, migrations 24–26, 28 and 30 in `app/src-tauri/src/lib.rs` |
| Instructions appended to the provider's prompt | `app/src-tauri/templates/HARNESS.template.md` |
| The library's own `AGENTS.md`, which Codex reads on its own | `app/src-tauri/templates/AGENTS.template.md` |
| Recorded provider output the bridge tests replay | `app/src-tauri/fixtures/harness/` |
| Frontend types, reads, commands | `app/src/lib/harness.ts` |
| Live state, event folding | `app/src/stores/harnessStore.ts` |
| Page, thread list, timeline, rows, composer | `app/src/pages/ChatPage.tsx`, `app/src/components/harness/` |
| The `@` menu's candidate files | `searchMentionFiles` in `app/src/lib/db.ts` |
| The `@` token, the menu and its keys, shared with a task body | `app/src/components/harness/useMentionMenu.ts`, `app/src/components/harness/MentionMenu.tsx` |
| Pictures pasted or dropped into any composer | `app/src-tauri/src/harness/attach.rs`, `app/src/lib/attachments.ts`, `app/src/hooks/useAttachments.ts`, `app/src/hooks/useFileDrop.ts`, `app/src/components/harness/AttachmentStrip.tsx` |
| A selection copied out of a thread as markdown | `app/src/lib/selectionMarkdown.ts` |
| Health and the per-job model rows in Settings → AI | `app/src/pages/settings/AiPage.tsx` |
| opencode's providers and models: the Settings line, the manager, the generic sign-in form | `app/src/components/settings/OpencodeProvidersSection.tsx`, `app/src/components/settings/OpencodeCatalogDialog.tsx`, `app/src/components/settings/OpencodeConnectDialog.tsx`, `app/src/lib/opencodeAuth.ts` |
| Which opencode models the picker is allowed to offer | `app/src/lib/opencodeCatalogue.ts` |
| Which opencode models a picker may offer | `filterOffered`/`unusableReason`/`isZen` in `app/src/lib/opencodeCatalogue.ts` |
| Whether each CLI is installed, shared by every picker | `app/src/hooks/useBridgeHealth.ts` |
| Which agent, model and level each headless job runs on | `app/src-tauri/src/harness/jobs.rs`, `app/src/lib/db.ts` |
| The lecture brief a dock thread is scoped to | `instructions` in `app/src-tauri/src/harness/mod.rs` |
| The frame grabs a message's moment carries | `lecture_grab_frames` in `app/src-tauri/src/chapters.rs`, `app/src/lib/lectures.ts` |
| The lecture player's dock chat, and its composer | `app/src/components/lectures/LectureChatPanel.tsx`, `app/src/components/lectures/LectureChatComposer.tsx` |
| `oculus agent` | `app/src-tauri/src/bin/oculus.rs` |

## How it connects

- **One event stream, two dialects.** Claude's `stream-json` lines
  (Anthropic stream events, a full `assistant` message per block, a `user`
  message per tool result, a `result` per turn) and Codex's JSON-RPC
  notifications (`item/started`, `item/agentMessage/delta`, `item/completed`,
  `turn/completed`, `thread/tokenUsage/updated`…) both become
  `HarnessEvent`: session started, user message, turn started, assistant and
  thinking deltas and their completed blocks, tool started / output / finished,
  usage, rate limits, turn finished, error, exited. Three variants have no
  provider behind them — `ThreadTitled`, the answer to the naming turn below,
  and `Queued`/`Unqueued`/`Rewound`, which are the queue and the rewind below —
  because they are persisted and forwarded by the same path as the rest. Tool
  calls carry a provider-neutral `ToolKind` plus the raw name; `classify` in
  `event.rs` is the one table both bridges share, and it knows an `oculus …`
  command from any other Bash.
- **Claude is a process per thread; Codex and opencode are one server each.**
  A Claude thread is one long-lived `claude -p --input-format stream-json`
  process that takes user turns on stdin and is resumed by session id
  (`--resume`) when a new message finds it gone. Codex is one
  `codex app-server` per app, started on first use, with every thread a
  `threadId` inside it — the protocol routes by thread, so a process per
  thread would buy nothing here. opencode is the same shape again: one
  `opencode serve` on a loopback port, one HTTP *session* per thread, and one
  SSE connection to `GET /event?directory=` for the whole app, routed by
  `properties.sessionID`. All three are handles behind the same `Harness`;
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
  it. Bash writes are refused by both sandboxes at the OS level, with one
  named exception: the database's three files, below.
- **opencode is the exception to that last sentence, and it is worth knowing
  before trusting it.** It has no sandbox at all: its permissions are a rule
  list its own runner checks. For the file tools that is real and measured —
  `edit` (which governs `write`, `edit` and `apply_patch` together; there is
  no separate `write` key) refuses every path outside `agents/` — but `bash`
  is matched by a glob over the command *string*, so the ruleset allow-lists
  `oculus …` and `ls …` and denies the rest. That stops `rm -rf ../courses`;
  it does not stop a redirect smuggled onto the end of an allowed command.
  An opencode thread is therefore one notch less contained than a Claude or
  Codex one, by the CLI's design rather than by this app's configuration.
  Three rule shapes were measured before the current one worked, and
  `opencode.rs`'s module docs list them: a leading `"*": "deny"` denies
  everything however many allows follow it (deny wins on the path check,
  exactly as in Claude's syntax, which is why the siblings of `agents/` are
  named individually here too); whichever rule comes *last* decides whether
  the tool is offered to the model at all, so an allow has to be last or the
  tool vanishes; and a pattern for a path inside the session directory has to
  be written *relative* to it, which is how `opencode.json` itself is put
  beyond the agent's reach.
- **The agent writes to the database through the CLI, and that took a hole in
  both sandboxes — three files wide.** `oculus project` and `oculus task` put
  a plan into the tables the app's board draws
  ([projects.md](./projects.md)), so a breakdown the student agrees to is rows
  rather than a markdown file in `agents/`. But `oculus.db` sits at the
  library root, outside the one writable root, and **a sandbox that lets
  SQLite open the database but not `oculus.db-wal` fails with "attempt to
  write a readonly database"** — which is what every `oculus task add` from a
  Claude or Codex thread did, transactionally and silently, while the
  templates told the agent to plan that way. So `paths::db_write_paths`
  names the three files (`oculus.db`, `-wal`, `-shm`) and both bridges hand
  them over: `filesystem.allowWrite` for Claude, `writableRoots` on the turn's
  `sandboxPolicy` — and `sandbox_workspace_write.writable_roots` in the
  thread's config, since `thread/start` takes no policy — for Codex.
  Measured, and measured as *files*: a seatbelt grant on a regular file works,
  and granting the folder instead would put the session cookie and the Ed
  token inside the agent's reach. opencode needed nothing, having no sandbox
  to open.
  The CLI stays the only door: `oculus.db` is still `Edit`-denied for
  Claude and opencode, and `sqlite3` is denied by name, because the binary is
  what knows that a column id must exist, that `done_at` follows the
  destination column's kind, and that a whole breakdown belongs in one
  transaction — none of which a `sqlite3` a model reached for would honour.
  Codex has no per-path rules to pair with its sandbox, which is why the hole
  is three files and not a folder.
  Both templates say so. The board picks the write up because
  `useBackendEvents` watches this event stream for a finished tool call whose
  command names one of those and fires `PROJECTS_UPDATED_EVENT`; it matches on
  the command text rather than `classify`'s `ToolKind`, since `is_oculus_cli`
  only word-matches the first few words and `cd … && oculus task add` reads as
  plain Bash.
- **The prompt is appended, not replaced — except on opencode, where it is
  the whole prompt.** `HARNESS.template.md`, rendered with the real data-dir
  path and the course folders on disk, goes in as `--append-system-prompt`
  (Claude) or `developerInstructions` (Codex). opencode has no append at all:
  an agent's `prompt` *replaces* the system prompt, and it lives in a config
  document, not in a call — so the rendered template is the `oculus` agent's
  `prompt` inside `agents/opencode.json`, rewritten on every server start,
  and the per-thread half of the brief (the subject, the lecture) rides the
  first message of the session because there is nowhere else to put it.
  opencode reads `agents/AGENTS.md` natively, the way Codex does. It
  says where the library is, that `oculus` is on PATH and what it is for,
  that writes stay in `agents/`, and where memory goes. Codex also reads
  `agents/AGENTS.md` on its own, so that file's opener now covers both the
  course folders it is symlinked into and the folder it lives in.
- **The child's environment is edited twice.** `ANTHROPIC_API_KEY` and
  `OPENAI_API_KEY` are stripped, so a key in the shell cannot silently move
  a subscription session onto API billing. The same strip does a second job
  for opencode, which reads both names itself: measured, a server started
  with either set grows a whole provider nobody chose — sixteen Anthropic
  models, sixty-one OpenAI ones — so without the strip the app would offer a
  different catalogue launched from a terminal than from the Dock. Stripped,
  opencode sees only what `opencode auth` holds — which is why [signing a
  provider in](#signing-in-to-a-provider) writes to *that* store rather than
  passing a key through the environment, and why building it did not have to
  soften this. And the `oculus` binary's
  directory is put first on PATH — `AGENTS.md` tells the agent to run
  `oculus grep`, and advice that resolves to "command not found" is worse
  than none. Claude's auto-memory is switched off in the same settings
  document: asked to write to `memories/`, it reached for
  `~/.claude/projects/…/memory/` instead, and the library has its own layer.
- **Finding the binaries is the sidecar's problem again.** A Dock-launched
  app has launchd's PATH. `discover.rs` tries `OCULUS_CLAUDE_BIN` /
  `OCULUS_CODEX_BIN` / `OCULUS_OPENCODE_BIN`, then PATH, then where the installers put things, then a
  login shell's `command -v`; the answer is cached, failures included, since
  re-asking a login shell on every send would make a missing CLI slow as
  well as absent. The `--version` probe behind `health` is cached beside it,
  because that answer is no longer read only by Settings → AI — every model
  picker asks it now (below), and three process spawns per menu is the cost
  the login-shell fallback was cached to avoid. So `harness_health` takes a
  `recheck` flag: Settings' button passes it and drops both caches, for right
  after an install, and nothing else does.
- **And a missing CLI can now be installed from the row that says so**, which
  is [Installing an agent](#installing-an-agent) below. The same three steps
  find `brew`, `npm`, `bun` and `curl` — `discover::tool`, cached beside the
  providers — so the offer matches the machine rather than assuming Homebrew,
  and the login-shell step is what makes that answer true at all, since `brew`
  lives where launchd's PATH does not look.
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
- **A refusal there is sticky, which is why the CLI is allowed by name.**
  Claude's sandbox auto-allows Bash only for commands its analyser can
  statically vouch for, and a plan is what it cannot: a `--brief` carrying
  newlines, a loop over subjects, a compound line. Measured on one thread, 36
  of 39 `oculus` calls cleared it and the three that did not included the
  `project create` the turn existed for — after which *every* remaining
  approval in that session was auto-denied too. So `claude.rs` allows
  `Bash(oculus:*)` outright. It is a prompt rule and not a sandbox one: the
  seatbelt still bounds what the command may touch, deny still beats allow, and
  `sqlite3` stays shut. The other two bridges already had this and needed no
  change — opencode's ruleset allow-lists `oculus` / `oculus *` by name, and
  Codex has no prompt to fall through to.
- **Interrupt is a control message, not a kill.** Claude takes a
  `control_request` of subtype `interrupt` on stdin; Codex takes
  `turn/interrupt` with the active turn id. The process stays up either way,
  and the turn closes with status `interrupted`. What that costs to get right
  is [Stopping a turn](#stopping-a-turn) below.
- **A thread runs one turn at a time, and the manager is what makes that
  true.** Both CLIs accept a second message mid-turn; neither does anything
  good with it, and the two do different wrong things, which is why the wait
  is here rather than left to them. See [One turn at a
  time](#one-turn-at-a-time).
- **`run_once` has a caller outside chat now.** One prompt, one turn, no
  thread and no rows — `oculus agent` was its only user, as the smallest
  end-to-end proof of a bridge, and the chaptering job in
  [chapters.md](./chapters.md) is the first real one. That job is worth reading
  as the shape of a *headless* agent job here: it names its own provider,
  model and reasoning level from the registry below rather than chat's
  selection, it collects
  the `AssistantMessage` text and parses it itself, its `on_event` closure is
  what the lecture panel draws a live "what it is doing" line from — the same
  `ToolStarted` titles the timeline uses, in a job with no timeline — and the
  database write is Rust's — the agent replies with JSON and never goes near a table, because
  chapters are derived data rather than the student's own planning. Thread id
  0's raw log (`agents/threads/0.ndjson`) is where such a run's provider lines
  land, alongside the shared Codex server's.
- **Every raw line is kept.** `agents/threads/<id>.ndjson` gets each provider
  line as it arrives (id 0 is the shared Codex server and headless runs). It
  is how a translation bug is diagnosed without re-running an agent, and the
  recordings in `fixtures/harness/` that the bridge tests replay are these
  files. `app-server` is marked experimental; when its shapes drift, a
  recording is the difference between a morning and a week.
- **opencode runs on its v1 session API, and that is not a detail.** The
  bridge was written against v2 — `/api/session`, `/api/event`, the
  `session.next.*` event family — and moved wholesale, because on 1.18.31 a
  v2 prompt on any provider whose credential lives in `auth.json` failed
  inside the server with `ModelUnavailableError` and emitted no event at
  all. That is OpenRouter, which is every student here, so the whole
  provider story was unreachable over v2 while the same session over v1
  answered. The v2 translator and its recordings are kept — they still fold
  a recorded stream, and the tests still replay it — until v1 has run on
  real threads for a while, but the endpoints are not dual: one `dispatch`
  routes by wherever the session id is and picks the translator off the
  envelope (`properties` is v1, `data` is v2).
- **opencode specifics worth not rediscovering.** `--port 0` means "prefer
  4096", not "pick a free port", so the real port is parsed off the one
  stdout line that says it. Every call names `?directory=<the session dir>`,
  because a bare `/event` or `/session` binds to the server's own cwd —
  wherever the app was launched from — and would stream a different
  instance's sessions. The bridge checks that `oculus` is in
  `GET /agent?directory=` before creating a session: a session created
  without it runs as a built-in agent with none of the containment. And the
  agent and model a session is *created* with are decorative — a prompt that
  does not repeat them runs as opencode's stock `build` agent — so the route
  holds both and every prompt sends them. `prompt_async` answers 204 with no
  body, so nothing about a turn is known until the stream says it. A turn
  opens on the user's `message.updated` and closes on the completed
  assistant one; `session.idle` fires on every turn and is the backstop for
  a turn nothing else will ever close — no assistant row at all, or one
  created and never completed — except during an abort, where idle arrives
  *before* the partial answer and would close the turn early. An interrupt
  is `POST /abort`, arriving as `session.error{MessageAbortedError}` and
  mapped to `interrupted` rather than to an error row. The drain watchdog —
  a user message with no assistant one after it in thirty seconds — is
  inherited from v2, where a turn whose model could not be resolved emitted
  **nothing at all** and `POST /api/session/{id}/wait`, which the schema
  offers for exactly this, answered 503 straight away; on v1 the known cases
  say so themselves and it is belt to the stream's braces. The free
  `opencode/*` Zen models cannot be driven over the HTTP API at all (the gateway answers
  "OpenCode's free tier can only be used in OpenCode"), so that 400 is
  rewritten into a sentence a student can act on — and, since that is a whole
  provider the picker should never have offered, [the catalogue](#which-models-reach-the-picker)
  is what now keeps it out of the menu rather than out of the error row.
- **The same server is also the credential store's door.** opencode reaches
  two hundred and eighteen providers and answers for the ones `opencode auth`
  holds a key for, which used to mean a terminal. `opencode serve` exposes the
  whole auth surface — the provider list, a declarative form spec per provider,
  the credential write, and the OAuth flows including the loopback listener the
  browser comes back to — so Settings drives it over the connection the bridge
  already has. [Signing in to a provider](#signing-in-to-a-provider) is the
  shape of it.
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
- **Not every notification is about a thread.**
  `account/rateLimits/updated` is about the subscription, and the shared
  server sends it with no `threadId` on it — so the route lookup that every
  other notification needs silently ate it, and Codex showed a context ring
  with no plan windows behind it while the fixture test passed. Account-scoped
  methods are translated and dispatched *before* the lookup
  (`translate_account`), into a sink that belongs to the harness rather than
  to a session, and reach the webview on thread id 0. That is also why
  `harness-event` carries the provider: an event with no thread has no row to
  read it off. Claude needs none of this — its processes are one per thread,
  so its windows arrive on a thread's own stream.
- **The windows can be asked for, not only waited for.** The push comes with
  a model call, so on its own the meter shows the last turn's numbers —
  after a night, a window that has since reset. `account/rateLimits/read`
  answers the same object with no turn and no quota, so the bridge reads it
  when the server starts and again when the Chat page opens
  (`harness_refresh_rate_limits`), and the answer goes out on the account
  sink exactly like the push. The page-open read never starts the server: a
  visit is not a reason to spawn a CLI, and a server that has just started
  has already seeded itself. Claude has no equivalent over `stream-json` —
  its control protocol has a `get_usage` request, but the bridge does not
  speak it — so there the stored snapshot stands until the next turn.
- **Claude specifics.** `result.usage` sums every request in the turn — that
  is spend, not context — so context tokens come from the last `assistant`
  message's usage instead. Tool inputs are never taken from the streamed
  `input_json_delta`s; the complete call arrives on the `assistant` line
  right after. A `content_block_start` can carry a prefix of text the
  deltas do not repeat. `CLAUDECODE` and `CLAUDE_CODE_ENTRYPOINT` are
  stripped from the child env because a dev app started from inside a
  Claude Code session inherits them and the CLI refuses to nest.

## The thread list

The conversations column groups threads under the subject they were scoped
to, General included, and each row is the agent's monochrome mark beside the
thread's name (`app/src/components/harness/ThreadList.tsx`). The model is not
on the row: it can change per send, the composer already shows it, and the
list answers "which conversation", not "on what". A group's header carries a
`+` that opens a new thread already scoped to that subject — the shortest
path to the scope choice below, made from the one place it is already
answered.

**The column is the reader's to size, and to put away.** It drags wider from
the seam and folds to nothing from the button beside *New thread* or with
⌘⌥B (`useResizablePanel` in `app/src/hooks/useResizablePanel.ts`, driven from
`ChatPage`); both the width and the fold are remembered in `localStorage`.
Folded means gone, not narrowed to a rail — the app's own sidebar pattern — so
the way back in is a button in the chat header, with dragging the seam out as
the second. Two details are load-bearing. The panel keeps its width while
folded and the inner box is laid out at *that* width throughout, so the fold
clips the list rather than reflowing every row on its way out; and the grip
sits on the seam with a negative margin cancelling its own width, because a
handle that occupied 4px of layout would shift the timeline sideways by its own
thickness. Dragging below half the minimum folds the panel instead of pinning
it at the minimum, and dragging back out resumes from zero so the seam stays
under the pointer.

The shortcut is ⌘B's neighbour and needs two things that are easy to get
wrong. `AppLayout`'s ⌘B now returns early on ⌥, or the one chord would close
both the sidebar and this panel. And the test is on `e.code`, not `e.key`:
macOS lets ⌥ rewrite the character a key produces, so ⌥B arrives as `∫` and a
`key === "b"` comparison never fires.

**A row is a row, not a label.** Every pixel of it opens the thread: the
padding and the provider mark sit *inside* the button and the flex row is left
to stretch it, the way the app's own sidebar rows are built. With the padding
on the wrapping div instead, the hit target was the title's 16px line box
inside a 28px row while the hover fill covered the whole row — so a click on a
row that had lit up under the pointer landed on the div and did nothing, most
often when switching threads quickly, which is when you stop settling on the
text. The right-hand strip is the one exception and is meant to be: the delete
control, and a spinner in its place while a turn runs.

**Deleting commits on `mousedown`, and that is the whole feature.** The trash
arms a *Yes* which takes focus on mount, so that clicking anywhere else backs
out. But WebKit does not focus a button when you press it — it clears the
focus it had — so pressing *Yes* blurred *Yes*, the blur cancelled, React
unmounted the confirm synchronously, and the click then landed on a node no
longer in the tree. Every delete was dropped, silently, and looked exactly like
a click that never arrived. So the confirm commits on `mousedown` (with Enter
and Space handled by hand, since there is no click handler left to synthesise
them into) the way the ✕ beside it already did, and `ChatPage` no longer
swallows the rejection if the command itself fails.

**Switching does not blank the timeline.** `open` in `harnessStore` hands the
rows of the thread being left to the one arriving, for the few milliseconds the
read takes, and returns early rather than re-reading the thread already open.
Arriving with no rows put the page in the state that *is* the empty composer
— no rows and nothing running — so every switch flashed the "Ask Oculus
anything" hero and re-mounted the composer under it before the rows landed.

A group folds away: the label and the caret beside it are one control, and the
right-hand slot holds the thread count until hover swaps it for the `+`. The
caret sits with the name because that is whose state it reports — beside the
`+` it read as a second button acting on the group. Open, it waits for the
pointer; folded, it stays, since then it is the only thing saying so. Which
groups are folded is kept in `localStorage`, and what is stored is the
*collapsed* keys, so a subject scoped for the first time arrives expanded. Two
edges are handled where they would otherwise mislead: a folded group shows a
spinner in place of its count while a thread inside it is running, since the
row that would say so is folded away; and its `+` expands the group before
opening the new thread, which would otherwise be started out of sight.

**And the groups are the reader's to arrange.** Recency is the default and is
right for the threads inside a group, but the *order of the subjects* is a
standing preference — the one you are in this week belongs at the top whether
or not it was the last one you typed in — so a header drags the group to
another place in the column and the arrangement is kept in `localStorage`
beside the folded set. Only the keys that have been dragged are stored: a
subject scoped for the first time is in none of them and keeps its recency
position **above** the arranged ones, because it has a conversation in it right
now and burying it under an order made before it existed would hide the thread
that created it. A drop writes every key back, so that exception lasts exactly
until the next drag.

The drag is pointer events, not HTML5 drag-and-drop — the shape the tab strip's
reorder already uses (`app/src/components/tabs/TopTabBar.tsx`) — which sidesteps
the `dataTransfer.setData` trap that WebKit cancels a drag over. What it does
*not* borrow from the tab strip is the live shuffle: a group is as tall as the
threads under it, so instead of the others sliding out of the way, a line is
drawn in the gap the group would land in. Every edge is measured once when the
lift starts and nothing changes height during the drag, so the maths stays
true. The click that ends a drag is suppressed, or finishing a move would fold
the group you had just finished moving.

**A group shows five threads and offers the rest.** A subject accumulates
dozens of conversations, and listing every one of them buries the groups under
it, so each group draws a page at a time with a *Show N more* row at its tail
that adds another five. How far a group has been expanded is kept in memory
only, not `localStorage`: a fresh window starts every group short again, and
folding a group away drops its expansion, which is also the way back to a short
list. The open thread is always drawn however far down its group it sits —
paging it out of view would leave the page with no row highlighted at all.

The column reserves its scrollbar gutter whether or not it overflows, through
`overflow-y: scroll` rather than `auto`. With classic scrollbars on, a bar that
appears only once the list is long enough takes its width out of every row, so
threads arriving and leaving jogged the whole column sideways. The modern
spelling, `scrollbar-gutter: stable`, is a **no-op in this WebKit** — measured
in a bare `WKWebView`: `CSS.supports` returns true and the property computes to
`stable`, and the content box is the same width with it as without. Always-on
overflow is the one that reserves; nothing is drawn in the gutter while the
list fits.

**The tab wears the conversation's name.** A project and a lecture tab are
titled by the thing they hold, and a chat tab is no different — except that
nothing links to a thread, so the page is what puts the name in its own route.
`ChatPage` replaces `/chat` with `/chat?n=<title>` as the open thread changes
and again when the naming turn below lands, and `tabInfo`
(`app/src/components/tabs/tabInfo.tsx`) reads the query the way it reads a
project's. Nothing open means no query, and the tab is plainly *Chat*. One
thing to know before leaning on it: `activeId` lives in `harnessStore` and
there is one of it, so two chat tabs are two views of the same open thread and
will carry the same name. Making them independent means the page owning its
thread id the way the lecture dock already does, not another query parameter.

**The name is the model's own, and it costs a turn.** Neither CLI names a
conversation over its protocol — Claude's `stream-json` carries no title
event and `app-server` sends none — so a thread is born titled with the first
line of its first message and renamed once the first exchange is done.
`Harness::name_thread` runs that naming turn *outside* the thread: its own
short-lived Claude process, or a throwaway thread on the shared Codex server.
Sending it down the thread's own session would put a question the student
never asked into the timeline and spend the thread's context on it. Which CLI
and model it costs is the `threadNaming` row of the registry below — a cheap
model by default, and no longer whichever provider the thread happens to be
on.

The claim is the guard. `store::claim_naming` hands back the exchange only if
it also wins the race to flip `title_generated` from 0 to 1, so two turns
finishing at once cannot both pay for a name, and a naming turn that fails
leaves the first-line title rather than retrying on every message. The answer
comes back as a `ThreadTitled` event on the ordinary stream — written by the
consumer thread, forwarded to the webview — and `clean_title` refuses
anything that reads like prose instead of a name, because the fallback is
better than a sentence.

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
titled "Thought" with the text behind it.

**Which part of a row gives way is the row's own answer**, and it has to be,
because the same timeline draws in a 760px column and in the lecture dock's
300px one. A row with an `em` — a path, a command — holds its verb and
truncates the `em`, which is both the long part and the part worth clipping. A
row *without* one has the long text in the title: a bundle's whole summary, an
error's first line. That title truncates instead (`RowShell` in
`app/src/components/harness/WorkRow.tsx`), and nothing is lost to the clip,
since every one of those rows opens.

The cost of getting it wrong is not the row. `overflow-y: auto` computes the
**x** axis to `auto` as well, so a row that cannot narrow does not simply
overflow its own line — the scroller it sits in gains a horizontal axis and the
entire conversation slides sideways under it, bubbles and replies included.
Both of the timeline's scrollers are pinned to one axis with an explicit
`overflow-x-hidden` for that reason (`app/src/pages/ChatPage.tsx`,
`app/src/components/lectures/LectureChatPanel.tsx`); the things that genuinely
need to scroll across — code blocks, tables — carry their own scroller and are
unaffected. It is the same computed-axis trap the one-line composers already
carry a comment about.

**A library path opens the file, in a tool row or in the answer itself.** When that title is a library path —
`courses/<subject>/…`, optionally with the `../` the agent carries because
every thread runs from `agents/` — it is a link into the side panel rather
than text to retype into the ⌘K palette (`libraryPath` / `openLibraryPath` in
`app/src/lib/openFile.ts`). It is matched on *shape*, not resolved, so a
timeline of a hundred tool rows costs no queries; the lookup
(`getFileByRelativePath`) happens on the click, and a path with no row falls
back to the system viewer, since a file that never made it through a sync is
still a file on disk. This is why a row is a div holding two controls rather
than one button wrapping another — a path is a link and the rest of the row
is a disclosure, and nesting those would be both invalid and unreachable from
a keyboard. The same matcher gates markdown links in the agent's prose
(`MdComponents`): a `courses/…` href becomes a button into the panel, because
the anchor everything else uses carries `target="_blank"` and the webview has
nowhere to take such a path but a blank new tab. Web URLs stay anchors — those
are caught in `AppLayout`'s capture phase and routed to an in-app browser tab.

**Three shapes the same path arrives in, because the agent writes what its own
tools gave it.** `HARNESS.template.md` asks for the library path and the
matcher takes it, but a model that has just read a file cites it the way it
read it: absolutely (`/…/com.tchan.oculus/courses/…`), with a `:97` line
number on the end, and pointing at the parser's `.md` rather than the document.
All three used to fall through the matcher to the anchor, and the anchor
resolved the path against the app's own origin — `http://localhost:1420/Users/…`,
a 404 in a browser tab. So `libraryPath` now also takes an absolute path,
anchored on a leading `/` so a *command* that merely contains one stays a
command, and trims a line suffix; and `openLibraryPath` looks through a `.md`
with no row of its own to the file it was parsed from (`parsedMdSource` in
`app/src/lib/fileTypes.ts`, the inverse of `parsedMdRelPath`). The link
destination is percent-encoded by the markdown parser on its way to `href`,
which is why the matcher decodes before it matches — the data directory has a
space in it. Anything left that has no URL scheme at all is a path to
somewhere this app cannot go, and it renders as the text it was rather than as
an anchor that can only 404. Command output is the one thing
outside markdown set in monospace, through `CodeText` in
`app/src/components/markdown/MdComponents.tsx`, which is where the font
lives.

**A web search is the one row whose title arrives late, and the one that used
to lie about failing.** Codex announces a search with `item/started` and an
*empty* query, then sends the queries and the results on `item/completed` —
with **no `status` field at all** (measured on 0.153.4). The bridge compared
that absent status to `"completed"` the way it does for a command, so every
search the student ever ran was drawn `failed` in red while the model was
answering perfectly well out of the results it got back. So a missing status
now reads as a search that ran, and `HarnessEvent::ToolFinished` carries an
optional `title` — the one event that may rename the row it closes — which
`store.rs` writes to `content` and `harnessStore` folds the same way, so the
row can finally say what was searched for the way Claude's `WebSearch` row
does from its first event. The verb comes off the provider's tool name rather
than the kind (`toolVerb` in `app/src/lib/harness.ts`): a search and a fetch
are both `ToolKind::Web`, and "Fetched" over a list of queries names the wrong
thing. All three agents can search: opencode's `websearch` and `webfetch` are
allowed in its ruleset for parity, since the two that carry a subscription
have the web natively and an agent that cannot look anything up is a
different agent.

### Copying out of a thread

**A selection copied or dragged out of the timeline is markdown**, not the
words as they are set. What a browser would put on the clipboard is the
*rendering*: a heading without its `#`, a table as a run of words, a formula as
KaTeX's positioned glyphs — twice over, since KaTeX also emits MathML — and a
fenced block with no fence. The Copy button under each message has never had
this problem, because it hands over the message's own source string; a drag
across half an answer has no source string, so `selectionMarkdown`
(`app/src/lib/selectionMarkdown.ts`) walks the DOM inside the selection and
writes markdown back out of it. `Timeline`'s root takes the `copy` and
`dragstart` events and puts the result on `text/plain`.

Three shapes are handled by name rather than by walking them. A `.katex`
subtree is read for the `<annotation>` holding its TeX and never descended
into, which is what keeps one formula from arriving as two unreadable ones. A
mermaid diagram is an SVG whose labels in document order are not a diagram, so
the figure carries its own fence source in `data-md`
(`app/src/components/markdown/Mermaid.tsx`). And the furniture is *marked*, not
guessed: a message's action row and a "Show more" toggle carry
`data-copy-skip`, so a selection dragged across several messages does not come
out with a clock time between them.

Two details are load-bearing. Whitespace is collapsed the way the browser draws
it **except** where the box says `white-space: pre-wrap`, which is the question
bubble — its line breaks are the student's own. And the drag handler sets data
without `preventDefault`, because on `dragstart` that cancels the drag outright
(root `CLAUDE.md`); only the copy handler prevents the default, or the browser
writes its own flavours back over ours.

Plain text is deliberately not offered yet: the choice belongs in a right-click
menu, which this app does not have.

### Maths in a reply

Coursework here is formulas, so the appended prompt asks for LaTeX — `$…$`
inline and `$$…$$` displayed — and `Timeline` renders it with remark-math and
rehype-katex. Two things make that hold up:

- **The prompt has to ask.** Without a rule for it a model writes formulas as
  Unicode — `cos(θ_B/2)|0⟩` — which is not maths, just characters that look
  like it, and no renderer can recover it. `HARNESS.template.md` names the two
  delimiters and rules out the Unicode; `reading.rs`'s prompt already said the
  same thing, and the chat brief was the one that did not.
- **`\(…\)` and `\[…\]` never reach remark-math**, which is the form a model
  reaches for by default. CommonMark treats a backslash before ASCII
  punctuation as an escape, so `\(x\)` is parsed to a literal `(x)` before the
  maths plugin looks for a delimiter — the formula comes out as prose with
  stray brackets, indistinguishable from the model not having used maths at
  all. `normalizeMath` in `MdComponents.tsx` rewrites the pair into dollars
  first, stepping over code fences and spans. The prompt is the fix; this is
  the net under it.

KaTeX's own stylesheet is imported by `MdComponents.tsx` rather than by a
viewer, so it arrives with the components every renderer already shares.
`.md-compact .katex` in `app/src/index.css` brings 1.21em down to something
that sits in 13px text and gives a display formula its own horizontal scroller
— without one, a long derivation has no width to shrink to and widens the
player's dock. The class is not chat's: `CompactMd` in `MdComponents.tsx` is
what wraps a renderer in it, and the reading copy's lines in the player's dock
go through the same components.

### Diagrams in a reply

A ```mermaid fence in a reply is **drawn**. The renderer is shared with every
other markdown surface — `app/src/components/markdown/Mermaid.tsx`, reached
from `MD_COMPONENTS.pre`, and [frontend.md](./frontend.md) has the non-obvious
things about how it draws. Two of them are chat's, though:

- **The prompt has to ask, the same way it does for maths.** A model does not
  volunteer a diagram into a chat it has no reason to think can show one, so
  the Diagrams section of `HARNESS.template.md` says the fence renders, when
  a shape beats a sentence, and which diagram types are worth trusting. It
  says to keep LaTeX out of node labels, where KaTeX never runs, and it asks
  for a diagram that can be taken in at a glance — not because a big one
  cannot be read (the expand control in its corner opens it full-window,
  zoomable and pannable) but because a reply that needs a lightbox to be
  understood has moved the work onto the reader.
- **A fence is seen while it is still being written.** The reply streams, so
  the component is handed `flowchart TD` before the arrows exist and a node
  before its closing bracket. It settles for 150ms, asks `mermaid.parse`
  before rendering, and shows the fence as a code block until one succeeds —
  so a half-written diagram reads as source becoming a picture, and a diagram
  that never parses simply stays the source it was.

**Every message has a row of actions under it**, and it is under rather than
beside for one reason: a control floating next to a bubble has to be placed
against text of unknown width, while a row below it is just a row. The row's
height is always taken and only its contents fade in on hover, so a thread at
rest is still only what was said, and the pointer crossing a message never
pushes the rest of the thread down a line.

- A **question** carries when it was asked, then Copy, Edit and Rewind.
- An **answer** carries Copy and Retry — Retry asks the question above it
  again, unchanged, which is the same move as an edit that changed nothing
  ([going back](#going-back)).
- A **queued** message says `Queued` where the time goes, and offers Edit and
  Remove; it is the same bubble, dashed ([one turn at a
  time](#one-turn-at-a-time)).

Copy answers for itself by turning into a tick for a moment: this app has no
toasts, and a copy button that does nothing visible is indistinguishable from
one that failed. It goes through `copyText` in `app/src/lib/utils.ts`, which
keeps the old `execCommand` path as a fallback — `navigator.clipboard` needs
a secure context, which the dev server is and the packaged app's custom
scheme is not always.

A question long enough to bury the answer under it **folds**: past about ten
lines the bubble clips under a fade with *Show more* inside it, so a thread of
pasted briefs still reads as a list of exchanges. Whether one is long enough is
measured from the rendered node rather than counted from the text — how many
lines a paste becomes is the column's decision, and the column changes width
with the panel (`useOverflows` in `app/src/components/harness/Timeline.tsx`).

Editing a question happens in the bubble rather than back in the composer —
the thread is where the question is — and the box is that bubble grown to the
column's width with its Cancel and Send inside it.

Whether the editing actions are offered at all depends on whether the thread
is busy, which changes twice a turn — so the two message rows read that from
the store themselves rather than taking it as a prop, which would re-render
every committed row, markdown and all, at both ends of every turn.

**The questions are the index.** A rail of ticks sits in the left gutter, one
per question asked (`app/src/components/harness/ThreadMap.tsx`); clicking a
tick scrolls to that question and hovering names it. *Every* question on
screen is drawn longer and in the accent, not just one — a viewport holding
three of them and marking one says the other two are somewhere else. They are
in order, so what is in frame is a contiguous range; when none is on screen —
a long reply filling the view — the question that reply answers is lit
instead, so the rail always says where you are. The ticks are *evenly spaced in one block*,
not placed proportionally to where their message sits in the scroll — placed
that way they spread over the whole viewport and a thread read as two
far-apart clusters of dashes rather than as one index. Even spacing gives up
"how far apart", which the scrollbar already says, and keeps the count and the
position, which is what the rail is for. It reads geometry and never the
stream: where each question starts is measured off the DOM (the user bubble
carries a `data-msg-id`) into a ref, so a turn growing the thread twenty times
a second re-renders nothing here. The rail hides itself when the window is too
narrow to have a gutter beside the 760px column.

**Nothing on screen re-renders per token.** The stream is the load here — a
provider sends tens of deltas a second and the thread they land in is the
most expensive thing the app draws, so the page is arranged so that the two
meet as rarely as possible. Three parts, measured over a real 38-row thread
in the preview harness:

- **Deltas are buffered, not applied.** `harnessStore` holds assistant text,
  reasoning and tool output in a module-level buffer and folds it into `live`
  on a 48ms timer, so the page renders ~20 times a second however fast the
  CLI talks. Every other event — the ones that commit a row — flushes the
  buffer first, so text can never arrive after the row it belongs to.
- **Committed rows are memoised, and the turn subscribes where it is drawn.**
  A row Rust wrote will not change again, so `Timeline` memoises all of them
  and `ChatPage` subscribes slice by slice rather than to the store whole.
  The two things that do move mid-turn read the store themselves: `LiveTail`
  (streaming text, reasoning, "Working…") and the one tool row whose output
  is still arriving. Parsed tool `meta` is cached per row object in
  `parseToolMeta` — a single thread can carry 300KB of command output — and
  the KaTeX plugins are only loaded over text that has a maths delimiter
  (`MATH` in `app/src/components/markdown/MdComponents.tsx`). Mermaid is the
  same bet one step further out: ~1MB of parsers and layout engines behind a
  dynamic `import()`, so a session that never opens a diagram never fetches
  it, and one that opens four fetches once.
- **Following the stream watches for growth.** A `scrollTo` per delta forced a
  layout of the whole thread and yanked the page down whenever the reader
  had scrolled up; a `ResizeObserver` on the column scrolls only while the
  bottom is already where they are. It watches the scroller as well as the
  column, because a composer that wraps onto another line takes that height
  off the box above it without changing anything about the content — the last
  rows go behind the composer and no scroll event ever fires.

Together: 400 deltas over that thread went from 402 React commits and 6.2s of
render (15ms each — a dropped frame per token) to 34 commits and 55ms, and
opening the thread from 53ms of render to 39ms.

**Rows are keyed by thread.** `items` in `app/src/stores/harnessStore.ts` is a
map of thread id to rows, not one timeline: two views — the Chat page and the
lecture player's dock — can hold different threads at the same time, and a
single "what is on screen" would have dropped the rows of whichever one was not
active. `live`, `queued` and `contextDrift` were already per thread and the
event fold already routed by thread id, so only the push changed: it asks
whether a thread's rows are held rather than whether they are the open ones.
The map keeps every thread opened this session — which is what makes coming
back to one instant — and only deleting a thread or a view calling `release`
shrinks it, since a single thread can carry 300KB of tool output.

The composer is bb's: Enter sends, Shift+Enter breaks a line. A thread keeps
its provider; the model can change per send, which on Claude means the next
process. While a turn runs the box still takes a message — it is queued, not
sent — so the control row carries two buttons and not one: stop is always
reachable, and send appears beside it as soon as there is something to queue.

**Usage is a wheel, not a footer.** The line under the box that spelled out
context, spend and every rate-limit window is now
`app/src/components/harness/UsageMeter.tsx` — a 16px ring in the control row
beside the send button, opening a popover of bars: this thread's context,
then the account's windows (Claude's 5-hour and weekly; Codex's primary and
secondary) with when each one resets. Three decisions are load-bearing. The
ring is context and nothing else: it is the number that moves every turn, it
is thread-local like the composer it sits in, and a ring that silently
switched to whichever number was worst could not be read at that size. Spend
is gone — these are subscription CLIs, so the dollars the provider reports
price tokens nobody is billed for, and a number that is never charged is
noise beside two that bind. And the control row is one height throughout
(24px, the picker's): a send button larger than the picker beside it reads as
the box's subject rather than its verb.

## One turn at a time

A message typed while the agent is working does not reach the CLI. It waits
in `Queue` (`app/src-tauri/src/harness/mod.rs`), and Rust sends it the moment
the running turn closes. This is not caution — it is the only way the thread
reads in order, and each CLI was measured getting it wrong in its own way:

- **Claude queues it itself** and starts a second turn the instant the first
  `result` is out. The rows are fine, but the *turn* boundary is not: the
  composer went idle between them, the spinner stopped, and stop had nothing
  to stop for as long as the gap lasted.
- **Codex folds it into the running turn.** A second `turn/start` comes back
  with the *same* turn id — no error, no new turn — so the question lands in
  the timeline above the answer to the previous one, there is one
  `turn/completed` for both, and the reply to the first question can look
  like it never came. This is what "the message just disappears" was.
- **opencode was the one that would take it correctly** — the v2 prompt
  endpoint offered `delivery: "queue"`, which holds a message until the turn
  ends — and it was never used, nor is there an equivalent on the v1
  `prompt_async` the bridge runs on now. The harness owns the `Queued`/`Unqueued` rows and
  the one-turn-at-a-time rule; letting the server own the same invariant for
  one provider out of three would put it in two places, and the half the
  webview draws would be the half that could not see the queue.

So the queue is the manager's. A pending message is **not part of the
conversation**: no row is written for it, it is held in memory rather than
the database, and that is exactly why it can still be rewritten or dropped
before it goes out. The webview draws it as a dashed bubble at the end of the
thread (`Pending` in `app/src/components/harness/Timeline.tsx`), folded from
the `Queued`/`Unqueued`
events — an edit arrives as `Queued` again under the same id, so one variant
covers "new" and "changed". `harness_queued` is how a reloaded page finds out
what Rust is still holding.

Two things had to be true for the queue not to strand a message. A thread is
released for its next turn by `TurnFinished` and nothing else, so both bridges
now promise exactly one per message they accept: Claude keeps an `expecting`
flag from a message going in until the `result` that closes it, so a process
that dies in between — a model name the CLI rejects kills it before the first
stream event — still closes the turn; and Codex takes the turn id from the
`turn/start` response rather than waiting for the `turn/started` notification,
which arrives a second later, after its MCP servers and hooks have warmed. In
that second an interrupt had no turn to name and a dead server had no turn to
fail.

## Stopping a turn

Stop means nothing more goes out. The running turn is interrupted *and*
whatever was queued behind it is dropped — and handed back to the composer,
since the student typed those words and never saw them sent
(`harness_interrupt` returns them; `restore` in
`app/src/stores/harnessStore.ts` carries them to the box).

What the turn leaves behind took a recording of each CLI to get right; both
are in `fixtures/harness/`, and the two tests replay them.

- **The half-written answer is kept.** Claude sends it as an ordinary
  `assistant` line before the `result`, so it was already a row. Codex sends
  nothing: the deltas simply stop and the `item/completed` that carries the
  whole text never comes — so the bridge accumulates the streamed text and
  commits it itself when `turn/completed` says `interrupted`. Without that,
  stopping a turn wiped everything the agent had said off the screen, because
  live text has no row behind it.
- **A stopped turn is not an error.** Claude's closing `result` calls itself
  one: `is_error: true`, `subtype: "error_during_execution"`, `stop_reason:
  null`, and an `errors` array holding the CLI's own diagnostic —
  `[ede_diagnostic] result_type=user last_content_type=n/a stop_reason=null`.
  That string was reaching the timeline as a red row every time stop was
  pressed. `terminal_reason: "aborted_streaming"` is the only field that says
  what really happened, so that and the flag the bridge sets when it asks are
  what decide; the diagnostic is dropped and the turn closes `interrupted`.
  The same line reports zeros for every token and cost, which used to blank
  the thread's usage, so it is skipped too.
- **The stop is a row.** `TurnFinished { interrupted }` writes an
  `interrupted` item — the one row kind with nothing in it — and the timeline
  draws it as a quiet centred line, *You stopped the response*. The answer
  above it breaks off mid-sentence on purpose; without the line the thread
  reads as an agent that gave up, and it has to survive a reload for the same
  reason.

## Going back

Three actions share one move — the thread is truncated at a row, and that row
and everything after it stop being rows (`store::truncate_from`, announced as
a `Rewound` event so a second window is not left showing rows that are gone).
What differs is what happens next:

| | What it does | Command |
| --- | --- | --- |
| **Edit** | Truncate, then send the new text as the next turn | `harness_edit_resend` |
| **Retry** | Truncate at the question above the answer, then send it again unchanged | `harness_edit_resend` |
| **Rewind** | Truncate, and hand the question's words back to the composer. Nothing is sent | `harness_rewind` |

Rewind is Claude Code's, **without the branching**: there is one thread, so
going back means the rest is gone rather than parked on a side branch that
something then has to draw, name and let you switch between. Its whole point
is that it does not send — it is for picking the conversation up yourself,
which is why the words land in the composer, through the same `restore` path
stop uses, instead of going straight to the agent.

**The agent goes back with the thread.** Both CLIs can be told to forget a
turn, on the same control channel the stop button uses, and both were asked in
a round-trip before this was built:

| | How | What it names |
| --- | --- | --- |
| Claude | `control_request` / `rewind_conversation` | the uuid of the user message |
| Codex | `thread/revert` | the id of the turn to revert before |
| opencode | `POST /session/{id}/revert` | the id of the message to **drop**, and everything after it |

None of them will say that identifier twice, so it is learned once — when the
turn goes out — and kept on the question's row as `anchor` (migration 28), arriving
as a `TurnAnchor` event like everything else the bridges learn. Codex says its
turn id in the `turn/start` reply. opencode says nothing synchronously —
`prompt_async` answers 204 with no body — so its `msg_…` is read off the event
stream instead, as the first user `message.updated` of the turn. Its revert is
inclusive and lazy: one call naming the question's own id drops that message
*and* everything after it, so the first message of a session can be rewound
like any other, and nothing is actually deleted until the next prompt, when
`message.removed` events say which rows went. Claude says nothing: its `stream-json`
output never echoes the message we sent, so the uuid is read out of the CLI's
own transcript at `~/.claude/projects/<slug>/<session>.jsonl`. That file is a
tree rather than a list — every row names its `parentUuid` — so the question
is found by walking up from the turn's first answer, past the attachments the
CLI threads in, never stopping on a `user` row that is a tool result
(`anchor_for`). The folder is not announced either: `memory_paths` on the
`init` line would give it away but is null whenever auto-memory is off, which
is how this bridge runs it, so the slug is rebuilt the way the CLI builds it —
every character of the working directory that is not a letter or a digit
becomes `-`, measured against real folders rather than assumed.

The provider is rewound *before* the rows go, and a thread whose process has
exited is resumed for it without a turn: the control channel is live as soon
as the session is, so nothing is spent on the model.

**When it cannot reach the agent.** A question asked before migration 28 has
no anchor to name, and a session the CLI has since dropped cannot be resumed.
The rows still go — refusing to edit would be worse — but `Rewound` carries
`context: false` and the timeline says *the agent still remembers what was
removed here*, so the mismatch is read there rather than discovered later in
an answer that refers to an exchange no longer on screen. Neither provider
puts files back: Claude splits that into a separate `rewind_files`, and
Codex's schema says outright that reverting them is the client's job.

All three are offered only while the thread is idle: rewriting rows under a
running turn would delete ones it is still writing.

## The model picker

One control does agent, model and reasoning level —
`app/src/components/harness/ModelPicker.tsx`, ported from bb's
`ModelReasoningPicker`. The trigger reads `[mark] Model Name Level ⌄`; the
menu is a strip of provider marks (underlined when active), the models of the
active provider, and a row of levels under a rule. **Nothing inside the menu
closes it** — agent, model and level are one decision made in three clicks,
and a menu that shut on the first of them sent you back to the trigger to
finish the thought; it closes on an outside click, Escape, or the trigger.
The marks are the `currentColor` SVGs in
`app/src/components/harness/ProviderMark.tsx`, monochrome like bb's so they
sit in the palette rather than fighting the indigo. opencode's is the one
two-tone mark, and it is still one colour: the vendor's light and dark files
differ only in being composed against their own backgrounds, so the inner
block is drawn as the same ink at a fraction of its alpha and the theme takes
care of itself. They are held in a
`Record<Provider, …>` rather than picked by a ternary, so a provider added to
the union is a compile error until its mark exists — the ternary that used to
be there answered "not Claude" with the Codex mark.

Every picker in the app assembles its list through one hook,
`app/src/hooks/useProviderModels.ts`, which names no provider: which
catalogues are compiled in and which are fetched from a CLI is a property of
the `PROVIDERS` entry in `app/src/lib/harness.ts`. That file is the one place
a provider is declared.

**The menu knows whether the agent is on the machine.** Discovery's answer used
to reach only Settings → AI, so the composer offered Claude's compiled-in
catalogue to a student with no Claude Code installed and the truth arrived as
an error row after sending; Codex and opencode, whose lists come from their
CLIs, failed the fetch and drew "No models available" — the same sentence an
installed opencode with no credentials gets. So health is shared app-wide
through `app/src/hooks/useBridgeHealth.ts` — a module-level cache and one
in-flight promise rather than a store, since it is one value that arrives once
and is re-asked only when Settings rechecks — and rides `useProviderModels`
into the picker as a field on each provider. Three states, because they are
three different facts: *unknown* until the check lands, where a provider is
drawn exactly as it was before health existed rather than flashing a wrong
answer; *missing*, which replaces the catalogue with the CLI's name and a
button into Settings → AI (through `navigateActive` in
`app/src/lib/tabRouters.ts`, since a popover has no router of its own) and dims
that agent's mark in the strip without removing it — seeing that Codex exists
and is absent is the point, and the mark stays clickable because the panel
behind it is the only thing that says why; and *installed*, where an empty list
is still "No models available". The gate is the `health` field and not a test
on an id, so Claude's static catalogue is withheld on exactly the same terms as
the two fetched ones, and a fourth agent costs nothing here.

Five things about it are deliberate:

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
  five (`low`…`max`); Codex declares a subset per model; opencode calls them
  *variants*, and the later versions that fill the field in declare a handful
  per model (`high`/`low`/`max` on most, plus `medium`/`xhigh` on a few) —
  which is what 1.18.2 declared for nothing and the row was built to light up
  for. The picker reads them off the selected row, so a model that only
  reasons a little never offers a level it would reject, and a model that
  declares none simply draws no level row rather than being offered an
  invented one.
- **The levels are sorted here, because no provider hands them over ranked.**
  opencode spells `variants` as an object keyed by level id, and a JSON map
  has no order worth keeping, so the row arrived alphabetically — High · Low ·
  Max, which reads like a ranking and is not one. `sortReasoning` in
  `app/src/lib/harness.ts` puts every catalogue's levels in the one canonical
  order, weakest first, taken from the key order of `REASONING_LABELS` so
  there is one table rather than two that can drift; an id this build has no
  name for keeps its place at the end. The *default* level is a separate
  question with the same root — `default_variant` in
  `app/src-tauri/src/harness/opencode.rs` asks for `high` by name and steps
  down from there, rather than taking whichever key sorted first.
- **opencode's list is a filtered one, and the filter is the provider's own
  business.** Its catalogue is the only one that advertises models which
  refuse every request, and the only one a single provider can add three
  hundred rows to, so what reaches the menu is what
  [the student](#which-models-reach-the-picker) has left ticked.
  The gate lives inside opencode's `PROVIDERS` entry — `fetchModels` runs the
  CLI's answer through `filterOffered` — so `useProviderModels` and
  `ModelPicker` still name no provider, which is the rule that file states
  about itself. An empty list is then not the same sentence for all three:
  a provider may carry an `emptyNote`, and opencode's sends the student to
  Settings → AI rather than leaving a menu that reads as broken.

Both CLIs bind the level when a session starts, not per turn — so changing it
mid-thread respawns the Claude process and restarts the Codex thread, which
`Harness::send` decides by comparing the level a live session was started with
against the one being asked for.

## Signing in to a provider

Claude Code and Codex are signed in with their own CLIs and carry the
student's subscription. opencode carries whatever `opencode auth` holds — and
until this stage the only way to put something there was a terminal, so the
picker above showed two providers out of two hundred and eighteen with nothing
on screen saying why. Settings → AI now lists them, says which are connected,
and connects and disconnects them
(`app/src/components/settings/OpencodeProvidersSection.tsx`).

**It goes through opencode's server, not around it.** `opencode serve` — the
same one the bridge already talks to — exposes `GET /provider` (the catalogue,
plus a `connected` list), `GET /provider/auth` (how each provider can be signed
in to), `PUT`/`DELETE /auth/{id}` (the credential itself) and the OAuth pair
`POST /provider/{id}/oauth/authorize` / `…/callback`. Nothing writes
`auth.json`, nothing drives the TUI and nothing shells out. The credential
lands in opencode's own store and is therefore shared with the opencode the
student runs in a terminal, which the section says out loud in a line under its
title. This also leaves the env strip above untouched: writing through
`PUT /auth` *is* the store the strip exists to make authoritative.

**The dialog is generic because the form is data.** A method arrives as a
spec — `oauth` or `api`, a label, and prompts that are text fields or selects,
each optionally conditional on another answer — so one component covers
`openai`'s three ways in, `github-copilot`'s enterprise branch, and the
providers opencode adds after this was written
(`app/src/components/settings/OpencodeConnectDialog.tsx`). Only ten providers
declare a method at all; the other two hundred and eight take a plain API key,
which is measured rather than assumed — a `{type:"api",key}` written to one of
them is accepted and the provider comes back connected — so "declares nothing"
renders the key form rather than a dead end. The `when` rule is evaluated
twice on purpose: in the dialog to decide what is drawn, and again in Rust
against the spec the server declares *now*, so an enterprise URL typed and then
abandoned by switching the select back cannot ride along.

**`auto` and `code` are the two OAuth shapes, and both open the system
browser** (`tauri-plugin-opener` — a student is far likelier to be signed in to
GitHub or OpenAI there than in the app's in-app browser). `auto` means opencode
finishes the flow itself: the redirect lands on a loopback listener *inside the
app's own server*, or it polls a device code, and the app only learns of it by
looking. `code` means the student pastes something back and the app posts it to
`…/oauth/callback`. Both were seen in 1.18.2 — OpenAI's browser flow and its
device flow are both `auto`.

**`connected` is not read off the file, which is the trap here.** A credential
written through `PUT /auth` answers `true` and the file on disk grows it, and
`GET /provider` keeps reporting the old list for the life of the instance.
`POST /instance/dispose` is what makes the instance re-read its store.

**The model list comes from `/config/providers`, and `/api/model` is a trap.**
The two read like the same question and are not: `/api/model` answers with the
models of the providers the running *instance* has instantiated, which bears no
relation to what the student signed in to. Measured on 1.18.2 with OpenRouter
connected — `/provider` called it connected with 369 models, `/api/provider`
listed two providers, and `/api/model` returned 33 models with **not one** of
OpenRouter's among them. It is not staleness: a server process spawned hours
*after* the credential was written answered identically, and a
`POST /instance/dispose` changed nothing, so neither refreshing nor restarting
reaches it. `/config/providers` is what `opencode models` itself prints — 378
on that machine, and the three providers the config can use rather than all
221. `parse_models` in `app/src-tauri/src/harness/opencode.rs` reads it, and a
test pins the shape, which the endpoint it replaced never had.

The symptom was a single line: a provider the student had just connected
offered no models at all, because the endpoint that was read did not list any
of them.

Disposing was measured to be survivable: the process stays up, the event stream
keeps heart-beating, sessions created before it are still readable by id, and
an OAuth loopback listener opened before it is still bound after. What it
releases is "all resources", so it is skipped while a turn is open and the
answer says so rather than showing a list it knows is behind. All of it names
the session directory — `?directory=`, the same scoping the session endpoints
and the event stream use — because the server's own cwd is wherever the app
was launched from, so an unscoped call would read and refresh a *different*
instance from the one the model list and every session belong to. `PUT`/`DELETE /auth/{id}` is the exception and takes
none: a credential belongs to the machine.

**Opening Settings does not start opencode.** The read would spawn
`opencode serve`, and *a visit is not a reason to spawn a CLI* — the same rule
the rate-limit read follows. So the section opens on a button and the click is
what starts it, after which the answer is kept for the window's life. Whether
opencode is installed at all is the free question `useBridgeHealth` already
answers, so a machine without it says so instead of failing a call.

**The list leads with what is connected.** Two hundred and eighteen rows is not
a list, so what is signed in comes first — it is what the picker will show and
the only thing there is to remove — then the handful that offer a real sign-in
flow, and the rest by name through a search box. A provider whose `source` is
`config` is connected because an `opencode.json` declares it, not because a
credential exists, so its row says where it came from instead of offering a
Disconnect that would do nothing — **Hide from Oculus** is what it offers
instead, because the alternative would be this app editing the student's own
`opencode.json`, which it has no business doing.

A key is never held. It travels from the field to `PUT /auth/{id}` and nowhere
else: not into `settings`, not into the keychain, and not into
`agents/threads/*.ndjson`, which only ever receives SSE payloads. Two paths
back out are closed rather than assumed shut. An error body quoting the request
is redacted of the secret the bridge is holding at that moment, on top of the
`scrub` that already guards `/api/model`'s key-bearing rows. And **`/provider`
itself echoes the credential** — a connected provider's row carries the real key
in a field `scrub` does not know the name of, measured — so the merge into the
rows Settings draws never reads that field, and a test asserts the serialized
rows cannot carry it.

## Which models reach the picker

Signing a provider in is not the same as having models worth offering, and
opencode is the one agent where those two facts come apart. Two hundred and
eighteen providers reach this app; some advertise models whose gateway refuses
everything, and one of them — OpenRouter — is three hundred rows on its own.

**There used to be a probe, and it is deleted.** An opencode model was offered
only once a real turn had been sent to it and had come back: `probe_model`
opened a session on the hidden `oculus-namer` agent and sent one sentence, four
models at a time, and a sweep ran on connect *and* whenever the provider
manager was opened with a provider it had no verdicts for. It answered the
question correctly and it was the wrong trade, because the evidence was bought
from the provider:

- Every probe was a **billed request**, and a sweep was as long as the
  catalogue — ~300 for OpenRouter alone, on a settings page, without a click.
- It was **two** billed requests per model, not one. The probe created a
  session per model, and opencode titles a new session on its own small model,
  so each probe carried a second request nothing here asked for.
- Tools were attached to the probe's turn, so a request the code described as
  "a handful of tokens" was measured at **~8.6K input tokens** against some
  providers.

Nothing in this repo may spend a student's credits to populate a menu. The
sweep, its progress events, its cancel flag and its stored verdicts are gone.

**Most of what it bought was already free**, and that part is kept:

- *Is the provider signed in?* `list_models` reads `GET /config/providers`,
  which lists the providers the config can actually **use** — three of 221 on
  the machine this was measured on. A provider with no credential contributes
  no models to the list at all, so the question never had to be asked.
- *Does the model exist?* Same list, same answer. The probe's `Model not
  found` class cannot arise from a menu built out of the list itself.
- *Can the model call a tool?* `capabilities.toolcall` on the row. This one
  matters more than it looks: the agent answers out of the student's own
  library, so a model that cannot call a tool does not fail loudly — it answers
  from nothing, confidently, which is worse than a red row. **69 of
  OpenRouter's 369** are in that state, and none of opencode's or the Spark
  endpoint's.
- *Is it one of opencode's free **Zen** models?* A static fact about the id.
  Their gateway answers `HTTP 400 … MissingSessionID … "OpenCode's free tier
  can only be used in OpenCode"` to anything that is not the opencode TUI, so
  every model under provider id `opencode` refuses every request from this
  bridge, always. `isZen` reads that off the id for nothing; the probe used to
  buy the same fact one model at a time.

What no free check can see is a credential that is present but **stale** — a
401. That one fails once, in the timeline, in the provider's own words, which
`friendly` already turns into a sentence. One failed message is the honest
price; ~600 billed requests was not.

**So three rules are left, and none of them costs anything.** An opencode
model is offered when it can run here at all, it is not a Zen model, its
provider is not hidden, and the student has not hidden it.

The first of those is `unusableReason`, and it is the half that needs the model
*row* rather than its id — so `filterOffered` is where the capability half and
the catalogue half meet, and it is the only path the composer's menu takes.
Settings counts through the same call rather than a local test, so a provider
row and the picker cannot come to different numbers. Its flags are read in
`parse_models` and **absent means capable**: a provider whose rows say nothing
about tools must not have its whole catalogue gated out, and a claim that turns
out to be wrong costs one failed turn where a missing claim read as a refusal
would cost the provider. The text-in and text-out flags are assertions rather
than filters — every model in the catalogue today passes both — kept because
OpenRouter's own catalogue already carries transcription, embedding, rerank and
image models, and the day opencode surfaces one it should not reach a chat
composer.

The store for the other two is one JSON value in `settings` under
`opencode_catalogue` (`app/src/lib/opencodeCatalogue.ts`), `isOffered` is the
single place the rule is written, and **an absent entry means offered** — the
reverse of what it meant while the probe existed, when "no entry" meant "not
checked yet". The catalogue now records only what was switched off, so a
provider nobody has touched has no rows at all. It still reads catalogues the
probe build wrote, keeping `hidden` and discarding the verdicts: a purchased
`ok: false` must not keep a model out of a picker that no longer re-checks
anything.

Blocked models stay **in** the Settings list rather than being filtered out of
it, drawn with the reason and an unticked, disabled checkbox — `blockedBecause`
in `app/src/components/settings/OpencodeCatalogDialog.tsx` writes the one
sentence each, so the list and its counts cannot disagree about which rows are
on offer. A student who signed in to opencode's free tier, or who goes looking
for a coder model that turns out to have no tool calling, is owed the sentence
rather than an absence.

**What no free check can see is a model gated behind the provider's own
verification** — Meta's Muse Spark family, for one. Nothing marks it: opencode
lists it `status: "active"` with full capabilities and ordinary pricing, and
OpenRouter's public `/api/v1/models` has no verification field either
(`top_provider.is_moderated` is content moderation, not an age gate). Verified
2026-09-18 against both catalogues. The probe caught those empirically, at the
price described above; they now fail once in the timeline like a stale key.

**The surface is a dialog, and Settings → AI keeps one line.** The provider
list, the 218-row search, connect and disconnect, and a per-model list with its
search and its ticks are more than a settings page should hold beside four
other sections — so they live in `OpencodeCatalogDialog.tsx` and the page shows
a button plus what the *stored catalogue* knows. That summary is now what has
been **hidden** ("2 models hidden"), not what is offered: counting offered
models would mean knowing how many exist, which means starting a CLI, and
opening Settings must not do that. The click on *Manage providers* is what
spawns `opencode serve`, which is the rule the rate-limit read already keeps.

**Inside the dialog the query is the mode, and there is one scroll region.**
The obvious layout was built first and does not work: connected list, hidden
list, then a search with its results under it, all sharing one column. Every
group competes with every other for the same height and the results always
lose — a student with five providers signed in had about two rows of room to
scroll two hundred in, and pinning the top half only moved the squeeze. So the
search field is the only pinned thing; empty, the body is the student's own
providers, and typing gives the body over to the matches. Looking for one of
218 providers is a find, not a browse, and nothing in the connected list helps
with it. Row actions follow: *Models* keeps a label because it is the way
further in, while Disconnect, Hide and Show are icons with tooltips — three
words of chrome per row made the provider's own name the least emphatic thing
in it. *Check* was the fourth and is gone with the sweep.

**Every model row is tickable except a blocked one.** A tick is a promise that
the composer will offer the model, and the gate will not offer a Zen model or
one that cannot call a tool — so an enabled checkbox there would be a control
that appears to work and changes nothing. That was the rule for a failed probe
too, and it survives only for the refusals that are knowable without asking
anybody. Everywhere else a tick is the whole fact: an untick removes the row
rather than storing `hidden: false`, and disconnecting a provider forgets
everything stored under it.

## Signing in to an agent

Installing a CLI and being able to use it are different facts, and until this
stage only the first one was on screen anywhere. An expired OAuth session
surfaced as a red timeline row reading *"Failed to authenticate: OAuth session
expired and could not be refreshed"* — true, unactionable, and identical in
shape to a row about a syntax error in a file. The student's next move was to
find a terminal, remember which of three CLIs the thread was on, and guess the
subcommand. `app/src-tauri/src/harness/signin.rs` is the way through, and it
does two separable things.

**Reading a failure, and drawing it as a state rather than a crash.**
`signin::is_auth_failure` takes a provider's own error text and says whether it
is that provider saying it has no usable credentials. `HarnessEvent::error_for`
is what calls it — the bridges use it wherever the error is something the
*provider* reported, and the plain `HarnessEvent::error` stays for the manager
failing to start a bridge, which is nobody's credential. `Error` therefore
carries `auth: Option<Provider>`, `store.rs` puts the same fact in the error
row's otherwise-unused `meta` (`{"auth":"claude"}`, so no migration), and
`harnessStore.ts` folds it into the live row too — without that second write
the card a student needs *right now* would only appear after a reload. The
timeline's `ErrorRow` branches on it into a sign-in card
(`app/src/components/harness/WorkRow.tsx`).

**The lists are kept tight on purpose, and that is the design decision here.**
A false positive is worse than a miss: it sends a student to re-authenticate
over a failure that had nothing to do with their account, and the sign-in they
then do will not fix it. So every entry is a whole clause rather than a word —
`"auth"` alone would match a compile error about a file named `auth.rs`,
`"expired"` alone would match a cached download — and `401` never counts on its
own, since a byte count, a line number and a path can all carry those three
digits.

**The two live flows are genuinely different shapes**, measured rather than
assumed, which is why `ProviderInfo.signIn` in `app/src/lib/harness.ts` is
`"code" | "callback" | null` and not a boolean:

- `claude auth login` prints an authorize URL and then **blocks reading a
  pasted code off stdin**. Its last prompt carries no trailing newline, so a
  `lines()` loop never emits it — nothing waits for one. The dialog offers its
  paste field as soon as the URL arrives and `signin::submit_code` writes the
  answer to the child's stdin. This is also the one place the module departs
  from `install.rs`, which nulls stdin precisely so anything that asks a
  question fails instead of hanging; here the question is the point.
- `codex login` starts its **own loopback listener** on port 1455, prints the
  URL, and finishes by itself when the browser redirect comes back. It never
  reads stdin, so the dialog only waits.
- opencode is deliberately absent. Its credentials are per *provider*, not per
  CLI, and [signing in to a provider](#signing-in-to-a-provider) already owns
  that whole surface through the server the bridge is already talking to.
  Driving `opencode auth login` as a subprocess would be a second, worse door
  to the same store — it is a TUI with arrow-key menus, and it could only write
  what the existing path already writes. So `status` answers `signedIn: null`,
  which is *not answerable from here* and a different fact from "no", and
  `start` returns an error naming the dialog that is the real answer.

**The URL opens in the system browser, and still rides the event.** Rust calls
`tauri_plugin_opener::open_url` on the first line carrying one, because the
student is probably already signed in to claude.com or chatgpt.com there, where
Oculus' own in-app browser (`app/src-tauri/src/browser.rs`) has a cookie jar
seeded for Canvas and nothing else. The URL is shown in the dialog with a Copy
button anyway: an `open` that silently failed must not be a dead end with a
spinner on it.

**Nothing here is cached, which is the difference from `discover::health`.**
Health is asked by every model picker, three spawns a menu, so Rust memoises it
for the life of the process. Sign-in state is asked in three places — a
settings row, the composer's line above the box, and an error row a student is
looking at *because something just failed* — and a cached "signed out" that
outlived the sign-in that fixed it would be the one wrong answer that matters.
So `signin::status` spawns the CLI every time, and `useSignInStatus.ts` holds
the only cache there is, dropped by Settings' *Recheck* and by the end of a
run. The binary lookup underneath is still cached: where a CLI *is* does not
change when its credentials do.

**Three states, and `unknown` draws nothing** — the same discipline
`providerHealth` follows, for the same reason. Until the probe lands, a
provider is neither signed in nor signed out, and a line that flashed the wrong
answer for a beat would be worse than no line. And the composer's warning
**never disables sending**: the status is a cached read of a store the student
can change in a terminal without this app hearing about it, and locking the box
on a stale read would be the app refusing to do the one thing it is for.

**No deadline is imposed from above**, on the same rule a parse and an embed
follow. An OAuth page can sit open for as long as a person takes to find a
password, switch accounts, or answer a second factor on a phone in another
room. A timeout could only abandon a flow that was still going; `signin::cancel`
is what ends one, and closing the dialog is not it — the run is held by whatever
*opened* the dialog, so hiding it strands nothing and reopening resumes the same
run.

## Installing an agent

Settings → AI said where each CLI was, or that it was missing and which
`OCULUS_*_BIN` would override that; the model picker now says "not installed"
too and links here. Neither offered a way through, so the student's next move
was to leave the app, work out which of four install routes their machine
wants, and come back. The row now carries **Install**
(`app/src-tauri/src/harness/install.rs`,
`app/src/components/settings/InstallAgentDialog.tsx`).

It is deliberately **not** a package-manager wrapper, and the four rules it is
built on are the whole design:

- **The command is shown, verbatim, with Copy — always.** Every route is a
  literal string taken from the vendor's own documentation, with the URL it
  came from in a comment beside it so the next person can re-check rather than
  trust. Copy is the path that always works: a locked-down machine, a student
  who would rather run it in their own terminal, or a route this machine
  cannot run. It is there beside the button that would run it, not instead
  of it.
- **The offer matches the machine.** `brew`, `npm`, `bun` and `curl` are
  found exactly as the CLIs are, login shell included, and only the routes
  whose tool is present are offered to run. Each route also has to land the
  binary somewhere `discover::well_known_dirs` already looks — `~/.local/bin`,
  `~/.opencode/bin`, `~/.bun/bin`, `/opt/homebrew/bin`, npm-global — or the
  install would succeed and the row would go on saying "missing". A machine
  with none of the four is not a dead end and not a dead button: it gets the
  commands to copy and no Install at all.
- **The click on the button showing the command is the confirmation**, and
  there is no other. Nothing that needs `sudo` is ever offered or run — a GUI
  app has no terminal to put a password prompt in — so a provider whose only
  route here would need elevation gets the command and stops there. The child
  runs through `$SHELL -lc` for the profile's PATH, with **stdin on
  `/dev/null`**, so anything that decides to ask a question fails instead of
  hanging behind a dialog that would go on saying "working". And the webview
  names a provider and a manager, never a command: the string comes out of
  Rust's table, because an invoke that took the text would be a shell for
  anything that reached the webview.
- **The finish rechecks.** `discover::binary` caches its failures for the life
  of the process, so a CLI installed a second ago stays missing until
  `discover::forget()` runs — and the webview has a second cache in
  `useBridgeHealth`. Rust deliberately forgets neither: the run's last event
  carries `done`, and the frontend fires `recheck()`, the one path that drops
  both. That is also why the run is held by the Settings section rather than by
  the dialog — closing the dialog mid-install would otherwise strand the output
  *and* the recheck.

Output streams line by line on one `app.emit` event, stdout and stderr drained
by a thread each into one channel (read one after the other, a child that fills
the unread pipe deadlocks), so their interleaving is approximate and the `done`
event is always last. The dialog is the only listener and the only surface:
this app has no toasts, and an install is a foreground thing the student is
watching rather than a background job for the sidebar.

**macOS only, and said so in the code.** The routes are macOS ones, and
discovery itself is macOS-shaped — `binary_name` answers a bare `claude` with
no `.cmd`/`.exe`, the non-unix `is_executable` is only "is it a file", and npm
on Windows installs `claude.cmd` resolved through `PATHEXT` — so `scoop` and
`winget` would be buttons that install something this app then cannot find.
The rest of the app leans the same way (launchd's PATH, `/opt/homebrew`, the
keep-alive LaunchAgent).

## Per-job models

"Nothing is defaulted out of sight" is not only about the composer. The app
hands work to an agent that nobody is talking to — naming a thread, chaptering
a lecture ([chapters.md](./chapters.md)) — and each of those **names its own
agent, model and reasoning level** too, in a row of Settings → AI that is the
*same* `ModelPicker` the composer uses. What the row shows is what the CLI is
told, and there is no per-provider default hiding behind it.

The registry is one JSON value in `settings` under `job_models`:
`harness::jobs` (`app/src-tauri/src/harness/jobs.rs`) reads it, because the
jobs themselves run in Rust, and `getJobModels` / `setJobModels` in
`app/src/lib/db.ts` write it, shaped like the app's other settings rows —
tolerant on read, so a half-written or older value costs a job its
configuration rather than its run. Both sides carry the defaults and have to
agree on them: either can be the one resolving an unconfigured job.

- **Chapters** default to Codex on `gpt-5.6-luna` at `xhigh`; the CLI's
  `--provider` / `--model` / `--effort` still override the configured
  selection for one run, and with no flag `oculus lecture chapters` runs what
  the row says.
- **Thread naming** was `TITLE_MODEL_CLAUDE`, a constant in `mod.rs`, and is
  now a row like any other, defaulting to Claude on Haiku 4.5 at `low`. It
  costs the rule that naming followed the thread's own provider: a thread now
  gets named by whichever CLI the row names, whatever it was itself run on.
  That is the point of the registry — the student has said who pays for the
  naming turn — and a namer whose CLI is not installed fails the way a missing
  CLI always has, leaving the first-line title rather than hanging a thread.
- **Adding a job** is a `Job` variant with a key and a default in `jobs.rs`, a
  matching entry in `JOBS` and `DEFAULT_JOB_MODELS` in `db.ts`, and nothing
  else: the Settings section renders whatever is in that list.

## Subject scope and `@`

The composer carries two more controls than a bare prompt box, both in
`app/src/components/harness/Composer.tsx`.

**A subject, or General.** `SubjectSelect.tsx` scopes the thread, sitting
above the box rather than in the control row under it, and only while the
thread is new — the choice is made once, before the first message, and an
open thread cannot change it, so a dead control under every later message
would spend the row on nothing. The menu lists this term's subjects only
(plus whatever a thread is already scoped to), as icon and code; past
subjects are reachable from the sidebar, not from here. It is not a
sandbox — every thread runs from `agents/` and reads all of `../courses/`
either way — it says which subject the questions are about, so "what's due
this week" has an answer. Picking one appends a short section to the
instructions naming that course folder and its memory bucket —
`agents/memories/<CODE>/`, which is under the thread's own writable root
rather than in the course folder, because the course folder's copy is a
symlink and a path the sandbox refuses is not an instruction
(`instructions()` in `app/src-tauri/src/harness/mod.rs`); General appends
nothing and gets the library-wide brief. The scope is stored on the thread
(`harness_threads.subject_id`) and locks once the thread exists, for the same
reason the provider does: all three CLIs bind the appended instructions at
session start, so a re-scope would be a lie until the process was restarted. Rust
reads it back off the row rather than trusting the payload, and joins
`subjects` for the folder name so a renamed subject cannot leave a thread
pointing at a folder that is gone.

**`@` picks a file.** The menu lists files narrowed to the thread's subject,
ordered by prefix match then by what was opened recently, and only ones the
agent can actually read — markdown as written, everything else once it has
been parsed, the same predicate retrieval uses. Choosing one writes
its **library path** into the message and nothing else. No content is
attached and nothing is retrieved here: the agent already has the library in
front of it and its own tools for opening a file, and a path is what it was
missing. That path is also exactly what `oculus read` takes, which
`HARNESS.template.md` tells the agent.

**The query may hold spaces, and it has to.** Stopping the token at the first
whitespace put every multi-word title permanently out of reach — and matching
a spaced query literally would find nothing either, because `safe_filename` in
`app/src-tauri/src/paths.rs` turns "Paper Topics" into `Paper_Topics` on the
way to disk, so no filename in a library contains a space at all. So the words
are ANDed instead: every word must appear in the filename
(`searchMentionFiles` in `app/src/lib/db.ts`, shared with the
"N matching files have no markdown" count so the two cannot disagree), which
is what makes `@paper topics` reach `INFO30006_Paper_Topics_14.docx`. What
keeps a stray `@` in prose from leaving a menu armed over the Enter key is
three cheap guards — the character after the `@` must not be whitespace, the
token stops at four words and 60 characters, and a backtick ends it so a token
cannot reach back across a mention already picked — and, still, that the menu
only opens when the query matched a file.

**A mention is drawn as a chip and sent as a path**, and those being two
different strings is the whole design. `app/src/components/markdown/FileChip.tsx`
is one definition used in three places — the composer's box while the message
is being typed, the question bubble once it is sent, and a path the agent
quotes back in its own prose — so a mention never changes appearance as it
moves through the thread, while the message itself stays the backticked
library path the agent needs. The chip is built from the path *alone*
(`pathFile` and `splitLibraryPaths` in `app/src/lib/openFile.ts`, whose folder
table mirrors `category_from_path` in `paths.rs`), which keeps the timeline's
rule intact: shape, never resolution, so a hundred of them cost no queries and
the row is only read when one is clicked. Only a fence whose *whole* content
is a library path counts — a backticked command or flag is prose that happens
to be fenced and stays monospace, since that is the one place monospace is
still for code.

**The `@` machinery is one piece, and it is not the composer's.** The token
parser, the file lookup, the "no markdown" count, the selected row, the
keyboard handling and the measured flip all live in
`app/src/components/harness/useMentionMenu.ts` over
`app/src/components/harness/MentionMenu.tsx`, because a **task body** wants the
same mentions (`TaskBody` in `app/src/pages/TaskPage.tsx`, `docs/projects.md`).
A call site supplies only the two things that are genuinely its own: the scope
`@` narrows to, and what its own Enter means — the menu claims the keys it
handles by `preventDefault`, so a box whose Enter sends, or saves, is guarded
by a `defaultPrevented` check and the two never fight over a keystroke. A
second copy of the menu would have stayed correct exactly until one of them
grew a rule the other lacked, which is `FileChip`'s argument applied to the
thing that produces the chip.

That chip is why the composer's box is **no longer a `<textarea>`**: a
textarea cannot draw an icon inside its text.
`app/src/components/harness/MentionInput.tsx` is a contenteditable whose
mentions are atomic `contenteditable="false"` spans, and it keeps the DOM as
the source of truth while someone types — React renders its chunk model only
on a structural change (a pick, a restore, a send) and bumps the editor's
`key` so the box is rebuilt rather than diffed against a tree the typist has
been editing underneath it. `readEditor` walks the live nodes back into the
message, where a chip is its path again and WebKit's no-break space and the
zero-width caret guards are dropped. Two things that look like superstition
and are not: the guard in front of a leading chip exists because WebKit has
nowhere to put a caret before a `contenteditable="false"` element that starts
a block, and the explicit `select-text` exists because `index.css` turns
selection off on `body` and hands it back per *tag* — a `div` is not on that
list and a `textarea` was. A third: **the box scrolls itself.** A
contenteditable follows its caret only for the edits the browser believes it
made, so a Shift+Enter inserted through `execCommand` on a box that has
reached its `max-h` — and every caret placed by hand after a structural
re-render, since setting a range is not an edit — leaves the caret under the
bottom edge with the typing going on out of sight. `revealCaret` measures the
caret's line and scrolls *that box* (`scrollIntoView` would drag the thread
behind it), and answers the end of the box by going to the bottom rather than
by measuring: the only thing to measure there is WebKit's trailing placeholder
`<br>`, whose rect sits on the caret's line about half the time and one line
below it the rest, which lands the reveal a line short on alternate presses.

**The menu hangs off the `@`, not off the box** — a completion popup where the
token was typed rather than a panel as wide as the composer pinned under the
whole thing. `app/src/components/harness/useMentionMenu.ts` reads the `@`'s own
client rect and `app/src/components/harness/MentionMenu.tsx` draws itself
`fixed` at it, portalled to the body, at a width of its own and clamped inside
the viewport on both axes. Two decisions in there
are load-bearing. It anchors on the **`@` and not the live caret**, because the
query grows as it is typed and a caret-tracking list slides sideways while you
read it — and because that makes a second reading idempotent, which is what
lets a scroll re-measure instead of closing the menu (the editor is itself a
scroller: a message long enough to fill it scrolls on every keystroke). And the
rect comes from a range **extended back over the token**, from the `@` to the
caret, because in WebKit a *collapsed* range's `getBoundingClientRect()` can
come back all zeros; the token is known to sit inside one text node, so the
`@` is addressable directly, and the fallbacks below it step down to the
selection's own element rather than to 0,0 — a menu in the window's corner
would be worse than one that is too wide. The arithmetic is plain CSS pixels,
which is only safe because this app's zoom is the webview's page zoom and never
a CSS `zoom` on a container (root `CLAUDE.md`).

From there the menu **opens downwards and only flips up when it would not
fit**, measured against the caret's line. The same composer sits in three very different places —
the middle of the home page, the middle of the chat hero, and pinned to the
bottom of an open thread — so which way is out of the way is a fact about the
viewport, not about the call site, and it is measured after the menu is in the
DOM rather than against its `max-height`, so a three-file list is judged on the
90px it occupies and not the 256px it is allowed. Opening upwards
unconditionally was right for the thread and wrong everywhere else: on the home
page the list covered the subject pill and the cards above it while the lower
half of the page sat empty.

The earlier BYOK chat agent did the opposite — it read the file, embedded the
query and packed the result into the request — because its model could only
see what the prompt carried. A CLI agent can open the file itself, so that
whole path was dropped rather than ported.

## A picture in a message

A screenshot of a worked solution, a photo of handwriting, a diagram from
somewhere else: **pasting or dropping an image into a composer attaches it**,
and it reaches the agent the same way a mention does — as a path.

**Every composer, on the same terms.** The page box and the lecture dock's box
are deliberately separate components — they answer different questions, and one
of them is 300px wide beside a playing video — but a screenshot dragged onto
either is the same gesture asking for the same thing, so the whole of it is
shared: `app/src/hooks/useAttachments.ts` holds the pending list, the paste, the
drop, the write on send and the refusals, and
`app/src/components/harness/AttachmentStrip.tsx` draws the chips. What a box
passes in is only the wording of a refusal (the dock has no `@` menu to point
at) and whether its chips are the compact size. A drop that worked in one box
and did nothing in the other read as the app being broken rather than as two
boxes with different jobs.

A CLI agent has no channel for an image. It reads files, so an image has to
*be* a file before it can be talked about, which makes this the same move the
`@` menu already makes and the same one a dock message's frame grabs make
(`lecture_grab_frames`, below). `harness_attach_image` takes the clipboard's
bytes as base64, `harness_attach_file` takes a dropped file's path, and both
write into `agents/attachments/` and answer with `./attachments/<name>` —
relative to `agents/`, because that is every thread's working directory
(`app/src-tauri/src/harness/attach.rs`). The composer appends those paths to
the message, fenced, so the bubble draws them and the agent opens them with the
one matcher both already use. The bubble **lifts them out of the prose and
draws them above it, as cards of their own rather than inside it**
(`liftPictures`, `app/src/components/harness/Timeline.tsx`) — the shape every
chat UI has settled on. One draws at its own size; several become a
three-across grid that wraps, letterboxed on the bubble's ground rather than
cropped square, because what is attached here is a screenshot of a question and
a crop of one is unreadable. Lifting them also closes up the blank line and the
doubled space the paths leave behind, and keeps them clear of the fold, since a
picture is the question's subject and never the part worth hiding. A question
that is *only* pictures draws no bubble at all. `HARNESS.template.md` says what such a path is
and to open it before answering.

**Clicking one opens it in the app, not in Preview.** A card a few centimetres
wide and a 56px chip in the composer are both identifiers rather than
something you can read, so each opens the shared viewer
(`ImageLightbox`, `app/src/components/ui/Lightbox.tsx`) — the diagram
lightbox generalised, so a screenshot gets the same fit, zoom and pan a
mermaid figure does. It replaced handing the path to `openLibraryPath`, which
found no library row for an attachment and fell through to opening the file in
the OS: a window over the top of the app for something the app can draw.

**The reply draws pictures as well as the question does.** The shared renderer
(`app/src/components/markdown/MdComponents.tsx`) resolves a markdown
`![…](…)` whose src is a path in this library — an attachment, or an image
under `courses/` — through the same asset URL the bubble builds, and draws a
fenced `agents/attachments/…` as the picture rather than as a code span. So a
picture reads the same whichever side of the conversation named it, and a
markdown image is a thing an agent can actually write. A src that matches
neither and has no URL scheme keeps its alt text: an image fails *visibly*
where a link fails silently, and WebKit's broken-image glyph parked in the
middle of a reply is worse than a word. `InlineMd` still refuses pictures
outright — its output sits inside a button that seeks.

**Inside `agents/` and nowhere else**, because that is the one folder every
bridge can both read and write: a picture written beside `courses/` would be
refused by Claude's and Codex's sandboxes at the moment the agent went to look
at it.

**Nothing is written until send.** The composer holds the clipboard's `File`
or the dropped path and draws it from memory, so a screenshot pasted and then
thought better of leaves nothing on disk to sweep up — and a write that fails
keeps the message in the box and says why, since a path pointing at nothing is
worse than a send that did not happen.

**The claimed filename never reaches the filesystem.** The bytes are sniffed,
the extension follows from what they actually are, and the stem is the app's
own timestamp — which removes path traversal, the extension lie and the
collision in one move. A file that is not a picture is refused rather than
attached, and 20 MB is the cap, so a video dropped by mistake stops here.

**The drop is Tauri's event, not the page's.** Tauri's own drag-and-drop
handler sits in front of the webview, so a file dragged in from Finder never
reaches a React `onDrop` — the page sees nothing at all. Switching that handler
off would hand file drops to WebKit and take in-page dragging with it, which
the project board still needs, so `useFileDrop`
(`app/src/hooks/useFileDrop.ts`) listens to Tauri's events instead. They arrive
with no target, so the element is found by arithmetic against a
`getBoundingClientRect` — and both the channel they arrive on and the units
that arithmetic is in are traps, below.

**The position is in points, whatever its type says.** Tauri hands it over as a
`PhysicalPosition` and it is not one: wry reads macOS's `draggingLocation` and
subtracts it from the view's frame height without ever multiplying by the
backing scale factor, so what arrives is the window's own logical coordinates.
Dividing those by `devicePixelRatio` — the conversion the type asks for —
halves every point on a retina screen and folds the whole window into its
top-left quarter, so a drop on a composer at the bottom of the window reports
as the middle of the thread, hits nothing, and is swallowed in silence with
every handler correctly attached. The hook measures the scale instead of
assuming it: points to CSS pixels is the viewport's width over the window's own
logical width (`innerSize` over `scaleFactor`), which is `1` unzoomed and stays
right when the page is zoomed, re-read on a resize and as each drag enters.

**The *webview's* events and not the window's**, which is a distinction with
teeth here, and it runs the opposite way to what the API suggests. Tauri
synthesizes a drop as a **window** event only when the runtime built that
webview as the window's own content (`WebviewKind::WindowContent`); otherwise
it is a **webview** event, emitted to `EventTarget::Webview` — and Tauri's
`filter_target` has no arm matching a `Window` listener against a `Webview`
emit. This app is never the first case. `app/src-tauri/Cargo.toml` turns on
tauri's `unstable` feature, which is what `Window::add_child` needs for the
in-app browser (`app/src-tauri/src/browser.rs`), and `tauri-runtime-wry` picks
the main window's own webview kind under exactly that cfg: with `unstable` on
it is `WindowChild`. So every drop here is webview-addressed whether or not a
browser tab is open, and how many webviews the window holds never enters into
it. A window listener subscribes cleanly, reports success and is then never
called once — which is the whole difficulty: nothing to see but a drag that
does nothing, with paste still fine.

**A hidden tab's composer is still at those coordinates.** Because the events
carry no target, every mounted `useFileDrop` tests the same point against its
own rect — and a pane that is not the active tab is hidden with `visibility`,
never `display: none`, so that switching tabs does not cost a scroll position
or a torn-down webview (`app/src/components/tabs/TabPane.tsx`). Its rect is
therefore live and in the same place as the visible one's. Without a check, one
drop lands in two boxes, and the invisible one keeps the picture until
something sends it. The hook reads the element's computed `visibility`, which
inherits, so asking the box answers for the pane above it.

**A dropped path needs no scope entry; an attachment does.** The asset
protocol refuses anything outside `assetProtocol.scope`
(`app/src-tauri/tauri.conf.json`), and a picture in `agents/attachments/` is
neither a course file nor a lecture — so the scope names it, or every picture
in every thread draws as WebKit's broken-image glyph while the file sits
happily on disk. The file the student *drops* is the exception that needs no
entry: Tauri adds a dropped path to the runtime scope as it delivers the
event, which is why the composer's strip can preview it straight from
`convertFileSrc` before anything has been written.

**A task body writes on paste instead**, and that is the one place this rule
is reversed (`TaskBody` in `app/src/pages/TaskPage.tsx`). A body has no send to
defer to — the picture has to be *in* the text while you are still writing
around it — so it is written immediately and inserted at the caret as a
markdown image, whose path is the same shape the composer fences. The cost is a
file left in `agents/attachments/` if the picture is then deleted from the
text, and that is the accepted trade against an image tag pointing at nothing.

The lecture player's dock composer is a sibling rather than a variant of this
one (`LectureChatComposer.tsx`) — a plain textarea in a narrow panel, with the
moment chip the page box has no playhead to build — but it takes attachments on
exactly the same terms, through the same hook and the same strip. Its chips are
the compact size and its refusal names a typed path rather than `@`; everything
else is shared. The frame grabs below are *other* pictures its messages can
carry, not the only ones.

## Lecture scope, and the moment

A thread can also be scoped to one **recording** —
`harness_threads.lecture_id` (migration 30), for the conversation the lecture
player's dock holds beside the video. It is the subject scope's shape with a
narrower subject and one thing it cannot do: NULL clears rather than cascades
for the reason `subject_id` does — the conversation is the student's own and
the lecture merely scopes it — and it is fixed at creation, because all three
CLIs bind the appended instructions at session start.

**Rust reads the subject off the lecture's row, not off the payload.** The
player has no subject picker — the recording answers that question already —
so a `subjectId` sent alongside a lecture would be the webview repeating a
fact the database holds, and a stale one would point the brief at the wrong
course folder. `store::create_thread` looks it up; a lecture thread's
`subject_id` and `lecture_id` cannot disagree.

**The lecture section is appended after the subject one**, both from
`instructions()`. It names the recording folder as `../lectures/<id>/` — every
thread runs from `agents/`, so that is the path the agent can paste straight
into a read — its `transcript.vtt` when one is actually on disk, the course
folder where the deck is, and **the chapter list inline**. The chapters are
inlined and the transcript is not, on the same argument the chaptering job
makes about its own two files ([chapters.md](./chapters.md)): a dozen short
lines cost nothing and a tool call to fetch them is a turn the student waits
through, where twenty thousand words of transcript is something the agent
should open the part of that it needs. The list is read on every send rather
than built once, so a lecture chaptered after the conversation started gets
its chapters on the next turn — and the brief is resolved before *any* session
is spawned, the one a rewind brings back up included, since a session opened
without it would answer without it for the rest of the thread.

**The rest of the section is paid for by a turn that went looking.** A
recorded thread (`agents/threads/26.ndjson`) spent a minute on one question:
four reads scrolling a VTT whose every other line is a `NOTE CONF` block of
recogniser confidence numbers the agent had no way to know was noise, two
`oculus files` calls to discover which PDF the slide deck was, and two
refusals from guessing that `grep` and `read` take a subject positionally the
way `files` does. None of that is the model being slow — it is the brief
naming a course folder and leaving the rest to be rediscovered on every
thread. So the section also carries **the recording's date** (Echo360 titles
are the timetable's — `MULT20015_2026_SM2 TU L105` — so the date is the only
thing on the row that says which week, and therefore which deck), **the shape
of the VTT** and how to seek in it by timestamp, and **the deck hunt written
out with its real flags**. Nothing links a recording to its slide deck in the
database, so that last one is a recipe rather than a fact; if a link ever
exists, it replaces the recipe.

The section closes with the sentence the dock depends on: the student is
watching this lecture, and a message may carry the moment it was sent at — a
timestamp, the last minute of transcript, and a frame of every stream the
capture has — and that the moment is usually enough, so the deck is for when a
question needs the exact notation rather than a first move. It also says what
two frames are: two cameras on one second, either of which can be the one with
the teaching on it, so both are worth opening before deciding a frame shows
nothing.

**The moment rides the prompt; it never becomes the message.**
`SendOptions.context` is appended to what the CLI receives, after the
student's text, under a `---` and a heading. The row stores what was typed and
nothing else — a timeline that read back a transcript excerpt as the question
would be a timeline of something nobody asked. What does go on the row is
`at`, the playhead's second, in the user item's `meta`, so the bubble can say
"at 3:40"; it travels the way every other fact does, on
`HarnessEvent::UserMessage`, so `store::apply` is the one place that writes
it. `harness_edit_resend` and the rewind path carry both like any other
option.

**`lecture_grab_frames` is the picture half of that moment.** One JPEG per
downloaded stream of the playhead's second, into
`lectures/<id>/frames/live/<seconds>-source<n>.jpg` — its own subfolder, so a
message's grab can never collide with a chaptering run's frames in the folder
above, and overwritten freely. It reuses that job's probe-and-grab rather than
a bare ffmpeg call, because the splash-screen defence
([chapters.md](./chapters.md)) is exactly as load-bearing for a frame the
*student* asked about. What it returns are the paths the **agent** can read,
`../lectures/<id>/frames/live/<seconds>-source<n>.jpg` with the stream's
number beside each: the webview never opens the files — it puts the strings in
the message. A lecture with no downloaded recording is refused the way
`chapters::run` refuses one.

**Every source is grabbed, not the one on screen.** Echo360 numbers the
streams rather than naming them and neither is reliably the one being taught
from: in a theatre where the lecturer works at the whiteboard, source 1 holds
the room's idle splash for the hour and the derivation being asked about is
only ever on source 2 — so a moment built from the visible pane alone hands
the agent a picture of nothing to reason about, which is what it then says.
The student is asking about the *second*, not about the pane they happen to
have in front, so the moment carries every view of it and the brief tells the
agent that two frames are two cameras on one second rather than two moments.
A stream that will not decode drops its own line; only a lecture with nothing
on disk is an error. The second grab costs ~200 ms.

**The live grab is wider than a chaptering run's** — `LIVE_GRAB_WIDTH` (1536,
a cap, so a 1280-wide stream passes through unscaled) against `GRAB_WIDTH`
(768). A run writes fifty frames of slides into one prompt and wants them
small; a message writes one or two and the question can be about a whiteboard,
where 768px is the difference between the agent reading the notation and
seeing grey marks.

## The dock's chat

The player's dock reads a recording three ways, and the third is a
conversation about it ([frontend.md](./frontend.md) for the dock itself). It is
the Claude Code side panel's shape at a third of the width: the thread's name,
a history button, a new-thread button, the timeline, a composer.

**Available on every recording, which is what makes the dock unconditional.**
Chapters need a job to have been run and Transcript needs a file on disk; a
conversation needs neither, so the Chat tab is never filtered out of the strip
and there is no lecture whose dock is empty.

**The timeline is the page's, unchanged.** `Timeline` takes the same
`questions` and `pending` actions here, so edit, retry, rewind and the queued
bubble all work in the dock; the stick-to-bottom scroll is the one piece that
was lifted out of `ChatPage.tsx` into `app/src/hooks/useStickToBottom.ts`, so
"has the reader scrolled away" has one answer rather than two. What the dock
does not draw is the question rail — it hides itself below a 760px column
anyway — and the usage wheel, which is a number for the page's composer to
carry.

**The panel owns its thread id; the store's `activeId` stays the Chat page's.**
That is what the per-thread `items` map above is for. `load` puts the dock's
rows in it and `release` takes them out when the tab or the lecture changes,
because a dock walked through twenty lectures would otherwise hold twenty
timelines. Which thread each lecture is on lives in a module-level `Map`, for
the reason `startedAt` does in `useLectureChapters`: the player unmounts on
every app-tab switch, and a conversation that reset to the newest thread under
someone who had deliberately gone back to an older one is a worse lie than no
memory at all. `null` is a real answer in that map — it is *New thread* having
been pressed — so the key's presence, not its value, is what says the choice
has been made. A lecture not in it opens on its most recent thread
(`getLectureThreads`), or on the empty composer.

**The history popover borrows `ProviderMark` and nothing else.**
`ThreadList` is the page's column — groups, a fold, delete confirms — and is
built for it; this list answers one question, so it is the thread's name, its
agent's mark and how long ago it was. The same threads also appear on the Chat
page under their subject, with the app's lecture icon on the row, so nothing
lives only in the dock. Opening one there works; it simply has no playhead to
attach.

**The composer is a sibling of the page's, not a variant of it.** The two
boxes carry different things because they answer different questions. The page's
opens a conversation, so it has the subject select and the `@` file menu; the
dock's has neither, because the lecture fixes the subject and the agent has
already been handed the recording folder (a file is typed as a path). What it
adds is the moment: a toggle and a chip, on by default. Threading a `compact`
flag and four "not here" props through one component would have left the shared
one harder to read than both of them are apart. The model picker *is* shared,
and it is the same session-wide selection the page's composer sets — every send
still names an explicit model and level.

**The chip must not re-render the dock.** `TranscriptPanel` is memoised against
a player that re-renders four times a second on `timeupdate`, and the whole
reason the virtualised transcript stays smooth beside a decoding video is that
nothing time-varying reaches it. So the playhead travels as a **ref** whose
identity never changes: the chip ticks itself off it once a second — the shape
`Running`'s elapsed clock uses in `ChaptersPanel` — and the send reads it there
rather than from a prop that would be a frame behind. The Chat tab's props are
a third memoised bag beside the Chapters tab's and the enhanced transcript's,
built over stable values only.

**The moment is built by the player at send time**
(`buildMoment` in `LecturePlayer.tsx`): the second itself, the chapter the
playhead is in, the transcript cues of the minute before inlined as plain text,
and the paths from `lectureGrabFrames`, listed by source number with the
number's usual meaning (`SOURCE_HINT`) beside it. It goes out as `SendOptions.context` with
the second as `SendOptions.at` — appended to the prompt, never the message, as
above. A minute of transcript is a few hundred words where the file is twenty
thousand, which is why this one is inlined and the file is only named. **A
frame that cannot be grabbed drops its line and the message still goes**: a
lecture whose video was never downloaded is refused by `lecture_grab_frames`,
and losing the question over a missing picture would be the wrong half to lose.
Sending does not touch playback.

The chip shows the timestamp it will send and tracks the playhead until the
send; what freezes is the row. `at` rides `HarnessEvent::UserMessage` as well
as the row Rust writes, so the bubble says "at 3:40" from the moment it appears
rather than after a reload, in the body font with `tabular-nums` beside the
time the question was asked.

## Stages

Built: the three bridges with recorded fixtures and replay tests, `oculus
agent` as the headless proof, tables and lifecycle, the page, subject scope
and the `@` file menu, the message queue, stopping a turn, going back
(edit, retry, rewind), the per-job model registry above, [signing in to opencode's
providers](#signing-in-to-a-provider), [signing an agent back
in](#signing-in-to-an-agent), and the lecture player's dock chat. Not yet: approvals and native questions routed to the UI, steering
mid-turn (bb's `turn/steer` and a second stdin line — the queue is the
waiting-room version of it, not steering), the plan/todo card, branching (rewind
deliberately does not).

**The third bridge was the API path, and it arrived as a bridge.** This page
used to list "a third bridge for the API path when BYOK returns" as the plan:
the dormant provider layer would wake up beside the CLIs and chat would have
two kinds of backend. That is reversed. opencode *is* BYOK — the provider
catalogue, the credential store, the streaming client and the model library
belong to a program that already does all of it, and everything reaches the
app through the seam that already existed. So there is one `HarnessEvent`
stream, one timeline, one containment rule and one job registry, instead of a
second code path with its own tools, citations and spend ledger. The BYOK layer
is deleted rather than dormant.

The half of BYOK that was left to a terminal — *where the key comes from* — is
now in the app too, and it did not cost a code path either: opencode's server
already owns the provider catalogue, the credential store and the browser
flows, so [signing in to a provider](#signing-in-to-a-provider) is more HTTP
over the connection the bridge already holds. Keys still live in opencode's
store rather than this app's, which is what keeps the sentence above true.

The plan/todo card is still outstanding despite projects being built, because
the two are different things: that card would draw the provider's *own*
in-turn todo list (`ToolKind::Plan`, collapsed today), which lives and dies
with the turn, where a project is the student's, persists, and is edited on a
board long after the thread has moved on.
