import type { CanvasElement } from './model';

/** Element ID → its replacement, or null to remove it. An ID not in the scene is appended (drawn, pasted, new text). */
export type ElementChanges = ReadonlyMap<string, CanvasElement | null>;

/** Keeps layer order and every untouched element's identity; returns the input array when nothing changes. */
export function applyElementChanges(elements: CanvasElement[], changes: ElementChanges): CanvasElement[] {
  if (!changes.size) return elements;
  let changed = false;
  const seen = new Set<string>(), next: CanvasElement[] = [];
  for (const element of elements) {
    if (!changes.has(element.id)) { next.push(element); continue; }
    seen.add(element.id);
    const update = changes.get(element.id);
    if (update !== element) changed = true;
    if (update) next.push(update);
  }
  for (const [id, element] of changes) if (element && !seen.has(id)) { next.push(element); changed = true; }
  return changed ? next : elements;
}
