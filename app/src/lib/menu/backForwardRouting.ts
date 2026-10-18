/**
 * What the View menu's Back and Forward (⌘[ and ⌘]) do. The menu takes those
 * keys before the app's webview sees them, so a focused control that gives
 * them another meaning (a drawing canvas's layer order) registers here; else
 * the pane's history steps (`components/tabs/strip/useTabMenuEvents.ts`).
 */

type BackForwardHandler = (delta: 1 | -1) => boolean;

const handlers = new Set<BackForwardHandler>();

/** Offer Back and Forward to `handler` first; it returns whether it took it. */
export function onBackForward(handler: BackForwardHandler): () => void {
  handlers.add(handler);
  return () => void handlers.delete(handler);
}

/** Whether a handler took the step. A browser page holding native focus keeps its own history. */
export function claimBackForward(delta: 1 | -1): boolean {
  if (!document.hasFocus()) return false;
  for (const handler of handlers) if (handler(delta)) return true;
  return false;
}
