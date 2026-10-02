import { create } from "zustand";
import {
  defaultSelectionFor,
  parseItemMeta,
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

/** A thread's mid-turn streams, which are not yet rows in the database. */
export interface LiveTurn {
  running: boolean;
  streaming: string;
  thinking: string;
  toolOutput: Record<string, string>;
}

const IDLE: LiveTurn = { running: false, streaming: "", thinking: "", toolOutput: {} };

/** One shared empty array: zustand compares by reference, so a fresh `[]`
 *  would re-render the timeline on every store write. */
const EMPTY: HarnessItem[] = [];

interface HarnessState {
  threads: HarnessThread[];
  /** Whether `threads` has been read once. Until then a route's thread id
   *  can't be told from a deleted one, so the page must not give up on it. */
  threadsLoaded: boolean;
  /** How many Chat views show each thread. A tab's thread lives in its route
   *  (`chatHref`); the store only counts views so the dock's `release` can't
   *  empty a timeline another view is reading. A count, so two tabs on one
   *  thread don't release it from each other. */
  holds: Record<number, number>;
  /** Rows per thread: any number of Chat tabs and the lecture dock hold one. */
  items: Record<number, HarnessItem[]>;
  live: Record<number, LiveTurn>;
  rateLimits: Partial<Record<Provider, RateWindow[]>>;
  /** Composer selection for the *next* thread; an open thread keeps its own. */
  provider: Provider;
  model: string | null;
  /** A per-turn dial, not stored on the thread. */
  reasoning: string | null;
  /** Subject scope for the *next* thread; null is the general one. */
  subjectId: number | null;
  subjects: Subject[];
  /** A mirror of Rust's queue, folded from `queued`/`unqueued` events. */
  queued: Record<number, QueuedMessage[]>;
  /** Threads where a rewind could not reach the agent. Never cleared: the
   *  agent's context still holds what left the screen. */
  contextDrift: Record<number, boolean>;

  loadThreads: () => Promise<void>;
  /** A view put this thread on screen: count it, and read its rows if nobody
   *  else was. `borrowFrom` is the thread the view showed a moment ago. */
  hold: (id: number, borrowFrom?: number | null) => Promise<void>;
  /** The view moved off it. Rows stay cached for an instant switch back, but
   *  the thread is no longer protected from `release`. */
  unhold: (id: number) => void;
  /** Read a thread's rows and queue into the map (the lecture dock calls it). */
  load: (id: number) => Promise<void>;
  setProvider: (p: Provider) => void;
  setModel: (m: string | null) => void;
  setReasoning: (level: string | null) => void;
  setSubject: (id: number | null) => void;
  loadSubjects: () => Promise<void>;
  apply: (env: HarnessEnvelope) => void;
  /** Fold the buffered deltas into `live` now. */
  flushLive: () => void;
  removed: (id: number) => void;
  /** Forget one thread's rows; with `removed`, the only way the map shrinks,
   *  so a view leaving a thread must call it. */
  release: (id: number) => void;
}

/** Deltas arrive per token; buffering caps renders at one per tick. Every
 *  non-delta event flushes first, so no row overtakes its text. */
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

/** A row from a live event, keyed on the Rust row id when there is one. */
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
  threadsLoaded: false,
  holds: {},
  items: {},
  live: {},
  rateLimits: {},
  queued: {},
  contextDrift: {},
  provider: "claude",
  subjectId: null,
  subjects: [],
  // Empty until `useProviderModels` lands the list.
  ...defaultSelectionFor("claude"),

  loadThreads: async () => {
    const threads = await getHarnessThreads();
    set((s) => {
      const live = { ...s.live };
      for (const t of threads) {
        if (t.status === "running" && !live[t.id]) live[t.id] = { ...IDLE, running: true };
      }
      return { threads, live, threadsLoaded: true };
    });
  },

  hold: async (id, borrowFrom) => {
    const first = !get().holds[id];
    set((s) => ({
      holds: { ...s.holds, [id]: (s.holds[id] ?? 0) + 1 },
      // Borrow the outgoing thread's rows while reading: no rows is the empty
      // composer's state, so a switch would flash it.
      items:
        id in s.items || borrowFrom == null || !s.items[borrowFrom]
          ? s.items
          : { ...s.items, [id]: s.items[borrowFrom] },
    }));
    // A second view on a held thread reads nothing: `apply` keeps it live.
    if (first) await get().load(id);
  },

  unhold: (id) =>
    set((s) => {
      const n = (s.holds[id] ?? 0) - 1;
      const holds = { ...s.holds };
      if (n > 0) holds[id] = n;
      else delete holds[id];
      return { holds };
    }),

  load: async (id) => {
    // Claim the key first: "held" is the test the guards below and `apply` make.
    if (!(id in get().items)) set((s) => ({ items: { ...s.items, [id]: EMPTY } }));
    // Guard against a release mid-read. A failed read still writes `[]` to
    // replace any borrowed rows.
    const rows = await getHarnessItems(id).catch(() => [] as HarnessItem[]);
    if (id in get().items) set((s) => ({ items: { ...s.items, [id]: rows } }));
    const waiting = await harnessQueued(id).catch(() => [] as QueuedMessage[]);
    if (id in get().items) set((s) => ({ queued: { ...s.queued, [id]: waiting } }));
  },

  // Agents share no model ids or levels, so a switch replaces both.
  setProvider: (provider) => set({ provider, ...defaultSelectionFor(provider) }),
  setModel: (model) => set({ model }),
  setReasoning: (reasoning) => set({ reasoning }),
  setSubject: (subjectId) => set({ subjectId }),
  loadSubjects: async () => set({ subjects: await getSubjects() }),

  release: (id) =>
    set((s) => {
      // Never one a Chat view holds, or the dock unmounting would empty a tab
      // showing the same thread mid-turn.
      if (s.holds[id] || !(id in s.items)) return s;
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
        // A Chat tab still pointing at it notices the thread is gone from
        // this list and walks itself back to the empty composer (`ChatPage`).
        threads: s.threads.filter((t) => t.id !== id),
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
      // Any held thread, not just the active one (the dock's is never active).
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
          // Live rows carry the same `meta` Rust stores (`store::apply`).
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
          live = { ...prev, streaming: "", thinking: "" };
          break;
        case "tool_finished": {
          if (held) {
            for (let i = rows.length - 1; i >= 0; i--) {
              if (rows[i].kind === "tool" && rows[i].ref_id === event.id) {
                const meta = parseItemMeta(rows[i]);
                rows = [...rows];
                rows[i] = {
                  ...rows[i],
                  // As Rust does for `ToolFinished` (`harness/event.rs`).
                  content: event.title?.trim() ? event.title : rows[i].content,
                  meta: JSON.stringify({ ...meta, ok: event.ok, output: event.output }),
                };
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
          push(rowFrom(env, "error", event.message, event.auth ? { auth: event.auth } : undefined));
          break;
        case "permission_needed":
          push(
            rowFrom(env, "permission", event.target ?? "", {
              tool: event.tool,
              action: event.action,
              target: event.target,
              rule: event.rule,
            }),
          );
          break;
        // `queued` is both "new" and "edited".
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
        // Account-scoped, not per thread.
        case "rate_limits":
          return { rateLimits: { ...s.rateLimits, [env.provider]: event.windows } };
        case "turn_finished": {
          // The bridge has committed the live tail. Stay running if a queued
          // message is about to go out, so the spinner does not blink.
          if (event.status === "interrupted") push(rowFrom(env, "interrupted", ""));
          const more = (s.queued[threadId]?.length ?? 0) > 0;
          live = { ...IDLE, running: more };
          touchThread({ status: event.status === "failed" ? "error" : more ? "running" : "idle" });
          break;
        }
        case "thread_titled":
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

export const itemsFor = (id: number | null, items: Record<number, HarnessItem[]>): HarnessItem[] =>
  (id != null && items[id]) || EMPTY;

export const anyRunning = (live: Record<number, LiveTurn>) => Object.values(live).some((l) => l.running);
