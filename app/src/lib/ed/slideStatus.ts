import { useMemo } from "react";
import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { LessonIndex } from "./lessons";

/** Ed's slide statuses in the one direction they move. */
const RANK: Record<string, number> = { unseen: 0, seen: 1, completed: 2 };
const rank = (status: string | null | undefined) => (status == null ? -1 : (RANK[status] ?? -1));

type StatusMap = ReadonlyMap<number, string>;

/**
 * What Ed answered for slides opened here (`markSlideOpened` in `views.ts`),
 * what the Lessons tab read on open (`refreshSlideStatuses`) and what a
 * passing submission proved, by Ed slide id. The index on disk only learns
 * a status at the next sync, so the sidebar's ticks and the Lessons tab's
 * counts read this over it. It is kept in the webview's storage, so a
 * restart does not drop what only this store knows.
 */
export const useSlideStatusStore = create<{ byId: StatusMap }>()(
  persist<{ byId: StatusMap }, [], [], { byId: [number, string][] }>(
    () => ({ byId: new Map<number, string>() }),
    {
      name: "oculus-ed-slide-statuses",
      partialize: (s) => ({ byId: [...s.byId] }),
      merge: (saved, current) => ({
        ...current,
        byId: new Map((saved as { byId?: [number, string][] } | undefined)?.byId ?? []),
      }),
    },
  ),
);

/** Whether a submission of this Ed status completes its challenge's slide,
 *  as Ed's web app does after a mark: passed, or submitted without marking. */
export const completesSlide = (submissionStatus: string) =>
  submissionStatus === "passed" || submissionStatus === "submitted";

/** Record Ed's `status` for slide `id`; a status never moves back. */
export function recordSlideStatus(id: number, status: string): void {
  useSlideStatusStore.setState((s) => {
    if (rank(status) <= rank(s.byId.get(id))) return s;
    return { byId: new Map(s.byId).set(id, status) };
  });
}

/** `index` with each slide's status raised to what this session recorded. */
export function withSlideStatuses(index: LessonIndex, byId: StatusMap): LessonIndex {
  let changed = false;
  const slides = index.slides.map((slide) => {
    const known = slide.id == null ? undefined : byId.get(slide.id);
    if (known == null || rank(known) <= rank(slide.status)) return slide;
    changed = true;
    return { ...slide, status: known };
  });
  return changed ? { ...index, slides } : index;
}

/** The lesson's index as this session knows it: `withSlideStatuses`, live. */
export function useLiveLessonIndex<T extends LessonIndex | null>(index: T): T {
  const byId = useSlideStatusStore((s) => s.byId);
  return useMemo(() => (index ? withSlideStatuses(index, byId) : index) as T, [index, byId]);
}
