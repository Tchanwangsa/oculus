import { create } from "zustand";
import {
  defaultSelectionFor,
  getHarnessItems,
  getHarnessThreads,
  harnessQueued,
  type HarnessEnvelope,
  type HarnessItem,
  type HarnessThread,
  type Provider,
  type QueuedMessage,
  type RateWindow,
  type ThreadUsage,
} from "@/lib/harness";
import { getSubjects, type Subject } from "@/lib/db";

/**
 * What is on screen for a thread mid-turn and nowhere in the database: the
 * assistant text and reasoning still streaming, and command output still
 * arriving. Everything else the timeline shows is a row Rust wrote.
 */
export interface LiveTurn {
  running: boolean;
  streaming: string;
  thinking: string;
  toolOutput: Record<string, string>;
}

const IDLE: LiveTurn = { running: false, streaming: "", thinking: "", toolOutput: {} };

/** The answer for a thread the map does not hold. One array for every such
 *  answer, because a view selects `items[id] ?? EMPTY` and zustand compares by
 *  reference: a fresh `[]` per render re-renders the timeline — the most
 *  expensive thing the app draws — on every store write, forever. */
const EMPTY: HarnessItem[] = [];

interface HarnessState {
  threads: HarnessThread[];
  /** null is the empty composer — a thread is created by the first send. */
  activeId: number | null;
  /** Rows per thread, not one timeline. Two views hold a thread each — the
   *  Chat page and the lecture dock — so "the rows on screen" is no longer a
   *  single answer. */
  items: Record<number, HarnessItem[]>;
  live: Record<number, LiveTurn>;
  rateLimits: Partial<Record<Provider, RateWindow[]>>;
  /** Composer selection for the *next* thread; an open thread keeps its own. */
  provider: Provider;
  model: string | null;
  /** Reasoning effort sent with each turn. Unlike the model it is not stored
   *  on the thread — it is a per-turn dial, so it stays session state and
   *  applies to whichever thread is open, the way bb treats it. */
  reasoning: string | null;
  /** Subject scope for the *next* thread; null is the general one. An open
   *  thread shows its own `subject_id` and cannot be re-scoped. */
  subjectId: number | null;
  /** Every subject, for the composer's picker and its `@` menu. */
  subjects: Subject[];
  /** Messages typed while a turn was running, per thread, in the order they
   *  will go out. Rust holds the real queue; these are folded from its
   *  `queued`/`unqueued` events, and nothing here decides when one is sent. */
  queued: Record<number, QueuedMessage[]>;
  /** Threads where a rewind could not reach the agent — a question older than
   *  the anchor, or a provider session that is gone. The agent still holds the
   *  exchange that left the screen, which is worth saying once, so this stays
   *  set for the life of the thread rather than being cleared on the next
   *  turn: the context does not forget later either. */
  contextDrift: Record<number, boolean>;
  /** Text handed back to the composer, because stopping a turn drops what was
   *  waiting behind it and the student typed those words. The counter is what
   *  the composer watches: the same text twice is still two restores. */
  restore: { text: string; n: number } | null;
  restored: () => void;

  loadThreads: () => Promise<void>;
  open: (id: number | null) => Promise<void>;
  /** Read a thread's rows and its queue into the map without making it the
   *  open one — how a second view puts a thread on screen beside whatever the
   *  Chat page is already showing. `open` is this plus the active id. */
  load: (id: number) => Promise<void>;
  setProvider: (p: Provider) => void;
  setModel: (m: string | null) => void;
  setReasoning: (level: string | null) => void;
  setSubject: (id: number | null) => void;
  loadSubjects: () => Promise<void>;
  apply: (env: HarnessEnvelope) => void;
  /** Fold the buffered deltas below into `live` now. Exposed for the tests
   *  and for anything that needs the stream settled before it reads. */
  flushLive: () => void;
  removed: (id: number) => void;
  /** Forget one thread's rows. The map holds every thread opened this session,
   *  which is what makes switching back to one instant; `removed` and this are
   *  the only things that shrink it. A view that leaves a thread has to say
   *  so — a single thread can carry 300KB of tool output, and a dock walked
   *  through twenty lectures would otherwise keep twenty timelines. */
  release: (id: number) => void;
}

/**
 * Deltas arrive per token — tens a second — and each one used to be a
 * `set`, so every keystroke of the model's re-rendered the whole timeline.
 * They are buffered here instead and folded in on a timer, which caps the
 * page at one render per tick however fast the provider talks. Only the three
 * delta events buffer; every other event flushes first, so nothing can
 * overtake the row it belongs to.
 */
const FLUSH_MS = 48;

interface PendingDeltas {
  streaming: string;
  thinking: string;
  toolOutput: Record<string, string>;
}

const pending = new Map<number, PendingDeltas>();
let timer: ReturnType<typeof setTimeout> | null = null;

function buffer(threadId: number): PendingDeltas {
  let p = pending.get(threadId);
  if (!p) {
    p = { streaming: "", thinking: "", toolOutput: {} };
    pending.set(threadId, p);
  }
  if (timer == null) timer = setTimeout(() => useHarnessStore.getState().flushLive(), FLUSH_MS);
  return p;
}

/** A synthetic row from a live event, keyed on the Rust row id when there is
 *  one so a reload lines up with what was shown. */
function rowFrom(env: HarnessEnvelope, kind: HarnessItem["kind"], content: string, meta?: unknown): HarnessItem {
  return {
    id: env.itemId ?? -Date.now() - Math.floor(Math.random() * 1000),
    thread_id: env.threadId,
    kind,
    ref_id: env.event.type === "tool_started" ? env.event.id : null,
    content,
    meta: meta === undefined ? null : JSON.stringify(meta),
    created_at: new Date().toISOString(),
  };
}

export const useHarnessStore = create<HarnessState>((set, get) => ({
  threads: [],
  activeId: null,
  items: {},
  live: {},
  rateLimits: {},
  queued: {},
  contextDrift: {},
  restore: null,
  provider: "claude",
  subjectId: null,
  subjects: [],
  // A provider whose catalogue is compiled in — Claude's is the only one —
  // lets the composer open already pointing at a real model. A fetched one
  // (Codex, opencode) opens on nothing and is filled the moment its list
  // lands (`useProviderModels`).
  ...defaultSelectionFor("claude"),

  loadThreads: async () => {
    const threads = await getHarnessThreads();
    set((s) => {
      // A thread the DB says is running is one we heard start; the store's
      // live map is authoritative for the spinner once mounted.
      const live = { ...s.live };
      for (const t of threads) {
        if (t.status === "running" && !live[t.id]) live[t.id] = { ...IDLE, running: true };
      }
      return { threads, live };
    });
  },

  open: async (id) => {
    if (id == null) {
      // Nothing to clear: the empty composer has no thread id, and `itemsFor`
      // answers for a null one with the shared empty array.
      set({ activeId: null });
      return;
    }
    // Re-opening the thread already on screen would re-read it for nothing,
    // and the reader would watch it happen.
    if (get().activeId === id) return;
    set((s) => ({
      activeId: id,
      // The rows of the thread being left carry the one arriving for the beat
      // the read takes. No rows under a selected thread is the empty
      // composer's own state (`ChatPage`) — which under a map is what a thread
      // with no key yet looks like — so every switch flashed the hero and
      // re-mounted the composer under it before the rows landed. A stale row
      // for a few milliseconds is invisible; that was not. A thread the map
      // already holds draws its own rows at once and borrows nothing.
      items:
        id in s.items || s.activeId == null || !s.items[s.activeId]
          ? s.items
          : { ...s.items, [id]: s.items[s.activeId] },
    }));
    await get().load(id);
  },

  load: async (id) => {
    // Claim the key before reading. "Is this a thread we hold" is the test the
    // guards below and `apply` both make, and `load` has no active id to ask
    // about instead — the dock's thread is never the active one.
    if (!(id in get().items)) set((s) => ({ items: { ...s.items, [id]: EMPTY } }));
    // Guard both answers against the thread being let go while the query was
    // in flight. A read that fails still writes its empty answer: what is on
    // screen may be the rows borrowed above, and they would otherwise sit
    // under this thread's name for good.
    const rows = await getHarnessItems(id).catch(() => [] as HarnessItem[]);
    if (id in get().items) set((s) => ({ items: { ...s.items, [id]: rows } }));
    // The queue is in Rust's memory, not the database, so a page that has
    // just loaded — or a thread opened in another window — has to ask.
    const waiting = await harnessQueued(id).catch(() => [] as QueuedMessage[]);
    if (id in get().items) set((s) => ({ queued: { ...s.queued, [id]: waiting } }));
  },

  // No two agents share model ids or a level vocabulary, so switching agent
  // replaces both rather than carrying a selection that cannot apply. What
  // decides whether there is a new selection to give is whether that
  // provider's catalogue is compiled in or fetched — not which provider it is
  // (`defaultSelectionFor`). A fetched one is empty for the beat before its
  // CLI answers, and the picker fills it in.
  setProvider: (provider) => set({ provider, ...defaultSelectionFor(provider) }),
  restored: () => set({ restore: null }),
  setModel: (model) => set({ model }),
  setReasoning: (reasoning) => set({ reasoning }),
  setSubject: (subjectId) => set({ subjectId }),
  loadSubjects: async () => set({ subjects: await getSubjects() }),

  release: (id) =>
    set((s) => {
      // Never the open one. Two views can hold the same thread — the dock and
      // the Chat page sitting on the same conversation — and a dock that let
      // go of it on unmount would empty the page's timeline under it and stop
      // `apply` writing to it, which reads as a thread that died mid-turn.
      // Only the dock releases, so the page's thread is the one to protect;
      // the cost is one map entry held after both views have left it.
      if (id === s.activeId || !(id in s.items)) return s;
      const items = { ...s.items };
      delete items[id];
      return { items };
    }),

  removed: (id) => {
    pending.delete(id);
    set((s) => {
      const queued = { ...s.queued };
      delete queued[id];
      const items = { ...s.items };
      delete items[id];
      return {
        threads: s.threads.filter((t) => t.id !== id),
        activeId: s.activeId === id ? null : s.activeId,
        items,
        queued,
      };
    });
  },

  flushLive: () => {
    if (timer != null) {
      clearTimeout(timer);
      timer = null;
    }
    if (pending.size === 0) return;
    const batch = [...pending.entries()];
    pending.clear();
    set((s) => {
      const live = { ...s.live };
      for (const [id, p] of batch) {
        const prev = live[id] ?? IDLE;
        let toolOutput = prev.toolOutput;
        for (const [ref, text] of Object.entries(p.toolOutput)) {
          if (toolOutput === prev.toolOutput) toolOutput = { ...prev.toolOutput };
          toolOutput[ref] = (toolOutput[ref] ?? "") + text;
        }
        live[id] = {
          ...prev,
          streaming: prev.streaming + p.streaming,
          thinking: prev.thinking + p.thinking,
          toolOutput,
        };
      }
      return { live };
    });
  },

  apply: (env) => {
    const { threadId, event } = env;

    // The three streaming events buffer; everything else is a row, and a row
    // has to land after the text that preceded it.
    switch (event.type) {
      case "assistant_delta":
        buffer(threadId).streaming += event.text;
        return;
      case "thinking_delta":
        buffer(threadId).thinking += event.text;
        return;
      case "tool_output_delta": {
        const p = buffer(threadId);
        p.toolOutput[event.id] = (p.toolOutput[event.id] ?? "") + event.text;
        return;
      }
      default:
        get().flushLive();
    }

    set((s) => {
      const prev = s.live[threadId] ?? IDLE;
      // Rows land for any thread whose timeline someone is holding, not only
      // the open one: gating on the active id dropped every row of the
      // lecture dock's thread whenever the Chat page had another one up.
      const held = threadId in s.items;
      let live: LiveTurn | null = null;
      let rows = s.items[threadId] ?? EMPTY;
      let threads = s.threads;

      const push = (row: HarnessItem) => {
        if (held) rows = [...rows, row];
      };
      const touchThread = (patch: Partial<HarnessThread>) => {
        const i = threads.findIndex((t) => t.id === threadId);
        if (i >= 0) {
          threads = [...threads];
          threads[i] = { ...threads[i], ...patch, updated_at: new Date().toISOString() };
        }
      };

      switch (event.type) {
        case "user_message":
          // The same `meta` Rust writes for the row (`store::apply`), so the
          // bubble says "at 3:40" from the moment it appears rather than only
          // after a reload. `undefined` leaves `meta` null, which is every
          // message not sent from the lecture dock.
          push(rowFrom(env, "user", event.text, event.at == null ? undefined : { at: event.at }));
          live = { ...prev, running: true };
          touchThread({ status: "running" });
          break;
        case "turn_started":
          live = { ...prev, running: true };
          touchThread({ status: "running" });
          break;
        case "assistant_message":
          push(rowFrom(env, "assistant", event.text));
          live = { ...prev, streaming: "" };
          break;
        case "thinking":
          push(rowFrom(env, "thinking", event.text));
          live = { ...prev, thinking: "" };
          break;
        case "tool_started":
          push(
            rowFrom(env, "tool", event.title, {
              kind: event.kind,
              name: event.name,
              input: event.input,
              ok: null,
              output: null,
            }),
          );
          // Text streamed before the call belongs to the call's message; the
          // committed row already carries it.
          live = { ...prev, streaming: "", thinking: "" };
          break;
        case "tool_finished": {
          if (held) {
            for (let i = rows.length - 1; i >= 0; i--) {
              if (rows[i].kind === "tool" && rows[i].ref_id === event.id) {
                const meta = rows[i].meta ? JSON.parse(rows[i].meta!) : {};
                rows = [...rows];
                rows[i] = { ...rows[i], meta: JSON.stringify({ ...meta, ok: event.ok, output: event.output }) };
                break;
              }
            }
          }
          const toolOutput = { ...prev.toolOutput };
          delete toolOutput[event.id];
          live = { ...prev, toolOutput };
          break;
        }
        case "error":
          push(rowFrom(env, "error", event.message));
          break;
        // The queue is Rust's; this only draws it. `queued` is both "new" and
        // "edited" — the id is the key either way.
        case "queued": {
          const list = s.queued[threadId] ?? [];
          const msg = { id: event.id, text: event.text };
          const next = list.some((q) => q.id === event.id)
            ? list.map((q) => (q.id === event.id ? msg : q))
            : [...list, msg];
          return { queued: { ...s.queued, [threadId]: next } };
        }
        case "unqueued":
          return {
            queued: { ...s.queued, [threadId]: (s.queued[threadId] ?? []).filter((q) => q.id !== event.id) },
          };
        // A question was edited or taken back: it and everything after it are
        // gone. The rows Rust has already deleted; this is the copy on screen.
        // `context` is whether the agent went back with them — recorded even
        // for a thread that is not open, since it is a fact about the thread.
        case "rewound": {
          const drift = event.context
            ? s.contextDrift
            : { ...s.contextDrift, [threadId]: true };
          return {
            contextDrift: drift,
            ...(held
              ? {
                  items: {
                    ...s.items,
                    [threadId]: rows.filter((i) => i.id > 0 && i.id < event.from_item_id),
                  },
                }
              : {}),
          };
        }
        case "usage": {
          const usage: ThreadUsage = {
            inputTokens: event.input_tokens,
            outputTokens: event.output_tokens,
            contextTokens: event.context_tokens,
            contextWindow: event.context_window,
            costUsd: event.cost_usd,
          };
          touchThread({ usage: JSON.stringify(usage) });
          break;
        }
        // The windows are the account's, not the thread's: Codex reports them
        // off the shared server with no thread attached at all.
        case "rate_limits":
          return { rateLimits: { ...s.rateLimits, [env.provider]: event.windows } };
        case "turn_finished": {
          // Whatever was still streaming has just been committed as a row by
          // the bridge — including the half-written answer of a turn that was
          // stopped — so clearing the live tail here loses nothing.
          //
          // The thread is only *idle* if nothing is waiting behind this turn.
          // Rust sends the next queued message the moment this event lands,
          // and a spinner that stopped for those few milliseconds — with the
          // composer swapping stop for send and back — read as the answer
          // having finished when it had not started.
          // The stop leaves a row behind (`store.rs`), and it has to land
          // now rather than on the next reload — it is the line that says
          // why the answer above it stops mid-sentence.
          if (event.status === "interrupted") push(rowFrom(env, "interrupted", ""));
          const more = (s.queued[threadId]?.length ?? 0) > 0;
          live = { ...IDLE, running: more };
          touchThread({ status: event.status === "failed" ? "error" : more ? "running" : "idle" });
          break;
        }
        case "thread_titled":
          // The name arrives a beat after the first turn ends, from a naming
          // turn of its own; the row is already written.
          touchThread({ title: event.title });
          break;
        case "session_started":
          touchThread({ provider_session_id: event.provider_session_id, model: event.model ?? undefined });
          break;
        case "exited":
          if (prev.running) {
            live = { ...IDLE };
            touchThread({ status: "idle" });
          }
          break;
      }

      return {
        items: held && rows !== s.items[threadId] ? { ...s.items, [threadId]: rows } : s.items,
        threads,
        live: live ? { ...s.live, [threadId]: live } : s.live,
      };
    });
  },
}));

export const liveFor = (id: number | null, live: Record<number, LiveTurn>): LiveTurn =>
  (id != null && live[id]) || IDLE;

export const itemsFor = (id: number | null, items: Record<number, HarnessItem[]>): HarnessItem[] =>
  (id != null && items[id]) || EMPTY;

export const anyRunning = (live: Record<number, LiveTurn>) => Object.values(live).some((l) => l.running);
