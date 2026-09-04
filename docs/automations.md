# Automations and the Inbox

An automation is a graph you draw: **trigger** nodes — a schedule, or an app
event — joined by links to **source**, **condition** and **action** nodes. A
trigger fires, the executor walks its outgoing links, and actions that produce
something for you to read deliver it to the **Inbox**.

Links are wires, not just arrows. A trigger *exposes* what its event carried —
a sync-complete trigger has a port for the run's new files, one for the
updated ones, one for both — and every downstream node has named input slots
you wire those ports into. A node works on what you gave it, so "summarise
what changed and put it in my Inbox" is four nodes and three wires, not a node
called "digest".

## Where

| Piece | Location |
| --- | --- |
| Node kinds, ports, values, graph walk, schedule timing, text templates | `app/src/lib/automations.ts` |
| Palette copy + a node's starting config | `app/src/components/automations/catalog.ts` |
| Canvas, node, palette, inspector | `app/src/components/automations/` |
| The branch/rule builder | `app/src/components/automations/RuleEditor.tsx` |
| Ready-made graphs behind "Start from a template" | `app/src/lib/automationTemplates.ts` |
| Pages | `app/src/pages/AutomationsPage.tsx` (list), `app/src/pages/AutomationEditorPage.tsx` (canvas) |
| Scheduler tick + startup resume | `app/src/hooks/useAutomations.ts` |
| Event trigger (`sync-complete`) | `app/src/hooks/useBackendEvents.ts` |
| Summarising files into an Inbox item | `app/src/lib/digest.ts` |
| Per-file summary / free-form generation | `llm_summarize`, `llm_generate` in `app/src-tauri/src/llm.rs` |
| Tables (`automations`, `inbox_items`, `inbox_item_entries`) | migrations 18, 20 and 21 in `app/src-tauri/src/lib.rs` |
| `local_events`, what `action.calendar` writes | migration 22; see [calendar.md](./calendar.md) |
| Inbox state + unread count | `app/src/stores/inboxStore.ts` |

## The node kinds

| Kind | Group | What it does |
| --- | --- | --- |
| `trigger.schedule` | Trigger | Daily, on chosen weekdays, or on an interval. |
| `trigger.event` | Trigger | `sync-complete` (carries the run id) or `app-start`. |
| `source.inbox` | Read | Recent Inbox items as one markdown block, plus how many. |
| `source.calendar` | Read | A forward window of classes, due dates, lectures and notes, plus how many. |
| `source.files` | Read | Files already in the library, filtered by age, subject and category — a `files` value, so it feeds Summarise exactly as a sync's lists do. |
| `condition.if` | Logic | Any number of named branches, each with its own rules; the first whose rules match takes the value. |
| `action.sync` | Action | Runs a sync of the selected subjects — and exposes that run's file lists, same three ports as the event trigger. |
| `action.summarise` | Action | Attaches "summarise each of these, like this" to a file list. |
| `action.ai` | Action | One prompt over the slots you wired in; the reply leaves by its `text` port. |
| `action.inbox` | Action | Delivers whatever is wired in: summaries become an item with a row per file, anything else a note. |
| `action.calendar` | Action | Writes an event to `local_events` — see [calendar.md](./calendar.md). |
| `action.notify` | Action | Desktop notification, via `tauri-plugin-notification`. |

**A source is not a trigger.** It has a `then` input and only runs when
something upstream reaches it, so *when* a graph looks at your calendar is a
separate decision from *what* it looks at. That is what makes the set
combinatorial rather than a list of features: any trigger can pull in any
state.

**Ports are derived, never stored** (`inputPorts` / `outputPorts`): the event
trigger's outputs depend on which event it listens for, and an AI node's input
slots are whatever the user named in the inspector. Storing them would mean two
copies of one fact, one of them stale — so instead the canvas prunes wires whose
ports have gone.

Text fields understand `{{…}}` placeholders naming **that node's own input
slots** — `{{input}}` is whatever landed in the slot called `input`,
`{{input.count}}` how many things it carried, `{{input.names}}` their filenames
— plus the globals `name`, `date`, `time` (`GLOBAL_VARS`). Several wires into
one slot merge: file lists concatenate, mixed kinds meet as text.

## How it connects

- **The graph is JSON on the row; runtime state is columns.** `enabled` and the
  anchors are real columns because the scheduler queries them every 30s; the
  node/link structure is a blob because adding a node kind should never need a
  migration. Migration 18 rewrote the old `sync_schedules` rows into two-node
  chains (`trigger.schedule` → `action.sync`) and dropped that table — the
  feature was renamed, not replaced, so existing schedules keep firing.
- **Anchors are per trigger, not per automation** (migration 20's
  `trigger_state`, a JSON map keyed by node id). One graph may hold several
  triggers, and "daily at 09:00" has to stay due on a graph whose other trigger
  runs every 30 minutes — a single row-level anchor cannot express that. The
  map is runtime state and lives outside `graph` on purpose: firing must not
  rewrite the drawing. A trigger with no entry falls back to the row's
  `anchor_at`, which is what migrated graphs use until their first firing.
- **Triggers fire where their signal lives.** Schedules are driven by a 30s
  tick in `useAutomations` (frontend, like the scheduler it replaced — there is
  no Rust timer); `sync-complete` is dispatched from the `scrape-complete`
  handler in `useBackendEvents`, which is the only place a run id exists;
  `app-start` has no other home, so it too comes from `useAutomations`. The
  firing is stamped *before* the walk so a failing action cannot refire every
  tick.
- **The canvas is a view over that graph, fully controlled.** Every drag,
  connection and config edit rewrites `{nodes, links}` and hands it back to
  `AutomationEditorPage`, which debounces the SQLite write by 400ms and flushes
  it on unmount. Node positions live in the graph blob; a graph drawn before
  the editor existed gets laid out by depth on first open and keeps the
  positions from then on.
- **Measured node sizes are handed back to React Flow, or nodes vanish.**
  Because every node object is rebuilt from the graph on each change, React
  Flow drops the size it measured — and it renders an unmeasured node
  `visibility: hidden`. Nothing re-measures it either: the element's own box
  never changed, so the resize observer stays quiet, and the node was gone for
  good the moment you dragged it. `AutomationCanvas` therefore keeps the
  `dimensions` changes in local state and passes them back as `measured`. They
  stay out of the graph blob — a node's height follows from its ports, so it is
  derived, not part of the document. A node it has never measured is given the
  standard box rather than nothing, because a node added after mount was
  measured never to receive a `dimensions` event at all: "not measured yet"
  must mean "assume the usual size", never "invisible".
- **The palette is a search, and it is dismissed by the canvas.** The node list
  is long enough that browsing it is slower than typing, so "Add node" (and
  ⌘K) opens a `cmdk` palette that matches on title, blurb and per-kind
  keywords. Dropping a wire on empty canvas opens the same palette narrowed to
  the kinds that can take that value — compatibility is *probed*, by building a
  node from the spec's defaults and asking `canConnect`, so the palette and the
  canvas can never disagree about what may be wired. Closing it is the canvas's
  job rather than Radix's: the pane runs on d3-zoom, which swallows the mouse
  events the outside-click detection watches for, so the popup would sit there
  while you drew underneath it. `onPaneClick`/`onNodeClick`/`onMoveStart` close
  it instead, with a one-shot pass for the pane click that *ends* a wire drag —
  otherwise the gesture that opens the palette immediately dismisses it.
- **Panning is bounded to the graph.** Dragging the empty pane pans the canvas,
  and React Flow will happily pan for ever — one ordinary drag used to leave a
  blank canvas with the graph parked off-screen. `translateExtent` is therefore
  recomputed from the node bounds plus a margin of one viewport less a visible
  strip, measured at the tightest zoom because that is where the viewport
  covers the fewest flow units. Dragging a node outwards grows the bounds with
  it, so the limit never fences the graph in.
- **A condition is a router, and it never spends a model call.** Branches are
  named, ordered, and evaluated top to bottom until one matches — first match
  wins, so the order on screen is the order of evaluation and a catch-all
  belongs last. `Otherwise` is the branch for "none of them". Each branch holds
  rules (all of them, or any of them) built from an operand, an operator and an
  operand: what came in (its count, its text, its filenames), a fact about the
  clock, or a literal. Crucially the evaluator reads values through the
  synchronous `plainText`, never `valueToText` — reading a `summaries` value is
  what *makes the summaries happen*, so a condition that "looked at" its input
  would quietly spend minutes and tokens just to pick a branch. Old two-port
  conditions upgrade on parse: the single rule becomes a branch that keeps the
  id `true` so existing wires survive, and links leaving the old `false` port
  are rewritten to `else`.
- **A counted listing carries its own length.** A `text` value may hold an `n`,
  and `valueCount` returns it. Without that, gating on how much arrived meant
  gating *instead of* using it: a source's count leaves by its own port, a
  branch forwards the single value it was given, and a node fed both the branch
  and a direct wire runs whenever *either* is live — so the gate would have no
  teeth. With it, "the fortnight's deadlines, but only if there are any" is one
  wire, and `{{deadlines.count}}` in a prompt says the real number.
- **`any` may be wired into a file slot, and only `any` may.** A condition's
  branches are pass-throughs typed `any`, carrying whatever reached the
  condition; what that is cannot be known until the run. Refusing the wire
  would ban "only if files arrived → summarise them", which is the first thing
  anyone draws. A branch fed something that is not a file list summarises
  nothing — a quiet no-op, not a crash.
- **A `summaries` value is a promise of work, not the work.** The summarise
  node hands on a file list plus an instruction; the node that *reads* it does
  the model calls. That is what lets "Add to Inbox" put the item on screen with
  one pending row per file and fill them in as they land — an eager summariser
  could only hand over a finished blob minutes later, with nothing to show in
  between and nothing to resume from. A prompt that never mentions the slot
  never pays for the summaries.
- **The walk is by readiness, once per node, fail-fast.** A node runs once every
  wire into it has settled — its source having run, or been skipped — and only
  if at least one of those wires carries a value. Fan-in therefore sees every
  branch rather than firing on whichever arrived first; a node reached only
  through a condition's untaken branch is skipped, and the skip propagates. A
  node that throws stops the run: an Inbox item about a sync that never happened
  is nonsense. A cycle never settles, so it simply never runs.
- **Old graphs are upgraded on parse, then written back once.** `parseGraph`
  rewrites tuple links into ported ones, splits the retired all-in-one
  `action.scrape_digest` into the `action.summarise` → `action.inbox` pair it
  always was underneath, and translates the old fact-bag placeholders
  (`{{fileList}}`, `{{changedFiles}}`) into slot references. It is idempotent
  and derives any id it invents from the node it replaces, because it runs on
  every parse; `migrateAutomationGraphs` (from `useAutomations`) persists the
  result at launch.
- **The summaries wait for parsing, and that is why they live in the frontend.**
  A scrape reports complete once bytes are on disk, but a PDF has no text
  until the sidecar parses it — seconds to minutes later, signalled by events
  that already land in the frontend. So the item is created immediately in
  `pending` and each entry fills in as its text becomes readable
  (`scan_parsed_files` is the readiness check; markdown-native files — Canvas
  pages, announcements, Ed threads — are ready at once). Entries are summarised
  **one at a time**: a local model holds one set of weights, so parallel calls
  only put more memory in flight (see [llm.md](./llm.md) for the memory
  preflight). The instruction the graph asked for is stored on the item
  (migration 21) — a fill resumed after a quit has to ask the same question.
- An entry that never parses is marked `skipped`, not left spinning; the item
  closes to `ready` when every entry reaches a terminal state, and
  `useAutomations` resumes any item still `pending` at startup.
- **An Inbox note reuses the digest's item/entry pair** rather than adding a
  table: the entry row already carries markdown and the Inbox already renders
  it. A note has no file behind it, so its `relative_path` is empty and its
  `action` is `note` — which is how `InboxPage` knows not to draw a clickable
  filename.
- **Inbox rows hold soft references only** — no foreign keys to `sync_runs`
  or `files`. `clearAllFiles` in `app/src/lib/db.ts` wipes those tables, and
  a digest you have already read should survive resetting the library.
- **Notification permission is asked for on first use**, inside the notify
  action, not at launch: a system prompt only earns its place once a graph
  actually wants one.
- The summarise-after-sync automation ships **disabled**: it spends tokens on
  every sync, so turning it on is a decision, not a default.
