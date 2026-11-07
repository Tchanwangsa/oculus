import { measureCanvas, traceCanvas } from '@/lib/canvas/diagnostics';
import { useCallback, useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent, type PointerEvent, type MouseEvent, type ClipboardEvent } from 'react';
import { LABELLED_TYPES, type CanvasDocument, type CanvasElement, type CanvasTool, type CanvasViewport, type Point } from '@/lib/canvas/model';
import { boundsContain, elementBounds, pointInBounds, screenToWorld, selectionBounds, zoomAt, type Bounds } from '@/lib/canvas/geometry';
import { hitTest } from '@/lib/canvas/engine';
import { paintCanvas, pointEditTarget, elementResizeHandles, selectedBounds } from '@/lib/canvas/render';
import { pointDrag, pointHandleAt } from '@/lib/canvas/arrow/handles';
import { endAt, movePoint, removePoint } from '@/lib/canvas/arrow/points';
import { EDITABLE, onEdit } from '@/lib/menu/editRouting';
import { onBackForward } from '@/lib/menu/backForwardRouting';
import { CANVAS_CLIPBOARD_MIME, serializeCanvasClipboard, parseCanvasClipboard, cloneCanvasElements } from '@/lib/canvas/renderClipboard';
import { applyElementChanges, type ElementChanges } from '@/lib/canvas/changes';
import { canReorder, reorderElements, type LayerMove } from '@/lib/canvas/layers';
import { bindArrowEnd, findBindingSnap, followBindings, moveElements, sameSnap, sceneLookup, type BindingSnap } from '@/lib/canvas/bindings';
import { moveSegment, releaseSegment } from '@/lib/canvas/elbow/segments';
import { commitTextEdit, labelTargetAt, stylePatchChanges } from '@/lib/canvas/labels';
import { resizeBounds, resizeElements } from '@/lib/canvas/resize';
import { placeImagesIn, type CanvasImageSource } from '@/lib/canvas/mediaPlacement';
import { TouchViewportGesture } from '@/lib/canvas/touchViewport';
import { applyCanvasTransaction, diffCanvasContent, hasCanvasChanges, sameValue, type CanvasTransaction } from '@/lib/canvas/collaboration';
import { CAPTURE_LIFT_DELAY_MS, type CaptureHoldIndicator, copyRegionImage, rasterizeRegion, regionHandleAt, moveRegion, regionBounds, resizeRegion, type CanvasRegion } from '@/lib/canvas/region';
import { asDataUrl } from '@/lib/canvas/importMedia';
import { secureId } from '@/lib/canvas/ids';
import {
  clampCamera, isLocked, keepOnPage, notebookPaper, pageContaining, pageInView, pageView, selectableElements, visiblePageCentre, zoomLimits, type CanvasKind, type PaperTemplate,
} from '@/lib/canvas/paper';
import { CANVAS_ZOOM_LIMITS } from '@/lib/canvas/zoom';
import {
  addPage as addNotebookPage, duplicatePage, insertPage, insertPdfPages, movePage, removePage as removeNotebookPage, setPageSize, setPageTemplate, type PageSize,
} from '@/lib/canvas/pages';
import { elementsAtPage, shownBounds } from '@/lib/canvas/pageHits';
import type { CanvasImportedImage } from '@/lib/canvas/importMedia';
import {
  adoptElementStyle, colorLabel, DEFAULT_DRAWING_STYLE, inspectorSections, inspectorTarget, inspectorValues, newElementStyle, patchDrawingStyle,
  patchElementStyle, type DrawingStyle, type InspectorTarget, type StylePatch,
} from '@/lib/canvas/style';

/** `text` writes standalone text (`element` is the existing one or a template for a new one); `label` writes into a shape. */
export interface TextDraft { kind: 'text' | 'label'; element: CanvasElement; text: string; existing: boolean }
export interface CanvasInteractionOptions {
  /** When given, pan and zoom go only here and never through `onChange`, so the host keeps its camera local. */
  onViewportChange?: (viewport: CanvasViewport) => void;
  /** Lifting the eraser after a completed erase returns to the tool chosen before it. */
  eraserAutoReturn?: boolean;
  /**
   * `notebook` binds the document to its pages: they are drawn, the camera stays on them, ink starts only on a page, stays
   * on one and is clipped to it. `canvas` (the default) is a whiteboard; any `paper` it holds is kept and never used.
   */
  kind?: CanvasKind;
}
type Gesture = {
  /** `write` is a touch tap with the Text tool, which opens the overlay on release so a second finger can still start navigating. */
  mode: 'pan' | 'draw' | 'move' | 'resize' | 'point' | 'segment' | 'marquee' | 'erase' | 'write' | 'capture' | 'crop' | 'capture-copy' | 'crop-confirm';
  start: Point;
  screen: Point;
  original: CanvasDocument;
  moved: boolean;
  pointerId: number;
  pointerType: string;
  id?: string;
  handle?: number;
  /** A point drag: the line or arrow being edited (with a midpoint's new point in it) and the point that follows the pointer; a segment drag: the elbow arrow and the segment's index. */
  base?: CanvasElement;
  index?: number;
  bounds?: Bounds;
  shift?: boolean;
};
/** One undo step: the transactions each way, plus the layer order to put back elements the step re-adds (a transaction appends them). */
type HistoryEntry = { undo: CanvasTransaction; redo: CanvasTransaction; undoOrder?: string[]; redoOrder?: string[] };
const normalize = (a: Point, b: Point): Bounds => ({ x: Math.min(a.x, b.x), y: Math.min(a.y, b.y), width: Math.abs(a.x - b.x), height: Math.abs(a.y - b.y) });
const newId = secureId;
const newSeed = () => Math.floor(Math.random() * 100000);
const NON_DRAWING = new Set<CanvasTool>(['select', 'hand', 'eraser', 'capture', 'crop']);
const sameContent = (a: CanvasDocument, b: CanvasDocument) => sameValue({ ...a, viewport: null }, { ...b, viewport: null });
const removes = (transaction: CanvasTransaction) => transaction.edits.some(edit => edit.after === null);

/** Puts each element `transaction` added back after the element it followed in `order`, rather than on top. */
function restoreLayerOrder(document: CanvasDocument, transaction: CanvasTransaction, order?: string[]): CanvasDocument {
  const added = new Set(transaction.edits.filter(edit => edit.before === null).map(edit => edit.id));
  if (!order || !added.size) return document;
  const byId = new Map(document.elements.map(element => [element.id, element]));
  const rest = document.elements.filter(element => !added.has(element.id)), present = new Set(rest.map(element => element.id));
  const after = new Map<string | null, CanvasElement[]>();
  let anchor: string | null = null;
  for (const id of order) {
    const element = added.has(id) ? byId.get(id) : undefined;
    if (element) { after.set(anchor, [...after.get(anchor) ?? [], element]); added.delete(id); }
    else if (present.has(id)) anchor = id;
  }
  const elements = [...after.get(null) ?? [], ...rest.flatMap(element => [element, ...after.get(element.id) ?? []])];
  for (const id of added) { const element = byId.get(id); if (element) elements.push(element); }
  return { ...document, elements };
}

export function useCanvasInteraction(document: CanvasDocument, onChange: (document: CanvasDocument) => void, options: CanvasInteractionOptions = {}) {
  const root = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const documentRef = useRef(document);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;
  const optionsRef = useRef(options);
  optionsRef.current = options;
  /** Documents this pane handed to `onChange`; one coming back is an echo, never a remote change. */
  const published = useRef(new WeakSet<CanvasDocument>());
  const [tool, setTool] = useState<CanvasTool>('select');
  const previousTool = useRef<CanvasTool>('freedraw');
  // Safari cannot say whether a Pencil is paired; its first pen contact turns one finger into navigation.
  const penSeen = useRef(false);
  const [style, setStyle] = useState<DrawingStyle>(DEFAULT_DRAWING_STYLE);
  const [grid, setGrid] = useState(true);
  const [selection, setSelection] = useState<string[]>([]);
  const [revision, setRevision] = useState(0);
  const [space, setSpace] = useState(false);
  const [resizeCursor, setResizeCursor] = useState<string | null>(null);
  const [region, setRegion] = useState<CanvasRegion | null>(null);
  const regionRef = useRef<CanvasRegion | null>(null);
  const [regionCopyState, setRegionCopyState] = useState<'idle' | 'copying' | 'copied' | 'duplicating'>('idle');
  const [regionError, setRegionError] = useState<string | null>(null);
  const [captureHold, setCaptureHold] = useState<CaptureHoldIndicator | null>(null);
  const captureHoldTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const captureGhost = useRef<{ snapshot: CanvasDocument; from: Bounds; to: Bounds } | undefined>(undefined);
  const [textDraft, setTextDraft] = useState<TextDraft | null>(null);
  /** The open draft and its latest text, read synchronously so a style change can commit it first and a later blur is a no-op. */
  const draftRef = useRef<TextDraft | null>(null);
  const draftValue = useRef('');
  const selected = useRef(new Set<string>());
  const gesture = useRef<Gesture | null>(null);
  const touchViewport = useRef(new TouchViewportGesture());
  const overrides = useRef(new Map<string, CanvasElement | null>());
  const marquee = useRef<Bounds | undefined>(undefined);
  const bindHint = useRef<BindingSnap | null>(null);
  const size = useRef({ width: 0, height: 0, pixelRatio: 1 });
  const frame = useRef(0);
  const lastPaste = useRef({ text: '', count: 0 });
  const past = useRef<HistoryEntry[]>([]);
  const future = useRef<HistoryEntry[]>([]);
  const [historyError, setHistoryError] = useState<string | null>(null);
  const viewSave = useRef<ReturnType<typeof setTimeout> | null>(null);
  /** A notebook opens fitted to its page width once, when its camera is still the untouched default. */
  const fitted = useRef(false);
  /** A pen whose press was refused off the page; its later moves must not start the stroke through pen recovery. */
  const refusedPointer = useRef<number | null>(null);
  /** The one flag every page behaviour keys off, read through `paintOptions` by the handlers. */
  const notebook = options.kind === 'notebook';
  const paintOptions = useRef({ grid, editingId: textDraft?.element.id, notebook });
  paintOptions.current = { grid, editingId: textDraft?.element.id, notebook };
  /** A notebook's pages, or nothing on a canvas. */
  const boundPaper = () => notebookPaper(documentRef.current, paintOptions.current.notebook);
  const zoomRange = () => {
    const paper = boundPaper();
    return paper && size.current.width > 0 ? zoomLimits(paper, size.current) : CANVAS_ZOOM_LIMITS;
  };
  /** `viewport` kept on a notebook's pages (`clampCamera`); unchanged, the same object, anywhere else. */
  const clampView = (viewport: CanvasViewport) => {
    const paper = boundPaper();
    return paper ? clampCamera(paper, size.current, viewport) : viewport;
  };
  /** Whether `point` is on a notebook's desk, where nothing new may start. */
  const offPage = (point: Point) => {
    const paper = boundPaper();
    return !!paper && !pageContaining(paper, point);
  };
  /** What the pointer at `point` can reach: in a notebook only elements whose own page holds it, since the rest is clipped. */
  const reachable = (elements: readonly CanvasElement[], point: Point) => {
    const paper = boundPaper();
    return paper ? elementsAtPage(paper, elements, point) : elements;
  };
  /** The bounds a box selection must enclose: in a notebook only the part of an element its page shows. */
  const boxBounds = (element: CanvasElement) => {
    const paper = boundPaper();
    return paper ? shownBounds(paper, element) : elementBounds(element);
  };
  /** The selection's centre, which a notebook keeps on a page. */
  const selectionCentre = (elements: readonly CanvasElement[]) => {
    const bounds = selectionBounds(elements.filter(element => selected.current.has(element.id)));
    return bounds && { x: bounds.x + bounds.width / 2, y: bounds.y + bounds.height / 2 };
  };
  /** A move of the selection by `dx`, `dy`, shortened in a notebook so the selection's centre stays on a page. */
  const moveDelta = (elements: readonly CanvasElement[], dx: number, dy: number) => {
    const paper = boundPaper(), centre = paper && selectionCentre(elements);
    return paper && centre ? keepOnPage(paper, centre, { x: dx, y: dy }) : { x: dx, y: dy };
  };
  /**
   * New elements moved as one so their centre is on a page in a notebook: to the nearest page edge, or with `toView` to
   * the middle of the page in view (for a paste from elsewhere).
   */
  const placeOnPage = (elements: CanvasElement[], toView = false): CanvasElement[] => {
    const paper = boundPaper(), bounds = paper && selectionBounds(elements);
    if (!paper || !bounds) return elements;
    const centre = { x: bounds.x + bounds.width / 2, y: bounds.y + bounds.height / 2 };
    let delta = keepOnPage(paper, centre, { x: 0, y: 0 });
    if (!delta.x && !delta.y) return elements;
    if (toView) {
      const { viewport } = documentRef.current, target = visiblePageCentre(pageInView(paper, viewport, size.current), viewport, size.current);
      delta = { x: target.x - centre.x, y: target.y - centre.y };
    }
    return elements.map(element => ({ ...element, x: element.x + delta.x, y: element.y + delta.y }));
  };

  const redraw = useCallback(() => {
    if (frame.current) return;
    frame.current = requestAnimationFrame(() => measureCanvas('canvas.paint', () => {
      frame.current = 0;
      const context = canvas.current?.getContext('2d');
      if (!context) return;
      paintCanvas(context, documentRef.current, {
        ...size.current, ...paintOptions.current, selected: selected.current,
        selectionColor: getComputedStyle(root.current!).getPropertyValue('--color-brand').trim() || '#5e6ad2',
        marquee: marquee.current ?? regionRef.current?.bounds, captureSelection: regionRef.current?.kind === 'capture' || gesture.current?.mode === 'capture', captureGhost: captureGhost.current, cropSource: regionRef.current?.source ? elementBounds(regionRef.current.source) : undefined, overrides: overrides.current, bindingHint: bindHint.current,
      });
    }, { elements: documentRef.current.elements.length, overrides: overrides.current.size }));
  }, []);

  const choose = useCallback((ids: Iterable<string>) => {
    selected.current = new Set(ids);
    setSelection([...selected.current]);
    redraw();
  }, [redraw]);

  const emit = useCallback((next: CanvasDocument) => { published.current.add(next); onChangeRef.current(next); }, []);

  /** `record` makes the change one undo step: the transaction from the current scene to `next`, so undo touches only this pane's edits. */
  const publish = useCallback((next: CanvasDocument, record = false) => {
    if (record) {
      const before = documentRef.current, redo = diffCanvasContent(before, next);
      if (hasCanvasChanges(redo)) {
        const undo = diffCanvasContent(next, before);
        const entry: HistoryEntry = { undo, redo };
        if (removes(redo)) entry.undoOrder = before.elements.map(element => element.id);
        if (removes(undo)) entry.redoOrder = next.elements.map(element => element.id);
        past.current = [...past.current.slice(-99), entry];
        future.current = [];
      }
    }
    documentRef.current = next;
    emit(next);
    setRevision(value => value + 1);
    redraw();
  }, [emit, redraw]);

  /** Every gesture preview goes through here, so bound arrows follow the shapes being previewed. */
  const previewChanges = (changes: ElementChanges) => {
    for (const [id, element] of changes) overrides.current.set(id, element);
    for (const [id, element] of followBindings(documentRef.current.elements, overrides.current)) overrides.current.set(id, element);
  };
  /** Every committed element edit goes through here, bound arrows following; returns whether the scene changed. One undo step. */
  const commitChanges = (changes: ElementChanges) => {
    const current = documentRef.current;
    const elements = applyElementChanges(current.elements, followBindings(current.elements, changes));
    if (elements === current.elements) return false;
    publish({ ...current, elements }, true);
    return true;
  };

  /** Layer order has its own publish: `commitChanges` keeps order. One undo step. */
  const reorder = useCallback((move: LayerMove) => {
    if (gesture.current || touchViewport.current.active) return;
    const current = documentRef.current;
    const elements = reorderElements(current.elements, selected.current, move);
    if (elements !== current.elements) publish({ ...current, elements }, true);
  }, [publish]);

  /** A step whose shapes someone else has changed since is refused and dropped, so it never overwrites or removes their work. */
  const history = useCallback((redo = false) => {
    if (gesture.current || touchViewport.current.active) return;
    const entry = (redo ? future : past).current.pop();
    if (!entry) return;
    const transaction = redo ? entry.redo : entry.undo;
    let next: CanvasDocument;
    try { next = restoreLayerOrder(applyCanvasTransaction(documentRef.current, transaction), transaction, redo ? entry.redoOrder : entry.undoOrder); }
    catch {
      setHistoryError(`${redo ? 'Redo' : 'Undo'} skipped: someone else has changed what it would restore, and their edit was kept.`);
      setRevision(value => value + 1);
      return;
    }
    (redo ? past : future).current.push(entry);
    setHistoryError(null);
    choose([]);
    publish(next);
  }, [choose, publish]);

  /** Moves the camera without a render, for every pointer sample of a pan. The one writer: a notebook's camera is clamped here. */
  const setView = useCallback((requested: CanvasViewport) => {
    const viewport = clampView(requested);
    documentRef.current = { ...documentRef.current, viewport };
    optionsRef.current.onViewportChange?.(viewport);
  }, []);
  const updateViewport = useCallback((viewport: CanvasViewport, save = true) => {
    setView(viewport);
    redraw();
    setRevision(value => value + 1);
    if (save && !optionsRef.current.onViewportChange) {
      if (viewSave.current) clearTimeout(viewSave.current);
      viewSave.current = setTimeout(() => { viewSave.current = null; emit(documentRef.current); }, 180);
    }
  }, [emit, redraw, setView]);
  /** A pan or two-finger navigation has ended: save the camera in the document, or only re-render when the host keeps it. */
  const settleView = () => {
    if (optionsRef.current.onViewportChange) setRevision(value => value + 1);
    else publish(documentRef.current);
  };

  /**
   * A document from outside this pane (another pane, a collaborator, a recovery copy) becomes the new base. The camera,
   * the gesture in progress, undo history, the text being written and the selection of shapes that still exist stay.
   */
  const adoptExternal = (incoming: CanvasDocument) => {
    const local = documentRef.current;
    if (sameContent(local, incoming)) return;
    const base = { ...incoming, viewport: local.viewport };
    documentRef.current = base;
    // New paper (a page added or removed elsewhere) can leave the camera off the pages.
    if (clampView(base.viewport) !== base.viewport) setView(base.viewport);
    const ids = new Set(base.elements.map(element => element.id)), had = new Set(local.elements.map(element => element.id));
    const gone = (id: string) => had.has(id) && !ids.has(id);
    const active = gesture.current;
    if (active) {
      active.original = { ...base, viewport: active.original.viewport };
      // A point or segment drag has nothing left to edit once its line is removed.
      if ((active.mode === 'point' || active.mode === 'segment') && active.id && gone(active.id)) {
        gesture.current = null; overrides.current.clear(); bindHint.current = null;
      }
    }
    for (const id of [...overrides.current.keys()]) if (gone(id)) overrides.current.delete(id);
    if ([...selected.current].some(gone)) choose([...selected.current].filter(id => !gone(id)));
    setRevision(value => value + 1);
  };
  useLayoutEffect(() => {
    if (documentRef.current !== document && !published.current.has(document)) adoptExternal(document);
    redraw();
  }, [document, redraw]);
  useEffect(() => { redraw(); }, [grid, textDraft, redraw]);
  useEffect(() => {
    const element = canvas.current;
    const container = root.current;
    if (!element || !container) return;
    const resize = () => {
      const rect = container.getBoundingClientRect();
      const pixelRatio = window.devicePixelRatio || 1;
      size.current = { width: rect.width, height: rect.height, pixelRatio };
      element.width = Math.round(rect.width * pixelRatio);
      element.height = Math.round(rect.height * pixelRatio);
      const paper = boundPaper(), { viewport } = documentRef.current;
      if (paper && !fitted.current && rect.width > 0) {
        fitted.current = true;
        if (viewport.x === 0 && viewport.y === 0 && viewport.zoom === 1) { setView(pageView(paper, rect)); setRevision(value => value + 1); }
      }
      // A notebook's saved camera, or one a resize has left off the pages, comes back onto them.
      const settled = documentRef.current.viewport;
      if (rect.width > 0 && clampView(settled) !== settled) { setView(settled); setRevision(value => value + 1); }
      redraw();
    };
    const observer = new ResizeObserver(resize);
    observer.observe(container);
    resize();
    return () => {
      observer.disconnect();
      cancelAnimationFrame(frame.current);
      frame.current = 0;
      if (viewSave.current) { clearTimeout(viewSave.current); viewSave.current = null; emit(documentRef.current); }
    };
  }, [emit, redraw]);
  useEffect(() => onEdit(redo => {
    if (!root.current?.contains(window.document.activeElement) || window.document.activeElement?.matches('input,textarea,[contenteditable=true]')) return false;
    history(redo); return true;
  }), [history]);
  // The View menu's Back and Forward hold ⌘[ and ⌘]; with a selection they step its layer instead.
  useEffect(() => onBackForward(delta => {
    if (!selected.current.size || !root.current?.contains(window.document.activeElement) || window.document.activeElement?.matches('input,textarea,[contenteditable=true]')) return false;
    reorder(delta === 1 ? 'forward' : 'backward'); return true;
  }), [reorder]);
  useEffect(() => {
    const element = root.current;
    if (!element) return;
    const beforeInput = (event: InputEvent) => {
      if ((event.target as HTMLElement).closest('input,textarea,[contenteditable=true]')) return;
      if (event.inputType !== 'historyUndo' && event.inputType !== 'historyRedo') return;
      event.preventDefault(); event.stopPropagation(); history(event.inputType === 'historyRedo');
    };
    element.addEventListener('beforeinput', beforeInput);
    return () => element.removeEventListener('beforeinput', beforeInput);
  }, [history]);
  useEffect(() => {
    const element = canvas.current;
    if (!element) return;
    const wheel = (event: WheelEvent) => {
      event.preventDefault();
      if (gesture.current || touchViewport.current.active) return;
      const viewport = documentRef.current.viewport;
      const rect = element.getBoundingClientRect();
      const screen = { x: event.clientX - rect.left, y: event.clientY - rect.top };
      if (event.ctrlKey || event.metaKey) updateViewport(zoomAt(viewport, screen, viewport.zoom * Math.exp(-event.deltaY * 0.01), zoomRange()));
      else updateViewport({ ...viewport, x: viewport.x - event.deltaX, y: viewport.y - event.deltaY });
    };
    element.addEventListener('wheel', wheel, { passive: false });
    return () => element.removeEventListener('wheel', wheel);
  }, [updateViewport]);

  const coordinates = (event: { clientX: number; clientY: number }) => {
    const rect = canvas.current!.getBoundingClientRect();
    const screen = { x: event.clientX - rect.left, y: event.clientY - rect.top };
    return { screen, point: screenToWorld(screen, documentRef.current.viewport) };
  };
  const openDraft = (draft: TextDraft) => { draftRef.current = draft; draftValue.current = draft.text; setTextDraft(draft); };
  const startText = (point: Point, element?: CanvasElement) => {
    choose([]);
    const template: CanvasElement = element ?? { id: newId(), type: 'text', x: point.x, y: point.y, width: 0, height: 0, seed: newSeed(), ...newElementStyle('text', style) };
    openDraft({ kind: 'text', element: template, text: element?.text ?? '', existing: Boolean(element) });
  };
  /** The shape keeps its selection while its label is written, so the inspector shows its text options. */
  const startLabel = (shape: CanvasElement) => {
    choose([shape.id]);
    openDraft({ kind: 'label', element: shape, text: shape.text ?? '', existing: true });
  };
  /** Text under the pointer, a shape's outline, or the inside of a shape that can take a label. */
  const writableAt = (point: Point) => {
    const current = documentRef.current, tolerance = 6 / current.viewport.zoom;
    const elements = reachable(current.elements, point), id = hitTest(elements, point, tolerance);
    const hit = elements.find(element => element.id === id);
    if (hit && (hit.type === 'text' || LABELLED_TYPES.has(hit.type))) return hit;
    return labelTargetAt(elements, point, tolerance);
  };
  const startWriting = (point: Point, element: CanvasElement) => element.type === 'text' ? startText(point, element) : startLabel(element);
  const clearCaptureHold = () => {
    if (captureHoldTimer.current !== null) clearTimeout(captureHoldTimer.current);
    captureHoldTimer.current = null; setCaptureHold(null);
  };
  useEffect(() => () => { if (captureHoldTimer.current !== null) clearTimeout(captureHoldTimer.current); }, []);
  const updateRegion = (next: CanvasRegion | null) => {
    clearCaptureHold();
    regionRef.current = next; setRegion(next); setRegionCopyState('idle'); setRegionError(null); marquee.current = next?.bounds; redraw();
  };
  const cancelRegion = () => { cancelGesture(); updateRegion(null); setTool('select'); root.current?.focus({ preventScroll: true }); };
  const startCrop = (element?: CanvasElement) => {
    const source = element ?? documentRef.current.elements.find(item => selected.current.size === 1 && selected.current.has(item.id));
    if (!source || source.type !== 'image' || isLocked(source) || gesture.current) return;
    choose([]); setTool('crop'); root.current?.focus({ preventScroll: true });
    updateRegion({ kind: 'crop', source, snapshot: documentRef.current, bounds: elementBounds(source) });
  };
  const completeRegion = (expected: CanvasRegion, image: CanvasImageSource, at?: Point) => {
    if (!expected || regionRef.current !== expected || gesture.current) return false;
    const { bounds, source } = expected;
    if (source && !sameValue(documentRef.current.elements.find(item => item.id === source.id), source)) throw new Error('This image changed while cropping. Cancel and crop it again.');
    const [element] = placeOnPage([{
      ...(source ?? { id: newId(), type: 'image', stroke: 'transparent', fill: 'transparent', strokeWidth: 0, style: 'clean', seed: 0 }),
      x: at?.x ?? bounds.x + (source ? 0 : 24), y: at?.y ?? bounds.y + (source ? 0 : 24), width: bounds.width, height: bounds.height,
      dataUrl: image.dataUrl, naturalWidth: image.width, naturalHeight: image.height, sourceName: image.name,
    }]);
    commitChanges(new Map([[element.id, element]]));
    cancelRegion(); choose([element.id]);
    return true;
  };
  const applyCrop = async (expected = regionRef.current) => {
    if (expected?.kind !== 'crop' || gesture.current) return;
    setRegionError(null);
    try {
      const result = await rasterizeRegion(expected, paintOptions.current.notebook);
      completeRegion(expected, { dataUrl: await asDataUrl(result.blob), width: result.width, height: result.height, name: expected.source?.sourceName ?? 'Cropped image.png' });
    } catch (error) {
      if (regionRef.current === expected) setRegionError(error instanceof Error ? error.message : String(error));
    }
  };
  const copyRegion = async () => {
    const expected = regionRef.current;
    if (expected?.kind !== 'capture' || gesture.current) return;
    setRegionCopyState('copying'); setRegionError(null);
    try {
      await copyRegionImage({ ...expected, snapshot: documentRef.current }, paintOptions.current.notebook);
      if (regionRef.current === expected) setRegionCopyState('copied');
    } catch (error) {
      if (regionRef.current === expected) { setRegionCopyState('idle'); setRegionError(error instanceof Error ? error.message : String(error)); }
    }
  };
  const duplicateCapture = async (expected: CanvasRegion, at: Point) => {
    setRegionCopyState('duplicating'); setRegionError(null);
    try {
      const result = await rasterizeRegion({ ...expected, snapshot: documentRef.current }, paintOptions.current.notebook);
      completeRegion(expected, { dataUrl: await asDataUrl(result.blob), width: result.width, height: result.height, name: 'Canvas capture.png' }, at);
    } catch (error) {
      if (regionRef.current === expected) setRegionError(error instanceof Error ? error.message : String(error));
    } finally { if (regionRef.current === expected) setRegionCopyState('idle'); }
  };
  const duplicateRegion = () => {
    const expected = regionRef.current;
    if (expected?.kind === 'capture' && !gesture.current) void duplicateCapture(expected, { x: expected.bounds.x + 24, y: expected.bounds.y + 24 });
  };
  const armCaptureHold = (active: Gesture) => {
    const expected = regionRef.current;
    if (active.mode !== 'capture' || !active.bounds || active.handle !== -1 || expected?.kind !== 'capture') return;
    setCaptureHold({ point: active.start, phase: 'holding' });
    captureHoldTimer.current = setTimeout(() => {
      captureHoldTimer.current = null;
      if (gesture.current !== active || active.moved || regionRef.current !== expected) return;
      active.mode = 'capture-copy';
      const offset = 10 / documentRef.current.viewport.zoom;
      marquee.current = moveRegion(active.bounds!, { x: offset, y: -offset });
      captureGhost.current = { snapshot: documentRef.current, from: expected.bounds, to: marquee.current };
      setCaptureHold({ point: active.start, phase: 'lifted' }); setResizeCursor('grabbing'); redraw();
    }, CAPTURE_LIFT_DELAY_MS);
  };
  useEffect(() => { traceCanvas('tool.committed', { tool }); }, [tool]);
  const chooseTool = (next: CanvasTool) => {
    traceCanvas('tool.request', { previous: tool, next, gesture: gesture.current?.mode ?? 'none' });
    if (next === 'crop') { startCrop(); return; }
    if (regionRef.current) { cancelGesture(); updateRegion(null); }
    if (next === 'eraser' && tool !== 'eraser') previousTool.current = tool;
    setTool(next); bindHint.current = null; if (next !== 'select') choose([]);
  };
  /** Binds an arrow end at the pointer (⌘/Ctrl places it freely) and shows the target it binds to. */
  const bindEnd = (arrow: CanvasElement, end: 'start' | 'end', point: Point, event: { metaKey: boolean; ctrlKey: boolean }) => {
    const bound = bindArrowEnd(arrow, end, point, documentRef.current.elements, documentRef.current.viewport.zoom, { live: overrides.current, free: event.metaKey || event.ctrlKey });
    bindHint.current = bound.snap;
    return bound.arrow;
  };
  // WebKit throws when the pointer is already gone; the gesture still runs without capture.
  const capturePointer = (event: PointerEvent<HTMLCanvasElement>) => {
    try { event.currentTarget.setPointerCapture(event.pointerId); } catch { /* the gesture continues uncaptured */ }
  };
  const releasePointer = (event: PointerEvent<HTMLCanvasElement>) => {
    try { if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId); } catch { /* already released */ }
  };
  /** Drops the gesture's preview without committing it; the scene was never changed during the gesture. */
  const dropGesture = () => { traceCanvas('gesture.drop', { gesture: gesture.current?.mode ?? 'none' }); clearCaptureHold(); captureGhost.current = undefined; gesture.current = null; overrides.current.clear(); marquee.current = undefined; bindHint.current = null; };
  /** Commits the preview unchanged so lifting the pointer preserves the ink already drawn. */
  const commitGesture = () => commitChanges(new Map(overrides.current));
  const eraseAt = (elements: CanvasElement[], point: Point) => {
    const erasable = reachable(elements, point).filter(element => element.type !== 'image' && !isLocked(element) && overrides.current.get(element.id) !== null);
    const id = hitTest(erasable, point, 10 / documentRef.current.viewport.zoom);
    if (id) previewChanges(new Map([[id, null]]));
  };
  const pointerDown = (event: PointerEvent<HTMLCanvasElement>, recoveringContact = false) => {
    traceCanvas('gesture.down', { tool, pointerId: event.pointerId, pointerType: event.pointerType, recoveringContact, gesture: gesture.current?.mode ?? 'none', penSeen: penSeen.current, touches: touchViewport.current.count });
    if (!recoveringContact && event.button !== 0 && event.button !== 1) return;
    if (!recoveringContact) refusedPointer.current = null;
    if (event.pointerType === 'pen') {
      penSeen.current = true;
      // A fresh pen down proves the last contact ended even when WebKit dropped its up: keep that stroke's ink as its own stroke.
      const previous = gesture.current;
      if (previous?.pointerType === 'pen') {
        const element = previous.mode === 'draw' && previous.id ? overrides.current.get(previous.id) : undefined;
        if (element?.type === 'freedraw' && element.points) { gesture.current = null; commitGesture(); }
        dropGesture();
      }
      // A palm or finger can land just before the Pencil; it must not claim the pen's stroke.
      if (gesture.current?.pointerType === 'touch' || touchViewport.current.count) { dropGesture(); touchViewport.current.reset(); }
    }
    if (event.pointerType === 'touch' && !textDraft) {
      // Palm contacts are ignored while the pen owns a stroke.
      if (gesture.current?.pointerType === 'pen') { event.preventDefault(); return; }
      capturePointer(event);
      if (touchViewport.current.down(event.pointerId, coordinates(event).screen, documentRef.current.viewport)) {
        // A second finger turns the first finger's gesture into navigation, without committing its preview.
        event.preventDefault();
        dropGesture(); redraw(); return;
      }
    } else if (touchViewport.current.active) { event.preventDefault(); return; }
    if (gesture.current || textDraft) { traceCanvas('input.ignored', { reason: textDraft ? 'text-draft' : 'active-gesture', gesture: gesture.current?.mode ?? 'none' }); return; }
    event.preventDefault();
    // WebKit can paste at a contenteditable's retained caret despite the canvas having DOM focus.
    globalThis.window?.getSelection()?.removeAllRanges();
    root.current?.focus({ preventScroll: true });
    capturePointer(event);
    const { point, screen } = coordinates(event);
    const current = documentRef.current;
    const initial: Gesture = { mode: 'draw', start: point, screen, original: current, moved: false, pointerId: event.pointerId, pointerType: event.pointerType, shift: event.shiftKey };
    overrides.current.clear();
    const region = regionRef.current;
    const inRegion = !!region && point.x >= region.bounds.x - 16 / current.viewport.zoom && point.x <= region.bounds.x + region.bounds.width + 16 / current.viewport.zoom && point.y >= region.bounds.y - 16 / current.viewport.zoom && point.y <= region.bounds.y + region.bounds.height + 16 / current.viewport.zoom;
    const outsideCrop = tool === 'crop' && region?.kind === 'crop' && regionHandleAt(region.bounds, point, current.viewport.zoom, event.pointerType) === undefined && (point.x < region.bounds.x || point.y < region.bounds.y || point.x > region.bounds.x + region.bounds.width || point.y > region.bounds.y + region.bounds.height);
    if (tool === 'hand' || space || event.button === 1) initial.mode = 'pan';
    else if (outsideCrop) initial.mode = 'crop-confirm';
    else if (event.pointerType === 'touch' && penSeen.current && !inRegion) initial.mode = 'pan';
    else if (tool === 'capture' && !region) initial.mode = 'capture';
    else if (tool === 'crop' || tool === 'capture') {
      const region = regionRef.current;
      if (!region) { releasePointer(event); return; }
      initial.mode = tool; initial.bounds = region.bounds; initial.base = region.source;
      initial.handle = regionHandleAt(region.bounds, point, current.viewport.zoom, event.pointerType, tool === 'capture');
      if (initial.handle === undefined) {
        if (point.x < region.bounds.x || point.y < region.bounds.y || point.x > region.bounds.x + region.bounds.width || point.y > region.bounds.y + region.bounds.height) {
          if (tool === 'capture') { updateRegion(null); initial.bounds = undefined; initial.base = undefined; }
          else { releasePointer(event); return; }
        }
        initial.handle = -1;
      }
    } else if (tool === 'text') {
      if (event.pointerType === 'touch') initial.mode = 'write';
      else {
        const target = writableAt(point);
        if (target) startWriting(point, target); else if (!offPage(point)) startText(point);
        return;
      }
    } else if (tool === 'eraser') {
      initial.mode = 'erase';
      eraseAt(current.elements, point);
      choose([]);
    } else if (tool === 'select') {
      const bounds = selectedBounds(current, selected.current);
      const selectedElements = current.elements.filter(element => selected.current.has(element.id));
      const handle = bounds ? elementResizeHandles(selectedElements, bounds).findIndex(p => Math.hypot(p.x - point.x, p.y - point.y) < 10 / current.viewport.zoom) : -1;
      const editing = pointEditTarget(selectedElements), pointHandle = editing && pointHandleAt(editing, point, current.viewport.zoom);
      const id = hitTest(reachable(selectableElements(current.elements), point), point, 6 / current.viewport.zoom);
      if (editing && pointHandle) {
        initial.id = editing.id;
        if (pointHandle.kind === 'segment') Object.assign(initial, { mode: 'segment', base: editing, index: pointHandle.segment });
        else { initial.mode = 'point'; Object.assign(initial, pointDrag(editing, pointHandle)); }
      } else if (handle >= 0 && bounds) { initial.mode = 'resize'; initial.bounds = bounds; initial.handle = handle; }
      else if (!event.shiftKey && bounds && !editing && pointInBounds(point, bounds, 5 / current.viewport.zoom)) initial.mode = 'move';
      else if (id) {
        const found = current.elements.find(element => element.id === id);
        // A page is a backdrop for annotations: drag on an unselected page to
        // box-select ink, or click it first to move the page itself.
        if (found?.type === 'image' && !selected.current.has(id)) {
          initial.mode = 'marquee'; initial.id = id;
          if (!event.shiftKey) choose([]);
          gesture.current = initial; redraw(); return;
        }
        initial.mode = 'move';
        if (event.shiftKey) {
          const ids = new Set(selected.current);
          if (ids.has(id)) ids.delete(id); else ids.add(id);
          choose(ids);
        } else if (!selected.current.has(id)) choose([id]);
        if (found) setStyle(current => adoptElementStyle(current, found));
      } else { initial.mode = 'marquee'; if (!event.shiftKey) choose([]); }
    } else {
      if (offPage(point)) { refusedPointer.current = event.pointerId; releasePointer(event); return; }
      let element: CanvasElement = {
        id: newId(), type: tool, x: point.x, y: point.y, width: 0, height: 0,
        ...newElementStyle(tool, style), seed: newSeed(),
        ...(['freedraw', 'line', 'arrow'].includes(tool) ? { points: [{ x: 0, y: 0 }, { x: 0, y: 0 }] } : {}),
      };
      if (tool === 'arrow') element = bindEnd(element, 'start', point, event);
      initial.id = element.id;
      previewChanges(new Map([[element.id, element]]));
      choose([]);
    }
    gesture.current = initial;
    clearCaptureHold(); armCaptureHold(initial);
    redraw();
  };

  const pointerMove = (event: PointerEvent<HTMLCanvasElement>) => {
    // A pen move with the contact button down whose pointerdown never arrived starts the stroke here. Pencil hover
    // can report pressure, so only `buttons` counts.
    if ((!gesture.current || gesture.current.pointerType === 'touch') && event.pointerType === 'pen' && (event.buttons & 1) !== 0 && !textDraft && refusedPointer.current !== event.pointerId) pointerDown(event, true);
    if (event.pointerType === 'touch') {
      const viewport = touchViewport.current.move(event.pointerId, coordinates(event).screen, zoomRange());
      if (touchViewport.current.active) { event.preventDefault(); if (viewport) updateViewport(viewport, false); return; }
    }
    const active = gesture.current;
    if (!active) {
      if ((tool === 'crop' || tool === 'capture') && regionRef.current) {
        const { point } = coordinates(event), region = regionRef.current;
        const handle = regionHandleAt(region.bounds, point, documentRef.current.viewport.zoom, event.pointerType, tool === 'capture');
        const inside = point.x >= region.bounds.x && point.x <= region.bounds.x + region.bounds.width && point.y >= region.bounds.y && point.y <= region.bounds.y + region.bounds.height;
        setResizeCursor(handle === undefined ? inside ? 'move' : 'default' : ['nwse-resize', 'nesw-resize', 'nwse-resize', 'nesw-resize', 'ns-resize', 'ew-resize', 'ns-resize', 'ew-resize'][handle]);
        return;
      }
      if (tool === 'select') {
        const { point } = coordinates(event), current = documentRef.current;
        const bounds = selectedBounds(current, selected.current);
        const elements = current.elements.filter(element => selected.current.has(element.id));
        const handle = bounds ? elementResizeHandles(elements, bounds).findIndex(p => Math.hypot(p.x - point.x, p.y - point.y) < 10 / current.viewport.zoom) : -1;
        const inside = !event.shiftKey && bounds && !pointEditTarget(elements) && pointInBounds(point, bounds, 5 / current.viewport.zoom);
        setResizeCursor(handle < 0 ? inside ? 'move' : null : ['nwse-resize', 'nesw-resize', 'nwse-resize', 'nesw-resize', 'ns-resize', 'ew-resize', 'ns-resize', 'ew-resize'][handle]);
      }
      // Hovering with the arrow tool shows where a new arrow's start would bind.
      if (tool !== 'arrow') return;
      const { point } = coordinates(event), current = documentRef.current;
      const snap = event.metaKey || event.ctrlKey ? null : findBindingSnap(current.elements, point, current.viewport.zoom);
      if (!sameSnap(snap, bindHint.current)) { bindHint.current = snap; redraw(); }
      return;
    }
    if (active.pointerId !== event.pointerId) return;
    event.preventDefault();
    const { point, screen } = coordinates(event);
    const distance = Math.hypot(screen.x - active.screen.x, screen.y - active.screen.y);
    const selectionGesture = ['marquee', 'move', 'resize', 'point', 'segment', 'crop-confirm'].includes(active.mode) || (active.mode === 'capture' && !!active.bounds);
    const threshold = selectionGesture || active.mode === 'capture-copy' ? active.pointerType === 'mouse' ? 4 : 10 : 2;
    if (distance > threshold && captureHoldTimer.current !== null) clearCaptureHold();
    active.moved ||= distance > threshold;
    if (active.mode === 'crop-confirm' && active.moved) active.mode = 'pan';
    if (selectionGesture && !active.moved) {
      if (active.mode === 'marquee') marquee.current = normalize(active.start, active.start);
      return;
    }
    if (active.mode === 'pan') {
      const viewport = active.original.viewport;
      setView({ ...viewport, x: viewport.x + screen.x - active.screen.x, y: viewport.y + screen.y - active.screen.y });
    } else if (active.mode === 'draw' && active.id) {
      const element = overrides.current.get(active.id)!;
      let width = point.x - active.start.x, height = point.y - active.start.y;
      if (event.shiftKey && element.type !== 'freedraw') {
        if (element.type === 'line' || element.type === 'arrow') {
          const angle = Math.round(Math.atan2(height, width) / (Math.PI / 4)) * Math.PI / 4;
          const length = Math.hypot(width, height); width = Math.cos(angle) * length; height = Math.sin(angle) * length;
        } else { const side = Math.max(Math.abs(width), Math.abs(height)); width = Math.sign(width || 1) * side; height = Math.sign(height || 1) * side; }
      }
      // Safari batches pen samples into one event; the coalesced ones keep short, fast marks and curves.
      const coalesced = element.type === 'freedraw' ? event.nativeEvent?.getCoalescedEvents?.() ?? [] : [];
      const samples = [...coalesced, event].map(sample => {
        const position = coordinates(sample).point;
        return { x: position.x - active.start.x, y: position.y - active.start.y };
      });
      const last = element.points?.[element.points.length - 1];
      const points = element.type === 'freedraw'
        ? [...(element.points ?? []), ...samples.filter((sample, index) => {
          const previous = index ? samples[index - 1] : last;
          return !previous || sample.x !== previous.x || sample.y !== previous.y;
        })]
        : element.points ? [{ x: 0, y: 0 }, { x: width, y: height }] : undefined;
      // x/y are restated because a bound start has moved the previewed arrow off the pointer-down point.
      const next = { ...element, x: active.start.x, y: active.start.y, width, height, points };
      previewChanges(new Map([[element.id, next.type === 'arrow' ? bindEnd(next, 'end', point, event) : next]]));
    } else if (active.mode === 'move') {
      let dx = point.x - active.start.x, dy = point.y - active.start.y;
      if (event.shiftKey) { if (Math.abs(dx) > Math.abs(dy)) dy = 0; else dx = 0; }
      const delta = moveDelta(active.original.elements, dx, dy);
      previewChanges(moveElements(active.original.elements, selected.current, delta.x, delta.y));
    } else if (active.mode === 'point' && active.base && active.index !== undefined) {
      // An arrow end binds where the pointer is; Shift aligns the dragged point's segment to 45°.
      const next = movePoint(active.base, active.index, point, event.shiftKey);
      const end = next.type === 'arrow' ? endAt(next, active.index) : undefined;
      previewChanges(new Map([[next.id, end ? bindEnd(next, end, point, event) : next]]));
    } else if (active.mode === 'segment' && active.base && active.index !== undefined) {
      const delta = { x: point.x - active.start.x, y: point.y - active.start.y };
      previewChanges(new Map([[active.base.id, moveSegment(active.base, active.index, delta, sceneLookup(documentRef.current.elements, overrides.current))]]));
    } else if (active.mode === 'resize' && active.bounds && active.handle !== undefined) {
      const original = active.bounds;
      previewChanges(resizeElements(active.original.elements, selected.current, original, resizeBounds(original, active.handle, point, event.shiftKey)));
    } else if (active.mode === 'capture-copy' && active.bounds && regionRef.current) {
      const offset = 10 / documentRef.current.viewport.zoom;
      marquee.current = moveRegion(active.bounds, { x: point.x - active.start.x + offset, y: point.y - active.start.y - offset });
      captureGhost.current = { snapshot: documentRef.current, from: regionRef.current.bounds, to: marquee.current };
    } else if ((active.mode === 'crop' || active.mode === 'capture') && active.bounds && active.handle !== undefined) {
      marquee.current = active.handle < 0 ? moveRegion(active.bounds, { x: point.x - active.start.x, y: point.y - active.start.y }, active.base) : resizeRegion(active.bounds, active.handle, point, active.base, documentRef.current.viewport.zoom);
    } else if (active.mode === 'capture') {
      marquee.current = regionBounds(active.start, point);
    } else if (active.mode === 'marquee') marquee.current = normalize(active.start, point);
    else if (active.mode === 'erase') {
      eraseAt(active.original.elements, point);
    }
    redraw();
  };

  /** Ends a finger that is part of two-finger navigation; returns whether it was. */
  const endTouch = (event: PointerEvent<HTMLCanvasElement>) => {
    if (event.pointerType !== 'touch' || !touchViewport.current.has(event.pointerId)) return false;
    const navigating = touchViewport.current.active;
    touchViewport.current.up(event.pointerId, documentRef.current.viewport);
    if (!navigating) return false;
    event.preventDefault();
    releasePointer(event);
    if (!touchViewport.current.count) settleView();
    return true;
  };
  const pointerUp = (event: PointerEvent<HTMLCanvasElement>) => {
    traceCanvas('gesture.up', { tool, pointerId: event.pointerId, gesture: gesture.current?.mode ?? 'none', gesturePointerId: gesture.current?.pointerId ?? -1 });
    if (refusedPointer.current === event.pointerId) refusedPointer.current = null;
    if (endTouch(event)) return;
    const active = gesture.current;
    if (!active || active.pointerId !== event.pointerId) return;
    // A quick down and up may have no move between them; the release is the stroke's last sample.
    pointerMove(event);
    clearCaptureHold();
    gesture.current = null; bindHint.current = null;
    releasePointer(event);
    if (active.mode === 'crop-confirm') { void applyCrop(); redraw(); return; }
    if (active.mode === 'capture-copy' && regionRef.current?.kind === 'capture' && marquee.current) {
      const expected = regionRef.current, at = { x: marquee.current.x, y: marquee.current.y };
      captureGhost.current = undefined; marquee.current = expected.bounds;
      if (active.moved) void duplicateCapture(expected, at);
      setResizeCursor(null); redraw(); return;
    }
    if (active.mode === 'capture' || active.mode === 'crop') {
      const bounds = marquee.current;
      if (active.moved && bounds && bounds.width > 0 && bounds.height > 0) {
        const next: CanvasRegion = { kind: active.mode, snapshot: active.mode === 'crop' ? regionRef.current!.snapshot : documentRef.current, source: active.mode === 'crop' ? regionRef.current?.source : undefined, bounds };
        updateRegion(next);
        if (active.mode === 'crop' && active.pointerType !== 'mouse') void applyCrop(next);
      } else marquee.current = regionRef.current?.bounds;
      redraw(); return;
    }
    if (active.mode === 'pan') settleView();
    else if (active.mode === 'write' && !active.moved) {
      const target = writableAt(active.start);
      if (target) startWriting(active.start, target); else if (!offPage(active.start)) startText(active.start);
    } else if (active.mode === 'marquee' && marquee.current) {
      const bounds = marquee.current;
      const ids = active.shift ? new Set(selected.current) : new Set<string>();
      if (!active.moved && active.id) {
        if (ids.has(active.id)) ids.delete(active.id); else ids.add(active.id);
      } else {
        for (const element of active.original.elements) {
          const shown = !isLocked(element) && boxBounds(element);
          if (shown && boundsContain(bounds, shown)) ids.add(element.id);
        }
      }
      choose(ids);
    } else if (overrides.current.size && (active.moved || active.mode === 'erase' || (active.mode === 'draw' && overrides.current.get(active.id!)?.type === 'freedraw'))) {
      // A freedraw tap is a dot, so it commits without moving.
      commitGesture();
      if (active.mode === 'draw' && active.id && tool !== 'freedraw') { choose([active.id]); setTool('select'); }
    }
    if (active.mode === 'erase' && optionsRef.current.eraserAutoReturn) chooseTool(previousTool.current);
    overrides.current.clear(); marquee.current = undefined;
    setRevision(value => value + 1);
    redraw();
  };
  /** Escape or a cancelled pointer: the preview is dropped and a pan returns to where it started. Never switches tools. */
  const cancelGesture = () => {
    touchViewport.current.reset();
    const active = gesture.current;
    if (active?.mode === 'pan') setView(active.original.viewport);
    dropGesture(); marquee.current = regionRef.current?.bounds; redraw();
  };
  const pointerCancel = (event: PointerEvent<HTMLCanvasElement>) => {
    traceCanvas('gesture.cancel', { tool, pointerId: event.pointerId, gesture: gesture.current?.mode ?? 'none' });
    if (refusedPointer.current === event.pointerId) refusedPointer.current = null;
    if (endTouch(event)) return;
    // Only the contact that owns the gesture cancels it; a palm's cancel leaves the pen's stroke alone.
    if (gesture.current?.pointerId === event.pointerId) cancelGesture();
  };
  const deleteSelected = () => {
    if (!selected.current.size) return;
    if (draftRef.current) { draftRef.current = null; setTextDraft(null); }
    commitChanges(new Map([...selected.current].map(id => [id, null])));
    choose([]);
  };
  const commitText = (text = draftValue.current) => {
    const draft = draftRef.current;
    if (!draft) return;
    draftRef.current = null;
    const current = documentRef.current;
    const base = current.elements.find(item => item.id === draft.element.id) ?? draft.element;
    const element = commitTextEdit(base, draft.kind, text);
    if (draft.kind === 'text' || current.elements.includes(base)) commitChanges(new Map([[base.id, element]]));
    setTextDraft(null); setTool('select'); choose(element ? [element.id] : []);
    requestAnimationFrame(() => root.current?.focus({ preventScroll: true }));
  };
  const flushDraft = () => { if (draftRef.current) commitText(draftValue.current); };
  const insertImages = (images: readonly CanvasImageSource[], at?: Point) => {
    if (gesture.current || !images.length) return;
    flushDraft();
    if (regionRef.current) cancelRegion();
    const elements = placeImagesIn(documentRef.current, paintOptions.current.notebook, images, size.current, at);
    commitChanges(new Map(elements.map(element => [element.id, element])));
    choose(elements.map(element => element.id));
    setTool('select');
  };
  const selectedTypes = () => documentRef.current.elements.filter(element => selected.current.has(element.id)).map(element => element.type);
  /** While writing, a style change restyles the draft (and its shape or text, if they exist) and the overlay follows; commit re-measures. */
  const restyleDraft = (draft: TextDraft, patch: StylePatch, preview = false) => {
    const element = patchElementStyle(draft.element, patch);
    if (draft.existing) {
      const changes = new Map([[element.id, element]]);
      if (preview) previewChanges(changes); else commitChanges(changes);
    }
    if (element === draft.element) return;
    const next = { ...draft, element };
    draftRef.current = next; setTextDraft(next); redraw();
  };
  const changeStyle = (patch: StylePatch) => {
    const draft = draftRef.current;
    const types = draft ? [draft.element.type] : selectedTypes();
    setStyle(current => patchDrawingStyle(current, patch, types.length ? types : NON_DRAWING.has(tool) ? [] : [tool as CanvasElement['type']]));
    overrides.current.clear();
    if (draft) restyleDraft(draft, patch);
    else if (!commitChanges(stylePatchChanges(documentRef.current.elements, selected.current, patch))) redraw();
  };
  /** A live look at a continuous control (opacity) without an undo step; `changeStyle` commits it. */
  const previewStyle = (patch: StylePatch) => {
    if (gesture.current) return;
    overrides.current.clear();
    if (draftRef.current) restyleDraft(draftRef.current, patch, true);
    else previewChanges(stylePatchChanges(documentRef.current.elements, selected.current, patch));
    redraw();
  };
  const currentPageIndex = () => {
    const paper = boundPaper();
    return paper ? pageInView(paper, documentRef.current.viewport, size.current).index : 0;
  };
  const goToPage = (index: number) => {
    const paper = boundPaper();
    if (paper) updateViewport(pageView(paper, size.current, index, documentRef.current.viewport.zoom));
  };
  /**
   * A notebook page operation as one undo step, through the same transaction as a stroke so it reaches shared devices,
   * then the view moves to page `focus` (of the result). Selected elements the operation removed leave the selection,
   * and a capture or crop in progress ends, since it holds the old layout.
   */
  const editPages = (change: (document: CanvasDocument) => CanvasDocument, focus: (next: CanvasDocument) => number) => {
    if (gesture.current || touchViewport.current.active || !boundPaper()) return;
    flushDraft();
    const current = documentRef.current, next = change(current);
    if (next === current) return;
    if (regionRef.current) cancelRegion();
    const ids = new Set(next.elements.map(element => element.id));
    if ([...selected.current].some(id => !ids.has(id))) choose([...selected.current].filter(id => ids.has(id)));
    publish(next, true);
    goToPage(focus(next));
  };
  const pageCount = (document: CanvasDocument) => document.paper!.pages.length;
  const addPage = () => editPages(addNotebookPage, next => pageCount(next) - 1);
  const removePage = (index: number) => {
    const page = boundPaper()?.pages[index];
    if (page) editPages(current => removeNotebookPage(current, page.id), next => Math.min(index, pageCount(next) - 1));
  };
  /** A blank page beside page `index`, like it in size and template. */
  const insertPageBeside = (index: number, side: 'before' | 'after') => {
    const page = boundPaper()?.pages[index], at = side === 'before' ? index : index + 1;
    if (page) editPages(current => insertPage(current, at, page), () => at);
  };
  const pageControls = {
    insert: insertPageBeside,
    duplicate: (index: number) => editPages(current => duplicatePage(current, index), () => index + 1),
    move: (from: number, to: number) => editPages(current => movePage(current, from, to), () => to),
    setTemplate: (index: number, template: PaperTemplate) => editPages(current => setPageTemplate(current, index, template), () => index),
    setSize: (index: number, size: PageSize) => editPages(current => setPageSize(current, index, size), () => index),
    /** PDF pages, each on a new page of its own size, after page `index`. */
    insertPdfPages: (index: number, sizes: readonly PageSize[], images: readonly CanvasImportedImage[]) =>
      editPages(current => insertPdfPages(current, index + 1, sizes, images), () => index + 1),
  };
  const fit = () => {
    const paper = boundPaper();
    if (paper) { updateViewport(pageView(paper, size.current, currentPageIndex())); return; }
    const bounds = selectionBounds(documentRef.current.elements);
    if (!bounds) { updateViewport({ x: 0, y: 0, zoom: 1 }); return; }
    const zoom = Math.min(2, Math.max(CANVAS_ZOOM_LIMITS.min, Math.min((size.current.width - 160) / Math.max(bounds.width, 1), (size.current.height - 180) / Math.max(bounds.height, 1))));
    updateViewport({ x: (size.current.width - bounds.width * zoom) / 2 - bounds.x * zoom, y: (size.current.height - bounds.height * zoom) / 2 - bounds.y * zoom, zoom });
  };
  const zoomTo = (zoom: number) => updateViewport(zoomAt(documentRef.current.viewport, { x: size.current.width / 2, y: size.current.height / 2 }, zoom, zoomRange()));
  const zoom = (factor: number) => zoomTo(documentRef.current.viewport.zoom * factor);
  const keyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if ((event.target as HTMLElement).closest('input,textarea,[contenteditable=true]')) return;
    const key = event.key.toLowerCase(), mod = event.metaKey || event.ctrlKey;
    if (key === 'escape') { cancelGesture(); cancelRegion(); choose([]); }
    else if (key === 'enter' && regionRef.current?.kind === 'crop') { event.preventDefault(); event.stopPropagation(); void applyCrop(); return; }
    else if (mod && key === 'c' && regionRef.current?.kind === 'capture') { event.preventDefault(); event.stopPropagation(); void copyRegion(); return; }
    else if (regionRef.current) return;
    else if (key === ' ') { event.preventDefault(); setSpace(true); }
    else if (mod && key === 'z') { event.preventDefault(); history(event.shiftKey); }
    else if (mod && key === 'y') { event.preventDefault(); history(true); }
    else if (mod && key === 'a') { event.preventDefault(); choose(documentRef.current.elements.filter(element => !isLocked(element)).map(element => element.id)); }
    else if (mod && key === 'd') {
      event.preventDefault();
      const source = documentRef.current.elements.filter(element => selected.current.has(element.id));
      if (!source.length) return;
      const elements = placeOnPage(cloneCanvasElements(source));
      commitChanges(new Map(elements.map(element => [element.id, element])));
      choose(elements.map(element => element.id));
    } else if (key === 'delete' || key === 'backspace') { event.preventDefault(); deleteSelected(); }
    else if (key === 'enter' && !mod && selected.current.size === 1) {
      const element = documentRef.current.elements.find(item => selected.current.has(item.id));
      if (element && (element.type === 'text' || LABELLED_TYPES.has(element.type))) { event.preventDefault(); startWriting({ x: element.x, y: element.y }, element); }
    }
    else if (mod && (event.code === 'BracketRight' || event.code === 'BracketLeft') && selected.current.size) {
      // In the app the menu claims ⌘[ and ⌘] (`onBackForward` above); ⌘⇧[ and ⌘⇧] always arrive here.
      event.preventDefault(); const up = event.code === 'BracketRight';
      reorder(event.shiftKey ? (up ? 'front' : 'back') : (up ? 'forward' : 'backward'));
    } else if (key.startsWith('arrow') && selected.current.size) {
      event.preventDefault(); const amount = event.shiftKey ? 10 : 1;
      const dx = key === 'arrowleft' ? -amount : key === 'arrowright' ? amount : 0;
      const dy = key === 'arrowup' ? -amount : key === 'arrowdown' ? amount : 0;
      const delta = moveDelta(documentRef.current.elements, dx, dy);
      commitChanges(moveElements(documentRef.current.elements, selected.current, delta.x, delta.y));
    } else if (!mod && !event.altKey) {
      const tools: Record<string, CanvasTool> = { v: 'select', h: 'hand', r: 'rectangle', o: 'ellipse', d: 'diamond', l: 'line', a: 'arrow', p: 'freedraw', t: 'text', e: 'eraser', s: 'capture' };
      if (tools[key]) { event.preventDefault(); chooseTool(tools[key]); }
      else if (key === 'g') setGrid(value => !value);
      else if (key === '1') fit();
      else if (key === '0') zoomTo(1);
      else if (key === '+' || key === '=') zoom(1.2);
      else if (key === '-') zoom(1 / 1.2);
    }
    if (event.defaultPrevented) event.stopPropagation();
  };
  const copy = (event: ClipboardEvent<HTMLDivElement>, cut = false) => {
    if ((event.target as HTMLElement).closest(EDITABLE) || gesture.current) return;
    if (regionRef.current?.kind === 'capture') {
      event.preventDefault(); event.stopPropagation(); if (!cut) void copyRegion(); return;
    }
    const elements = documentRef.current.elements.filter(element => selected.current.has(element.id));
    if (!elements.length) return;
    const text = serializeCanvasClipboard(elements);
    event.clipboardData.setData('text/plain', text);
    try { event.clipboardData.setData(CANVAS_CLIPBOARD_MIME, text); } catch { /* Plain text remains portable where custom MIME is unavailable. */ }
    event.preventDefault(); event.stopPropagation();
    lastPaste.current = { text, count: 0 };
    if (cut) deleteSelected();
  };
  const paste = (event: ClipboardEvent<HTMLDivElement>) => {
    if ((event.target as HTMLElement).closest(EDITABLE) || gesture.current) return;
    const text = event.clipboardData.getData(CANVAS_CLIPBOARD_MIME) || event.clipboardData.getData('text/plain');
    const source = parseCanvasClipboard(text);
    if (!source?.length) return;
    event.preventDefault(); event.stopPropagation();
    const count = lastPaste.current.text === text ? lastPaste.current.count + 1 : 1;
    lastPaste.current = { text, count };
    const elements = placeOnPage(cloneCanvasElements(source, { x: 24 * count, y: 24 * count }), true);
    commitChanges(new Map(elements.map(element => [element.id, element])));
    choose(elements.map(element => element.id));
    setTool('select');
  };
  const doubleClick = (event: MouseEvent<HTMLCanvasElement>) => {
    if (tool !== 'select') return;
    const { point } = coordinates(event);
    // Double-clicking an arrow's interior point removes it; an elbow segment's handle releases the segment.
    const current = documentRef.current, editing = pointEditTarget(current.elements.filter(item => selected.current.has(item.id)));
    const handle = editing && pointHandleAt(editing, point, current.viewport.zoom);
    if (editing && handle?.kind === 'point' && !endAt(editing, handle.index)) { commitChanges(new Map([[editing.id, removePoint(editing, handle.index)]])); return; }
    if (editing && handle?.kind === 'segment') { commitChanges(new Map([[editing.id, releaseSegment(editing, handle.segment, sceneLookup(current.elements))]])); return; }
    const imageId = hitTest(reachable(selectableElements(current.elements), point), point, 0);
    const image = current.elements.find(item => item.id === imageId && item.type === 'image');
    if (image) { startCrop(image); return; }
    const element = writableAt(point);
    if (element) startWriting(point, element);
  };
  void revision;
  const targets: InspectorTarget[] = textDraft ? [inspectorTarget(textDraft.element, textDraft.kind === 'label')]
    : selection.length ? documentRef.current.elements.filter(element => selected.current.has(element.id)).map(element => inspectorTarget(element))
    : NON_DRAWING.has(tool) ? [] : [{ type: tool as CanvasElement['type'], label: false }];
  const inspector = { sections: inspectorSections(targets), values: inspectorValues(targets, style), colorLabel: colorLabel(targets) };
  const zoomPercent = Math.round(documentRef.current.viewport.zoom * 100), range = zoomRange(), paper = boundPaper();
  const layerMoves = (['front', 'forward', 'backward', 'back'] as const).filter(move => canReorder(documentRef.current.elements, selected.current, move));
  return {
    root, canvas, documentRef, tool, setTool: chooseTool, inspector, changeStyle, previewStyle, grid, setGrid, selection,
    region, startCrop, cancelRegion, completeRegion, copyRegion, duplicateRegion, captureHold, regionCopyState, regionError,
    canCrop: selection.length === 1 && documentRef.current.elements.some(item => selected.current.has(item.id) && item.type === 'image' && !isLocked(item)),
    textDraft, commitText, editText: (text: string) => { draftValue.current = text; },
    pointerDown: (event: PointerEvent<HTMLCanvasElement>) => measureCanvas('handler.pointerdown', () => pointerDown(event)),
    pointerMove: (event: PointerEvent<HTMLCanvasElement>) => measureCanvas('handler.pointermove', () => pointerMove(event), { tool, gesture: gesture.current?.mode ?? 'none' }),
    pointerUp: (event: PointerEvent<HTMLCanvasElement>) => measureCanvas('handler.pointerup', () => pointerUp(event)), pointerCancel, cancelGesture,
    pointerLeave: () => { setResizeCursor(null); if (!gesture.current && bindHint.current) { bindHint.current = null; redraw(); } }, doubleClick, keyDown, copy, cut: (event: ClipboardEvent<HTMLDivElement>) => copy(event, true), paste,
    keyUp: (event: KeyboardEvent<HTMLDivElement>) => { if (event.key === ' ') setSpace(false); },
    blur: () => setSpace(false), cursor: space || tool === 'hand' ? 'grab' : tool === 'select' || tool === 'crop' || (tool === 'capture' && region) ? resizeCursor ?? 'default' : tool === 'text' ? 'text' : 'crosshair',
    history, canUndo: past.current.length > 0, canRedo: future.current.length > 0, historyError, clearHistoryError: () => setHistoryError(null),
    deleteSelected, insertImages, reorder, layerMoves, fit, zoom, resetZoom: () => zoomTo(1), redraw,
    zoomPercent, canZoomOut: zoomPercent > Math.round(range.min * 100), canZoomIn: zoomPercent < Math.round(range.max * 100), notebook,
    /** A notebook's page controls and the page at the centre of the view; null on a canvas. */
    pages: paper ? {
      paper, count: paper.pages.length, index: currentPageIndex(), go: goToPage, add: addPage, remove: removePage, ...pageControls,
    } : null,
  };
}
