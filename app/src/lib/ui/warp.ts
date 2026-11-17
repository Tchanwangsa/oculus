/**
 * The warp: a same-document View Transition in which a composer the student
 * sent from morphs into the composer of the page its pane opens, and the rest
 * of the window cross-fades (`app/src/styles/warp.css`). The Overview's ask
 * bar uses it to become its new thread (`docs/harness.md`).
 *
 * One name, held by one element at a time: the source carries it for the old
 * snapshot, the landing composer (`useWarpLanding`) for the new one. Two live
 * elements with the name, such as a hidden tab's composer, would make the
 * browser skip the transition, so no composer carries it outside a warp.
 * Without View Transitions, or with reduced motion, it is a plain navigation.
 */
export const WARP_NAME = "composer-warp";

/** How long the opened page may take to draw its composer; the window shows
 *  the old snapshot meanwhile, so past this the transition animates whatever
 *  is drawn rather than hold the screen. */
const LANDING_WAIT_MS = 800;

/** Where the warp lands: a pane (`useTabId`) showing a thread. */
export interface WarpLanding {
  pane: number;
  thread: number;
  /** The landing composer has rendered with the name. */
  land: () => void;
}

let landing: WarpLanding | null = null;
const listeners = new Set<() => void>();

function setLanding(next: WarpLanding | null): void {
  landing = next;
  for (const l of listeners) l();
}

export function subscribeWarp(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function warpLanding(): WarpLanding | null {
  return landing;
}

function reducedMotion(): boolean {
  return typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/**
 * Run `go` (a navigation of `pane` to `thread`) inside a View Transition from
 * `source`. The transition's update callback waits until the thread page's
 * composer has rendered, since the router commits a navigation in a React
 * transition, after its lazy route has loaded.
 */
export function warpTo(source: HTMLElement | null, pane: number, thread: number, go: () => void): void {
  if (!source || typeof document.startViewTransition !== "function" || reducedMotion()) {
    go();
    return;
  }
  let land = () => {};
  const landed = new Promise<void>((resolve) => (land = resolve));
  source.style.viewTransitionName = WARP_NAME;
  const transition = document.startViewTransition(async () => {
    // The old snapshot is taken: the name moves to the landing composer.
    source.style.viewTransitionName = "";
    setLanding({ pane, thread, land });
    go();
    await Promise.race([landed, new Promise((resolve) => setTimeout(resolve, LANDING_WAIT_MS))]);
  });
  // A skipped transition rejects these; the navigation has happened anyway.
  transition.ready.catch(() => {});
  transition.finished
    .catch(() => {})
    .finally(() => {
      if (landing?.land === land) setLanding(null);
    });
}
