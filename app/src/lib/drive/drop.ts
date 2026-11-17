import type { DriveZone } from "./listing";
import { isWithin, parentRel } from "./paths";

/**
 * Dragging entries inside Drive. A drag between the student's own folders
 * moves; a drag out of the synced folders copies, since the original has to
 * stay; a drop into a folder that is not the student's is refused. The drag
 * is HTML5 so it can cross from the browser to the folder tree, which means
 * `dragstart` must put data on the transfer or WebKit cancels it silently
 * (`docs/ui.md`).
 */

/** The transfer type an entry drag carries: a JSON array of paths. */
export const DRIVE_DRAG_TYPE = "application/x-oculus-drive";

export interface DraggedEntry {
  rel: string;
  zone: DriveZone;
}

/** What is being dragged, for `dragover`, which may read a transfer's types
 *  but not its data. One window, one drag at a time. */
let active: DraggedEntry[] | null = null;

export function startEntryDrag(transfer: DataTransfer, entries: DraggedEntry[]): void {
  const rels = entries.map((e) => e.rel);
  transfer.setData(DRIVE_DRAG_TYPE, JSON.stringify(rels));
  transfer.setData("text/plain", rels.join("\n"));
  transfer.effectAllowed = "copyMove";
  active = entries;
}

export function endEntryDrag(): void {
  active = null;
}

/** The entries of the drag in progress, when it is one of Drive's. */
export function draggedEntries(transfer: DataTransfer | null): DraggedEntry[] | null {
  if (!transfer || !transfer.types.includes(DRIVE_DRAG_TYPE)) return null;
  return active;
}

export type DropPlan =
  /** The folder is not the student's: say so, drop nothing. */
  | { kind: "refuse" }
  /** Dropping where they already are, or a folder into itself. */
  | { kind: "none" }
  | { kind: "go"; move: string[]; copy: string[] };

/** What dropping `entries` on the folder `target` does. */
export function planDrop(entries: readonly DraggedEntry[], target: { rel: string; writable: boolean }): DropPlan {
  if (!target.writable) return { kind: "refuse" };
  const moving = entries.filter((e) => e.zone === "own");
  const copying = entries.filter((e) => e.zone === "synced");
  if (entries.some((e) => isWithin(target.rel, e.rel))) return { kind: "none" };
  const move = moving.filter((e) => parentRel(e.rel) !== target.rel).map((e) => e.rel);
  const copy = copying.map((e) => e.rel);
  return move.length || copy.length ? { kind: "go", move, copy } : { kind: "none" };
}
