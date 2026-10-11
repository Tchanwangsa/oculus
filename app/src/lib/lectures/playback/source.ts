import type { Lecture, SourceNum } from "@/lib/db";

export interface SourcePlan {
  source: SourceNum;
  /** Media-server URL for that source's file. */
  src: string;
  /** The element in the player's frame the video is moved into. */
  host: HTMLElement;
}

export const els = new Map<SourceNum, HTMLVideoElement>();
/** Sources currently on screen. The first of them is `state.leaderSource`. */
export const onScreen = new Set<SourceNum>();

/** The mutable state the sibling files share; a module-level `let` cannot be
 *  assigned from another file. */
export const state: {
  leaderSource: SourceNum | null;
  parkedHost: HTMLDivElement | null;
  saveTicker: ReturnType<typeof setInterval> | null;
  syncTicker: ReturnType<typeof setInterval> | null;
  current: Lecture | null;
  /** Last second written, so a paused element doesn't rewrite the same row. */
  lastWritten: number;
  /** The progress writes, chained so they land in order and can be awaited. */
  saving: Promise<void>;
  /** Up Next's Play: the lecture the next adoption starts, and from where. */
  autoplay: { id: string; at?: number } | null;
  /**
   * Bumped on every adoption. A player can adopt the elements before the one
   * that had them lets go; the token stops the old one parking them after the
   * handover.
   */
  claim: number;
} = {
  leaderSource: null,
  parkedHost: null,
  saveTicker: null,
  syncTicker: null,
  current: null,
  lastWritten: -1,
  saving: Promise.resolve(),
  autoplay: null,
  claim: 0,
};

/** Off screen rather than `display: none`, where a browser may stop media. */
export function parked(): HTMLDivElement {
  if (!state.parkedHost) {
    const host = document.createElement("div");
    host.setAttribute("aria-hidden", "true");
    // A real size, not 1×1: WebKit sizes the decode path to the picture, so a
    // tiny host brings the video back blurry until the decoder catches up.
    host.style.cssText =
      "position:fixed;left:-10000px;top:0;width:480px;height:270px;overflow:hidden;pointer-events:none";
    document.body.appendChild(host);
    state.parkedHost = host;
  }
  return state.parkedHost;
}

export function leaderVideo(): HTMLVideoElement | null {
  return state.leaderSource != null ? (els.get(state.leaderSource) ?? null) : null;
}

/** The element for one source, if it has ever been on screen. */
export function videoForSource(source: SourceNum): HTMLVideoElement | null {
  return els.get(source) ?? null;
}

export function videoFor(source: SourceNum): HTMLVideoElement {
  const existing = els.get(source);
  if (existing) return existing;
  const v = document.createElement("video");
  v.playsInline = true;
  v.preload = "metadata";
  v.className = "w-full h-full object-contain";
  v.dataset.lectureSource = String(source);
  parked().appendChild(v);
  els.set(source, v);
  return v;
}
