import { parseCanvasDocument, type CanvasDocument, type CanvasElement } from './model';

/** `afterId`, only on an addition, places it just above that element (`null`: at the bottom);
 * without it, or when that element is gone, the addition goes on top. */
export type ElementEdit = { id: string; before: CanvasElement | null; after: CanvasElement | null; afterId?: string | null };
export interface CanvasTransaction {
  edits: ElementEdit[];
  order?: { before: string[]; after: string[] };
  background?: { before: string; after: string };
}

/** Object-key order is irrelevant to conflict checks; arrays retain their order. */
export function sameValue(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (a === null || b === null || typeof a !== 'object' || typeof b !== 'object') return false;
  if (Array.isArray(a) || Array.isArray(b)) return Array.isArray(a) && Array.isArray(b) && a.length === b.length && a.every((v, i) => sameValue(v, b[i]));
  const keys = Object.keys(a);
  return keys.length === Object.keys(b).length && keys.every(key => Object.prototype.hasOwnProperty.call(b, key) && sameValue((a as any)[key], (b as any)[key]));
}

/** Only drawing content crosses the network. Viewports remain local to each tab. */
export function diffCanvasContent(before: CanvasDocument, after: CanvasDocument): CanvasTransaction {
  const old = new Map(before.elements.map(element => [element.id, element]));
  const next = new Map(after.elements.map(element => [element.id, element]));
  const edits: ElementEdit[] = [];
  for (const [id, element] of old) if (!sameValue(element, next.get(id))) edits.push({ id, before: element, after: next.get(id) ?? null });
  // An addition below an element that already existed was inserted mid-stack (an undone
  // delete, say) and names the element it sits on; one on top only appends, so concurrent
  // top additions from different clients all survive in arrival order.
  after.elements.forEach((element, index) => {
    if (old.has(element.id)) return;
    const edit: ElementEdit = { id: element.id, before: null, after: element };
    if (after.elements.slice(index + 1).some(el => old.has(el.id))) edit.afterId = index ? after.elements[index - 1].id : null;
    edits.push(edit);
  });
  const oldOrder = before.elements.filter(el => next.has(el.id)).map(el => el.id);
  const newOrder = after.elements.filter(el => old.has(el.id)).map(el => el.id);
  const transaction: CanvasTransaction = { edits };
  if (!sameValue(oldOrder, newOrder)) transaction.order = { before: oldOrder, after: newOrder };
  if (before.background !== after.background) transaction.background = { before: before.background, after: after.background };
  return transaction;
}

export const hasCanvasChanges = (transaction: CanvasTransaction) => !!(transaction.edits.length || transaction.order || transaction.background);

function validateTransaction(value: unknown): asserts value is CanvasTransaction {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Invalid canvas transaction.');
  const tx = value as CanvasTransaction;
  if (Object.keys(tx).some(key => !['edits', 'order', 'background'].includes(key)) || !Array.isArray(tx.edits) || tx.edits.length > 10000) throw new Error('Invalid canvas transaction fields.');
  const ids = new Set<string>();
  for (const edit of tx.edits) {
    if (!edit || typeof edit.id !== 'string' || !edit.id || ids.has(edit.id) || (edit.before === null && edit.after === null)) throw new Error('Invalid element edit.');
    ids.add(edit.id);
    if (edit.afterId !== undefined && (edit.before !== null || (edit.afterId !== null && (typeof edit.afterId !== 'string' || !edit.afterId)) || edit.afterId === edit.id)) throw new Error('Invalid element placement.');
    for (const element of [edit.before, edit.after]) {
      if (element === null) continue;
      if (element?.id !== edit.id) throw new Error('Element edit IDs do not match.');
      parseCanvasDocument(JSON.stringify({ type: 'oculus-canvas', version: 1, elements: [element], viewport: { x: 0, y: 0, zoom: 1 }, background: '#fff' }));
    }
  }
  if (tx.order) {
    const { before, after } = tx.order;
    if (!Array.isArray(before) || !Array.isArray(after) || before.length !== after.length || before.some(id => typeof id !== 'string' || !id) || new Set(before).size !== before.length || new Set(after).size !== after.length || after.some(id => !before.includes(id))) throw new Error('Invalid layer order.');
  }
  if (tx.background && (typeof tx.background.before !== 'string' || typeof tx.background.after !== 'string')) throw new Error('Invalid background edit.');
}

/** Compare only touched objects: independent edits merge even from the same old revision. */
export function applyCanvasTransaction(document: CanvasDocument, value: unknown): CanvasDocument {
  validateTransaction(value);
  const transaction = value;
  if (!hasCanvasChanges(transaction)) return document;
  const elements = new Map(document.elements.map(element => [element.id, element]));
  let ids = document.elements.map(element => element.id);
  for (const edit of transaction.edits) {
    const current = elements.get(edit.id) ?? null;
    if (sameValue(current, edit.after)) continue;
    if (!sameValue(current, edit.before)) throw new Error('A collaborator changed the same shape. Your draft is kept; download it or use the shared canvas.');
    if (!edit.after) { elements.delete(edit.id); ids = ids.filter(id => id !== edit.id); continue; }
    if (!elements.has(edit.id)) {
      const anchor = edit.afterId === null ? 0 : edit.afterId === undefined || !elements.has(edit.afterId) ? ids.length : ids.indexOf(edit.afterId) + 1;
      ids.splice(anchor, 0, edit.id);
    }
    elements.set(edit.id, edit.after);
  }
  let ordered = ids.map(id => elements.get(id)!);
  if (transaction.order) {
    const { before, after } = transaction.order;
    const affected = new Set(before);
    const current = ordered.filter(el => affected.has(el.id)).map(el => el.id);
    const wanted = after.filter(id => elements.has(id));
    if (!sameValue(current, wanted)) {
      if (!sameValue(current, before.filter(id => elements.has(id)))) throw new Error('A collaborator changed the same layer order. Your draft is kept.');
      let i = 0;
      ordered = ordered.map(el => affected.has(el.id) ? elements.get(wanted[i++])! : el);
    }
  }
  let background = document.background;
  if (transaction.background) {
    if (background !== transaction.background.before && background !== transaction.background.after) throw new Error('A collaborator changed the background. Your draft is kept.');
    background = transaction.background.after;
  }
  const result = { ...document, elements: ordered, background };
  parseCanvasDocument(JSON.stringify(result));
  return result;
}
