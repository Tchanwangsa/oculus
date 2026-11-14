import { create } from "zustand";

/** Notices the student closed, by thread and notice id (`ThreadBanners`).
 *  In memory: a dismissal lasts the session, and a new run's notice has a
 *  new id, so it shows again. */
export const useBannerDismissals = create<{
  dismissed: Record<number, string[]>;
  dismiss: (threadId: number, id: string) => void;
}>((set) => ({
  dismissed: {},
  dismiss: (threadId, id) =>
    set((s) => ({ dismissed: { ...s.dismissed, [threadId]: [...(s.dismissed[threadId] ?? []), id] } })),
}));
