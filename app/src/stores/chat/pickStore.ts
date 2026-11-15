import { create } from "zustand";

import { harnessSelectModel, pickSaves, pickSelection, type Selection } from "@/lib/harness";
import { useHarnessStore } from "@/stores/chat/harnessStore";

/**
 * The open chats' model picks on their way to Rust (`docs/harness.md`,
 * "Changing provider mid-chat"). A pick is held here from the click until
 * Rust has stored it, so the composer names it at once; one that names no
 * model yet (another provider, its catalogue still loading) is held until
 * the composer fills it. Every pick a click makes in one tick is one save.
 * A refusal keeps Rust's sentence for the picker to show, and the chat
 * keeps the selection it had.
 */
export interface PickState {
  /** Per thread: the pick the composer shows over the thread's selection. */
  held: Record<number, Selection>;
  /** Per thread: why Rust refused the last pick, until the next one. */
  refused: Record<number, string>;
  /** A pick on an open chat whose selection is `current`. */
  pick: (thread: number, current: Selection, change: Partial<Selection>) => void;
  /** A send went out carrying what the composer showed; Rust stores it with
   *  the message, so nothing is held any more. */
  sent: (thread: number) => void;
}

/** `save` stores a pick; `stored` is told each one Rust kept. Explicit so the
 *  store can be checked without IPC. */
export function createPickStore(
  save: (thread: number, selection: Selection) => Promise<void>,
  stored: (thread: number, selection: Selection) => void,
) {
  const scheduled = new Set<number>();
  return create<PickState>((set, get) => {
    const drop = (thread: number, selection: Selection) =>
      set((s) => {
        if (s.held[thread] !== selection) return s;
        const held = { ...s.held };
        delete held[thread];
        return { held };
      });
    const flush = (thread: number) => {
      scheduled.delete(thread);
      const selection = get().held[thread];
      if (!selection || !pickSaves(selection)) return;
      save(thread, selection)
        .then(() => {
          stored(thread, selection);
          drop(thread, selection);
        })
        .catch((e) => {
          // A newer pick is on its way; its own answer says what happened.
          if (get().held[thread] !== selection) return;
          drop(thread, selection);
          set((s) => ({ refused: { ...s.refused, [thread]: String(e) } }));
        });
    };
    return {
      held: {},
      refused: {},
      pick: (thread, current, change) => {
        const base = get().held[thread] ?? current;
        const next = pickSelection(base, change);
        set((s) => {
          const refused = { ...s.refused };
          delete refused[thread];
          return { held: { ...s.held, [thread]: next }, refused };
        });
        if (scheduled.has(thread)) return;
        scheduled.add(thread);
        queueMicrotask(() => flush(thread));
      },
      sent: (thread) => {
        const selection = get().held[thread];
        if (selection) stored(thread, selection);
        set((s) => {
          if (!(thread in s.held) && !(thread in s.refused)) return s;
          const held = { ...s.held };
          const refused = { ...s.refused };
          delete held[thread];
          delete refused[thread];
          return { held, refused };
        });
      },
    };
  });
}

export const usePickStore = createPickStore(
  (thread, s) => harnessSelectModel(thread, s.provider, s.model, s.effort),
  (thread, s) => useHarnessStore.getState().selected(thread, s),
);
