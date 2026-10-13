import { useMemo } from "react";
import { create } from "zustand";
import type { LessonIndex } from "./lessons";

/** Ed's slide statuses in the one direction they move. */
const RANK: Record<string, number> = { unseen: 0, seen: 1, completed: 2 };
const rank = (status: string | null | undefined) => (status == null ? -1 : (RANK[status] ?? -1));

type StatusMap = ReadonlyMap<number, string>;

/**
 * What Ed answered this app session for slides opened here (`markSlideOpened`
 * in `views.ts`), by Ed slide id: the index on disk only learns it at the
 * next sync, and the sidebar's ticks should not wait for one.
 */
export const useSlideStatusStore = create<{ byId: StatusMap }>(() => ({ byId: new Map() }));

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
