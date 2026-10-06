import { useEffect, useRef, type RefObject } from "react";

/**
 * Where ⌘F, ⌘G and ⇧⌘G go (`menu.rs` only emits them; `AppLayout` routes
 * them here). Every mounted find registers a target: a PDF viewer, a browser
 * page, each pane's DOM find (`components/ui/PageFind.tsx`). Of the targets
 * on screen one answers, the innermost of the first that applies:
 *
 * 1. the target holding keyboard focus — none if focus sits in a dialog or
 *    popover outside every target (the command palette);
 * 2. the target under the pointer;
 * 3. the target the last pointerdown or focusin landed in;
 * 4. the focused pane's page-level target.
 *
 * At steps 2–4 a page-level target or DOM find hands ⌘F to the one non-DOM
 * target directly inside it, so the pointer over a PDF's page header searches
 * the PDF. Focus (step 1) never hands off: a text field on a page with one
 * editor is how the rest of that page stays searchable.
 *
 * While a native browser page holds the keyboard `document.hasFocus()` is
 * false and the DOM's focus and hover mean nothing, so only step 4 applies.
 * Select All with nothing focused selects inside the same target
 * (`lib/editRouting.ts`).
 */

export interface FindTarget {
  /** What the target searches; it answers only while this is on screen. */
  root: HTMLElement;
  /** ⌘F: open the bar, or select its query when it is open. */
  open(): void;
  /** ⌘G / ⇧⌘G: the next or previous match, opening the bar if closed. */
  step(backwards: boolean): void;
  /** Set on a pane's page-level target: that pane's id. */
  page?: number;
  /** A DOM find (`PageFind`), which never takes another's hand-off. */
  dom?: boolean;
  /** Select All when this target is picked; absent, its root's contents. */
  selectAll?(): void;
}

export type FindAction = "open" | "next" | "prev";

/** What target choice reads off the document, abstracted for tests. */
export interface FindScene<N> {
  /** Inclusive, like `Node.contains`. */
  contains(outer: N, inner: N): boolean;
  hovered(root: N): boolean;
  documentFocused: boolean;
  /** The focused element, unless that is the body. */
  focus: N | null;
  /** `focus` sits in a dialog or popover. */
  focusInOverlay: boolean;
  /** Where the last pointerdown or focusin landed. */
  engaged: N | null;
  /** The active tab's focused pane. */
  pane: number | undefined;
}

/** Roots that all contain one node are nested, so the innermost is the one
 *  every other contains. */
function innermost<N, T extends { root: N }>(
  list: T[],
  contains: (outer: N, inner: N) => boolean,
): T | undefined {
  let best: T | undefined;
  for (const t of list) if (!best || contains(best.root, t.root)) best = t;
  return best;
}

type Pickable<N> = { root: N; page?: number; dom?: boolean };

/** `chosen`, or — when it is a page-level target or DOM find — its one
 *  non-DOM child: a target inside it with no other target in between. */
function handOff<N, T extends Pickable<N>>(
  targets: T[],
  chosen: T,
  contains: (outer: N, inner: N) => boolean,
): T {
  if (chosen.page == null && !chosen.dom) return chosen;
  const within = (outer: N, inner: N) => contains(outer, inner) && !contains(inner, outer);
  const inside = targets.filter((t) => within(chosen.root, t.root));
  const children = inside.filter((t) => !t.dom && !inside.some((o) => within(o.root, t.root)));
  return children.length === 1 ? children[0] : chosen;
}

/** The choice rule above, over targets already known to be on screen. */
export function pickFindTarget<N, T extends Pickable<N>>(
  targets: T[],
  scene: FindScene<N>,
): T | undefined {
  const { contains } = scene;
  const holding = (node: N | null) =>
    node == null ? undefined : innermost(targets.filter((t) => contains(t.root, node)), contains);
  if (scene.documentFocused) {
    const focused = holding(scene.focus);
    if (focused) return focused;
    if (scene.focusInOverlay) return undefined;
    const pointed =
      innermost(targets.filter((t) => scene.hovered(t.root)), contains) ?? holding(scene.engaged);
    if (pointed) return handOff(targets, pointed, contains);
  }
  if (scene.pane == null) return undefined;
  const page = innermost(targets.filter((t) => t.page === scene.pane), contains);
  return page && handOff(targets, page, contains);
}

const targets = new Set<FindTarget>();
let engaged: Node | null = null;

function engage(e: Event) {
  engaged = e.target instanceof Node ? e.target : null;
}

/** Radix portals these to the body, outside every pane. */
export const OVERLAY =
  "[role='dialog'], [role='alertdialog'], [role='menu'], [data-radix-popper-content-wrapper]";

/** Hidden tabs stay mounted under `visibility: hidden` (`TabPane.tsx`). */
export function onScreen(el: Element): boolean {
  return el.checkVisibility?.({ visibilityProperty: true }) ?? true;
}

export function registerFindTarget(target: FindTarget): () => void {
  if (!targets.size) {
    document.addEventListener("pointerdown", engage, true);
    document.addEventListener("focusin", engage, true);
  }
  targets.add(target);
  return () => {
    targets.delete(target);
    if (!targets.size) {
      document.removeEventListener("pointerdown", engage, true);
      document.removeEventListener("focusin", engage, true);
      engaged = null;
    }
  };
}

/** Registers `rootRef`'s element for the component's life. Handlers are read
 *  through a ref, so fresh closures each render never go stale. */
export function useFindTarget(
  rootRef: RefObject<HTMLElement | null>,
  handlers: Pick<FindTarget, "open" | "step" | "selectAll">,
  page?: number,
  dom?: boolean,
): void {
  const ref = useRef(handlers);
  ref.current = handlers;
  useEffect(() => {
    const root = rootRef.current;
    if (!root) return;
    return registerFindTarget({
      root,
      page,
      dom,
      open: () => ref.current.open(),
      step: (backwards) => ref.current.step(backwards),
      selectAll: () => (ref.current.selectAll ? ref.current.selectAll() : selectContents(root)),
    });
  }, [rootRef, page, dom]);
}

/** Other targets' roots inside `root`: they search themselves, so a DOM
 *  find over `root` skips them. */
export function nestedFindRoots(root: HTMLElement): Set<HTMLElement> {
  const nested = new Set<HTMLElement>();
  for (const t of targets) if (t.root !== root && root.contains(t.root)) nested.add(t.root);
  return nested;
}

/** The target ⌘F would reach now, by the rule above; `pane` is the active
 *  tab's focused pane. Select All uses it too. */
export function currentFindTarget(pane: number | undefined): FindTarget | undefined {
  const active = document.activeElement;
  const focus = active && active !== document.body ? active : null;
  return pickFindTarget<Node, FindTarget>(
    [...targets].filter((t) => t.root.isConnected && onScreen(t.root)),
    {
      contains: (outer, inner) => outer.contains(inner),
      hovered: (root) => (root as Element).matches(":hover"),
      documentFocused: document.hasFocus(),
      focus,
      focusInOverlay: !!focus?.closest(OVERLAY),
      engaged: engaged?.isConnected ? engaged : null,
      pane,
    },
  );
}

/** Selects the content inside `el` — from its first `data-selectable` block to
 *  its last — and nothing outside it. WebKit paints a scripted selection over
 *  `user-select: none` text, so a page with no content selects nothing. */
export function selectContents(el: Node): void {
  const selection = window.getSelection();
  if (!selection) return;
  const blocks =
    el instanceof Element && el.matches("[data-selectable]")
      ? [el]
      : el instanceof Element || el instanceof Document
        ? [...el.querySelectorAll("[data-selectable]")]
        : [];
  if (blocks.length === 0) return void selection.removeAllRanges();
  // The outermost last block, so code inside a reply doesn't cut its tail.
  const last = blocks[blocks.length - 1].closest("[data-selectable]:not([data-selectable] *)")!;
  const range = document.createRange();
  range.setStart(blocks[0], 0);
  range.setEnd(last, last.childNodes.length);
  selection.removeAllRanges();
  selection.addRange(range);
}

/** Sends a find menu event to one target. */
export function routeFind(action: FindAction, pane: number | undefined): void {
  const target = currentFindTarget(pane);
  if (!target) return;
  if (action === "open") target.open();
  else target.step(action === "prev");
}
