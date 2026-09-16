# The harness: CLI agents as chat

Chat is a coding agent the student already has — Claude Code or Codex —
run as a subprocess from the library, with its output folded into one
timeline. No API key, no per-token billing: the CLIs carry the student's own
subscription, which is the whole reason for driving them rather than the
model APIs. The shape is bb's (get-bb/bb) with its plugin system taken out:
one bridge per provider, one normalized event stream, a timeline that only
ever sees the stream.

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
| Finding the binaries from a GUI app | `app/src-tauri/src/harness/discover.rs` |
| Thread and timeline rows | `app/src-tauri/src/harness/store.rs`, migrations 24–26, 28 and 30 in `app/src-tauri/src/lib.rs` |
| Instructions appended to the provider's prompt | `app/src-tauri/templates/HARNESS.template.md` |
| The library's own `AGENTS.md`, which Codex reads on its own | `app/src-tauri/templates/AGENTS.template.md` |
| Recorded provider output the bridge tests replay | `app/src-tauri/fixtures/harness/` |
| Frontend types, reads, commands | `app/src/lib/harness.ts` |
| Live state, event folding | `app/src/stores/harnessStore.ts` |
| Page, thread list, timeline, rows, composer | `app/src/pages/ChatPage.tsx`, `app/src/components/harness/` |
| The `@` menu's candidate files | `searchMentionFiles` in `app/src/lib/db.ts` |
| Health and the per-job model rows in Settings → AI | `app/src/pages/settings/AiPage.tsx` |
| Which agent, model and level each headless job runs on | `app/src-tauri/src/harness/jobs.rs`, `app/src/lib/db.ts` |
| The lecture brief a dock thread is scoped to | `instructions` in `app/src-tauri/src/harness/mod.rs` |
| The frame grab a message's moment carries | `lecture_grab_frame` in `app/src-tauri/src/chapters.rs`, `app/src/lib/lectures.ts` |
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
- **The agent can now write to the database, and that does not loosen the rule
  above.** `oculus project` and `oculus task` put a plan into the tables the
  app's board draws ([projects.md](./projects.md)), so a breakdown the student
  agrees to is rows rather than a markdown file in `agents/`. The CLI is the
  door in both directions and `oculus.db` stays on the deny list: the binary
  is what knows that a column id must exist, that `done_at` follows the
  destination column's kind, and that a whole breakdown belongs in one
  transaction — none of which a `sqlite3` a model reached for would honour.
  Both templates say so. The board picks the write up because
  `useBackendEvents` watches this event stream for a finished tool call whose
  command names one of those and fires `PROJECTS_UPDATED_EVENT`; it matches on
  the command text rather than `classify`'s `ToolKind`, since `is_oculus_cli`
  only word-matches the first few words and `cd … && oculus task add` reads as
  plain Bash.
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
are caught in `AppLayout`'s capture phase and routed to an in-app browser tab. Command output is the one thing
outside markdown set in monospace, through `CodeText` in
`app/src/components/markdown/MdComponents.tsx`, which is where the font
lives.

### Maths in a reply

Coursework here is formulas, so the appended prompt asks for LaTeX — `$…$`
inline and `$$…$$` displayed — and `Timeline` renders it with remark-math and
rehype-katex. Two things make that hold up:

- **The prompt has to ask.** Without a rule for it a model writes formulas as
  Unicode — `cos(θ_B/2)|0⟩` — which is not maths, just characters that look
  like it, and no renderer can recover it. `HARNESS.template.md` names the two
  delimiters and rules out the Unicode; `recap.rs`'s prompt already said the
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
`.chat-md .katex` in `app/src/index.css` brings 1.21em down to something that
sits in 13px text and gives a display formula its own horizontal scroller —
without one, a long derivation has no width to shrink to and widens the
player's dock.

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
  (`MATH` in `app/src/components/markdown/MdComponents.tsx`).
- **Following the stream watches for growth.** A `scrollTo` per delta forced a
  layout of the whole thread and yanked the page down whenever the reader
  had scrolled up; a `ResizeObserver` on the column scrolls only while the
  bottom is already where they are.

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
reads in order, and both CLIs were measured getting it wrong in their own
way:

- **Claude queues it itself** and starts a second turn the instant the first
  `result` is out. The rows are fine, but the *turn* boundary is not: the
  composer went idle between them, the spinner stopped, and stop had nothing
  to stop for as long as the gap lasted.
- **Codex folds it into the running turn.** A second `turn/start` comes back
  with the *same* turn id — no error, no new turn — so the question lands in
  the timeline above the answer to the previous one, there is one
  `turn/completed` for both, and the reply to the first question can look
  like it never came. This is what "the message just disappears" was.

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

Neither will say that identifier twice, so it is learned once — when the turn
goes out — and kept on the question's row as `anchor` (migration 28), arriving
as a `TurnAnchor` event like everything else the bridges learn. Codex says its
turn id in the `turn/start` reply. Claude says nothing: its `stream-json`
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
active provider, and a row of levels under a rule. Choosing a model closes
the menu, choosing a level does not — the level is the fine adjustment after
the coarse one. The marks are the `currentColor` SVGs in
`app/src/components/harness/ProviderMark.tsx`, monochrome like bb's so they
sit in the palette rather than fighting the indigo. They are held in a
`Record<Provider, …>` rather than picked by a ternary, so a provider added to
the union is a compile error until its mark exists — the ternary that used to
be there answered "not Claude" with the Codex mark.

Every picker in the app assembles its list through one hook,
`app/src/hooks/useProviderModels.ts`, which names no provider: which
catalogues are compiled in and which are fetched from a CLI is a property of
the `PROVIDERS` entry in `app/src/lib/harness.ts`. That file is the one place
a provider is declared.

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

The menu **opens downwards and only flips up when it would not fit**
(`Composer.tsx`). The same composer sits in three very different places — the
middle of the home page, the middle of the chat hero, and pinned to the bottom
of an open thread — so which way is out of the way is a fact about the
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

## Lecture scope, and the moment

A thread can also be scoped to one **recording** —
`harness_threads.lecture_id` (migration 30), for the conversation the lecture
player's dock holds beside the video. It is the subject scope's shape with a
narrower subject and one thing it cannot do: NULL clears rather than cascades
for the reason `subject_id` does — the conversation is the student's own and
the lecture merely scopes it — and it is fixed at creation, because both CLIs
bind the appended instructions at session start.

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
inlined and the transcript is not, for the split
[chapters.md](./chapters.md) makes about the same two files: a dozen short
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
timestamp, the last minute of transcript, and a frame — and that the moment is
usually enough, so the deck is for when a question needs the exact notation
rather than a first move.

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

**`lecture_grab_frame` is the picture half of that moment.** One JPEG of the
playhead's second into `lectures/<id>/frames/live/<seconds>.jpg` — its own
subfolder, so a message's grab can never collide with a chaptering run's
frames in the folder above, and overwritten freely. It reuses that job's
probe-and-grab rather than a bare ffmpeg call, because the splash-screen
defence ([chapters.md](./chapters.md)) is exactly as load-bearing for a frame
the *student* asked about. What it returns is the path the **agent** can read,
`../lectures/<id>/frames/live/<seconds>.jpg`: the webview never opens the file
— it puts the string in the message. A lecture with no downloaded recording is
refused the way `chapters::run` refuses one.

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
a second memoised bag beside `chapters`, built over stable values only.

**The moment is built by the player at send time**
(`buildMoment` in `LecturePlayer.tsx`): the second itself, the chapter the
playhead is in, the transcript cues of the minute before inlined as plain text,
and the path from `lectureGrabFrame`. It goes out as `SendOptions.context` with
the second as `SendOptions.at` — appended to the prompt, never the message, as
above. A minute of transcript is a few hundred words where the file is twenty
thousand, which is why this one is inlined and the file is only named. **A
frame that cannot be grabbed drops its line and the message still goes**: a
lecture whose video was never downloaded is refused by `lecture_grab_frame`,
and losing the question over a missing picture would be the wrong half to lose.
Sending does not touch playback.

The chip shows the timestamp it will send and tracks the playhead until the
send; what freezes is the row. `at` rides `HarnessEvent::UserMessage` as well
as the row Rust writes, so the bubble says "at 3:40" from the moment it appears
rather than after a reload, in the body font with `tabular-nums` beside the
time the question was asked.

## Stages

Built: the two bridges with recorded fixtures and replay tests, `oculus
agent` as the headless proof, tables and lifecycle, the page, subject scope
and the `@` file menu, the message queue, stopping a turn, going back
(edit, retry, rewind), the per-job model registry above, and the lecture player's dock chat. Not yet: approvals and native questions routed to the UI, steering
mid-turn (bb's `turn/steer` and a second stdin line — the queue is the
waiting-room version of it, not steering), the plan/todo card, branching (rewind
deliberately does not), and a third bridge for the API path when BYOK
returns.

The plan/todo card is still outstanding despite projects being built, because
the two are different things: that card would draw the provider's *own*
in-turn todo list (`ToolKind::Plan`, collapsed today), which lives and dies
with the turn, where a project is the student's, persists, and is edited on a
board long after the thread has moved on.
