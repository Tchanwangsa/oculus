function targetElement(target: EventTarget | null): Element | null {
  const node = target as Node | null;
  return node?.nodeType === 1 ? node as Element : node?.parentElement ?? null;
}

/** Text editing keeps native selection, copy/paste menus and caret behavior. */
export function isCanvasTextEditorTarget(target: EventTarget | null): boolean {
  const element = targetElement(target);
  if (!element) return false;
  if (element.closest('input,textarea')) return true;
  // The closest declaration matters: contenteditable=false can nest inside a host.
  const editor = element.closest('[contenteditable]');
  if (!editor) return false;
  const value = editor.getAttribute('contenteditable')?.toLowerCase();
  return value === '' || value === 'true' || value === 'plaintext-only';
}

export function preventCanvasChromeSelection(event: Event): void {
  if (!isCanvasTextEditorTarget(event.target)) event.preventDefault();
}

/** Suppress native Safari canvas touch defaults; drawing uses Pointer Events.
 * Restricting this to the canvas preserves toolbar clicks and text editor controls.
 */
export function preventNativeCanvasTouch(event: Event): void {
  if (targetElement(event.target)?.tagName === 'CANVAS' && !isCanvasTextEditorTarget(event.target)) event.preventDefault();
}

/** Scope to the canvas app shell, alongside user-select/touch-callout CSS.
 * Touch cancellation is confined to the actual canvas. Events continue propagating,
 * and the drawing/pan handlers continue to consume Pointer Events normally.
 */
export function installBrowserSelectionGuard(root: HTMLElement): () => void {
  root.addEventListener('selectstart', preventCanvasChromeSelection, true);
  root.addEventListener('contextmenu', preventCanvasChromeSelection, true);
  root.addEventListener('touchstart', preventNativeCanvasTouch, { capture: true, passive: false });
  root.addEventListener('touchend', preventNativeCanvasTouch, { capture: true, passive: false });
  return () => {
    root.removeEventListener('selectstart', preventCanvasChromeSelection, true);
    root.removeEventListener('contextmenu', preventCanvasChromeSelection, true);
    root.removeEventListener('touchstart', preventNativeCanvasTouch, true);
    root.removeEventListener('touchend', preventNativeCanvasTouch, true);
  };
}
