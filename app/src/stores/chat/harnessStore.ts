import { create } from "zustand";
import { readQuestionDrafts, saveQuestionDrafts, pruneQuestionDrafts, type QuestionDraft, type QuestionDrafts } from "./questionDrafts";
import {
  DEFAULT_CHAT_PROVIDER,
  defaultSelectionFor,
  providerAvailability,
  parseItemMeta,
  getHarnessThreads,
  harnessLoad,
  liveRunId,
  runFromRow,
  type HarnessEnvelope,
  type HarnessItem,
  type HarnessRequest,
  type HarnessRun,
  type HarnessThread,
  type Provider,
  type QueuedMessage,
  type RateWindow,
  type Selection,
  type ThreadUsage,
} from "@/lib/harness";
import { getSubjects, type Subject } from "@/lib/db";
import { sqliteUtcToMs } from "@/lib/format/format";
import { useWidgetStore } from "./widgetStore";
import { useProposalVersionStore } from "./proposalVersionStore";
import { useProposalStore } from "./proposalStore";
import { forgetFolds } from "./runFolds";

/** A thread's mid-turn streams, which are not yet rows in the database. */
export interface LiveTurn {
  running: boolean;
  streaming: string;
  thinking: string;
  toolOutput: Record<string, string>;
  /** How the newest turn heard of ended (`turn_finished`), a turn the CLI
   *  started itself included, which has no run to say so. */
  ended?: "completed" | "interrupted" | "failed";
}

const IDLE: LiveTurn = { running: false, streaming: "", thinking: "", toolOutput: {} };

/** One shared empty array: zustand compares by reference, so a fresh `[]`
 *  would re-render the timeline on every store write. */
const EMPTY: HarnessItem[] = [];
const NO_RUNS: Record<number, HarnessRun> = {};
const NO_REQUESTS: Record<number, HarnessRequest> = {};

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
  /** Each held thread's runs by id, read with its rows and kept by
   *  `user_message` and `run_settled`; the timeline folds by them. */
  runs: Record<number, Record<number, HarnessRun>>;
  live: Record<number, LiveTurn>;
  rateLimits: Partial<Record<Provider, RateWindow[]>>;
  /** Composer selection for the *next* thread; an open thread keeps its own. */
  provider: Provider;
  model: string | null;
  /** A per-turn dial, not stored on the thread. */
  reasoning: string | null;
  subjects: Subject[];
  /** `subjects` has been read once; before that an empty list says nothing. */
  subjectsLoaded: boolean;
  /** Each thread's waiting messages: a mirror of its queued runs, read
   *  with its rows and replaced by every `queue` event. */
  queued: Record<number, QueuedMessage[]>;
  /** Each thread's requests by id, read with its rows and replaced by every
   *  `request_opened` and `request_updated`; a `request` row names one. */
  requests: Record<number, Record<number, HarnessRequest>>;
  /** Threads where a rewind could not reach the agent. Never cleared: the
   *  agent's context still holds what left the screen. */
  contextDrift: Record<number, boolean>;
  questionDrafts: QuestionDrafts;
  setQuestionDraft: (thread: number, request: number, question: string, draft: QuestionDraft) => void;
  clearQuestionDraft: (thread: number, request: number) => void;

  loadThreads: () => Promise<void>;
  /** A view put this thread on screen: count it, and read its rows if nobody
   *  else was. `borrowFrom` is the thread the view showed a moment ago. */
  hold: (id: number, borrowFrom?: number | null) => Promise<void>;
  /** The view moved off it. Rows stay cached for an instant switch back, but
   *  the thread is no longer protected from `release`. */
  unhold: (id: number) => void;
  /** Read a thread's rows and queue into the map (`hold` calls it). */
  load: (id: number) => Promise<void>;
  setProvider: (p: Provider) => void;
  setModel: (m: string | null) => void;
  setReasoning: (level: string | null) => void;
  /** Rust stored this selection for the thread (`harnessSelectModel`
   *  resolved): what `selected` will say, written now so the composer does
   *  not show the old one while the event is on its way. */
  selected: (id: number, selection: Selection) => void;
  loadSubjects: () => Promise<void>;
  apply: (env: HarnessEnvelope) => void;
  /** `apply` without the repeat check. `rowsOnly` is `load` folding what
   *  arrived during its read onto the snapshot: rows and runs, since the
   *  rest of each envelope was folded as it arrived. */
  fold: (env: HarnessEnvelope, rowsOnly?: boolean) => void;
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

/** The newest envelope applied; Rust numbers them in order. */
let lastSequence = 0;
/** Envelopes per thread that arrived while `load` was reading its rows. */
const loading = new Map<number, HarnessEnvelope[]>();
/** The revision each held thread's rows were read at (`harnessLoad`). An
 *  envelope at or below it tells of rows the snapshot already holds. */
const loaded = new Map<number, number>();
/** Folded into `live` only; re-folding them after a read would repeat text. */
const LIVE_ONLY = new Set(["assistant_delta", "thinking_delta", "tool_output_delta"]);
/** Events after which the streamed answer text is a row, or over. */
const TAKES_STREAM = new Set(["assistant_message", "tool_started", "turn_finished"]);
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
    run_id: env.runId ?? null,
  };
}

export const useHarnessStore = create<HarnessState>((set, get) => ({
  threads: [],
  threadsLoaded: false,
  holds: {},
  items: {},
  runs: {},
  live: {},
  rateLimits: {},
  queued: {},
  requests: {},
  contextDrift: {},
  questionDrafts: readQuestionDrafts(),
  setQuestionDraft: (thread, request, question, draft) => set((s) => {
    const asked = s.requests[thread]?.[request];
    const questions = asked?.payload.questions as { id: string; secret: boolean }[] | undefined;
    if (asked?.kind !== "question" || asked.status !== "pending" || !questions?.some((q) => q.id === question && !q.secret)) return s;
    return { questionDrafts: saveQuestionDrafts({ ...s.questionDrafts, [thread]: { ...s.questionDrafts[thread], [request]: { ...s.questionDrafts[thread]?.[request], [question]: draft } } }) };
  }),
  clearQuestionDraft: (thread, request) => set((s) => ({ questionDrafts: pruneQuestionDrafts(s.questionDrafts, thread, (id) => id !== request) })),
  provider: DEFAULT_CHAT_PROVIDER,
  subjects: [],
  subjectsLoaded: false,
  // Empty until `useProviderModels` lands the list.
  ...defaultSelectionFor(DEFAULT_CHAT_PROVIDER),

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
    // What arrives during the read may be newer than it: re-fold it on top.
    const during: HarnessEnvelope[] = [];
    loading.set(id, during);
    const snapshot = await harnessLoad(id).catch(() => null);
    loading.delete(id);
    if (!(id in get().items)) return;
    if (snapshot) loaded.set(id, snapshot.revision);
    else loaded.delete(id);
    const rows = snapshot?.items ?? [];
    const runs = snapshot ? Object.fromEntries(snapshot.runs.map((r) => [r.id, runFromRow(r)])) : NO_RUNS;
    const waiting: QueuedMessage[] = snapshot?.queued ?? [];
    const asked = snapshot ? Object.fromEntries((snapshot.requests ?? []).map((r) => [r.id, r])) : NO_REQUESTS;
    set((s) => ({
      items: { ...s.items, [id]: rows },
      runs: { ...s.runs, [id]: runs },
      queued: { ...s.queued, [id]: waiting },
      requests: { ...s.requests, [id]: asked },
      questionDrafts: pruneQuestionDrafts(s.questionDrafts, id, (request) => asked[request]?.status === "pending" || asked[request]?.delivery === "sending"),
    }));
    // Only what the snapshot does not hold yet: `fold` skips the rest by revision.
    for (const env of during) get().fold(env, true);
    if (!snapshot) return;
    // The streaming answer is the snapshot's draft, then every delta told at
    // or above its revision (the draft holds the ones below), up to the
    // next row that takes the text. Deltas that arrived during the read
    // were folded as they came, so the tail is rebuilt, not appended.
    get().flushLive();
    let streaming = snapshot.draft?.text ?? "";
    for (const env of during) {
      const fresh = env.revision == null || env.revision >= snapshot.revision;
      const newer = env.revision == null || env.revision > snapshot.revision;
      if (env.event.type === "assistant_delta") streaming += fresh ? env.event.text : "";
      else if (newer && TAKES_STREAM.has(env.event.type)) streaming = "";
    }
    // A run the snapshot shows going out or being answered is a turn in
    // flight, whatever this view had heard of it.
    const answering = liveRunId(runs) != null;
    set((s) => {
      const prev = s.live[id];
      if (!prev && !streaming && !answering) return s;
      const base = prev ?? IDLE;
      return { live: { ...s.live, [id]: { ...base, running: base.running || answering, streaming } } };
    });
  },

  // Agents share no model ids or levels, so a switch replaces both.
  setProvider: (selected) => {
    const provider = providerAvailability(selected).supported ? selected : DEFAULT_CHAT_PROVIDER;
    set({ provider, ...defaultSelectionFor(provider) });
  },
  setModel: (model) => set({ model }),
  setReasoning: (reasoning) => set({ reasoning }),
  selected: (id, { provider, model, effort }) =>
    set((s) => ({ threads: s.threads.map((t) => (t.id === id ? { ...t, provider, model, effort } : t)) })),
  loadSubjects: async () => set({ subjects: await getSubjects(), subjectsLoaded: true }),

  release: (id) =>
    set((s) => {
      // Never one a Chat view holds, or the dock unmounting would empty a tab
      // showing the same thread mid-turn.
      if (s.holds[id] || !(id in s.items)) return s;
      loaded.delete(id);
      const items = { ...s.items };
      delete items[id];
      const runs = { ...s.runs };
      delete runs[id];
      return { items, runs };
    }),

  removed: (id) => {
    pending.delete(id);
    loaded.delete(id);
    useWidgetStore.getState().prune(id);
    useProposalVersionStore.getState().prune(id);
    useProposalStore.getState().prune(id);
    set((s) => {
      const queued = { ...s.queued };
      delete queued[id];
      const requests = { ...s.requests };
      delete requests[id];
      const items = { ...s.items };
      delete items[id];
      const runs = { ...s.runs };
      delete runs[id];
      const live = { ...s.live };
      delete live[id];
      const holds = { ...s.holds };
      delete holds[id];
      const contextDrift = { ...s.contextDrift };
      delete contextDrift[id];
      return {
        // A Chat tab still pointing at it notices the thread is gone from
        // this list and walks itself back to the empty composer (`ChatPage`).
        threads: s.threads.filter((t) => t.id !== id),
        items,
        runs,
        queued,
        requests,
        live,
        holds,
        contextDrift,
        questionDrafts: pruneQuestionDrafts(s.questionDrafts, id, () => false),
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
    if (env.sequence != null) {
      if (env.sequence <= lastSequence) return;
      lastSequence = env.sequence;
    }
    loading.get(env.threadId)?.push(env);
    get().fold(env);
  },

  fold: (env, rowsOnly = false) => {
    const { threadId, event } = env;
    // Rows the snapshot already holds are not written again: a rewind told
    // twice would take rows written after it.
    const at = loaded.get(threadId);
    const stale = at != null && env.revision != null && env.revision <= at;
    if (rowsOnly && (stale || LIVE_ONLY.has(event.type))) return;
    if (event.type === "rewound" && !stale && !rowsOnly) {
      forgetFolds(threadId);
      useWidgetStore.getState().prune(threadId, event.from_item_id);
      useProposalVersionStore.getState().prune(threadId, event.from_item_id);
      useProposalStore.getState().prune(threadId, event.from_item_id);
    }
    if (event.type === "proposal") {
      if (!rowsOnly) useProposalStore.getState().receive(event.proposal);
      return;
    }

    switch (event.type) {
      case "assistant_delta":
        // One told below the snapshot's revision is already in its draft.
        if (at != null && env.revision != null && env.revision < at) return;
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
      // Its rows and runs take the event unless the snapshot already has it.
      const held = threadId in s.items && !stale;
      let live: LiveTurn | null = null;
      let rows = s.items[threadId] ?? EMPTY;
      let threads = s.threads;
      let runs = s.runs[threadId] ?? NO_RUNS;
      const setRun = (run: HarnessRun) => {
        if (held) runs = { ...runs, [run.id]: run };
      };
      let asked = s.requests[threadId] ?? NO_REQUESTS;
      let questionDrafts = s.questionDrafts;
      const setRequest = (request: HarnessRequest) => {
        if (held) asked = { ...asked, [request.id]: request };
        if (request.status !== "pending" && request.delivery !== "sending") questionDrafts = pruneQuestionDrafts(questionDrafts, threadId, (id) => id !== request.id);
      };

      // A row Rust updated in place (the same provider message again) replaces its copy.
      const push = (row: HarnessItem) => {
        if (!held) return;
        const at = env.itemId == null ? -1 : rows.findIndex((r) => r.id === env.itemId);
        rows = at < 0 ? [...rows, row] : rows.map((r, i) => (i === at ? row : r));
      };
      const touchThread = (patch: Partial<HarnessThread>) => {
        const i = threads.findIndex((t) => t.id === threadId);
        if (i >= 0) {
          threads = [...threads];
          threads[i] = { ...threads[i], ...patch, updated_at: new Date().toISOString() };
        }
      };
      // The list's Approval and Input, for every thread, loaded or not; it does not
      // reorder the list.
      const waiting = (count: number, questions = 0) => {
        const i = threads.findIndex((t) => t.id === threadId);
        if (i >= 0 && (threads[i].pending_requests !== count || threads[i].pending_questions !== questions)) {
          threads = [...threads];
          threads[i] = { ...threads[i], pending_requests: count, pending_questions: questions };
        }
      };

      switch (event.type) {
        case "user_message":
          // Live rows carry the same `meta` Rust stores (`store::apply`).
          push(rowFrom(env, "user", event.text, event.at == null ? undefined : { at: event.at }));
          // Until `run_settled` brings the stored interval, the clock is ours.
          if (env.runId != null) {
            setRun({ id: env.runId, status: "running", delivery: "unknown", answer_item_id: null, started_ms: Date.now(), ended_ms: null });
          }
          live = { ...prev, running: true };
          touchThread({ status: "running" });
          break;
        case "continued":
          push(rowFrom(env, "continuation", event.text, { proposal: event.proposal }));
          if (env.runId != null) {
            setRun({ id: env.runId, status: "running", delivery: "unknown", answer_item_id: null, started_ms: Date.now(), ended_ms: null });
          }
          live = { ...prev, running: true };
          touchThread({ status: "running" });
          break;
        case "selected":
          touchThread({ provider: event.provider, model: event.model, effort: event.effort });
          break;
        case "permission_mode":
          touchThread({ permission_mode: event.mode });
          break;
        case "request_opened":
          // The row holds the id only; the request is in `requests`.
          push({ ...rowFrom(env, "request", ""), ref_id: String(event.request.id), content: null });
          setRequest(event.request);
          waiting(event.waiting, event.waiting_questions);
          break;
        case "request_updated":
          setRequest(event.request);
          waiting(event.waiting, event.waiting_questions);
          break;
        case "handoff": {
          const run = runs[event.run_id];
          if (run) setRun({ ...run, handoff: { kind: event.kind, status: event.status, from: event.from, omitted: event.omitted } });
          break;
        }
        case "run_selected": {
          const run = env.runId == null ? undefined : runs[env.runId];
          if (run) setRun({ ...run, provider: event.provider, model: event.model, effort: event.effort });
          break;
        }
        case "run_settled": {
          if (!held) break;
          if (env.runId != null) {
            const before = runs[env.runId];
            setRun({
              // What it was sent with outlives the settlement; a stop does not.
              ...(before?.provider ? { provider: before.provider, model: before.model, effort: before.effort } : {}),
              ...(before?.resolved_model ? { resolved_model: before.resolved_model } : {}),
              ...(before?.handoff ? { handoff: before.handoff } : {}),
              id: env.runId,
              status: event.status ?? "completed",
              delivery: event.delivery,
              answer_item_id: event.answer_item_id,
              started_ms: sqliteUtcToMs(event.started_at ?? null) ?? before?.started_ms ?? null,
              ended_ms: sqliteUtcToMs(event.ended_at ?? null) ?? Date.now(),
            });
          }
          const work = new Set(event.work_item_ids);
          rows = rows.map((r) =>
            r.id === event.answer_item_id
              ? { ...r, display_role: "answer" }
              : work.has(r.id)
                ? { ...r, display_role: "commentary" }
                : r,
          );
          break;
        }
        case "stop_requested": {
          const run = runs[event.run_id];
          if (run && run.ended_ms == null) setRun({ ...run, stopping: true, stop_requested_ms: Date.now() });
          break;
        }
        case "stop_unresponsive": {
          const run = runs[event.run_id];
          if (run && run.ended_ms == null) setRun({ ...run, stopping: true, unresponsive: { others: event.others } });
          break;
        }
        case "delivery_notice":
          push(rowFrom(env, "delivery_notice", event.text));
          break;
        case "compaction":
          push(rowFrom(env, "compaction", ""));
          break;
        case "unprompted_turn":
          push(rowFrom(env, "unprompted", "", { after: event.after }));
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
          push(rowFrom(env, "thinking", event.text, event.ms == null ? undefined : { ms: event.ms }));
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
        case "tool_reviewed":
          if (held) {
            for (let i = rows.length - 1; i >= 0; i--) {
              if (rows[i].kind === "tool" && rows[i].ref_id === event.id) {
                rows = [...rows];
                rows[i] = { ...rows[i], meta: JSON.stringify({ ...parseItemMeta(rows[i]), review: event.review }) };
                break;
              }
            }
          }
          break;
        case "tool_finished": {
          if (held) {
            for (let i = rows.length - 1; i >= 0; i--) {
              if (rows[i].kind === "tool" && rows[i].ref_id === event.id) {
                const meta = parseItemMeta(rows[i]);
                rows = [...rows];
                rows[i] = {
                  ...rows[i],
                  // As Rust does for `ToolFinished` (`harness/event/mod.rs`).
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
          push(
            rowFrom(
              env,
              "error",
              event.message,
              event.auth || event.limit ? { ...(event.auth && { auth: event.auth }), ...(event.limit && { limit: true }) } : undefined,
            ),
          );
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
        // The whole list, as saved; one the snapshot already holds is skipped.
        case "queue":
          return stale ? s : { queued: { ...s.queued, [threadId]: event.messages } };
        case "rewound": {
          const drift = event.context || rowsOnly
            ? s.contextDrift
            : { ...s.contextDrift, [threadId]: true };
          const keep = (i: HarnessItem) => i.id > 0 && i.id < event.from_item_id;
          const kept = rows.filter(keep);
          // Rust deleted the runs asked from the removed rows.
          const gone = new Set(rows.filter((i) => !keep(i) && i.run_id != null).map((i) => i.run_id));
          const left = Object.fromEntries(Object.entries(runs).filter(([id]) => !gone.has(Number(id))));
          // Their requests went with the rows that showed them.
          const stay = Object.fromEntries(
            Object.entries(asked).filter(([, r]) => r.item_id != null && r.item_id < event.from_item_id),
          );
          return {
            contextDrift: drift,
            questionDrafts: pruneQuestionDrafts(questionDrafts, threadId, (id) => id in stay),
            ...(held
              ? {
                  items: { ...s.items, [threadId]: kept },
                  runs: { ...s.runs, [threadId]: left },
                  requests: { ...s.requests, [threadId]: stay },
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
          return rowsOnly ? s : { rateLimits: { ...s.rateLimits, [env.provider]: event.windows } };
        case "turn_finished": {
          // The bridge has committed the live tail. Stay running if a queued
          // message is about to go out, so the spinner does not blink; a
          // held one is not (a stop or a failed turn holds the queue).
          if (event.status === "interrupted") push(rowFrom(env, "interrupted", ""));
          const more = (s.queued[threadId] ?? []).some((q) => !q.held);
          live = { ...IDLE, running: more, ended: event.status };
          touchThread({
            status: event.status === "failed" ? "error" : more ? "running" : "idle",
            settled_at: new Date().toISOString(),
          });
          break;
        }
        case "thread_titled":
          touchThread({ title: event.title });
          break;
        case "session_started": {
          // The thread's model is the student's pick for the next send; what
          // the CLI says it is running belongs to the run it answers.
          const known = threads.find((t) => t.id === threadId)?.model;
          touchThread({ provider_session_id: event.provider_session_id, ...(known == null && event.model ? { model: event.model } : {}) });
          const run = env.runId == null ? undefined : runs[env.runId];
          if (run && event.model && event.model !== run.model) setRun({ ...run, resolved_model: event.model });
          break;
        }
        case "exited":
          if (prev.running) {
            live = { ...IDLE, ended: prev.ended };
            touchThread({ status: "idle" });
          }
          break;
      }

      return {
        items: held && rows !== s.items[threadId] ? { ...s.items, [threadId]: rows } : s.items,
        runs: held && runs !== (s.runs[threadId] ?? NO_RUNS) ? { ...s.runs, [threadId]: runs } : s.runs,
        requests: held && asked !== (s.requests[threadId] ?? NO_REQUESTS) ? { ...s.requests, [threadId]: asked } : s.requests,
        questionDrafts,
        threads: rowsOnly ? s.threads : threads,
        live: live && !rowsOnly ? { ...s.live, [threadId]: live } : s.live,
      };
    });
  },
}));

export const itemsFor = (id: number | null, items: Record<number, HarnessItem[]>): HarnessItem[] =>
  (id != null && items[id]) || EMPTY;

export const runsFor = (id: number | null, runs: Record<number, Record<number, HarnessRun>>) =>
  (id != null && runs[id]) || NO_RUNS;

export const anyRunning = (live: Record<number, LiveTurn>) => Object.values(live).some((l) => l.running);
