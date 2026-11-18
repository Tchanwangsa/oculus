import { applyCanvasTransaction, diffCanvasContent, hasCanvasChanges, sameValue, type CanvasTransaction } from './collaboration';
import type { CanvasDocument, CanvasViewport } from './model';

/** Edits after a sent operation are relative to its result, including a quick undo
 * that returns to the original shared scene. A net diff alone loses that undo.
 */
export function clientPendingTransactions(
  shared: CanvasDocument,
  current: CanvasDocument,
  inflight?: CanvasTransaction,
): CanvasTransaction[] {
  const base = inflight ? applyCanvasTransaction(shared, inflight) : shared;
  const after = diffCanvasContent(base, current);
  const pending = inflight ? [inflight] : [];
  if (hasCanvasChanges(after)) pending.push(after);
  return pending;
}

/** A snapshot can already contain an operation whose HTTP reply was lost. */
export function containsClientTransaction(shared: CanvasDocument, transaction: CanvasTransaction): boolean {
  return !hasCanvasChanges(diffCanvasContent(shared, applyCanvasTransaction(shared, transaction)));
}

/** Rebase outstanding local edits without publishing an identical replacement scene.
 * Acknowledging a stroke changes the pending queue, not the visible drawing. Keeping
 * its identity avoids an external-document reset racing the next Pencil gesture.
 */
export function reconcileClientDocument(
  shared: CanvasDocument,
  pending: readonly CanvasTransaction[],
  current: CanvasDocument,
  viewport: CanvasViewport = current.viewport,
): CanvasDocument {
  let next = shared;
  for (const transaction of pending) next = applyCanvasTransaction(next, transaction);
  next = { ...next, viewport };
  return sameValue(next, current) ? current : next;
}
