import { bindingKey, withoutBinding } from './bindings';
import { secureId } from './ids';
import { createCanvasDocument, parseCanvasDocument, serializeCanvasDocument, type CanvasElement, type Point } from './model';

export const CANVAS_CLIPBOARD_MIME = 'application/vnd.oculus.canvas+json';

/** Plain text carries the same portable document envelope as the custom MIME. */
export function serializeCanvasClipboard(elements: CanvasElement[]): string {
  return serializeCanvasDocument({ ...createCanvasDocument(), elements });
}

export function parseCanvasClipboard(text: string): CanvasElement[] | null {
  try { return parseCanvasDocument(text).elements; }
  catch { return null; }
}

/** Bindings to elements cloned in the same batch point at their clones; bindings to anything else are dropped. */
export function cloneCanvasElements(elements: readonly CanvasElement[], offset: Point = { x: 24, y: 24 }, id: () => string = secureId): CanvasElement[] {
  const ids = new Map(elements.map(element => [element.id, id()]));
  return elements.map(element => {
    let clone: CanvasElement = { ...structuredClone(element), id: ids.get(element.id)!, x: element.x + offset.x, y: element.y + offset.y };
    for (const end of ['start', 'end'] as const) {
      const key = bindingKey(end), binding = clone[key], target = binding && ids.get(binding.elementId);
      if (binding) clone = target ? { ...clone, [key]: { ...binding, elementId: target } } : withoutBinding(clone, end);
    }
    return clone;
  });
}
