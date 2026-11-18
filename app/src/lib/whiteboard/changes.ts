import type { WhiteboardElement } from './model';

/** Element ID → its replacement, or null to remove it. An ID not in the scene is appended (drawn, pasted, new text). */
export type ElementChanges = ReadonlyMap<string, WhiteboardElement | null>;
/** Scene element ID → IDs added by the same change that sit directly above it, in order, rather than on top. */
export type StackedAdditions = ReadonlyMap<string, readonly string[]>;

/** Keeps layer order and every untouched element's identity; returns the input array when nothing changes. */
export function applyElementChanges(elements: WhiteboardElement[], changes: ElementChanges, above?: StackedAdditions): WhiteboardElement[] {
  if (!changes.size) return elements;
  let changed = false;
  const seen = new Set<string>(), next: WhiteboardElement[] = [];
  for (const element of elements) {
    const update = changes.has(element.id) ? changes.get(element.id) : element;
    if (changes.has(element.id)) { seen.add(element.id); if (update !== element) changed = true; }
    if (update) next.push(update);
    for (const id of above?.get(element.id) ?? []) {
      const added = changes.get(id);
      if (added && !seen.has(id)) { seen.add(id); next.push(added); changed = true; }
    }
  }
  for (const [id, element] of changes) if (element && !seen.has(id)) { next.push(element); changed = true; }
  return changed ? next : elements;
}
