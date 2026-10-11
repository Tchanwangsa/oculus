import { frameOrigin } from "@/lib/maths/geometry";
import { syncScrollFade } from "@/lib/ui/scrollFade";
import { fieldTemplate } from "../../../tools/mathPalette";
import type { MathView } from "../index";
import { pendingBox } from "../pending";
import { listKey, moved, type ListKind } from "./keys";
import { commandOptions, entryOptions, type CommandOption } from "./options";
import { placeList, visibleBounds } from "./place";

export { listKey, moved, type ListKey, type ListKind } from "./keys";
export { commandOptions, entryOptions, type CommandOption } from "./options";
export { GAP, placeList } from "./place";

/** Rows the picks number for their 1–9 keys. */
const KEYED = 9;

/** What a view's list shows: which list, the pending name it was filled
 *  for (the command list's), its options and the highlighted one (-1 for
 *  none). `closed` is a bare `\`'s list closed by Esc. */
interface ListState {
  kind: ListKind;
  pending: string | null;
  options: CommandOption[];
  active: number;
  closed: boolean;
}

const lists = new WeakMap<MathView, ListState>();

const el = (cls: string, text?: string) => {
  const e = document.createElement("span");
  e.className = cls;
  if (text != null) e.textContent = text;
  return e;
};

const listEl = (view: MathView) => view.popover.firstElementChild as HTMLElement;

/**
 * The open list: in command mode the pending `\command`'s, filled again
 * when the name changed (the field's picks for a bare `\`, its first row
 * highlighted), else the picks if Space opened them; null when none is.
 */
function listOf(view: MathView): ListState | null {
  const { pending, mode } = view.field;
  const state = lists.get(view);
  if (mode === "command" && pending != null) {
    if (state?.kind === "command" && state.pending === pending) return state.closed ? null : state;
    const closed = pending === "" && state?.kind === "command" && state.closed;
    const options = pending ? commandOptions(pending) : entryOptions(view.host.picks?.() ?? []);
    const next: ListState = { kind: "command", pending, options, active: 0, closed };
    lists.set(view, next);
    if (!closed) fill(view, next);
    return closed ? null : next;
  }
  if (state?.kind === "command") {
    lists.delete(view);
    return null;
  }
  return state ?? null;
}

function fill(view: MathView, state: ListState) {
  const list = listEl(view);
  list.replaceChildren();
  list.scrollTop = 0;
  state.options.forEach((option, i) => {
    const row = el("cm-math-view-popover-row");
    row.setAttribute("role", "option");
    row.dataset.index = String(i);
    const preview = el("cm-math-view-popover-preview");
    const html = option.preview();
    if (html) preview.innerHTML = html;
    row.append(preview, el("cm-math-view-popover-name", option.label));
    if (state.kind === "picks" && i < KEYED) row.append(el("cm-math-view-popover-key", String(i + 1)));
    list.append(row);
  });
  (view.popover.lastElementChild as HTMLElement).hidden = state.kind !== "picks";
}

/** The list's rows marked for the highlighted option, which is scrolled
 *  into the list's view (only the list's: never the note's scroller). */
function highlight(view: MathView, state: ListState) {
  const list = listEl(view);
  const rows = list.children;
  for (let i = 0; i < rows.length; i++) rows[i].setAttribute("aria-selected", String(i === state.active));
  const row = rows[state.active] as HTMLElement | undefined;
  if (row) {
    if (row.offsetTop < list.scrollTop) list.scrollTop = row.offsetTop;
    else if (row.offsetTop + row.offsetHeight > list.scrollTop + list.clientHeight) {
      list.scrollTop = row.offsetTop + row.offsetHeight - list.clientHeight;
    }
  }
  syncScrollFade(list, "y");
}

/** The option goes in as a template, for the typed `\name` if one is
 *  pending (the model drops it), the caret in the first empty slot; the
 *  host counts it as used (`onPick`). */
function accept(view: MathView, option: CommandOption) {
  if (lists.get(view)?.kind === "picks") closeList(view);
  if (view.host.onPick) view.host.onPick(option.entry, view);
  else view.run({ template: fieldTemplate(option.template) });
}

/** The picks give way to the full toolbox (Space again, or its "All maths tools" row). */
function more(view: MathView) {
  closeList(view);
  view.host.onMoreTools?.(view);
}

/** Closes the open list: the picks go; a bare `\`'s list stays closed
 *  while the `\` is pending. */
export function closeList(view: MathView) {
  const state = lists.get(view);
  if (!state) return;
  if (state.kind === "picks") lists.delete(view);
  else state.closed = true;
  view.popover.hidden = true;
}

/** The picks close on any change to the field: a step, a caret move. */
export function closePicks(view: MathView) {
  if (lists.get(view)?.kind === "picks") closeList(view);
}

/** Opens the picks at the caret: the host's entries (`picks`), none
 *  highlighted, and a row on to the full toolbox. */
export function openPicks(view: MathView) {
  const state: ListState = {
    kind: "picks",
    pending: null,
    options: entryOptions(view.host.picks?.() ?? []),
    active: -1,
    closed: false,
  };
  lists.set(view, state);
  fill(view, state);
  drawPopover(view);
}

/** Whether a list is showing. */
export function listOpen(view: MathView): boolean {
  return listOf(view) != null && !view.popover.hidden;
}

/** The list's element: a box holding a scroller of rows, where a press
 *  accepts a row (the view's own `mousedown` keeps the focus), and the
 *  picks' row on to the full toolbox. */
export function createPopover(view: MathView): HTMLElement {
  const popover = el("cm-math-view-popover");
  popover.hidden = true;
  const list = el("cm-math-view-popover-list");
  list.setAttribute("role", "listbox");
  list.setAttribute("aria-label", "Maths commands");
  list.addEventListener("scroll", () => syncScrollFade(list, "y"));
  const tools = el("cm-math-view-popover-more");
  tools.setAttribute("role", "button");
  tools.append(el("cm-math-view-popover-more-label", "All maths tools"), el("cm-math-view-popover-key", "Space"));
  tools.hidden = true;
  popover.append(list, tools);
  popover.addEventListener("click", (e) => {
    const target = e.target as Element;
    if (target.closest(".cm-math-view-popover-more")) return more(view);
    const row = target.closest<HTMLElement>(".cm-math-view-popover-row");
    const option = row && listOf(view)?.options[Number(row.dataset.index)];
    if (option) accept(view, option);
  });
  return popover;
}

/** A key for the open list (`listKey`); true when the list took it. */
export function popoverKey(view: MathView, e: KeyboardEvent): boolean {
  const state = listOf(view);
  if (!state || view.popover.hidden) return false;
  const action = listKey(e, state.kind, state.options.length, state.active, state.pending === "");
  if (action === null) return false;
  if (action === "more") more(view);
  else if (action === "close") closeList(view);
  else if (action === "dismiss") {
    closeList(view);
    return false;
  } else if ("accept" in action) accept(view, state.options[action.accept]);
  else {
    state.active = moved(state.active, action.move, state.options.length);
    highlight(view, state);
  }
  return true;
}

/**
 * The open list hung from the caret, or from the start of the `\name`
 * typed there: under it, or above near the bottom of what is visible (the
 * window, the note's scroller), slid sideways to stay on screen. Hidden
 * when nothing matches.
 */
export function drawPopover(view: MathView) {
  const { field, popover, measured, frame, dom } = view;
  const id = field.head;
  const x = measured.x[id];
  const state = listOf(view);
  // From the `\name`'s left edge, which the rendering holds (`pending.ts`).
  const at =
    (field.mode === "command" ? pendingBox(view) : null) ??
    (Number.isFinite(x) ? { left: x, right: x, top: measured.top[id], bottom: measured.bottom[id] } : null);
  if (!state || state.options.length === 0 || !at) {
    popover.hidden = true;
    return;
  }
  popover.hidden = false;
  highlight(view, state);
  const o = frameOrigin(frame);
  const caret = { left: o.x + at.left, right: o.x + at.left, top: o.y + at.top, bottom: o.y + at.bottom };
  const size = { width: popover.offsetWidth, height: popover.offsetHeight };
  const { left, top } = placeList(caret, size, visibleBounds(dom));
  const box = dom.getBoundingClientRect();
  popover.style.left = `${left - box.left - dom.clientLeft}px`;
  popover.style.top = `${top - box.top - dom.clientTop}px`;
}
