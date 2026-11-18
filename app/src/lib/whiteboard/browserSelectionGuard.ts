function targetElement(target: EventTarget | null): Element | null {
  const node = target as Node | null;
  return node?.nodeType === 1 ? node as Element : node?.parentElement ?? null;
}

/** Text editing keeps native selection, copy/paste menus and caret behavior. */
export function isWhiteboardTextEditorTarget(target: EventTarget | null): boolean {
  const element = targetElement(target);
  if (!element) return false;
  if (element.closest('input,textarea')) return true;
  // The closest declaration matters: contenteditable=false can nest inside a host.
  const editor = element.closest('[contenteditable]');
  if (!editor) return false;
  const value = editor.getAttribute('contenteditable')?.toLowerCase();
  return value === '' || value === 'true' || value === 'plaintext-only';
}

export function preventWhiteboardChromeSelection(event: Event): void {
  if (!isWhiteboardTextEditorTarget(event.target)) event.preventDefault();
}

/** Suppress native Safari canvas touch and gesture defaults; drawing uses Pointer Events.
 * Restricting this to the canvas preserves toolbar clicks and text editor controls.
 */
export function preventNativeWhiteboardTouch(event: Event): void {
  if (targetElement(event.target)?.tagName === 'CANVAS' && !isWhiteboardTextEditorTarget(event.target)) event.preventDefault();
}

/** Scope to the whiteboard app shell, alongside user-select/touch-callout CSS.
 * Touch and native gesture cancellation is confined to the actual canvas. Events continue propagating,
 * and the drawing/pan handlers continue to consume Pointer Events normally.
 */
export function installBrowserSelectionGuard(root: HTMLElement, { suppressNativeGestures = false, touchEventRoot = root }: { suppressNativeGestures?: boolean; touchEventRoot?: HTMLElement } = {}): () => void {
  root.addEventListener('selectstart', preventWhiteboardChromeSelection, true);
  root.addEventListener('contextmenu', preventWhiteboardChromeSelection, true);
  // A body listener gives the companion a stable touch region across canvas repaints.
  const touch = (event: Event) => {
    const target = targetElement(event.target);
    if (touchEventRoot === root || (target && root.contains(target))) preventNativeWhiteboardTouch(event);
  };
  for (const type of ['touchstart', 'touchmove', 'touchend']) touchEventRoot.addEventListener(type, touch, { capture: true, passive: false });
  // The companion owns native gestures; desktop WebKit needs them for trackpad zoom.
  const nativeGestures = suppressNativeGestures ? ['gesturestart', 'gesturechange', 'gestureend'] : [];
  for (const type of nativeGestures) root.addEventListener(type, preventNativeWhiteboardTouch, { capture: true, passive: false });
  return () => {
    root.removeEventListener('selectstart', preventWhiteboardChromeSelection, true);
    root.removeEventListener('contextmenu', preventWhiteboardChromeSelection, true);
    for (const type of ['touchstart', 'touchmove', 'touchend']) touchEventRoot.removeEventListener(type, touch, true);
    for (const type of nativeGestures) root.removeEventListener(type, preventNativeWhiteboardTouch, true);
  };
}
