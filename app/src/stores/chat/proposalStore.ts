import { create } from "zustand";
import { proposalApi, type Proposal, type ProposalApi, type ProposalState } from "@/lib/harness/proposals";

interface ProposalStore {
  rows: Record<string, Proposal>;
  drafts: Record<string, ProposalState>;
  busy: Record<string, boolean>;
  errors: Record<string, string | undefined>;
  /** `undone` marks the reply to Undo, which may reopen a card as pending. */
  receive: (row: Proposal, undone?: boolean) => void;
  open: (thread: number, item: number, block: number, kind: string, state: ProposalState, amends?: string | null) => Promise<void>;
  edit: (id: string, state: ProposalState) => void;
  flush: (id: string) => Promise<void>;
  flushThread: (thread: number) => Promise<void>;
  act: (id: string, action: "apply" | "discard" | "undo" | "start") => Promise<void>;
  prune: (thread: number, fromRow?: number) => void;
}

/** One lane per card orders debounced edits before decisions. Drafts are
 * shared across views, but a decided row always wins over a local draft. */
export function createProposalStore(api: ProposalApi = proposalApi, delay = 300) {
  const timers = new Map<string, ReturnType<typeof setTimeout>>();
  const lanes = new Map<string, Promise<unknown>>();
  const opening = new Map<string, Promise<void>>();
  const generations = new Map<number, number>();
  return create<ProposalStore>((set, get) => {
    const cancel = (id: string) => { clearTimeout(timers.get(id)); timers.delete(id); };
    const enqueue = <T,>(id: string, run: () => Promise<T>): Promise<T> => {
      const next = (lanes.get(id) ?? Promise.resolve()).catch(() => {}).then(run);
      lanes.set(id, next);
      void next.finally(() => { if (lanes.get(id) === next) lanes.delete(id); }).catch(() => {});
      return next;
    };
    const fail = (id: string, error: unknown) => set((s) => ({ errors: { ...s.errors, [id]: String(error) } }));
    return {
      rows: {}, drafts: {}, busy: {}, errors: {},
      receive: (row, undone = false) => {
        const previous = get().rows[row.id];
        if (previous && ["undone", "discarded", "superseded"].includes(previous.status) && row.status !== previous.status) return;
        // Undoing cheat entries reopens the card; any other late pending row is stale.
        if (row.status === "pending" && previous && previous.status !== "pending" && !undone) return;
        const phase = (state: ProposalState) => (state.job as { phase?: string } | undefined)?.phase;
        if (previous?.status === "created" && ["complete", "error"].includes(phase(previous.state) ?? "") && phase(row.state) === "running") return;
        if (row.status !== "pending") cancel(row.id);
        set((s) => {
          const drafts = { ...s.drafts };
          if (row.status !== "pending") delete drafts[row.id];
          return { rows: { ...s.rows, [row.id]: row }, drafts };
        });
        if (previous?.status !== row.status && (["created", "undone"].includes(row.status) || undone) && typeof window !== "undefined") {
          window.dispatchEvent(new CustomEvent("oculus:projects-updated"));
          window.dispatchEvent(new CustomEvent("oculus:calendar-updated"));
          if (row.kind === "NewCheatNotes" || row.kind === "EditItems") window.dispatchEvent(new CustomEvent("oculus:revision-updated"));
          if (row.kind === "NewNote") window.dispatchEvent(new CustomEvent("oculus:documents-changed"));
        }
      },
      open: (thread, item, block, kind, state, amends) => {
        const id = `${item}.${block}`;
        if (opening.has(id)) return opening.get(id)!;
        const generation = generations.get(thread) ?? 0;
        const work = api.open(thread, item, block, kind, state, amends).then((row) => {
          if ((generations.get(thread) ?? 0) === generation) get().receive(row);
        }).catch((error) => fail(id, error)).finally(() => opening.delete(id));
        opening.set(id, work);
        return work;
      },
      edit: (id, state) => {
        if (get().rows[id]?.status !== "pending" || get().busy[id]) return;
        cancel(id);
        set((s) => ({ drafts: { ...s.drafts, [id]: state }, errors: { ...s.errors, [id]: undefined } }));
        timers.set(id, setTimeout(() => { void get().flush(id).catch((error) => fail(id, error)); }, delay));
      },
      flush: (id) => {
        cancel(id);
        return enqueue(id, async () => {
          const draft = get().drafts[id];
          const current = get().rows[id];
          if (!draft || current?.status !== "pending") return;
          const generation = generations.get(current.thread_id) ?? 0;
          const row = await api.edit(id, draft);
          if ((generations.get(current.thread_id) ?? 0) !== generation) return;
          get().receive(row);
          set((s) => {
            if (s.drafts[id] !== draft) return {};
            const drafts = { ...s.drafts };
            delete drafts[id];
            return { drafts };
          });
        });
      },
      flushThread: async (thread) => {
        // A send may immediately follow typing. Keep flushing while an edit
        // made during an in-flight save is still waiting in the shared draft.
        let again = true;
        while (again) {
          const ids = Object.values(get().rows).filter((row) => row.thread_id === thread && (get().drafts[row.id] || lanes.has(row.id))).map((row) => row.id);
          await Promise.all(ids.map((id) => get().flush(id)));
          again = Object.values(get().rows).some((row) => row.thread_id === thread && row.status === "pending" && !!get().drafts[row.id]);
        }
      },
      act: async (id, action) => {
        if (get().busy[id]) return;
        cancel(id);
        set((s) => ({ busy: { ...s.busy, [id]: true }, errors: { ...s.errors, [id]: undefined } }));
        try {
          await enqueue(id, async () => {
            const row = get().rows[id];
            if (!row) throw new Error("The card has not loaded yet.");
            const generation = generations.get(row.thread_id) ?? 0;
            const state = get().drafts[id] ?? row.state;
            const next = action === "apply" ? await api.apply(id, state) : await api[action](id);
            if ((generations.get(row.thread_id) ?? 0) === generation) get().receive(next, action === "undo");
          });
        } catch (error) { fail(id, error); }
        finally { set((s) => s.rows[id] ? { busy: { ...s.busy, [id]: false } } : {}); }
      },
      prune: (thread, fromRow) => {
        generations.set(thread, (generations.get(thread) ?? 0) + 1);
        set((s) => {
          const rows = { ...s.rows }, drafts = { ...s.drafts }, errors = { ...s.errors }, busy = { ...s.busy };
          for (const [id, row] of Object.entries(rows)) {
            if (row.thread_id !== thread || (fromRow !== undefined && row.item_id < fromRow)) continue;
            cancel(id);
            delete rows[id]; delete drafts[id]; delete errors[id]; delete busy[id];
          }
          return { rows, drafts, errors, busy };
        });
      },
    };
  });
}

export const useProposalStore = createProposalStore();
