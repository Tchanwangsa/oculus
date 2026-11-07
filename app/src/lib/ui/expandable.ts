/**
 * Content that mounts only part of itself (the chat timeline's newest rows)
 * marks its root `EXPANDABLE` and listens with `onExpand`. Whatever must see
 * all of it — DOM find, Select All, a jump to an unmounted question — calls
 * `expandWithin` first, and never learns what it expanded. A collapsed fold
 * inside marks itself too, and opens for a find whose `query` it holds.
 */

export const EXPANDABLE = "data-expandable";
const EVENT = "oculus:expand";

export type ExpandRequest = {
  /** Mount before returning (`flushSync`), so the caller reads a complete DOM. */
  sync: boolean;
  /** Mount only as far back as this item; absent, mount everything. */
  upToId?: number;
  /** What a find is looking for: a fold holding it opens. */
  query?: string;
};

/** Asks every expandable element in `root` (or `root` itself) to mount its
 *  content. The event doesn't bubble: nested ones are each reached directly,
 *  after the outer ones have mounted them. */
export function expandWithin(root: Element | null | undefined, request: ExpandRequest): void {
  if (!root) return;
  const send = (el: Element) => el.dispatchEvent(new CustomEvent(EVENT, { detail: request }));
  if (root.hasAttribute(EXPANDABLE)) send(root);
  const reached = new Set<Element>();
  // Each pass reaches what the last one mounted (a timeline's rows, then a
  // fold inside them).
  for (let pass = 0; pass < 4; pass++) {
    const fresh = [...root.querySelectorAll(`[${EXPANDABLE}]`)].filter((el) => !reached.has(el));
    if (!fresh.length) break;
    for (const el of fresh) {
      reached.add(el);
      send(el);
    }
  }
}

/** Listens on `el` for `expandWithin`; returns the cleanup. */
export function onExpand(el: Element, handler: (request: ExpandRequest) => void): () => void {
  const listener = (e: Event) => handler((e as CustomEvent<ExpandRequest>).detail);
  el.addEventListener(EVENT, listener);
  return () => el.removeEventListener(EVENT, listener);
}
