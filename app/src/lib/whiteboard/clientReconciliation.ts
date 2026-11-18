import { applyWhiteboardTransaction, diffWhiteboardContent, hasWhiteboardChanges, sameValue, type WhiteboardTransaction } from './collaboration';
import type { WhiteboardDocument, WhiteboardViewport } from './model';

/** Edits after a sent operation are relative to its result, including a quick undo
 * that returns to the original shared scene. A net diff alone loses that undo.
 */
export function clientPendingTransactions(
  shared: WhiteboardDocument,
  current: WhiteboardDocument,
  inflight?: WhiteboardTransaction,
): WhiteboardTransaction[] {
  const base = inflight ? applyWhiteboardTransaction(shared, inflight) : shared;
  const after = diffWhiteboardContent(base, current);
  const pending = inflight ? [inflight] : [];
  if (hasWhiteboardChanges(after)) pending.push(after);
  return pending;
}

/** A snapshot can already contain an operation whose HTTP reply was lost. */
export function containsClientTransaction(shared: WhiteboardDocument, transaction: WhiteboardTransaction): boolean {
  return !hasWhiteboardChanges(diffWhiteboardContent(shared, applyWhiteboardTransaction(shared, transaction)));
}

/** Rebase outstanding local edits without publishing an identical replacement scene.
 * Acknowledging a stroke changes the pending queue, not the visible drawing. Keeping
 * its identity avoids an external-document reset racing the next Pencil gesture.
 */
export function reconcileClientDocument(
  shared: WhiteboardDocument,
  pending: readonly WhiteboardTransaction[],
  current: WhiteboardDocument,
  viewport: WhiteboardViewport = current.viewport,
): WhiteboardDocument {
  let next = shared;
  for (const transaction of pending) next = applyWhiteboardTransaction(next, transaction);
  next = { ...next, viewport };
  return sameValue(next, current) ? current : next;
}
