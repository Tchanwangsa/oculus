/**
 * Content that mounts only part of itself (the chat timeline's newest rows)
 * marks its root `EXPANDABLE` and listens with `onExpand`. Whatever must see
 * all of it — DOM find, Select All, a jump to an unmounted question — calls
 * `expandWithin` first, and never learns what it expanded.
 */

export const EXPANDABLE = "data-expandable";
const EVENT = "oculus:expand";

export type ExpandRequest = {
  /** Mount before returning (`flushSync`), so the caller reads a complete DOM. */
  sync: boolean;
  /** Mount only as far back as this item; absent, mount everything. */
  upToId?: number;
};

/** Asks every expandable element in `root` (or `root` itself) to mount its
 *  content. The event doesn't bubble: nested ones are each reached directly. */
export function expandWithin(root: Element | null | undefined, request: ExpandRequest): void {
  if (!root) return;
  const marked = [...root.querySelectorAll(`[${EXPANDABLE}]`)];
  if (root.hasAttribute(EXPANDABLE)) marked.unshift(root);
  for (const el of marked) el.dispatchEvent(new CustomEvent(EVENT, { detail: request }));
}

/** Listens on `el` for `expandWithin`; returns the cleanup. */
export function onExpand(el: Element, handler: (request: ExpandRequest) => void): () => void {
  const listener = (e: Event) => handler((e as CustomEvent<ExpandRequest>).detail);
  el.addEventListener(EVENT, listener);
  return () => el.removeEventListener(EVENT, listener);
}
