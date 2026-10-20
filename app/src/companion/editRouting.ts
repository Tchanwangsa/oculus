/**
 * `@/lib/menu/editRouting` in the companion build (`vite.companion.config.ts`
 * aliases it here). A browser has no native Edit menu taking ⌘Z first, so the
 * canvas's own keydown handles undo and registering is a no-op; the real module
 * would also pull the maths engine in through Select All.
 */
export { EDITABLE } from "@/lib/menu/editable";

export function onEdit(_handler: (redo: boolean) => boolean): () => void {
  return () => {};
}
