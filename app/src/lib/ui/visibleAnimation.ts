/** What decides whether a looping animation is worth drawing. */
export interface AnimationGate {
  /** The element is inside its scroller's visible box. */
  intersecting: boolean;
  /** `document.visibilityState` is `visible` (the window isn't minimised or hidden). */
  documentVisible: boolean;
  /** `prefers-reduced-motion: reduce`. */
  reducedMotion: boolean;
  /** The element's tab pane is in front. A hidden pane is `visibility:
   *  hidden`, which an IntersectionObserver still reports as intersecting. */
  tabActive: boolean;
}

/** Runs only while seen: on screen, in a visible document, in the front tab,
 *  and with motion allowed. */
export const animationRuns = (g: AnimationGate) =>
  g.intersecting && g.documentVisible && g.tabActive && !g.reducedMotion;

/** The custom property `text-shimmer` reads for its `animation-play-state`. */
export const SHIMMER_STATE = "--shimmer-state";
