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
| Manager, queue, `run_once`, per-thread `instructions` | `app/src-tauri/src/harness/manager/` (`queue.rs`, `one_off.rs`, `instructions.rs`) |
| Tauri commands (setup, models, turns, suggestions) | `app/src-tauri/src/harness/app/` |
| The event enum and tool classification (`classify`) | `app/src-tauri/src/harness/event/mod.rs`, `app/src-tauri/src/harness/event/classify.rs` |
| Bridges: Claude (`claude -p`), Codex (`app-server`), opencode (`serve`), Antigravity (`agy --print=`) | `app/src-tauri/src/harness/providers/claude/`, `app/src-tauri/src/harness/providers/codex/`, `app/src-tauri/src/harness/providers/opencode/`, `app/src-tauri/src/harness/providers/antigravity/` |
| Paths no agent may write; opencode's ruleset; agy's rules | `app/src-tauri/src/harness/protected.rs`, `app/src-tauri/templates/OPENCODE.template.json`, `app/src-tauri/src/harness/providers/antigravity/rules.rs` |
| Finding, installing, updating and signing in the CLIs | `app/src-tauri/src/harness/cli/discover/`, `app/src-tauri/src/harness/cli/install/`, `app/src-tauri/src/harness/cli/update/`, `app/src-tauri/src/harness/cli/signin/` |
| …their frontend | `app/src/hooks/agents/useBridgeHealth.ts`, `app/src/hooks/agents/useSignInStatus.ts`, `app/src/components/settings/agents/InstallAgentDialog.tsx`, `app/src/components/settings/agents/UpdateAgents.tsx`, `app/src/components/harness/SignInDialog.tsx`, `app/src/pages/settings/integrations/AgentsPage.tsx` |
| Hidden native processes, Windows job ownership and npm shim resolution | `app/src-tauri/src/runtime/platform/` |
| Process pipes, JSON lines and terminal turn failures shared by the bridges | `app/src-tauri/src/harness/child.rs` |
| Thread and timeline rows | `app/src-tauri/src/harness/store/` |
| The brief appended to each prompt; `AGENTS.md`; skills | `app/src-tauri/templates/HARNESS.template.md`, `app/src-tauri/templates/AGENTS.template.md`, `app/src-tauri/templates/skills/`, `app/src-tauri/src/agents/mod.rs` |
| Recorded provider output the bridge tests replay | `app/src-tauri/fixtures/harness/` |
| Provider table (`PROVIDERS`), platform capabilities, types, commands | `app/src/lib/harness/registry.ts`, `app/src/lib/harness/availability.ts`, `app/src/lib/harness/` |
| Live state and event folding | `app/src/stores/chat/harnessStore.ts`, `app/src/hooks/backend/useBackendEvents.ts` |
| Row titles: a reasoning block's "Thought for 12s" and latest line, an `oculus …` command's "Saved a memory" | `app/src/lib/harness/thought.ts`, `app/src/lib/harness/oculusCli.ts`, `app/src-tauri/src/harness/app/thought_clock.rs` |
| Page, thread list, recent list, timeline, rows, composer | `app/src/pages/tools/ChatPage.tsx`, `app/src/components/harness/` (`threads/`, `timeline/`, `composer/`; recent list: `threads/RecentThreads.tsx`) |
| Unsent text per thread (and per new-thread box), kept across switches and relaunches | `app/src/stores/chat/draftStore.ts` |
| Proposal dispatch context and confirmed jobs | `app/src-tauri/src/harness/app/proposal_context.rs`, `app/src-tauri/src/harness/app/proposal_jobs/` |
| Interactive reply fences, study widgets and their persisted progress | `app/src/components/markdown/openui/`, `app/src/stores/chat/widgetStore.ts` |
| Model picker and its catalogue hook | `app/src/components/harness/composer/ModelPicker.tsx`, `app/src/hooks/agents/useProviderModels.ts` |
| Settings → opencode: the provider and model tables, the connect flow, the offered-model gate | `app/src/pages/settings/integrations/OpencodePage.tsx`, `app/src/components/settings/agents/opencode/`, `app/src/components/settings/agents/OpencodeConnectDialog.tsx`, `app/src/lib/harness/opencodeCatalogue.ts` |
| Per-job models | `app/src-tauri/src/harness/jobs.rs`, `JOBS` in `app/src/lib/db/jobModels.ts`, `app/src/components/settings/jobs/`, `app/src/hooks/agents/useJobAudit.ts` |
| Subagents (Studio's research and scene helpers) | `app/src-tauri/src/harness/studio_agents.rs`, `app/src-tauri/templates/STUDIO_*.template.md`, `oculus studio agents` (`app/src-tauri/src/bin/oculus/commands/studio/agents.rs`) |
| One-off turns (`Harness::one_off`, `Harness::one_turn`): thread names, the editor's inline suggestions (`document_suggest`), where a lecture ends | `app/src-tauri/src/harness/manager/one_off.rs`, `app/src-tauri/src/harness/app/suggest.rs`, `app/src-tauri/src/harness/suggest/`, `app/src-tauri/src/lectures/lecture_end/` |
| `@` menu and mention input | `app/src/components/harness/mentions/useMentionMenu.ts`, `app/src/components/harness/mentions/MentionInput.tsx`, `app/src/components/markdown/FileChip.tsx` |
| Studio's thread column and New video box, which use `Composer`'s `leading` (chips after the subject's) and `placeholder` | `app/src/components/studio/thread/ThreadColumn.tsx`, `app/src/components/studio/new/NewVideo.tsx` ([studio-page.md](./studio-page.md)) |
| Voice dictation, microphone capture and cleanup job | `app/src/components/harness/dictation/`, `app/src/lib/dictation/recording.ts`, `app/src-tauri/src/harness/app/dictation.rs` |
| Send, queue and stop controls shared by both composers | `app/src/components/harness/composer/SendControls.tsx` |
| Pictures pasted or dropped into a composer | `app/src-tauri/src/harness/attach.rs`, `app/src/hooks/agents/useAttachments.ts`, `app/src/hooks/gestures/useFileDrop.ts` |
| Long pastes held as text cards: format, card, editor and viewer | `app/src/lib/harness/attachments.ts`, `app/src/components/harness/attachments/PastedText.tsx`, `app/src/components/harness/attachments/PastedTextEditor.tsx` |
| Library paths in rows and prose; copy as markdown | `app/src/lib/files/openFile.ts`, `app/src/lib/markdown/selection.ts` |
| Timeline window, off-screen layout, scroll anchoring, the expand protocol | `app/src/components/harness/timeline/useRowWindow.ts`, `app/src/components/harness/timeline/rowWindow.ts`, `app/src/components/harness/timeline/useRowVisibility.ts`, `app/src/components/harness/timeline/RetainedRow.tsx`, `app/src/hooks/ui/useStickToBottom.ts`, `app/src/lib/ui/followTimeline.ts`, `app/src/lib/ui/scrollAnchor.ts`, `app/src/lib/ui/expandable.ts` |
| The lecture dock's chat | `app/src/components/lectures/LectureChatPanel.tsx`, `app/src/components/lectures/LectureChatComposer.tsx`, `lecture_grab_frames` in `app/src-tauri/src/lectures/chapters/app.rs` |

Windows offers Codex and opencode; native Claude Code is disabled because its
Windows sandbox is unsupported. Settings shows “Needs WSL2 (coming soon)” for
Claude and model pickers omit it. `availability.ts` owns the frontend platform
capabilities. Antigravity has no in-app Windows install or update route; its
Settings row directs installation and updates to a terminal.

## Four dialects become one event stream

Every bridge translates into `HarnessEvent` (session, user message, turn,
unprompted turn, text/reasoning deltas, tool started/output/finished, usage,
rate limits, error, exited). `ThreadTitled`, `Queued`/`Unqueued`, `Rewound` and
`TurnAnchor` and `Proposal` have no provider behind them but ride the same path so they are
persisted the same way. `classify` in `event/classify.rs` is the one tool table.

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
- **Nothing survives a restart as running.** `harness::store::reconcile` runs
  at startup: threads left `running` go `idle`, and tool rows whose `meta.ok`
  is still null are closed as failed with "the session ended before this
  finished" — otherwise the row draws as running forever.
- **Settings storage is shared.** Job selections, rate limits and Antigravity
  approvals use `db::store::setting` / `db::store::set_setting`; each caller owns its
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
protected paths are one list in `protected.rs`: the library folders, the root files by pattern (the vault and its lock and temp files among them, so an agent cannot delete or truncate the sealed sessions) and the workspace folders. agy names the files instead.

- **Claude**: its sandbox with the cwd as the write root (plus the two holes
  below), `--add-dir` for
  reads (without it `ls ../courses` is refused), `Edit` denies on protected
  paths so `--add-dir` doesn't pull them into `acceptEdits`, and
  `--permission-prompts none`, since an unanswered prompt under `-p` hangs.
- **Codex**: `workspace-write` rooted at the cwd (plus the two holes below),
  `approvalPolicy: never`; any server request is declined.
- **opencode has no sandbox** — a rule list its runner checks. File tools are
  real (`edit` covers `write`/`edit`/`apply_patch`); `bash` is a glob over the
  command string, allow-listing `oculus …` and `ls …` — it stops `rm -rf`, not
  a redirect. Rule shapes: a leading `"*": "deny"` beats every later allow;
  the *last* rule decides whether the tool is offered at all, so an allow comes
  last; a path inside the session directory is written relative to it.
- **Antigravity**: [below](#antigravity-keeps-its-rules-in-the-students-global-settings).

**The database is the first hole, three files wide.** The CLI reads the board
and permits Studio metadata writes ([cli.md](./cli.md)), and a sandbox that opens
`oculus.db` but not `-wal` fails "attempt to write a readonly database". So
`library::paths::db_write_paths` names `oculus.db`, `-wal`, `-shm`: `allowWrite` for
Claude; `writableRoots` on the turn's `sandboxPolicy` plus
`sandbox_workspace_write.writable_roots` in thread config for Codex. Files,
not the folder — it holds keyd's vault.

**Studio is the second hole: `<data>/studio/`.** Studio's videos and its
component and theme libraries live there ([studio.md](./studio.md#videos-live-in-the-data-directory)),
and it holds no secrets. `library::paths::agent_write_dirs` names it once;
the manager creates it before a spawn, and each CLI renders it: `allowWrite`
for Claude; a writable root for Codex (`codex::writable_roots`, which keeps
only paths that exist, the database's absent sidecars included);
`"{{LIBRARY}}/studio/**": "allow"` in opencode's template, after every
library deny, since opencode's `*` crosses `/` and the last matching rule
wins, so `{{LIBRARY}}/*.json` alone would refuse a video's `video.json`;
`write_file(<data>/studio)` for agy. Each bridge's tests pin the grant and
that no deny covers a video's files. A library file is meant to change only
through `oculus studio components|themes promote`, which keeps the old text
in `.history/`; the sandbox cannot tell the CLI from a direct edit, so that
is a skill rule, a speed bump like the `sqlite3` deny, not a wall.

**The CLI stays the only door.** Claude merges `Edit(...)` denies into its
sandbox's `denyWrite`, so an `oculus.db*` deny would cancel the grant; instead
`sqlite3` is denied by name (`Bash(sqlite3:*)`) — a speed bump, not a wall.
opencode keeps its `oculus.db*` deny. `Bash(oculus:*)` is allowed by name
because Claude's sandbox auto-allows only Bash it can statically vouch for,
and one refusal (a multi-line `--brief`) auto-denies the rest of the session.
`useBackendEvents` refreshes planning data after a finished `oculus
project|task` call. `is_oculus_cli` only checks the first words, and
`cd … && oculus task list` reads as Bash. Proposal commands also notify
open boards after their committed writes.

**The child's environment is edited.** `ANTHROPIC_API_KEY`/`OPENAI_API_KEY`
are stripped so a shell key cannot move a subscription onto API billing, and so
opencode (which reads both) shows the same catalogue from a terminal as from
the Dock. `CLAUDECODE`/`CLAUDE_CODE_ENTRYPOINT` are stripped because the CLI
refuses to nest. The `oculus` binary's directory goes first on PATH, so a
stale dev binary shows up as `unrecognized subcommand`
([development.md](./development.md)). Claude's auto-memory is off. Every child also gets `OCULUS_AGENT=1`:
student-owned writes and long jobs are refused by the CLI with the proposal
component to use. Sign-in and keyd commands are refused outright. This applies
to one-off jobs and catalogue/sign-in subprocesses too; they invoke provider
CLIs, not gated Oculus write verbs. The app's native job paths do not use this
environment. Codex's shared app-server receives the environment at spawn and
sets `shell_environment_policy.set.OCULUS_AGENT` on each thread; opencode's
shared server inherits it at spawn for its shell children.

**Each sandbox lets `oculus` reach `oculus-keyd` its own way.** `oculus search`
and the other keyed commands connect to keyd's Unix socket from inside the
agent's sandbox
([architecture.md](./architecture.md#oculus-keyd-is-the-only-process-meant-to-read-its-key)).
The socket is `keyd_core::paths::socket(library)`, `keyd.sock` in the data
dir: the path `Client::at` dials and keyd's launchd registration binds, and
the harness takes its library from the same `paths::data_dir()` as the CLI.
No deny list covers it. What each CLI needs was measured on the CLI itself:

| CLI | What the connect needs | Where |
| --- | --- | --- |
| Claude | `sandbox.network.allowUnixSockets` naming that one socket, never `allowAllUnixSockets`; without it the connect fails with EPERM | `settings_json`, `claude.rs` |
| Codex | the `workspace-write` policy's network access, since Codex has no per-socket grant; without it the connect is EPERM | `network_access` in thread config and `networkAccess` on every turn's `sandboxPolicy`, `codex.rs` |
| Antigravity | nothing beyond `--sandbox`; its rules deny only writes to listed paths, and `command(oculus)` is allowed | `launch_args`, `antigravity_rules.rs` |
| opencode | nothing: it has no sandbox, and `bash` allows `oculus *` | `OPENCODE.template.json` |

Each bridge's tests pin its row, and that nothing it denies covers the socket.
The app is not involved: once keyd is installed, launchd starts it on the first
connect, so a chat agent's `oculus search` works with the app closed.

**The TTS server is the one network host a thread is granted.** `oculus
studio narrate` calls the server in Settings → Studio → Narration. At each spawn the
bridges read the `voice` row (`studio::voice::server_host`: the base URL's
host and port, or nothing without a row, a URL or a host; never a
wildcard), so a changed server reaches the next spawn. The Kokoro engine
needs no grant: it reads its packs from `<app data>/models/voice/` and
makes no request.

| CLI | Grant | What reaches the server |
| --- | --- | --- |
| Claude | `sandbox.network.allowedDomains: ["<host>:<port>"]` (omitted with no row; the CLI matches the port too); none on a one-off turn | A loopback server (`localhost`, `127.0.0.1`, `::1`) needs no entry on macOS: with `allowLocalBinding` a sandboxed command may connect to any localhost port, and the CLI sets `NO_PROXY` so loopback is dialled directly (Claude Code's sandboxing docs, "A command fails to reach a server on localhost"). Any other host is reachable only through the sandbox's filtering proxy (`HTTP(S)_PROXY`/`ALL_PROXY`), where the entry admits it; `oculus`'s TTS client (ureq 2) reads no proxy variable and dials directly, so a non-loopback server is not reached from a Claude thread |
| Codex | none needed: the `workspace-write` policy's `networkAccess` is already on (for keyd's socket) | Any host |
| opencode | none: no sandbox | Any host |
| Antigravity | `read_url(<host>)` in its rules, which its docs say joins the terminal sandbox's outbound allowlist | Not measured: its docs say sandboxed commands have no network by default and that `read_url` domains are added to the allowlist, and say nothing about localhost |

### Antigravity keeps its rules in the student's global settings

`--sandbox` bounds shell commands only, and `--dangerously-skip-permissions`
lets `write_to_file` write outside the library, so that flag is never passed.
`agy` 1.2.9 reads rules only from `~/.gemini/antigravity-cli/settings.json`,
so `providers/antigravity/rules.rs` merges Oculus's block there before every spawn (or
fails the spawn): writes on the three database files and `studio/`, the
`read_url` grant for the TTS server's host, the `oculus` binary's
real directory (the sandbox can't read through the `~/.local/bin` symlink),
read-only commands no flag turns into a write (`find`/`rg` are out — the
matcher is a word prefix), `sqlite3 <library>/oculus.db` denied per path
spelling (not `command(sqlite3)`, which bans it everywhere), and the student's
approvals. The entries are global — they apply in the student's own `agy` —
and `antigravity-rules.json` at the library root records which are Oculus's.
Generated file rules and suggested folder grants use `/` separators on every
platform; a suggested grant requires a fully absolute native path.

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
`agents/skills/` on Unix; Windows uses managed copies — and opencode's `skills.paths` names it. Nothing goes in
`$CODEX_HOME/skills`, which every project shares. The set stays small: every
CLI puts the skill index in every turn, headless jobs included.

**A skill is a folder of files.** `agents::docs::SKILLS` lists each skill's
name and its files, `SKILL.md` first; `oculus-studio` also ships
`reference.md`, `build.md` and `studio.d.ts` (`app/studio-core/studio.d.ts`),
which `SKILL.md` tells the agent to open when it needs them, so only the
index line costs a turn. Every write rewrites each file and deletes anything else
in the skill's folder: the folders are app-owned and agents may not write
them, so a stray file could only be a stale instruction.

Windows children start suspended, enter a Job Object with kill-on-close, then
resume. Stopping a turn or closing the app terminates descendants too; the Unix
startup stray sweep is disabled on Windows. Native commands use one helper to
suppress console windows.

## Finding, installing, updating and signing in the CLIs

A Dock-launched app has launchd's PATH, so `cli/discover/` tries
`OCULUS_{CLAUDE,CODEX,OPENCODE,ANTIGRAVITY}_BIN`, PATH, `well_known_dirs`, then
a login shell on Unix. Windows adds `%APPDATA%/npm`, `%LOCALAPPDATA%/Programs`
and the Node installation directory, searches PATHEXT with `.exe` first, and
skips the login shell. Provider and installer-tool discovery share that executable lookup order.
Supported npm `.cmd`/`.bat` shims are resolved from the package manifest and run
with `node`, so agent prompts and settings never pass through batch quoting.
Answers and the `--version` health probe are cached, failures
included (every picker asks); `harness_health`'s `recheck` drops both.

**Install** runs a literal vendor command (shown with Copy) only for routes
that land in `well_known_dirs`; no `sudo`, stdin on `/dev/null` so a question
fails instead of hanging, and the webview names a provider, never a command
string. Windows offers npm for Codex and npm/bun for opencode and runs the
package managers directly; self-update commands use noninteractive PowerShell.
Unix uses a login shell. Settings reads install offers before showing Install;
a null offer or an empty route list exposes no action. The opened dialog keeps
its offer while an installation finishes and health is rechecked.

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
`ProviderInfo.signIn` is `"code"` (`claude auth login` and `agy` block on a
pasted code), `"callback"` (`codex login` finishes on its own listener) or
`null` (opencode signs in [per provider](#opencode-providers-go-through-its-own-server)).
No deadline is imposed on a sign-in.

`agy` has no login subcommand and refuses a pipe, so `signin/agy/` runs bare
`agy` on a pseudo-terminal (`signin/pty.rs`: `openpty`, a 1000-column window
so the URL never wraps, `login_tty` for a session of its own). Its TUI's
redraws are cut into lines at cursor moves and each distinct line is sent
once per round. Enter answers the login-method menu (Google OAuth); `agy`
opens the authorize URL itself, so Rust does not open it again, and the URL
is read from its OSC 8 hyperlink. Google redirects to
`antigravity.google/oauth-callback`, a page that shows the code — there is no
loopback listener to finish on — and the pasted code is typed into the TUI.
A refused code shows `agy`'s sentence as the dialog's notice; Enter on
"Press any key to go back" and again on the menu starts a new round with a
new URL, so one run takes as many codes as the student pastes. An error
before any code ends the run instead of looping. Signed in, `agy` moves on
to a chat rather than exiting, so once a code is in and either its success
line shows or the screen has been still for 1.5 s, `agy models` is asked (at
most every 5 s); a yes — or no answer either way after the success line —
ends the run and kills `agy`'s process group, never before the check, so
`agy` has stored the token first. A code typed outside the code box is
refused, since in the menu or on the error screen the keys mean something
else. The live check of this route is the ignored test in
`signin/agy/smoke.rs`, which refuses to run without a scratch `HOME`.

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

- **Metadata is parsed once per row object** in `app/src/lib/harness/meta.ts`,
  shared by tool, error, permission and lecture-moment readers. `parseItemMeta`
  caches JSON by the immutable row object; a tool update replaces that object
  and gets a fresh parse. Malformed or non-object metadata draws an ordinary
  row with no permission action; unknown tool kinds have no icon.
- **A row decides what truncates** (`RowShell` in `WorkRow.tsx`) — it draws at
  760px and in the dock's 300px. Both scrollers carry `overflow-x-hidden`:
  `overflow-y: auto` makes x `auto` too, and one wide row slides the thread.
- **A reasoning row is words only.** Finished, it reads "Thought for 12s"
  and opens to the thought as muted markdown; while it streams, `LiveTail`
  shows a shimmering "Thinking" (`text-shimmer`) and the thought's latest
  readable sentence (`latestThought`). The duration is the row's `meta.ms`:
  opencode's part clock, else the consumer loop's `ThoughtClock` from the
  block's first delta. A block with no deltas, or a row from before, reads
  "Thought". A bundle of thoughts alone sums them.
- **An `oculus …` row says what it did** — "Saved a memory", "Searched the
  library for", "Moved 7 tasks to" — from `describeOculusCli`, which reads
  the shell line (chains counted, pipes and redirects dropped) and gives each
  command a glyph and a bundle clause ("saved 2 memories"). The command is
  still the opened row's detail; a running one shimmers its verb. An
  unrecognised command reads "Ran oculus" with its subcommand.
- **A Read of a picture expands to the picture** (`ReadPicture` in
  `WorkRow.tsx`), click for the lightbox. It loads over the asset protocol, so
  only `courses/`, `lectures/` and `agents/` under the data dir show; any other
  path fails to load and the row falls back to its args.
- **A citation opens the file at the cited spot.** One grammar
  (`app/src/lib/citations/`) reads an inline code span, a link's href or a
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
- **Copy/drag out is markdown** (`lib/markdown/selection.ts`).
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
`harness::store::claim_naming` flips `title_generated` atomically so only one turn pays
and a failure never retries; `clean_title` rejects prose.

## OpenUI fences draw interactive study widgets

`MD_COMPONENTS.pre` catches `openui` and `openui-lang` fences beside mermaid.
`OpenUIFence` waits 150 ms after the source stops changing before loading the
runtime. The library in `app/src/components/markdown/openui/library.ts` admits
Flashcards, Quiz, Steps, Explorer (sliders, values and charts), PageRef and Ask,
with Stack and Text for composition. Any parse error, unresolved reference,
incomplete source or runtime failure keeps the ordinary code fence visible.
Widgets render only after the runtime commits its initial state, so computed
values start with the declared defaults.

`remarkWidgets` numbers fences in syntax order. A committed reply supplies its
thread and row through `WidgetRow`; `widgetStore` keeps the runtime snapshot
under that thread/row/fence key in localStorage (`oculus.chatWidgets`), as
composer drafts do. Flashcard progress, quiz answers and scores, revealed
steps and slider bindings all use the runtime state. The saved program must
match before hydration; changed source starts fresh. Delete and rewind prune
the corresponding snapshots. A streaming tail has no committed row yet and
does not persist progress.

`Timeline` supplies `AssistantSend` from its follow-up action, so an Ask
button's `@ToAssistant` uses the same thread in Chat, Studio and the lecture
dock. Ask is hidden while running and outside a conversation. `PageRef` uses
the ordinary citation chips and file-opening path. The agent learns the fence
grammar from `app/src-tauri/templates/skills/oculus-ui/SKILL.md`, registered in
`app/src-tauri/src/agents/docs.rs` and named in the harness brief. Widget primitives follow the
[UI tokens](./ui.md#study-widgets-use-the-replys-primitives-and-semantic-tokens).

## The timeline mounts its newest rows and holds the view still

Opening a long thread mounts only its newest 30 rows (`PAGE` in
`app/src/components/harness/timeline/rowWindow.ts`); a 113-row thread with 35
maths-heavy replies mounts a page at a time.

- **The window is an item id, not a row index.** A row mounts from the row
  holding item `from` onwards; rows fold into bundles and append, and an
  index would drift with both. A bundle mounts whole. Appended rows never move
  the boundary; a rewind or edit that took its item re-opens on the newest
  page. The boundary is kept per thread, since a tab's `Timeline` outlives a
  thread switch.
- **A sentinel loads the page above.** A 1px row above the window is watched
  by an `IntersectionObserver` rooted at the nearest scrolling ancestor, with
  a viewport of top margin, so each page lands before the reader reaches it.
  The observer is re-hung per boundary, so a sentinel still in reach after a
  short page loads the next.
- **Following yields on any upward input.** `followTimeline` opens a thread at
  its latest row, then follows size changes only during an active reply while
  the reader is at the bottom (1px rounding tolerance). Upward wheel, touch
  or navigation-key input releases immediately, before native scrolling; any
  upward scroll movement also releases, including fractional pixels. Only a
  downward return to the bottom or the floating arrow re-arms following.
  `JumpToLatest` is shared by Chat, the lecture dock and Studio: an arrow over
  a nearly opaque, blurred surface. It appears after 16px of separation,
  stays at a fixed position, and fades out at the bottom over 150ms (instantly
  with reduced motion). Native rubber-band overscroll does not reveal it or
  trigger anchor corrections.
  Resize handling samples pending native movement before correcting layout,
  so it cannot undo a scroll whose event has not arrived yet.
- **The view is anchored by hand**, because WebKit has no `overflow-anchor`.
  While reading, `followTimeline` records on each scroll the first
  anchor row (`ANCHOR_ROW`) crossing the scroller's top, refined down to the
  block child being read, and its `ResizeObserver` scrolls by however far that
  node moved: rows above re-wrapping as the column is resized, growing, or
  arriving from the sentinel. It ignores a scroll event at the position it
  last wrote itself (`at`): that event arrives a frame later, when the next
  resize may have changed layout, and measuring then would lose the
  correction or unpin. Rects scale with page zoom and `scrollTop` doesn't, so
  rect deltas are divided by the scroller's rect height over `offsetHeight`;
  `ThreadMap`'s tops do the same. A placeholder anchor is refined to the
  paragraph being read when its host reconnects.
- **Off-screen rows leave the document without losing React state.**
  `RetainedRow` renders into a stable portal host. `useRowVisibility` watches
  row boxes within 200px of the scroller; outside that range it detaches the
  host and leaves a fixed-height box using the last observed content height.
  Resizing does not lay out or restyle those detached maths and text trees.
  Revealing reattaches the same host, so expanded tools and edits survive;
  native scroll offsets inside the host are recorded on scroll and restored.
  Frames and media stay connected with their layout skipped, because
  detaching an iframe destroys its browsing context. A focused or selected
  row and the row holding the scroll anchor stay connected. Height and
  intersection observers are shared, and adding rows scans direct children.
  `LiveTail` stays connected without a placeholder because its size changes
  on every fold. `useOverflows` retains a disconnected question's fold state
  instead of reading its zero `scrollHeight`; revealing measures it again.
  Row padding extends the paint box 4px on each edge without changing content
  width or gaps. Anything a row draws outside itself must portal (tooltips,
  hover cards and lightboxes do).
- **Positions in skipped rows are estimates until they draw**, so DOM find
  reveals a match, then reveals it again two frames later, and a `ThreadMap`
  jump is instant with a second pass (a smooth scroll would be cancelled by
  the anchoring as rows draw on the way). The rail measures a live collection
  of `thread-map-question-row` boxes, including newly mounted history, so resizing
  does not search through each reply's markdown and maths descendants. For
  a skipped question it measures the cached row box, and a jump explicitly
  reveals that question before reading its bubble's position.
- **Find, Select All and rail jumps mount what they need first**, through
  `app/src/lib/ui/expandable.ts`: `Timeline`'s root is marked expandable, and
  `expandWithin` asks every marked element in a root to mount. DOM find and
  Select All mount everything, synchronously, so the walk and the selection
  (and its markdown copy) see every message; they also reconnect
  detached row content before the DOM walk. A jump to an unmounted question
  mounts back to that question. A synchronous expand keeps the first mounted
  row in place itself, since the caller reads layout before the anchoring
  observer runs. Expanded, a thread stays expanded until it changes.

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
`app/src/lib/harness/route.ts`), never from the store, so every tab shows its own and
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

The queue drains only if every accepted message, and every turn the provider
starts itself, gets exactly one `TurnFinished`: Claude's `expecting` flag
closes a turn whose process died early, and Codex takes the turn id from the
`turn/start` reply, not the later `turn/started`.

Claude runs `Agent` (and a backgrounded Bash) in the background, and when one
finishes the CLI starts a turn on its own. The bridge drops the subagent's
lines (tagged `parent_tool_use_id`), keeps the tool row running until the
task's `task_notification`, and announces the CLI's turn with
`UnpromptedTurn`: a row ("Continued after … finished") and a `Queue::hold`,
so a message typed meanwhile waits. `expecting` is what tells the two apart —
set by `send`, consumed when a turn opens. The turn gets no anchor, since its
question is the CLI's synthetic notification row; rewinds name the student's
question. Recorded in
`app/src-tauri/fixtures/harness/claude-background-agent.ndjson` (the task
outlives its turn) and `claude-background-agent-fast.ndjson` (it finishes
inside it).

**Stop** interrupts and hands queued text back to the composer
(`harness_interrupt`); `restore` is `ChatPage`'s own state, since a store slot
would type it into every Chat tab. Codex never completes an interrupted message, so the
bridge commits the streamed text itself; Claude's closing `result` claims an
error, so `terminal_reason: "aborted_streaming"` decides and its zeroed usage
is skipped.

## Going back rewinds the agent too

Edit, Retry and Rewind truncate at a row (`harness::store::truncate_from`, announced as
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
  concrete model, since an alias changes meaning next release. Claude's
  "Default (recommended)" row is dropped (`claudeAsModels`), since it hides
  which model a run gets; the model it resolves to keeps the default mark. No
  Claude spawn passes it either: `claude::concrete_model` swaps a missing
  model or the `default` alias for what the catalogue resolves it to.
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
on another page, so opening Settings alone does not, and nor does Settings →
Jobs (see [Per-job models](#per-job-models)). The one other start is
onboarding's opencode card, on its *Connect OpenRouter* click
([onboarding.md](./onboarding.md#the-agent-step-lists-every-agent-as-an-equal)). The page is two tables,
Providers and Models, the second one row per model with opencode's own price,
limits and capabilities (`ModelFacts`, a free catalogue read; a missing figure
draws "—", never $0).

- **Recommended providers are pinned.** OpenRouter and OpenCode Go
  (`RECOMMENDED_PROVIDERS` in `opencodeCatalogue.ts`) sit first in both Providers
  tabs with a star, connected or not.
- **Models filter by provider, maker and status.** Maker is who built the
  model, not who serves it (`makerOf` in `models/maker.ts`: the author slug in
  an OpenRouter id, else the family or name). The header checkbox offers or
  hides every filtered model; a blocked model says why on hover.
- **Zen is greyed out.** Its provider (`opencode`) shows only under *All*, marked
  Unavailable with no Connect action; it never counts as connected, and none of its
  models is listed in Models.
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

## Dictation adds an editable message to the draft

Chat, Studio and the lecture dock have a microphone beside Send. Recording
replaces its toolbar with a live amplitude trace, elapsed time, Cancel and
Finish. Finish stops the microphone, transcribes the clip through the same
engine order and language as videos ([viewers.md](./viewers.md#dictation-transcribes-private-microphone-clips)),
then runs the `dictationCleanup` job in a separate one-off session. The brief
allows only hesitation and accidental repetition removal plus punctuation,
capitalization, spacing and paragraph cleanup. It keeps the original wording,
language, script, slang and mixed-language speech, with no translation,
paraphrasing or grammar rewriting. Dictated requests remain text to preserve. The cleaned text appends to the existing draft; it
never sends automatically. Settings → Jobs selects its model and fallbacks,
defaulting to a small model.

A read-only readiness check opens transcription setup when no engine is ready.
The microphone is owned by one composer across split panes. Clips stop after
ten minutes and are capped at 24 MiB. Cancel, switching drafts and unmounting
release capture and invalidate pending results. Hiding a tab also cancels
active capture, so a parked pane never keeps the microphone open. An already submitted engine
or model call may finish, but a cancelled transcription never starts cleanup
and no cancelled result enters another draft. Sending is blocked while a
recording or conversion is pending; typing and attachments stay available.

Failures keep the clip in memory for Retry transcription, or the successful
transcript for Retry cleanup and Use original transcript. Dismiss releases
both. An empty cleanup model chain surfaces the job's Settings hint and keeps
the original transcript. No dictation creates a thread or timeline row.
opencode uses the hidden, tool-denied `oculus-dictation` agent.

## Per-job models

Windows starts new chats with Codex. Job defaults use `gpt-5.6-luna`: lecture
chapters takes `xhigh`, and the other jobs take `medium`, except the two Studio
jobs, which keep their Codex entry and drop Claude's. A stored Claude job
selection resolves to its Windows default in memory without rewriting the
stored setting; supported provider selections keep their saved model and level.

Chaptering, the lecture end, thread naming and document suggestions each
name an ordered list of models (agent, model and level) in a Settings → Jobs
row using `ModelPicker`. The first runs; when its run fails for any reason —
the CLI won't start, the turn errors, the reply is empty or rejected (an empty
suggestion is an answer, not a failure) — the next one runs (`jobs::try_each`), and when all fail the error names each
model's failure. Only the model's turn is retried: chaptering does not detect
or grab frames again, and document suggestions keep their warm spare for the
first model only.

A turn that reports an error fails even if it spoke first: Claude's "session
limit" notice arrives as a reply plus an error, and is never kept as the job's
answer.

A list holds one model per agent, because an agent's models share one usage
limit and a second Claude model can't take over from a spent first. opencode
is the exception: it fronts many providers, each with a limit of its own, so it
takes several (never the same model twice). `withEntry` enforces it in the
dialog, where the add menu offers only the agents the list doesn't hold yet;
both parsers (`one_per_agent`, `onePerAgent`) keep the first of a stored list's
clashing entries.

An empty list switches the job off: it fails with "No model is set", and thread
naming is skipped. It never falls back to the default, which can be an
expensive model. The registry is one `settings` value, `job_models`, read by
`jobs.rs` and written by `jobModels.ts`, which must parse it alike: each job's
key holds an array (entries that don't parse are dropped one by one), or a
single selection object, still read as a one-model list. A missing or unreadable
value or key is the job's default; both sides carry the defaults and must agree
(a new job is a `Job` variant plus entries in `JOBS` and `DEFAULT_JOB_MODELS`).
On the CLI, `--provider`/`--model`/`--effort` override the first model and drop
the fallbacks for one run.

In Settings the row's picker is the first model; its fallbacks are ordered in
a dialog (`JobModelsDialog`) by dragging a row or by the arrow keys on its
handle (`ChainList`). Browsing another agent's tab changes nothing until
a model is picked (`ChainPicker`), so no entry is ever an agent without a model.
The dialog's add menu stays open on the model it just appended, so its level
is set in the same menu.

- **A model that leaves its agent's list is removed, never replaced.**
  `useJobAudit`, mounted in `AppLayout` and asking for catalogues only after
  10 s, drops it in place (later fallbacks move up) and records a notice under
  `job_model_notices`. It removes only on a definite answer: the agent's list
  arrived non-empty and lacks the model — for opencode, the model's own
  provider still lists others — so a failed fetch or a signed-out provider
  removes nothing. A model whose CLI is missing is flagged, not removed.
- **Notices and flags badge Settings** (a dot on the rail item and the Jobs
  row, `useJobAttention`) and print on the job's row until dismissed or the
  job's list is edited.

Neither the Jobs pickers nor the audit starts opencode: both call
`useProviderModels(…, { start: false })`, which asks it through `peekModels`,
reading only a server already up (`harness_opencode_models_if_running`). Found
down, it answers an empty list (so the audit removes nothing) and the picker's
opencode tab links to Settings → opencode, whose opening starts it. It is asked
again when `harnessOpencodeModels` next answers (`OPENCODE_RUNNING_EVENT`) —
the composer or the opencode page starting it.

**The two Studio jobs hold a model per agent, not fallbacks.** "Studio
research" (`studioResearch`) and "Studio scenes" (`studioScene`) are the
models a chat agent's [subagents](#subagents) run on while it makes a video.
Nothing tries them in order: an agent's subagents take the first entry for
that agent (`jobs::for_agent`), and an agent with none does the work itself.
Their `JOBS` rows carry `perAgent`, which offers only the agents whose
subagents take a model (not agy) and words the row and dialog for it ("Add a
model for another agent"). The defaults name Claude and Codex (Haiku 5.5 and
GPT-6-Luna for research; Opus 5.5 and GPT-6.1-Sol at high for scenes);
opencode's models depend on the student's providers, so opencode has none
until one is picked.

## Subagents

A chat agent may start helpers: the `oculus-studio` skill has a director
fan research and scene building out to them. The two roles are
`harness::studio_agents::ROLES`, `studio-research` and `studio-scene`.
`studio-research`'s system prompt (`templates/STUDIO_RESEARCH.template.md`)
points at the skill's "Research" section. `studio-scene`, which builds,
fixes and reviews a scene, gets `STUDIO_SCENE.template.md`, then the
skill's `build.md`, then the builder's sections of `reference.md`
(`BUILDER_SECTIONS`: Components to Looking, and Slides, not the director's video.json,
script.md, Narrate, Pronunciation, Render and Libraries), cut at their
`## ` headings from the same compiled-in files the skill ships, so a
builder reads no file for them and every builder starts from the same
system prompt. A test fails when a named heading is missing. Each CLI starts a
subagent its own way, so each gets the job's model its own way; `oculus
studio agents [--agent A]` prints, per agent, the model and the exact call,
which the skill tells the agent to use as printed.

| CLI | How a subagent starts | Where the model comes from | Measured |
| --- | --- | --- | --- |
| Claude | the `Agent` tool, `subagent_type: "studio-scene"` | `--agents` at spawn: one definition per role with a model, its full id and `effort` (the tool's own `model` takes only an alias) | One Haiku turn under `acceptEdits`, `--permission-prompts none` and the sandboxed settings: the definitions loaded and the subagent's `Write` and `Bash` ran under the parent's mode and sandbox, no denial. Subagents run in the background by default: the turn ended with a `result` while the helper ran, and the CLI then started a turn of its own on the completion notice, so the skill asks for `run_in_background: false` |
| Codex | `spawn_agent` with `model`, `reasoning_effort`, `fork_turns: "none"` | the call itself; Codex sets `model` only when a skill or the user asks, which the skill does. No role prompt: the scene brief opens with a sentence naming `build.md` and the builder's sections (`codex_scene_opening`, the same words in `SKILL.md` and `oculus studio agents`) | One `gpt-6-luna` turn through `app-server` with the thread's `workspace-write` policy and `approvalPolicy: never`: the child wrote under the writable roots, no server request, the parent waited (`wait`) and answered. 4 slots including the parent; a finished child holds its slot until `close_agent` |
| opencode | the `task` tool, `subagent_type: "studio-scene"` | named agents in `opencode.json` (`mode: subagent`, `model`, `variant`), written from the jobs at each server start | Not run live. Read from opencode 1.18's task tool: `subagent_depth` counts parents, so 1 lets the thread's session spawn and a subagent not; a subagent session takes its parent's permissions, with `task` and `todowrite` denied unless its agent allows them |
| agy | none | `invoke_subagent` takes a type name and no model | Not run: with no way to give the job's model, Studio starts none on agy and the agent does the work itself |

opencode's template sets `subagent_depth: 1` and `task: allow` for this; the
named agents carry no `permission` of their own, so they are never wider
than the thread. The hidden one-off agents keep `"*": "deny"`, `task`
included. A role off for an agent is left out of its definitions, and
`oculus studio agents` says to do that role's work in order.

A change in Settings → Jobs reaches Claude's definitions at the thread's
next spawn and opencode's at its server's next start; Codex reads it per
call, from `oculus studio agents`.

## Memory

Nothing carries between threads but `agents/` — `TASTE.md`, `memories/`,
`memories/<CODE>/` (`app/src-tauri/src/agents/`) — and the app never reads it
into a prompt. The contract is in the brief, not a skill, because nobody asks
for a memory and a skill would never trigger; it says read first, then write
on four triggers (a brief naming only a location left 9 memories over 51
threads). `oculus memory` (`app/src-tauri/src/agents/memory/`, [cli.md](./cli.md))
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

**A mention is a glyph and a name on screen and a path on the wire**
(`FileChip`, built from the path alone, drawn as a note draws a reference:
underlined, `referenceRestClass`, with no hover since a click there belongs to
the caret), which is why `MentionInput.tsx` is a contenteditable:
the DOM is the source of truth while typing, React re-renders only on
structural changes (bumping `key`), and `revealCaret` scrolls the box because
a contenteditable follows its caret only for edits it made. The menu anchors
on a range over the `@` token, since a collapsed range's rect can be zeros in
WebKit.
The box refuses control characters (`lib/harness/controlChars.ts`): WebKit can
turn an arrow key into text (U+001D for Right Arrow), which draws as tofu, so a
native `beforeinput` cancels it and a stored draft is stripped on load.
A web link (`https://…` or `[words](https://…)`, `lib/harness/links.ts`) draws
the same way (`LinkChip`: the site's icon and its words or host) in the
composer and in the sent bubble. It becomes one when pasted or when the space
or line break after it is typed, and a restored draft draws its links too; the
text on the wire is the link as written, so a conversion never moves the caret.
The menu's rows draw the chip's own glyph and colour
(`fileReferenceAppearance`), and the lecture dock's composer uses the same input
and menu, scoped to the lecture's subject.
A note's or task body's `@` (the note editor's own completion) writes the
same backticked path and draws the same reference
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
(`app/src/lib/harness/attachments.ts`), which round-trip exactly: a block opens at the
start or after a blank line and ends at the first closing tag alone on its
line, and a text line that is exactly that tag is sent with a backslash in
front. The question bubble lifts the blocks out as the same cards, in the
row above the words ahead of the pictures, and only the remaining prose is
measured and folded; a sent card opens `PastedTextViewer` (rendered markdown
and Copy), and a copied selection across one takes its block (`data-md`).
Words handed back by Stop or a rewind go through `useAttachments.takeBack`,
so their blocks return as cards, their pictures as thumbnails (kept
attachments, sent again by the same path rather than copied) and only the rest
lands in the box. The bubble's own edit box still shows the raw blocks, as it
shows picture paths.

### File drop

Tauri's handler sits in front of the webview, so a Finder drop never reaches
React; `useFileDrop` hit-tests Tauri's events against element rects. The Chat
page's conversation column and the lecture dock's Chat panel are the drop
targets, not just the box: each passes its own ref as the composer's `dropRef`
and draws `DropOverlay` (`app/src/components/ui/layout/DropOverlay.tsx`) from
`onDropping`.

- **The position is in points despite the `PhysicalPosition` type.** wry
  never applies the backing scale, so dividing by `devicePixelRatio` halves
  every point on retina and the drop hits nothing. The hook measures the scale
  (viewport width over `innerSize`/`scaleFactor`).
- **Only a webview listener fires.** A drop is a window event only for
  `WebviewKind::WindowContent`; tauri's `unstable` feature (for
  `Window::add_child`, `app/src-tauri/src/shell/browser/`) makes the main webview
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
  typed text plus `at`. `harness/app/send.rs` appends it under "What the student
  had open when this was sent", which an open notebook shares.
- **`lecture_grab_frames` grabs every stream** — either can hold the teaching —
  at `LIVE_GRAB_WIDTH` (1536), not chaptering's 768, since a whiteboard is
  unreadable at 768. A failed frame drops its line; the message still sends.
- **The dock** reuses `Timeline` and owns its thread id, as each Chat tab does, remembered per lecture in a module-level `Map` because the
  player unmounts on tab switch. The playhead reaches the chip as a stable ref,
  so nothing re-renders the memoised transcript. Lecture docks and Chat tabs
  use counted `hold`/`unhold` ownership of cached rows; closing one view does
  not release a thread still displayed by another.

## A notebook the student has open rides the prompt

`harnessSend` (`app/src/lib/harness/commands.ts`) adds `openNotebookContext()`
to `SendOptions.context` whenever a notebook is open in the editor
(`app/src/lib/canvas/openNotebooks.ts`, registered by `CanvasEditor`): each
notebook's title, the page in view and the `oculus read` command that lists its
page pictures, most recently touched first, at most three. So "is my working on
page 2 right?" needs no `@`, in any thread. Like the lecture's moment it rides
the prompt under "What the student had open when this was sent" and never the
row. The `oculus-library` skill tells the agent what to do with it: open the
page's picture, comment on the student's own working, not rewrite it, and not
touch `canvases/` ([drawing-canvas.md](./drawing-canvas.md#what-a-chat-agent-sees)).

## A Studio thread carries its video

A thread that makes a Studio video is linked to its folder by a
`studio_videos` row, written when `harness_send` creates the thread with
`SendOptions.studio` ([studio.md](./studio.md#a-video-has-a-thread)). Like the
lecture, the link is read back on every send and before any spawn
(`studio_brief` in `harness/app/send.rs`, a rewind's included), and
`instructions()` / `thread_sections()` append the "The video being made"
section after the subject and lecture ones: the absolute folder, format,
theme, subject library and length target read from video.json and the link,
and the order of work (direct it with the skill, draft, ask before the
final). It is the per-thread half the opencode and `agy` bridges send ahead
of the first message.

## Proposal context is read at dispatch, and confirmation lives in Rust

`chat_proposals` owns a card's edited fields, inclusion ticks and status.
Open/edit/apply/discard/undo commands publish its committed snapshot as a
`Proposal` harness event. Reloading and a second view read that same active
draft. The transaction and journal
contract are in [projects.md](./projects.md).

Pending proposals number reciprocal amendments of the same kind V1, V2, …,
using each row's immutable `initial_state` through
`app/src/lib/harness/proposalVersions.ts`. The selector in
`app/src/components/markdown/openui/proposals/VersionControl.tsx` switches the
active draft to that version's content. Modified compares it with that
version's original; Reset restores the original fields, rows and inclusion
ticks. Restoring content preserves the active card's amendment link and
confirmation identity. It never applies student-data writes on its own.

`app/src/stores/chat/proposalVersionStore.ts` parks each version's edits and
selection in localStorage (`oculus.proposalVersions`), so switching away and
back or relaunching retains them. A shared revision counter invalidates
outgoing editors in every mounted view: their final blur/unmount callbacks
cannot overwrite a reset or switch. Rewind and thread deletion prune parked
drafts alongside widget progress. Original snapshots stay in SQLite.

Every `start_turn` reads pending cards and newly decided statuses after the
lecture moment, appending "Cards in this thread" to the provider prompt.
The user row keeps only the student's typed text. Pending cards repeat on
every send; decided cards repeat until a send is accepted. A conditional
`told` write matches the exact status and state sent, so an edit or Undo while
a provider accepts the request cannot mark a newer version seen. Queued
messages read cards when released, after any intervening Create click.
Registering a replacement for a pending card carries the student's explicit
inclusion ticks forward on matching rows, including subtasks. Existing rows
match by id; new rows match by title or name and occurrence. Proposed field
values still come from the replacement; a decided card supplies no baseline.

Rewind explicitly deletes proposals at or after the removed item, and thread
deletion clears its proposals and timeline rows even when SQLite foreign keys
are off. `agent_changes` remains: removing conversation context does not undo
student data.

Confirmed jobs use `proposal_job_start`, which atomically decides a pending
card before launching the app's native job. Transcription and downloads
await those commands; lecture jobs follow their terminal events; sync and
parse retain the existing progress events. The row's `job` state persists
running, complete or error across views. Completion clears `told` so the next
message reports the outcome. Startup marks interrupted runs as errors and
keeps the card decided, avoiding an accidental second billed launch. Jobs
have no Undo or amendments.


## Revision writes and submitted answers have separate paths

The `oculus-revision` skill drafts course-style questions as unaccepted items and offers condensed `NewCheatNotes` proposals after concept explanations. The `revisionMarking` job marks a submitted written answer in one tool-less turn, with strict rubric JSON validation. State, scheduling and error behavior are in [revision.md](./revision.md).

Live Task/TaskList, Deadline/Upcoming, Lecture, StudioVideo, CheatNote and ReviewDue widgets read current rows by CLI-supplied id. `Task(number)` is live; `Task(string, ...)` remains a proposed new task. Lecture citations accept `lectures/<id>#t=<seconds>` and open the player at that moment.

## Gotchas

- Never pass `agy` `--dangerously-skip-permissions` — its file tools then write outside the library.
- Never put an `Edit` deny on `oculus.db*` in Claude's settings — it cancels the write grant and every board write fails readonly.
- Never grant the library folder instead of the three database files — it exposes keyd's vault.
- Never let Settings make a billed call, or start opencode anywhere but its own page or the onboarding opencode card's Connect button — see [No model is ever probed](#no-model-is-ever-probed).
- Read opencode models from `/config/providers`, never `/api/model` — the latter lists the instance's providers, not the signed-in ones.
- Every opencode call carries `?directory=<agents>` except `/auth/{id}` — unscoped, it binds to the server's launch cwd and another instance.
- Never drive opencode's v2 API (`/api/*`) — on 1.18.31 a prompt on an `auth.json` provider fails with no event.
- Resend agent and model on every opencode prompt — creation values are decorative, and a bare prompt runs the stock `build` agent; check `oculus` is in `GET /agent` before creating a session.
- `opencode serve --port 0` means "prefer 4096" — parse the real port from stdout.
- opencode's `session.idle` arrives before the partial answer during an abort; a 30s drain watchdog fails a turn the server went silent on.
- `providers::opencode::sweep` (`strays.rs`) kills strays at startup by `SERVE_ARGS` *and* `ppid == 1` — drop either and it kills a living app's or a hand-run server.
- Codex: `thread/start` takes `sandbox` (string), `turn/start` takes `sandboxPolicy` (object); effort is thread config; a resumed thread replays stale usage first.
- Claude: take tool inputs from the `assistant` line, not `input_json_delta`s; `content_block_start` can carry text the deltas don't repeat.
- Don't cache sign-in status in Rust — a stale "signed out" after a sign-in is the wrong answer that matters.
- The ⌘⌥B column fold tests `e.code` — ⌥B types `∫` — and `AppLayout`'s ⌘B returns early on ⌥, or one chord closes both panels.
- Never give an agent's HTML embed `allow-same-origin` — the page would run with the app's origin and reach the Tauri IPC.
- Thread-row delete commits on `mousedown` — WebKit blurs the focused confirm on press and unmounts it before `click`.
- The thread list reserves its gutter with `overflow-y: scroll`; `scrollbar-gutter: stable` is a no-op in this WebKit.
