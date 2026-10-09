# The harness: CLI agents as chat

Chat is a coding agent the student already has — Claude Code, Codex, opencode
or Antigravity (`agy`) — run as a subprocess from the library's `agents/`
folder, its output folded into one timeline. Claude Code, Codex and `agy` carry
the student's subscription; opencode carries whatever `opencode auth` holds and
is the app's only API path. The shape is bb's (get-bb/bb) without plugins: one
bridge per provider, one event stream, a timeline that only sees the stream.

## Where

| Piece | Location |
| --- | --- |
| Manager, queue, Tauri commands, `run_once`, per-thread `instructions` | `app/src-tauri/src/harness/mod.rs` |
| The event enum and tool classification (`classify`) | `app/src-tauri/src/harness/event.rs` |
| Bridges: Claude (`claude -p`), Codex (`app-server`), opencode (`serve`), Antigravity (`agy --print=`) | `app/src-tauri/src/harness/claude.rs`, `app/src-tauri/src/harness/codex.rs`, `app/src-tauri/src/harness/opencode.rs`, `app/src-tauri/src/harness/antigravity.rs` |
| Paths no agent may write; opencode's ruleset; agy's rules | `app/src-tauri/src/harness/protected.rs`, `app/src-tauri/templates/OPENCODE.template.json`, `app/src-tauri/src/harness/antigravity_rules.rs` |
| Finding, installing, updating and signing in the CLIs | `app/src-tauri/src/harness/discover.rs`, `app/src-tauri/src/harness/install.rs`, `app/src-tauri/src/harness/update.rs`, `app/src-tauri/src/harness/signin.rs` |
| …their frontend | `app/src/hooks/useBridgeHealth.ts`, `app/src/hooks/useSignInStatus.ts`, `app/src/components/settings/InstallAgentDialog.tsx`, `app/src/components/settings/UpdateAgents.tsx`, `app/src/components/harness/SignInDialog.tsx`, `app/src/pages/settings/AgentsPage.tsx` |
| Process pipes, JSON lines and terminal turn failures shared by the bridges | `app/src-tauri/src/harness/child.rs` |
| Thread and timeline rows | `app/src-tauri/src/harness/store.rs` |
| The brief appended to each prompt; `AGENTS.md`; skills | `app/src-tauri/templates/HARNESS.template.md`, `app/src-tauri/templates/AGENTS.template.md`, `app/src-tauri/templates/skills/`, `app/src-tauri/src/agents.rs` |
| Recorded provider output the bridge tests replay | `app/src-tauri/fixtures/harness/` |
| Provider table (`PROVIDERS`), types, commands | `app/src/lib/harness.ts` |
| Live state and event folding | `app/src/stores/harnessStore.ts`, `app/src/hooks/useBackendEvents.ts` |
| Page, thread list, recent list, timeline, rows, composer | `app/src/pages/ChatPage.tsx`, `app/src/components/harness/` (recent list: `RecentThreads.tsx`) |
| Unsent text per thread (and per new-thread box), kept across switches and relaunches | `app/src/stores/draftStore.ts` |
| Model picker and its catalogue hook | `app/src/components/harness/ModelPicker.tsx`, `app/src/hooks/useProviderModels.ts` |
| Settings → opencode: the provider and model tables, the connect flow, the offered-model gate | `app/src/pages/settings/OpencodePage.tsx`, `app/src/components/settings/opencode/`, `app/src/components/settings/OpencodeConnectDialog.tsx`, `app/src/lib/opencodeCatalogue.ts` |
| Per-job models | `app/src-tauri/src/harness/jobs.rs`, `JOBS` in `app/src/lib/db.ts`, `app/src/pages/settings/JobsPage.tsx` |
| One-off turns (`Harness::one_off`, `Harness::one_turn`): thread names, the editor's inline suggestions (`document_suggest`), where a lecture ends | `app/src-tauri/src/harness/mod.rs`, `app/src-tauri/src/harness/suggest.rs`, `app/src-tauri/src/lecture_end.rs` |
| `@` menu and mention input | `app/src/components/harness/useMentionMenu.ts`, `app/src/components/harness/MentionInput.tsx`, `app/src/components/markdown/FileChip.tsx` |
| Send, queue and stop controls shared by both composers | `app/src/components/harness/SendControls.tsx` |
| Pictures pasted or dropped into a composer | `app/src-tauri/src/harness/attach.rs`, `app/src/hooks/useAttachments.ts`, `app/src/hooks/useFileDrop.ts` |
| Long pastes held as text cards: format, card, editor and viewer | `app/src/lib/attachments.ts`, `app/src/components/harness/PastedText.tsx`, `app/src/components/harness/PastedTextEditor.tsx` |
| Library paths in rows and prose; copy as markdown | `app/src/lib/openFile.ts`, `app/src/lib/selectionMarkdown.ts` |
| The lecture dock's chat | `app/src/components/lectures/LectureChatPanel.tsx`, `app/src/components/lectures/LectureChatComposer.tsx`, `lecture_grab_frames` in `app/src-tauri/src/chapters.rs` |

## Four dialects become one event stream

Every bridge translates into `HarnessEvent` (session, user message, turn,
text/reasoning deltas, tool started/output/finished, usage, rate limits,
error, exited). `ThreadTitled`, `Queued`/`Unqueued`, `Rewound` and
`TurnAnchor` have no provider behind them but ride the same path so they are
persisted the same way. `classify` in `event.rs` is the one tool table.

| CLI | Process shape | Resumed by |
| --- | --- | --- |
| Claude Code | one `claude -p --input-format stream-json` per thread | `--resume` |
| Codex | one `codex app-server` per app, a `threadId` per thread | `thread/resume` |
| opencode | one `opencode serve` per app, a v1 HTTP session per thread, one SSE stream routed by `sessionID` | session id |
| Antigravity | one `agy --print=` per thread | `--conversation` |

- **Rust writes the rows; the webview folds the stream.** One consumer thread
  writes every event to `harness_threads`/`harness_items`, then emits
  `harness-event` with the row id — a tool's finish never overtakes its start,
  and a reload loses nothing.
- **Settings storage is shared.** Job selections, rate limits and Antigravity
  approvals use `store::setting` / `store::set_setting`; each caller owns its
  JSON shape and fallback policy.
- **Account events have no thread.** Codex's `account/rateLimits/updated` is
  dispatched before the route lookup (`translate_account`) on thread 0, which
  is why `harness-event` carries the provider.
- **Every raw line is kept** in `agents/threads/<id>.ndjson` (the fixtures).
- **`run_once` is the headless shape** — no thread, no rows; its jobs
  ([chapters.md](./chapters.md)) do their own database write in Rust.
- Chat's old tables (`llm_usage`, `chats`, `chat_messages`) have no readers.

## Every thread runs from `agents/`, and that is the containment

A thread rooted at the library writes straight into `courses/` under
`acceptEdits`. From `agents/`, each CLI refuses other writes its own way; the
protected paths are one list in `protected.rs`.

- **Claude**: its sandbox with the cwd as the only write root, `--add-dir` for
  reads (without it `ls ../courses` is refused), `Edit` denies on protected
  paths so `--add-dir` doesn't pull them into `acceptEdits`, and
  `--permission-prompts none`, since an unanswered prompt under `-p` hangs.
- **Codex**: `workspace-write` with one root, `approvalPolicy: never`; any
  server request is declined.
- **opencode has no sandbox** — a rule list its runner checks. File tools are
  real (`edit` covers `write`/`edit`/`apply_patch`); `bash` is a glob over the
  command string, allow-listing `oculus …` and `ls …` — it stops `rm -rf`, not
  a redirect. Rule shapes: a leading `"*": "deny"` beats every later allow;
  the *last* rule decides whether the tool is offered at all, so an allow comes
  last; a path inside the session directory is written relative to it.
- **Antigravity**: [below](#antigravity-keeps-its-rules-in-the-students-global-settings).

**The database is the one hole, three files wide.** `oculus project`/`task`
write the board ([projects.md](./projects.md)), and a sandbox that opens
`oculus.db` but not `-wal` fails "attempt to write a readonly database". So
`paths::db_write_paths` names `oculus.db`, `-wal`, `-shm`: `allowWrite` for
Claude; `writableRoots` on the turn's `sandboxPolicy` plus
`sandbox_workspace_write.writable_roots` in thread config for Codex. Files,
not the folder — it holds the session cookie and Ed token.

**The CLI stays the only door.** Claude merges `Edit(...)` denies into its
sandbox's `denyWrite`, so an `oculus.db*` deny would cancel the grant; instead
`sqlite3` is denied by name (`Bash(sqlite3:*)`) — a speed bump, not a wall.
opencode keeps its `oculus.db*` deny. `Bash(oculus:*)` is allowed by name
because Claude's sandbox auto-allows only Bash it can statically vouch for,
and one refusal (a multi-line `--brief`) auto-denies the rest of the session.
The board notices an agent write because `useBackendEvents` matches the
finished command's text against `oculus project|task` — `is_oculus_cli` only
checks the first words, and `cd … && oculus task add` reads as Bash.

**The child's environment is edited.** `ANTHROPIC_API_KEY`/`OPENAI_API_KEY`
are stripped so a shell key cannot move a subscription onto API billing, and so
opencode (which reads both) shows the same catalogue from a terminal as from
the Dock. `CLAUDECODE`/`CLAUDE_CODE_ENTRYPOINT` are stripped because the CLI
refuses to nest. The `oculus` binary's directory goes first on PATH, so a
stale dev binary shows up as `unrecognized subcommand`
([development.md](./development.md)). Claude's auto-memory is off.

### Antigravity keeps its rules in the student's global settings

`--sandbox` bounds shell commands only, and `--dangerously-skip-permissions`
lets `write_to_file` write outside the library, so that flag is never passed.
`agy` 1.2.9 reads rules only from `~/.gemini/antigravity-cli/settings.json`,
so `antigravity_rules.rs` merges Oculus's block there before every spawn (or
fails the spawn): writes on the three database files, the `oculus` binary's
real directory (the sandbox can't read through the `~/.local/bin` symlink),
read-only commands no flag turns into a write (`find`/`rg` are out — the
matcher is a word prefix), `sqlite3 <library>/oculus.db` denied per path
spelling (not `command(sqlite3)`, which bans it everywhere), and the student's
approvals. The entries are global — they apply in the student's own `agy` —
and `antigravity-rules.json` at the library root records which are Oculus's.

**A refusal ends the turn as `completed`** (an `ERROR` step reading
`permission check failed…`, then a `SUCCESS` result with `denied_actions`).
The bridge emits `PermissionNeeded` (`PermissionCard.tsx`);
`harness_antigravity_allow` stores the rule and drops the process, since a live
`agy` never re-reads rules or `--model`. A deny-rule refusal shares the prefix
but the agent carries on. `agy` has no control channel (interrupt signals the
child), no rewind and no `--append-system-prompt`; its tool parameters are
PascalCase (`CommandLine`, `AbsolutePath`).

## The brief is appended, except on opencode

`HARNESS.template.md` goes in as `--append-system-prompt` (Claude) or
`developerInstructions` (Codex). opencode's agent `prompt` *replaces* the
system prompt, so the brief is the `oculus` agent's prompt in
`agents/opencode.json`, rewritten on every server start. Codex, opencode and
`agy` read `agents/AGENTS.md` natively; where there is no per-call append
(opencode, `agy`) the per-thread half rides the first user message.

**Skills are one directory, found three ways.** Claude scans
`<cwd>/.claude/skills`, Codex `<cwd>/.agents/skills` — each a relative link to
`agents/skills/` — and opencode's `skills.paths` names it. Nothing goes in
`$CODEX_HOME/skills`, which every project shares. The set stays small: every
CLI puts the skill index in every turn, headless jobs included.

## Finding, installing, updating and signing in the CLIs

A Dock-launched app has launchd's PATH, so `discover.rs` tries
`OCULUS_{CLAUDE,CODEX,OPENCODE,ANTIGRAVITY}_BIN`, PATH, `well_known_dirs`, then
a login shell. Provider and installer-tool discovery share that executable lookup order.
Answers and the `--version` health probe are cached, failures
included (every picker asks); `harness_health`'s `recheck` drops both.

**Install** runs a literal vendor command (shown with Copy) only for routes
that land in `well_known_dirs`; no `sudo`, stdin on `/dev/null` so a question
fails instead of hanging, and the webview names a provider, never a command
string. macOS only.

**Update.** Settings → Agents checks each installed CLI against its newest
published version on every visit (`harness_updates`). The binary's
canonical path gives its source: `Caskroom`/`Cellar` or the Homebrew prefix
is Homebrew, a `node_modules` under `~/.bun` is bun, any other
`node_modules` is npm, anything else updates itself (the vendor script or
native installer). Source picks both the command and the endpoint, all free
and unauthenticated:

| Source | Command | Newest version from |
| --- | --- | --- |
| Homebrew | `brew upgrade --cask claude-code` / `--cask codex` / `brew upgrade anomalyco/tap/opencode` | `formulae.brew.sh/api/cask/<name>.json`; opencode's tap isn't served there, so npm's `opencode-ai` |
| npm | `npm install -g <pkg>@latest` (`@anthropic-ai/claude-code`, `@openai/codex`, `opencode-ai`) | `registry.npmjs.org/<pkg>/latest` |
| bun (opencode only) | `bun install -g opencode-ai@latest` | npm's `opencode-ai` |
| Self-managed | `'<binary path>' update` (`upgrade` for opencode) | Claude: `downloads.claude.ai/claude-code-releases/<channel>` (`stable` when `~/.claude/settings.json` sets `autoUpdatesChannel`, else `latest`); Codex, opencode: npm; Antigravity: its updater's `manifests/darwin_<arch>.json` |

Any other pairing (Antigravity outside its script, Claude under bun) falls
back to the CLI's own subcommand and is compared against what that installs.
A brew install compares against brew because the cask can trail npm by a
release, and comparing it to npm would offer an update that changes nothing.
Versions compare by their numeric dot parts (a `-`/`+` suffix is ignored);
only a strictly newer one offers *Update*, and a failed check offers none.
Registry answers are cached for 6 hours (a failure for 10 minutes); Recheck
drops them with `discover::forget`.

Updates run one at a time across all four CLIs — brew and npm hold global
locks — through the install runner, so the same login shell, `/dev/null`
stdin and `sudo` refusal apply, with the literal command behind *Output*
when one fails. The webview names only a provider. A running chat keeps the
binary it started with until its CLI process restarts — the process shapes
are in the table under [Four dialects](#four-dialects-become-one-event-stream).

**Sign-in.** `signin::is_auth_failure` (via `HarnessEvent::error_for`) matches
whole clauses only — a false positive sends a student to re-authenticate over
an unrelated error — and marks the error row's `meta` for a sign-in card.
`ProviderInfo.signIn` is `"code"` (`claude auth login` blocks on a pasted
code), `"callback"` (`codex login` finishes on its own listener) or `null`
(opencode signs in [per provider](#opencode-providers-go-through-its-own-server);
`agy` only in a terminal). No deadline is imposed on a sign-in.

## The timeline never re-renders per token

`harnessStore` buffers deltas and folds them every 48ms (a row-committing
event flushes first, so text never lands after its row); committed rows are
memoised and only `LiveTail` and the one tool row still receiving output read
the store per delta; stick-to-bottom (`useStickToBottom`) is a
`ResizeObserver`, not a `scrollTo` per delta. On a 38-row thread that took 400
deltas from 402 commits / 6.2s of render to 34 / 55ms. `items` is keyed by
thread, so each Chat tab and the lecture dock hold their own. Retry links retain
their original question row object, and work bundles compare their constituent
row identities; appending a row leaves earlier rows memoised. `LiveTail` reads
only running, thinking and assistant text, so tool output updates its tool row.

The model picker keeps a memo boundary around its catalogue: the provider list
and selection callbacks have stable identities, so composer draft keystrokes
do not rebuild model rows. Catalogue, provider, model and reasoning changes
still update the picker.

- **Metadata is parsed once per row object** in `app/src/lib/harness.ts`,
  shared by tool, error, permission and lecture-moment readers. `parseItemMeta`
  caches JSON by the immutable row object; a tool update replaces that object
  and gets a fresh parse. Malformed or non-object metadata draws an ordinary
  row with no permission action; unknown tool kinds have no icon.
- **A row decides what truncates** (`RowShell` in `WorkRow.tsx`) — it draws at
  760px and in the dock's 300px. Both scrollers carry `overflow-x-hidden`:
  `overflow-y: auto` makes x `auto` too, and one wide row slides the thread.
- **A Read of a picture expands to the picture** (`ReadPicture` in
  `WorkRow.tsx`), click for the lightbox. It loads over the asset protocol, so
  only `courses/`, `lectures/` and `agents/` under the data dir show; any other
  path fails to load and the row falls back to its args.
- **A citation opens the file at the cited spot.** One grammar
  (`app/src/lib/citations.ts`) reads an inline code span, a link's href or a
  bare path in prose: `courses/…`, `../courses/…`, `agents/…`, `lectures/…`,
  absolute, agent-cwd `./x`, plus `:97-120` / `#L97-L120` / `#page=12`. It is
  shape-only and decodes first (the data dir has a space); a course-relative
  path or bare filename is a *tail*, rendered as code until one cached
  `LIKE '%/<tail>'` finds a unique row. A line of a parsed `.md` maps to its
  PDF page because the `.md` is the `.pages.json` pages joined with `\n\n`;
  `openCitation` opens the file there in the side panel and `PDFViewer` marks
  the line's text in the text layer (`citation-hit`). Unmatched schemeless
  links render as text.
- **What an agent makes, it shows.** The brief sends pictures and HTML pages
  to `agents/outputs/` and has `![alt](outputs/x.html)` embed them;
  `OutputEmbed.tsx` draws a picture inline and a page in an
  `allow-scripts`-only sandboxed `srcdoc` iframe (never `allow-same-origin`,
  which would hand it the app's origin and IPC), with an injected `<base>`
  built by hand — `convertFileSrc` encodes `/` — and a height reporter the
  parent accepts only from its own frame.
- **Codex web search** completes with no `status` — absent means it ran — and
  `ToolFinished`'s optional `title` fills in the query `item/started` lacked.
- **Copy/drag out is markdown** (`selectionMarkdown.ts`).
- **Maths**: the brief asks for `$…$`/`$$…$$`; `normalizeMath` rewrites
  `\(…\)`/`\[…\]`, which CommonMark eats before remark-math.
  The brief also bans a bare `|` in maths inside a table (the table splits the
  cell first); agents use `\lvert`/`\rvert`.
- **Usage is a ring of context only** (`UsageMeter.tsx`); spend is hidden, as
  subscriptions aren't billed per token. Claude context is the last
  `assistant` usage, since `result.usage` sums the turn. Codex windows are read
  on server start and page open (`harness_refresh_rate_limits`, which never
  starts a server).

**Names cost a turn outside the thread.** No CLI emits a title, so
`Harness::name_thread` runs one on the `threadNaming` job's model;
`store::claim_naming` flips `title_generated` atomically so only one turn pays
and a failure never retries; `clean_title` rejects prose.

## One-off turns

Naming, the editor's suggestions and the lecture-end job share
`Harness::one_off`: a session outside any thread, no rows, no raw log, closed
(an opencode session deleted) after its turn. Its brief is Claude's
`--append-system-prompt`, Codex's `developerInstructions` or the head of
`agy`'s first message; opencode takes it as a hidden agent's prompt
(`oculus-namer`, `oculus-writer`, `oculus-lecture-end` in
`agents/opencode.json`). Claude runs with `--tools ""`, no skills, no MCP and
no persisted session, and Codex's thread is `ephemeral`; the opencode agents
end on a `"*": "deny"`. Codex and `agy` cannot drop their tools, so the brief
forbids them.

`Harness::one_turn` is the plain shape — prompt in, reply text out, no
timeout — for a job whose prompt carries everything inline. The app passes
its own harness, so Codex and opencode reuse the running server; the CLI makes
one per command and shuts it down after. Naming keeps its own 90-second
timeout.

**Suggestions** (`document_suggest`, the `documentSuggestions` job) are one
short turn per pause in typing. The prompt carries the note's path, up to
4000 characters before the caret and 1500 after, and what the character before
the caret means for spacing; a caret inside a word costs no turn. `clean`
strips quotes, fences, labels and echoes of the text either side, and keeps one
line of at most 30 words. The streamed deltas are read, not the committed
message, because Claude's trims the leading space that says "new word".

- **Only the newest request is wanted.** A new `request_id` (or
  `document_suggest_cancel`) cancels the turn in flight — Claude and `agy` are
  killed, Codex and opencode interrupted — and the stopped call answers `""`.
  The cancel is repeated after the prompt is written, since a server ignores
  an interrupt for a turn it has not started. An older id arriving late answers
  `""` without a turn.
- **One warm process.** Claude and `agy` read the prompt off stdin, so after
  each request the next one's process is spawned and left waiting; a different
  provider, model or level discards it. Codex and opencode only open a
  thread/session on their running server. The first suggestion is cold.
- No timeout: a provider error rejects with its message.

## Each Chat tab owns its conversation, in its route

`ChatPage` reads its thread from `/chat?t=<id>&n=<title>` (`chatHref` in
`app/src/lib/harness.ts`), never from the store, so every tab shows its own and
back walks the threads that tab has shown (opening pushes; a rename or the
composer becoming a thread replaces). `n` is there because `tabInfo` titles a
tab from its path alone; it is written once the thread is known and again when
the name lands.

- The store keeps only a count per thread (`holds`, via `hold` / `unhold`), so
  the dock's `release` cannot empty rows a Chat tab is reading. `hold` borrows
  the outgoing thread's rows until the read lands, or every switch flashes an
  empty timeline.
- A route naming a thread missing from the list walks back to bare `/chat` —
  only after the list has loaded, or a restored tab drops its thread.
- The conversations column (`ThreadList`) is a `SideNav`, the subject page's
  column ([ui.md](./ui.md)): New thread on top, lit while no thread is open,
  then threads grouped by subject. It resizes, and ⌘⌥B or the toggle in the
  page header beside the title (`ChatPage` owns the panel state) folds it
  away entirely. The chord acts only in the tab in front, since the subject
  column answers it too. Its resize handle goes with it, so it can't clash with a
  side panel's; the toggle or ⌘⌥B brings it back.
- Bare `/chat` is a new thread: the composer docked at the bottom as in a
  thread, and above it the three latest threads (`RecentThreads`) — provider
  mark, title, a chip naming its subject or General, age, and the last reply
  flattened to one muted line (`getThreadPreviews`, one read for the list).
  The list has its own scroller, since a thread's follows its bottom.

Deleting a thread drops its retained live output, view holds and context-drift
marker as well as rows and queue, so removed turns cannot leave the sidebar busy.

## One turn at a time

A message sent mid-turn waits in `Queue` until the turn closes. Left to the
CLIs, Claude leaves an idle gap between turns, Codex folds it into the running
turn (the first answer can vanish), and opencode's v1 API has no queue. A
queued message has no row, so it can be edited or removed; `harness_queued`
restores it after a reload.

The queue drains only if every accepted message gets exactly one
`TurnFinished`: Claude's `expecting` flag closes a turn whose process died
early, and Codex takes the turn id from the `turn/start` reply, not the later
`turn/started`.

**Stop** interrupts and hands queued text back to the composer
(`harness_interrupt`); `restore` is `ChatPage`'s own state, since a store slot
would type it into every Chat tab. Codex never completes an interrupted message, so the
bridge commits the streamed text itself; Claude's closing `result` claims an
error, so `terminal_reason: "aborted_streaming"` decides and its zeroed usage
is skipped.

## Going back rewinds the agent too

Edit, Retry and Rewind truncate at a row (`store::truncate_from`, announced as
`Rewound`); Edit/Retry then send (`harness_edit_resend`), Rewind returns the
words to the composer (`harness_rewind`). No branching; idle threads only; not
offered where `ProviderInfo.rewind` is false (Antigravity). The provider
rewinds first, resuming an exited session without a turn:

| | Call | Names |
| --- | --- | --- |
| Claude | `control_request` / `rewind_conversation` | the user message's uuid, plus the newest question's as `last_seen_user_message_uuid` (without it the CLI refuses past any later turn: `stale_target`) |
| Codex | `thread/revert` | the turn to revert before |
| opencode | `POST /session/{id}/revert` | the message to drop, inclusive; lands on the next prompt |

The id is stored on the question row as `anchor` (from `TurnAnchor`) when the
turn goes out: Codex's `turn/start` reply, opencode's first user
`message.updated`. Claude never echoes it, so `anchor_for` walks
`~/.claude/projects/<slug>/<session>.jsonl` up from the first answer (slug =
cwd with every non-alphanumeric as `-`, since `memory_paths` is null with
auto-memory off). With no anchor the rows go anyway and
`Rewound { context: false }` warns that the agent's context kept them. Files
are never restored.

## The model picker

One control for agent, model and level (`ModelPicker.tsx`).
`useProviderModels` names no provider — each `PROVIDERS` entry owns its
catalogue, and marks are a `Record<Provider, …>` so a new provider won't
compile without one.

- **Catalogues are the CLIs' own**, and every turn names model and level: Codex `model/list`; Claude the
  `initialize` response of a throwaway `claude -p` (no API call), cached on the
  binary's path and mtime and dropped on sign-in. A mounted picker re-asks when
  its CLI's path or version changes (an update from Settings → Agents). Aliases are stored as the
  concrete model, since an alias changes meaning next release.
- **Levels belong to the model.** Haiku declares none (no `--effort`);
  opencode's are variants; `agy` bakes them into the slug (`parse_models`
  splits, `model_slug` rejoins). `sortReasoning` orders by
  `REASONING_LABELS`; `default_variant` asks for `high` and steps down.
- Claude and Codex bind the level at session start, so a change respawns the
  process or restarts the Codex thread.

## opencode providers go through its own server

`opencode serve` exposes `/provider`, `/provider/auth`, `PUT`/`DELETE
/auth/{id}` and the OAuth pair, so Settings drives credentials over the
bridge's connection into opencode's own store (shared with the student's
terminal). A method spec is data rendered by one dialog; providers declaring
nothing take an API key. Opening the opencode page starts it; Settings opens
on another page, so opening Settings alone does not. The page is two tables,
Providers and Models, the second one row per model with opencode's own price,
limits and capabilities (`ModelFacts`, a free catalogue read; a missing figure
draws "—", never $0).

- **`connected` goes stale** after `PUT /auth` until `POST /instance/dispose`,
  which is skipped while a turn is open.
- **A key is never held**: field → `PUT /auth/{id}` only. Errors are redacted,
  and `/provider` echoes the key in a field `scrub` doesn't know, so the rows
  Settings draws never read it (a test asserts it).

## No model is ever probed

Nothing in Settings may spend a student's credits to populate a menu. A test
turn per model is **two** billed requests (the turn plus opencode's session
title) at up to ~8.6K input tokens each, across ~300 OpenRouter models, and
almost everything it would learn is free:

- *Signed in? Exists?* `/config/providers` lists only usable providers.
- *Can it call a tool?* `capabilities.toolcall` — 69 of OpenRouter's 369
  cannot, and such a model answers confidently from nothing.
- *Zen?* Every model under provider id `opencode` is refused over HTTP; `isZen`
  reads the id.

An opencode model is offered when `unusableReason` finds nothing (absent flags
mean capable), it is not Zen, and neither it nor its provider is hidden (one
`settings` value, `opencode_catalogue`). `filterOffered` is the composer's
only path and the opencode page counts through it. A stale key or a
provider-verified model (Meta's Muse Spark) fails once in the timeline.

## Per-job models

Chaptering, the lecture end, thread naming and document suggestions each
name their agent, model and level in a Settings → Jobs row
using `ModelPicker`. The registry is one
`settings` value, `job_models`, read by `jobs.rs` and written by `db.ts`; both
carry the defaults and must agree (a new job is a `Job` variant plus entries in
`JOBS` and `DEFAULT_JOB_MODELS`). A bad value costs the configuration, not the
run; CLI flags override for one run.

## Memory

Nothing carries between threads but `agents/` — `TASTE.md`, `memories/`,
`memories/<CODE>/` (`app/src-tauri/src/agents.rs`) — and the app never reads it
into a prompt. The contract is in the brief, not a skill, because nobody asks
for a memory and a skill would never trigger; it says read first, then write
on four triggers (a brief naming only a location left 9 memories over 51
threads). `oculus memory` (`app/src-tauri/src/memory.rs`, [cli.md](./cli.md))
writes file and index in one call, since the index line is the write an agent
skips.

## Subject scope and `@`

**A subject, or General** (`SubjectSelect.tsx`), locked after the first
message because every CLI binds appended instructions at session start. It is
not a sandbox; it appends the course folder and `agents/memories/<CODE>/`.

**`@` inserts a library path**, not content — the path is what `oculus read`
takes. Words are ANDed against filenames (`safe_filename` turns spaces into
`_`); the token is bounded (no space after `@`, four words, 60 characters, a
backtick ends it) and the menu opens only on a match.

**A mention is a chip on screen and a path on the wire** (`FileChip`, built
from the path alone), which is why `MentionInput.tsx` is a contenteditable:
the DOM is the source of truth while typing, React re-renders only on
structural changes (bumping `key`), and `revealCaret` scrolls the box because
a contenteditable follows its caret only for edits it made. The menu anchors
on a range over the `@` token, since a collapsed range's rect can be zeros in
WebKit.
A note's or task body's `@` (the note editor's own completion) writes the
same backticked path and draws the same chip
([editor.md](./editor.md#pictures-mentions-and-citations-resolve-to-library-files)).

## Attachments are files in `agents/attachments/`

The page and lecture dock share `SendControls` for send/queue/stop controls
and `useAttachments.prepare` for message assembly. Preparation writes pictures
before clearing the composer; a failed write keeps the draft and attachments.
A message reads: the typed text, each pasted text's block, then the pictures'
paths, blank-line separated.

A CLI reads files, so a picture becomes one — in `agents/`, the one folder
every CLI reads and writes. `harness_attach_image` (clipboard bytes) and
`harness_attach_file` (dropped path) write it on send (a task body, having no
send, writes on arrival) and answer `./attachments/<name>`, appended fenced.
The claimed filename never reaches disk: bytes are sniffed, the stem is a
timestamp, non-images are refused, 20 MB cap. `assetProtocol.scope` in
`app/src-tauri/tauri.conf.json` must name `agents/` or every picture (and
every page an agent made) draws broken; a dropped file is scoped by Tauri on
delivery.

### Pasted text is a card, sent inline

A plain-text paste of `LONG_PASTE` (1000 characters or 15 lines) or more
becomes a card in `AttachmentStrip` instead of text in the box; a clipboard
picture still wins. Cards are kept in `draftStore` under the draft key (a
second map, `oculus.chatPastes`), so they outlive a thread switch and a
relaunch like typed words; pictures stay in memory. Clicking one opens
`PastedTextEditor`, the text as a markdown document on `NoteField` (read
through its `onChange`, so the buttons never wait on a blur): Done keeps the
edits (as do Escape and the overlay), Remove drops the card, and Put in
message appends the text to the box after a blank line and drops the card.
The editor loads on demand, since it brings CodeMirror to a page that
otherwise has none.

No file is written. On send each card becomes a block in the message —
`<pasted_text>`, a newline, the text, a newline, `</pasted_text>` —
built by `withPastedText` and read back by `splitPastedText`
(`app/src/lib/attachments.ts`), which round-trip exactly: a block opens at the
start or after a blank line and ends at the first closing tag alone on its
line, and a text line that is exactly that tag is sent with a backslash in
front. The question bubble lifts the blocks out as the same cards, in the
row above the words ahead of the pictures, and only the remaining prose is
measured and folded; a sent card opens `PastedTextViewer` (rendered markdown
and Copy), and a copied selection across one takes its block (`data-md`).
Words handed back by Stop or a rewind go through `useAttachments.takeBack`,
so their blocks return as cards and only the rest lands in the box. The
bubble's own edit box still shows the raw blocks, as it shows picture paths.

### File drop

Tauri's handler sits in front of the webview, so a Finder drop never reaches
React; `useFileDrop` hit-tests Tauri's events against element rects. The Chat
page's conversation column and the lecture dock's Chat panel are the drop
targets, not just the box: each passes its own ref as the composer's `dropRef`
and draws `DropOverlay` (`app/src/components/ui/DropOverlay.tsx`) from
`onDropping`.

- **The position is in points despite the `PhysicalPosition` type.** wry
  never applies the backing scale, so dividing by `devicePixelRatio` halves
  every point on retina and the drop hits nothing. The hook measures the scale
  (viewport width over `innerSize`/`scaleFactor`).
- **Only a webview listener fires.** A drop is a window event only for
  `WebviewKind::WindowContent`; tauri's `unstable` feature (for
  `Window::add_child`, `app/src-tauri/src/browser.rs`) makes the main webview
  `WindowChild`, and `filter_target` never matches a `Window` listener to a
  `Webview` emit. The window listener subscribes fine and is never called.
- **A hidden tab's composer sits at the same coordinates** (panes hide with
  `visibility`), so the hook skips computed `visibility: hidden`.

## A lecture thread carries the moment

`harness_threads.lecture_id` scopes a thread to one recording, fixed at
creation; Rust reads the subject off the lecture row. `instructions()` appends
`../lectures/<id>/`, the transcript if present, the recording's date (Echo360
titles are timetable codes), the VTT's shape, the deck-finding recipe and the
chapter list inline — rebuilt on every send and before any spawn.

- **The moment rides the prompt, never the message.** `buildMoment` in
  `LecturePlayer.tsx` goes out as `SendOptions.context`; the row stores the
  typed text plus `at`.
- **`lecture_grab_frames` grabs every stream** — either can hold the teaching —
  at `LIVE_GRAB_WIDTH` (1536), not chaptering's 768, since a whiteboard is
  unreadable at 768. A failed frame drops its line; the message still sends.
- **The dock** reuses `Timeline` and owns its thread id, as each Chat tab does, remembered per lecture in a module-level `Map` because the
  player unmounts on tab switch. The playhead reaches the chip as a stable ref,
  so nothing re-renders the memoised transcript. Lecture docks and Chat tabs
  use counted `hold`/`unhold` ownership of cached rows; closing one view does
  not release a thread still displayed by another.

## Gotchas

- Never pass `agy` `--dangerously-skip-permissions` — its file tools then write outside the library.
- Never put an `Edit` deny on `oculus.db*` in Claude's settings — it cancels the write grant and every board write fails readonly.
- Never grant the library folder instead of the three database files — it exposes the session cookie and Ed token.
- Never let Settings make a billed call, or start opencode anywhere but its own page — see [No model is ever probed](#no-model-is-ever-probed).
- Read opencode models from `/config/providers`, never `/api/model` — the latter lists the instance's providers, not the signed-in ones.
- Every opencode call carries `?directory=<agents>` except `/auth/{id}` — unscoped, it binds to the server's launch cwd and another instance.
- Never drive opencode's v2 API (`/api/*`) — on 1.18.31 a prompt on an `auth.json` provider fails with no event.
- Resend agent and model on every opencode prompt — creation values are decorative, and a bare prompt runs the stock `build` agent; check `oculus` is in `GET /agent` before creating a session.
- `opencode serve --port 0` means "prefer 4096" — parse the real port from stdout.
- opencode's `session.idle` arrives before the partial answer during an abort; a 30s drain watchdog fails a turn the server went silent on.
- `opencode::sweep` kills strays at startup by `SERVE_ARGS` *and* `ppid == 1` — drop either and it kills a living app's or a hand-run server.
- Codex: `thread/start` takes `sandbox` (string), `turn/start` takes `sandboxPolicy` (object); effort is thread config; a resumed thread replays stale usage first.
- Claude: take tool inputs from the `assistant` line, not `input_json_delta`s; `content_block_start` can carry text the deltas don't repeat.
- Don't cache sign-in status in Rust — a stale "signed out" after a sign-in is the wrong answer that matters.
- The ⌘⌥B column fold tests `e.code` — ⌥B types `∫` — and `AppLayout`'s ⌘B returns early on ⌥, or one chord closes both panels.
- Never give an agent's HTML embed `allow-same-origin` — the page would run with the app's origin and reach the Tauri IPC.
- Thread-row delete commits on `mousedown` — WebKit blurs the focused confirm on press and unmounts it before `click`.
- The thread list reserves its gutter with `overflow-y: scroll`; `scrollbar-gutter: stable` is a no-op in this WebKit.
