import { create } from "zustand";
import {
  CLAUDE_MODELS,
  defaultSelection,
  getHarnessItems,
  getHarnessThreads,
  type HarnessEnvelope,
  type HarnessItem,
  type HarnessThread,
  type Provider,
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

interface HarnessState {
  threads: HarnessThread[];
  /** null is the empty composer — a thread is created by the first send. */
  activeId: number | null;
  items: HarnessItem[];
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

  loadThreads: () => Promise<void>;
  open: (id: number | null) => Promise<void>;
  setProvider: (p: Provider) => void;
  setModel: (m: string | null) => void;
  setReasoning: (level: string | null) => void;
  setSubject: (id: number | null) => void;
  loadSubjects: () => Promise<void>;
  apply: (env: HarnessEnvelope) => void;
  /** The thread is being created by a send that has not returned an id yet. */
  beginNew: () => void;
  removed: (id: number) => void;
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
  items: [],
  live: {},
  rateLimits: {},
  provider: "claude",
  subjectId: null,
  subjects: [],
  // Claude's catalogue is static, so the composer can open already pointing
  // at a real model. Codex's arrives from its CLI; the composer fills both in
  // the moment that list lands (`Composer.tsx`).
  ...defaultSelection(CLAUDE_MODELS),

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
      set({ activeId: null, items: [] });
      return;
    }
    set({ activeId: id, items: [] });
    const items = await getHarnessItems(id);
    // Guard against a switch while the query was in flight.
    if (get().activeId === id) set({ items });
  },

  // The two agents share no model ids and no level vocabulary, so switching
  // agent replaces both rather than carrying a selection that cannot apply.
  // Codex has no static catalogue, so its selection is empty for the beat
  // before its CLI answers and the composer fills it in.
  setProvider: (provider) =>
    set({
      provider,
      ...(provider === "claude" ? defaultSelection(CLAUDE_MODELS) : { model: null, reasoning: null }),
    }),
  setModel: (model) => set({ model }),
  setReasoning: (reasoning) => set({ reasoning }),
  setSubject: (subjectId) => set({ subjectId }),
  loadSubjects: async () => set({ subjects: await getSubjects() }),
  beginNew: () => set({ items: [] }),

  removed: (id) =>
    set((s) => ({
      threads: s.threads.filter((t) => t.id !== id),
      activeId: s.activeId === id ? null : s.activeId,
      items: s.activeId === id ? [] : s.items,
    })),

  apply: (env) => {
    const { threadId, event } = env;
    set((s) => {
      const prev = s.live[threadId] ?? IDLE;
      const onScreen = s.activeId === threadId;
      let live: LiveTurn | null = null;
      let items = s.items;
      let threads = s.threads;

      const push = (row: HarnessItem) => {
        if (onScreen) items = [...items, row];
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
          push(rowFrom(env, "user", event.text));
          live = { ...prev, running: true };
          touchThread({ status: "running" });
          break;
        case "turn_started":
          live = { ...prev, running: true };
          touchThread({ status: "running" });
          break;
        case "assistant_delta":
          live = { ...prev, streaming: prev.streaming + event.text };
          break;
        case "assistant_message":
          push(rowFrom(env, "assistant", event.text));
          live = { ...prev, streaming: "" };
          break;
        case "thinking_delta":
          live = { ...prev, thinking: prev.thinking + event.text };
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
        case "tool_output_delta":
          live = {
            ...prev,
            toolOutput: { ...prev.toolOutput, [event.id]: (prev.toolOutput[event.id] ?? "") + event.text },
          };
          break;
        case "tool_finished": {
          if (onScreen) {
            for (let i = items.length - 1; i >= 0; i--) {
              if (items[i].kind === "tool" && items[i].ref_id === event.id) {
                const meta = items[i].meta ? JSON.parse(items[i].meta!) : {};
                items = [...items];
                items[i] = { ...items[i], meta: JSON.stringify({ ...meta, ok: event.ok, output: event.output }) };
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
        case "rate_limits": {
          const t = threads.find((t) => t.id === threadId);
          if (t) {
            return { rateLimits: { ...s.rateLimits, [t.provider]: event.windows } };
          }
          break;
        }
        case "turn_finished":
          live = { ...IDLE };
          touchThread({ status: event.status === "failed" ? "error" : "idle" });
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
        items,
        threads,
        live: live ? { ...s.live, [threadId]: live } : s.live,
      };
    });
  },
}));

export const liveFor = (id: number | null, live: Record<number, LiveTurn>): LiveTurn =>
  (id != null && live[id]) || IDLE;

export const anyRunning = (live: Record<number, LiveTurn>) => Object.values(live).some((l) => l.running);
