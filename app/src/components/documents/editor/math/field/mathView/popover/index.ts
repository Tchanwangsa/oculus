import { frameOrigin } from "@/lib/maths/geometry";
import { syncScrollFade } from "@/lib/ui/scrollFade";
import { fieldTemplate } from "../../../tools/mathPalette";
import type { MathView } from "../index";
import { pendingBox } from "../pending";
import { listKey } from "./keys";
import { commandOptions, type CommandOption } from "./options";
import { placeList, visibleBounds } from "./place";

export { listKey, type ListKey } from "./keys";
export { commandOptions, type CommandOption } from "./options";
export { GAP, placeList } from "./place";

/** What a view's list shows: the pending name it was filled for, its
 *  options and the highlighted one. */
interface ListState {
  pending: string;
  options: CommandOption[];
  active: number;
}

const lists = new WeakMap<MathView, ListState>();

/** The list for the view's pending `\command`, filled again when the name
 *  changed; null outside command mode. */
function listOf(view: MathView): ListState | null {
  const { pending } = view.field;
  if (view.field.mode !== "command" || pending == null) {
    lists.delete(view);
    return null;
  }
  let state = lists.get(view);
  if (!state || state.pending !== pending) {
    state = { pending, options: commandOptions(pending), active: 0 };
    lists.set(view, state);
    fill(view, state);
  }
  return state;
}

function fill(view: MathView, state: ListState) {
  const list = view.popover.firstElementChild as HTMLElement;
  list.replaceChildren();
  list.scrollTop = 0;
  state.options.forEach((option, i) => {
    const row = document.createElement("span");
    row.className = "cm-math-view-popover-row";
    row.setAttribute("role", "option");
    row.dataset.index = String(i);
    const preview = document.createElement("span");
    preview.className = "cm-math-view-popover-preview";
    const html = option.preview();
    if (html) preview.innerHTML = html;
    const label = document.createElement("span");
    label.className = "cm-math-view-popover-name";
    label.textContent = option.label;
    row.append(preview, label);
    list.append(row);
  });
}

/** The list's rows marked for the highlighted option, which is scrolled
 *  into the list's view (only the list's: never the note's scroller). */
function highlight(view: MathView, state: ListState) {
  const list = view.popover.firstElementChild as HTMLElement;
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

/** The option goes in for the typed `\name`, as a template: the model
 *  drops the pending name and puts the caret in the first empty slot. */
function accept(view: MathView, option: CommandOption) {
  view.run({ template: fieldTemplate(option.template) });
}

/** The list's element: a box holding a scroller of rows, where a press
 *  accepts a row (the view's own `mousedown` keeps the focus). */
export function createPopover(view: MathView): HTMLElement {
  const popover = document.createElement("span");
  popover.className = "cm-math-view-popover";
  popover.hidden = true;
  const list = document.createElement("span");
  list.className = "cm-math-view-popover-list";
  list.setAttribute("role", "listbox");
  list.setAttribute("aria-label", "Maths commands");
  list.addEventListener("scroll", () => syncScrollFade(list, "y"));
  popover.append(list);
  popover.addEventListener("click", (e) => {
    const row = (e.target as Element).closest<HTMLElement>(".cm-math-view-popover-row");
    const option = row && lists.get(view)?.options[Number(row.dataset.index)];
    if (option && view.field.mode === "command") accept(view, option);
  });
  return popover;
}

/** ↑/↓ move the highlight; Space, Tab and Enter accept it. False when the
 *  list is empty or the key is not its own, so the model takes it (and
 *  commits the typed name as it is). */
export function popoverKey(view: MathView, e: KeyboardEvent): boolean {
  const state = listOf(view);
  if (!state) return false;
  const action = listKey(e, state.options.length);
  if (!action) return false;
  if (action === "accept") {
    accept(view, state.options[state.active]);
    return true;
  }
  const n = state.options.length;
  state.active = (state.active + action.move + n) % n;
  highlight(view, state);
  return true;
}

/**
 * In command mode, the palette's options for the `\name` typed at the
 * caret, hung from the name's start: under it, or above near the bottom of what is
 * visible (the window, the note's scroller), slid sideways to stay on
 * screen. Hidden when nothing matches.
 */
export function drawPopover(view: MathView) {
  const { field, popover, measured, frame, dom } = view;
  const id = field.head;
  const x = measured.x[id];
  const state = listOf(view);
  // From the `\name`'s left edge, which the rendering holds (`pending.ts`).
  const at =
    pendingBox(view) ??
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
