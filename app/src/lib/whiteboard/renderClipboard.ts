import { bindingKey, withoutBinding } from './bindings';
import { secureId } from './ids';
import { createWhiteboardDocument, parseWhiteboardDocument, serializeWhiteboardDocument, type WhiteboardElement, type Point } from './model';

export const WHITEBOARD_CLIPBOARD_MIME = 'application/vnd.oculus.whiteboard+json';

/** Plain text carries the same portable document envelope as the custom MIME. */
export function serializeWhiteboardClipboard(elements: WhiteboardElement[]): string {
  return serializeWhiteboardDocument({ ...createWhiteboardDocument(), elements });
}

export function parseWhiteboardClipboard(text: string): WhiteboardElement[] | null {
  try { return parseWhiteboardDocument(text).elements; }
  catch { return null; }
}

/** Bindings to elements cloned in the same batch point at their clones; bindings to anything else are dropped. */
export function cloneWhiteboardElements(elements: readonly WhiteboardElement[], offset: Point = { x: 24, y: 24 }, id: () => string = secureId): WhiteboardElement[] {
  const ids = new Map(elements.map(element => [element.id, id()]));
  return elements.map(element => {
    let clone: WhiteboardElement = { ...structuredClone(element), id: ids.get(element.id)!, x: element.x + offset.x, y: element.y + offset.y };
    for (const end of ['start', 'end'] as const) {
      const key = bindingKey(end), binding = clone[key], target = binding && ids.get(binding.elementId);
      if (binding) clone = target ? { ...clone, [key]: { ...binding, elementId: target } } : withoutBinding(clone, end);
    }
    return clone;
  });
}
