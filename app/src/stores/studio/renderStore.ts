import { create } from "zustand";

import {
  cancelRender,
  renderVideo,
  type RenderPreset,
  type RenderProgress,
} from "@/lib/studio/render";
import { useVideosStore } from "@/stores/studio/videosStore";

/** A render in flight, keyed by the video's absolute folder. */
export interface ActiveRender {
  slug: string;
  preset: RenderPreset;
  /** 0 to 1, or null while the plan is still being made. */
  fraction: number | null;
  /** What the render is doing, for the line under the bar. */
  phase: "planning" | "rendering" | "muxing";
}

interface RenderState {
  active: Record<string, ActiveRender>;
  /** The last failure per folder, until its next render starts. */
  errors: Record<string, string>;
  /** Bumped when a render for the folder ends, however it ended: a page
   *  showing its files reloads on it. */
  settled: Record<string, number>;
  start: (dir: string, slug: string, preset: RenderPreset) => Promise<void>;
  cancel: (dir: string) => void;
  apply: (progress: RenderProgress) => void;
}

/** The folder's name, which is the slug. */
const slugOf = (dir: string) => dir.split("/").filter(Boolean).pop() ?? dir;

/** Where a `rendering` event leaves the bar: segments done plus the part of
 *  this one, over all of them. */
function fractionOf(p: Extract<RenderProgress, { phase: "rendering" }>): number {
  const whole = p.of > 0 ? p.of : 1;
  const part = p.frames > 0 ? p.frame / p.frames : 0;
  return Math.min(1, Math.max(0, (p.segment - 1 + part) / whole));
}

/**
 * Studio renders, fed by `studio-render-progress` (`useStudioEvents`) and by
 * the page that started one. It lives here so a render keeps its bar through
 * navigation, and so the rail and the Recent list can mark it.
 */
export const useRenderStore = create<RenderState>((set, get) => {
  /** Ends the folder's render; a no-op when it already ended. */
  const settle = (dir: string, error?: string) => {
    if (!(dir in get().active)) return;
    set((s) => {
      const { [dir]: _ended, ...active } = s.active;
      const errors = { ...s.errors };
      if (error) errors[dir] = error;
      return { active, errors, settled: { ...s.settled, [dir]: (s.settled[dir] ?? 0) + 1 } };
    });
    void useVideosStore.getState().reload();
  };

  return {
    active: {},
    errors: {},
    settled: {},

    start: async (dir, slug, preset) => {
      if (dir in get().active) return;
      set((s) => {
        const { [dir]: _cleared, ...errors } = s.errors;
        return { errors, active: { ...s.active, [dir]: { slug, preset, fraction: null, phase: "planning" } } };
      });
      try {
        // Null is a cancel; a failure rejects with a sentence fit to show.
        await renderVideo(dir, preset);
        settle(dir);
      } catch (e) {
        settle(dir, e instanceof Error ? e.message : String(e));
      }
    },

    cancel: (dir) => {
      cancelRender(dir).catch((e) => console.error("cancel render failed", e));
    },

    apply: (p) => {
      switch (p.phase) {
        case "complete":
        case "cancelled":
          settle(p.dir);
          return;
        case "error":
          settle(p.dir, p.error);
          return;
        case "planning":
        case "muxing":
        case "rendering": {
          const prev = get().active[p.dir];
          const next: ActiveRender = {
            slug: prev?.slug ?? slugOf(p.dir),
            preset: prev?.preset ?? p.preset,
            fraction: p.phase === "rendering" ? fractionOf(p) : p.phase === "muxing" ? 1 : (prev?.fraction ?? null),
            phase: p.phase,
          };
          set((s) => ({ active: { ...s.active, [p.dir]: next } }));
        }
      }
    },
  };
});

/** Whether any video is rendering. */
export const anyRendering = (active: Record<string, ActiveRender>) => Object.keys(active).length > 0;
