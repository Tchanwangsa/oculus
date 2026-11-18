import { measureWhiteboard, traceWhiteboard } from '@/lib/whiteboard/diagnostics';
import { useCallback, useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent, type PointerEvent, type MouseEvent, type ClipboardEvent } from 'react';
import { WHITEBOARD_DEFAULTS, LABELLED_TYPES, type WhiteboardDocument, type WhiteboardElement, type WhiteboardTool, type WhiteboardViewport, type Point } from '@/lib/whiteboard/model';
import { kindTitle } from '@/lib/whiteboard/kindWords';
import { elementBounds, pointInBounds, screenToWorld, selectionBounds, zoomAt, type Bounds } from '@/lib/whiteboard/geometry';
import { hitTest, visibleElements } from '@/lib/whiteboard/engine';
import { drawElement, paintWhiteboard, pointEditTarget, elementResizeHandles, selectedBounds } from '@/lib/whiteboard/render';
import { LiveStroke } from '@/lib/whiteboard/liveStroke';
import { paintsIncrementally } from '@/lib/whiteboard/pens';
import { pointDrag, pointHandleAt } from '@/lib/whiteboard/arrow/handles';
import { endAt, movePoint, removePoint } from '@/lib/whiteboard/arrow/points';
import { EDITABLE, onEdit } from '@/lib/menu/editRouting';
import { onBackForward } from '@/lib/menu/backForwardRouting';
import { WHITEBOARD_CLIPBOARD_MIME, serializeWhiteboardClipboard, parseWhiteboardClipboard, cloneWhiteboardElements } from '@/lib/whiteboard/renderClipboard';
import { applyElementChanges, type ElementChanges, type StackedAdditions } from '@/lib/whiteboard/changes';
import { cutAlong, cutPieces, ERASER_SIZES, startCut, sweepTouches, type EraserMode, type EraserSize, type StrokeCut } from '@/lib/whiteboard/erase';
import { boxBounds, boxSelection } from '@/lib/whiteboard/boxSelect';
import { lassoSelection } from '@/lib/whiteboard/lasso';
import { activePreset, loadInkPresets, patchPreset, saveInkPresets, selectPreset, toolPresets, type InkPresets, type InkTool } from '@/lib/whiteboard/inkPresets';
import { buildTemplate, type WhiteboardTemplate } from '@/lib/whiteboard/templates';
import { elementPage } from '@/lib/whiteboard/renderPaper';
import { canReorder, reorderElements, type LayerMove } from '@/lib/whiteboard/layers';
import { bindArrowEnd, findBindingSnap, followBindings, moveElements, sameSnap, sceneLookup, type BindingSnap } from '@/lib/whiteboard/bindings';
import { moveSegment, releaseSegment } from '@/lib/whiteboard/elbow/segments';
import { commitTextEdit, labelTargetAt, stylePatchChanges } from '@/lib/whiteboard/labels';
import { resizeBounds, resizeElements } from '@/lib/whiteboard/resize';
import { placeImagesIn, type WhiteboardImageSource } from '@/lib/whiteboard/mediaPlacement';
import { TouchViewportGesture } from '@/lib/whiteboard/touchViewport';
import { VelocityTracker, eventTime, glide, gliding } from '@/lib/whiteboard/momentum';
import { loadCamera, loadToolState, saveCamera, saveToolState } from '@/lib/whiteboard/viewState';
import { applyWhiteboardTransaction, diffWhiteboardContent, hasWhiteboardChanges, sameValue, type WhiteboardTransaction } from '@/lib/whiteboard/collaboration';
import { CAPTURE_LIFT_DELAY_MS, type CaptureHoldIndicator, copyRegionImage, rasterizeRegion, regionHandleAt, moveRegion, regionBounds, resizeRegion, type WhiteboardRegion } from '@/lib/whiteboard/region';
import { asDataUrl } from '@/lib/whiteboard/importMedia';
import { secureId } from '@/lib/whiteboard/ids';
import {
  clampCamera, isLocked, keepOnPage, notebookPaper, pageContaining, pageInView, pageView, selectableElements, templateOf, visiblePageCentre, zoomLimits, type WhiteboardKind, type PaperTemplate,
} from '@/lib/whiteboard/paper';
import { WHITEBOARD_ZOOM_LIMITS } from '@/lib/whiteboard/zoom';
import {
  addPage as addNotebookPage, duplicatePage, insertPage, insertPdfPages, movePage, removePage as removeNotebookPage, setAllPageTemplates, setPageSize, setPageTemplate, type PageSize,
} from '@/lib/whiteboard/pages';
import { elementsAtPage } from '@/lib/whiteboard/pageHits';
import type { WhiteboardImportedImage } from '@/lib/whiteboard/importMedia';
import {
  adoptElementStyle, colorLabel, inspectorSections, inspectorTarget, inspectorValues, newElementStyle, patchDrawingStyle,
  patchElementStyle, type DrawingStyle, type InspectorTarget, type StylePatch,
} from '@/lib/whiteboard/style';

/** `text` writes standalone text (`element` is the existing one or a template for a new one); `label` writes into a shape. */
export interface TextDraft { kind: 'text' | 'label'; element: WhiteboardElement; text: string; existing: boolean }
export interface WhiteboardInteractionOptions {
  /** When given, pan and zoom go only here and never through `onChange`, so the host keeps its camera local. */
  onViewportChange?: (viewport: WhiteboardViewport) => void;
  /** Lifting the eraser after a completed erase returns to the tool chosen before it. */
  eraserAutoReturn?: boolean;
  /** Names the drawing for this device's saved camera (`viewState.ts`): it opens where it was left. */
  cameraKey?: string;
  /** `stroke` (the default) erases whole elements; `partial` cuts strokes where the eraser passes. */
  eraserMode?: EraserMode;
  /** The partial eraser's size (`ERASER_SIZES`, screen pixels); medium by default. */
  eraserSize?: EraserSize;
  /**
   * `notebook` binds the document to its pages: they are drawn, the camera stays on them, ink starts only on a page, stays
   * on one and is clipped to it. `whiteboard` (the default) is an infinite board; any `paper` it holds is kept and never used.
   */
  kind?: WhiteboardKind;
  /** Opens the host's image picker, which the I key asks for. */
  onInsertImage?: () => void;
}
type Gesture = {
  /** `write` is a touch tap with the Text tool, which opens the overlay on release so a second finger can still start navigating. */
  mode: 'pan' | 'draw' | 'move' | 'resize' | 'point' | 'segment' | 'marquee' | 'lasso' | 'erase' | 'write' | 'capture' | 'crop' | 'capture-copy' | 'crop-confirm';
  start: Point;
  screen: Point;
  original: WhiteboardDocument;
  moved: boolean;
  pointerId: number;
  pointerType: string;
  id?: string;
  ink?: LiveStroke;
  handle?: number;
  /** A point drag: the line or arrow being edited (with a midpoint's new point in it) and the point that follows the pointer; a segment drag: the elbow arrow and the segment's index. */
  base?: WhiteboardElement;
  index?: number;
  bounds?: Bounds;
  shift?: boolean;
};
/**
 * An erase in progress. Nothing leaves the scene until release: overrides hold the result (null for an erased
 * element, a cut stroke's first piece under its ID, its other pieces under new IDs) and `ghosts` what goes.
 */
type EraseState = {
  partial: boolean;
  /** World units: the partial eraser's radius, or the whole-stroke eraser's hit tolerance. */
  radius: number;
  last: Point;
  cuts: Map<string, StrokeCut>;
  /** A cut stroke's ID → its other pieces' IDs, which commit directly above it. */
  pieces: Map<string, string[]>;
  ghosts: Map<string, WhiteboardElement[]>;
  ghostList: WhiteboardElement[];
};
const erasable = (element: WhiteboardElement) => element.type !== 'image' && !isLocked(element);
/** The whole-stroke eraser's hit tolerance in screen pixels. */
const STROKE_ERASER_TOLERANCE = 10;
/** A partial cut drops pieces shorter than this many screen pixels (or half the stroke's width). */
const SHORTEST_PIECE = 2;
/** One undo step: the transactions each way, plus the layer order to put back elements the step re-adds (a transaction appends them). */
type HistoryEntry = { undo: WhiteboardTransaction; redo: WhiteboardTransaction; undoOrder?: string[]; redoOrder?: string[] };
const normalize = (a: Point, b: Point): Bounds => ({ x: Math.min(a.x, b.x), y: Math.min(a.y, b.y), width: Math.abs(a.x - b.x), height: Math.abs(a.y - b.y) });
const newId = secureId;
const newSeed = () => Math.floor(Math.random() * 100000);
const NON_DRAWING = new Set<WhiteboardTool>(['select', 'lasso', 'hand', 'eraser', 'capture', 'crop']);
/** Tools that stay chosen after a stroke, rather than selecting what they drew. */
const INK_TOOLS = new Set<WhiteboardTool>(['freedraw', 'highlighter']);
/** The element type a drawing tool makes: the highlighter draws freehand strokes with a marker pen. */
const toolType = (tool: WhiteboardTool) => (tool === 'highlighter' ? 'freedraw' : tool) as WhiteboardElement['type'];
const inkTool = (tool: WhiteboardTool): InkTool | null => tool === 'freedraw' || tool === 'highlighter' ? tool : null;
/** Rail keys, each its tool's first letter (`WhiteboardToolbar`); Shift+G is the grid, since G is Grab. */
const TOOL_KEYS: Record<string, WhiteboardTool> = {
  s: 'select', l: 'lasso', g: 'hand', c: 'capture', p: 'freedraw', h: 'highlighter', e: 'eraser', t: 'text',
  r: 'rectangle', o: 'ellipse', d: 'diamond', a: 'arrow', n: 'line',
};
const TEMPLATE_KEYS: Record<string, WhiteboardTemplate> = { m: 'matrix', y: 'axes2', z: 'axes3' };
const sameContent = (a: WhiteboardDocument, b: WhiteboardDocument) => sameValue({ ...a, viewport: null }, { ...b, viewport: null });
const removes = (transaction: WhiteboardTransaction) => transaction.edits.some(edit => edit.after === null);

/** Puts each element `transaction` added back after the element it followed in `order`, rather than on top. */
function restoreLayerOrder(document: WhiteboardDocument, transaction: WhiteboardTransaction, order?: string[]): WhiteboardDocument {
  const added = new Set(transaction.edits.filter(edit => edit.before === null).map(edit => edit.id));
  if (!order || !added.size) return document;
  const byId = new Map(document.elements.map(element => [element.id, element]));
  const rest = document.elements.filter(element => !added.has(element.id)), present = new Set(rest.map(element => element.id));
  const after = new Map<string | null, WhiteboardElement[]>();
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

/** What moved the camera, for the companion's diagnostics. */
type CameraCause = 'restore' | 'pan' | 'two-finger' | 'glide' | 'wheel' | 'page' | 'fit' | 'zoom' | 'first-fit' | 'resize' | 'external' | 'cancel' | 'publish' | 'other';
/** A single camera write of 150 screen pixels or a quarter's zoom is logged with its cause; pans and glides move in small steps. */
function traceJump(from: WhiteboardViewport, to: WhiteboardViewport, cause: CameraCause, clamped: boolean) {
  const dx = to.x - from.x, dy = to.y - from.y, zoom = to.zoom / from.zoom;
  if (Math.hypot(dx, dy) < 150 && zoom < 1.25 && zoom > 0.8) return;
  traceWhiteboard('camera.jump', { cause, clamped, dx: Math.round(dx), dy: Math.round(dy), fromZoom: Math.round(from.zoom * 1000) / 1000, toZoom: Math.round(to.zoom * 1000) / 1000 });
}

export function useWhiteboardInteraction(document: WhiteboardDocument, onChange: (document: WhiteboardDocument) => void, options: WhiteboardInteractionOptions = {}) {
  const root = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const inkLayer = useRef<HTMLDivElement>(null);
  const inkCanvas = useRef<HTMLCanvasElement>(null), tailCanvas = useRef<HTMLCanvasElement>(null);
  const live = useRef<LiveStroke | null>(null);
  const documentRef = useRef(document);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;
  const optionsRef = useRef(options);
  optionsRef.current = options;
  /** Documents this pane handed to `onChange`; one coming back is an echo, never a remote change. */
  const published = useRef(new WeakSet<WhiteboardDocument>());
  const [resumed] = useState(loadToolState);
  const [tool, setTool] = useState<WhiteboardTool>(resumed.tool);
  const previousTool = useRef<WhiteboardTool>('freedraw');
  // Safari cannot say whether a Pencil is paired; its first pen contact turns one finger into navigation.
  const penSeen = useRef(false);
  const [style, setStyle] = useState<DrawingStyle>(resumed.style);
  useEffect(() => { saveToolState({ tool, style }); }, [tool, style]);
  /** The pens and highlighters this device keeps ready; the one in use is what its tool draws with (`inkPresets.ts`). */
  const [presets, setPresets] = useState<InkPresets>(loadInkPresets);
  const changePresets = (change: (current: InkPresets) => InkPresets) => setPresets(current => {
    const next = change(current);
    if (next !== current) saveInkPresets(next);
    return next;
  });
  /** What a tool's next element is drawn with: an ink tool's preset in use over the shared defaults. */
  const toolStyle = (of: WhiteboardTool): DrawingStyle => { const ink = inkTool(of); return ink ? { ...style, ...activePreset(presets, ink) } : style; };
  const [grid, setGrid] = useState(true);
  const [selection, setSelection] = useState<string[]>([]);
  const [revision, setRevision] = useState(0);
  const [space, setSpace] = useState(false);
  const [resizeCursor, setResizeCursor] = useState<string | null>(null);
  const [region, setRegion] = useState<WhiteboardRegion | null>(null);
  const regionRef = useRef<WhiteboardRegion | null>(null);
  const [regionCopyState, setRegionCopyState] = useState<'idle' | 'copying' | 'copied' | 'duplicating'>('idle');
  const [regionError, setRegionError] = useState<string | null>(null);
  const [captureHold, setCaptureHold] = useState<CaptureHoldIndicator | null>(null);
  const captureHoldTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const captureGhost = useRef<{ snapshot: WhiteboardDocument; from: Bounds; to: Bounds } | undefined>(undefined);
  const [textDraft, setTextDraft] = useState<TextDraft | null>(null);
  /** The open draft and its latest text, read synchronously so a style change can commit it first and a later blur is a no-op. */
  const draftRef = useRef<TextDraft | null>(null);
  const draftValue = useRef('');
  const selected = useRef(new Set<string>());
  const gesture = useRef<Gesture | null>(null);
  const touchViewport = useRef(new TouchViewportGesture());
  /** The camera samples of the pan in progress, and the frame of the glide a released pan leaves (0 when none). */
  const panVelocity = useRef(new VelocityTracker());
  const glideFrame = useRef(0);
  /** The pending write of this device's saved camera; a pan or glide writes it once it pauses. */
  const cameraSave = useRef<ReturnType<typeof setTimeout> | null>(null);
  const storeCamera = () => {
    if (cameraSave.current) { clearTimeout(cameraSave.current); cameraSave.current = null; }
    const key = optionsRef.current.cameraKey;
    if (key) saveCamera(key, documentRef.current.viewport);
  };
  useEffect(() => {
    // Safari can discard the page without unmounting it; the camera is written as it goes.
    globalThis.addEventListener?.('pagehide', storeCamera);
    return () => { globalThis.removeEventListener?.('pagehide', storeCamera); if (cameraSave.current) storeCamera(); };
  }, []);
  /** Two-finger navigation applies once a frame (`TouchViewportGesture.view`); the pending frame and its latest event's time. */
  const navFrame = useRef(0), navTime = useRef(0);
  /** Where and when the running glide started, for its diagnostics trace. */
  const glideStats = useRef<{ start: number; from: WhiteboardViewport; source: string } | null>(null);
  const traceGlideEnd = (reason: string) => {
    const stats = glideStats.current, to = documentRef.current.viewport;
    glideStats.current = null;
    if (stats) traceWhiteboard('camera.glide-end', { reason, source: stats.source, durationMs: Math.round(performance.now() - stats.start), dx: Math.round(to.x - stats.from.x), dy: Math.round(to.y - stats.from.y) });
  };
  const overrides = useRef(new Map<string, WhiteboardElement | null>());
  const marquee = useRef<Bounds | undefined>(undefined);
  /** What releasing the selection box now would select, and the bounds painted for the elements it would add. */
  const marqueePreview = useRef<{ ids: Set<string>; boxes: Bounds[] } | null>(null);
  /** The loop the lasso has drawn so far, in world units. */
  const lasso = useRef<Point[] | undefined>(undefined);
  const erasing = useRef<EraseState | null>(null);
  /** The whole-stroke eraser's targets: what a point can reach minus what this erase already took; rebuilt only when either changes. */
  const eraseTargets = useRef<{ owner: EraseState; source: readonly WhiteboardElement[]; taken: number; targets: WhiteboardElement[] } | null>(null);
  /** Where a hovering mouse or pen is, for the partial eraser's ring. */
  const eraserHover = useRef<Point | null>(null);
  const bindHint = useRef<BindingSnap | null>(null);
  const size = useRef({ width: 0, height: 0, pixelRatio: 1 });
  const frame = useRef(0);
  const lastPaste = useRef({ text: '', count: 0 });
  const past = useRef<HistoryEntry[]>([]);
  const future = useRef<HistoryEntry[]>([]);
  const [historyError, setHistoryError] = useState<string | null>(null);
  const viewSave = useRef<ReturnType<typeof setTimeout> | null>(null);
  /** A notebook opens fitted to its page width once, when its camera is still the untouched default. */
  const fitted = useRef(false), restored = useRef(false);
  /** A refused or cancelled contact stays idle until release, even if WebKit sends another move with the pen down. */
  const refusedPointer = useRef<number | null>(null);
  /** The one flag every page behaviour keys off, read through `paintOptions` by the handlers. */
  const notebook = options.kind === 'notebook';
  const paintOptions = useRef({ grid, editingId: textDraft?.element.id, notebook });
  paintOptions.current = { grid, editingId: textDraft?.element.id, notebook };
  /** A notebook's pages, or nothing on a whiteboard. */
  const boundPaper = () => notebookPaper(documentRef.current, paintOptions.current.notebook);
  const activeStroke = () => {
    const active = gesture.current;
    const element = active?.mode === 'draw' && active.id ? overrides.current.get(active.id) : null;
    return element?.type === 'freedraw' ? element : null;
  };
  const beginInk = (element: WhiteboardElement) => {
    const layer = inkLayer.current;
    if (!layer) return;
    if (!live.current) {
      const ink = inkCanvas.current?.getContext('2d'), tail = tailCanvas.current?.getContext('2d');
      if (!ink || !tail) return;
      live.current = new LiveStroke(ink, tail);
    }
    const paint = paintsIncrementally(element) ? undefined : drawElement;
    // Apply opacity once to the joined pieces. A whole-painted pen uses drawElement's own alpha passes.
    layer.style.opacity = String(paint ? 1 : element.opacity ?? WHITEBOARD_DEFAULTS.opacity);
    live.current.begin(element, documentRef.current.viewport, size.current.pixelRatio, paint);
    live.current.draw(element, boundPaper());
    return live.current;
  };
  const refreshInk = () => {
    const element = gesture.current?.ink && activeStroke();
    if (element) beginInk(element);
  };
  const zoomRange = () => {
    const paper = boundPaper();
    return paper && size.current.width > 0 ? zoomLimits(paper, size.current) : WHITEBOARD_ZOOM_LIMITS;
  };
  /** `viewport` kept on a notebook's pages (`clampCamera`); unchanged, the same object, anywhere else. */
  const clampView = (viewport: WhiteboardViewport) => {
    const paper = boundPaper();
    return paper ? clampCamera(paper, size.current, viewport) : viewport;
  };
  /** Whether `point` is on a notebook's desk, where nothing new may start. */
  const offPage = (point: Point) => {
    const paper = boundPaper();
    return !!paper && !pageContaining(paper, point);
  };
  /** What the pointer at `point` can reach: in a notebook only elements whose own page holds it, since the rest is clipped. */
  const reachable = (elements: readonly WhiteboardElement[], point: Point) => {
    const paper = boundPaper();
    return paper ? elementsAtPage(paper, elements, point) : elements;
  };
  /** The selection's centre, which a notebook keeps on a page. */
  const selectionCentre = (elements: readonly WhiteboardElement[]) => {
    const bounds = selectionBounds(elements.filter(element => selected.current.has(element.id)));
    return bounds && { x: bounds.x + bounds.width / 2, y: bounds.y + bounds.height / 2 };
  };
  /** A move of the selection by `dx`, `dy`, shortened in a notebook so the selection's centre stays on a page. */
  const moveDelta = (elements: readonly WhiteboardElement[], dx: number, dy: number) => {
    const paper = boundPaper(), centre = paper && selectionCentre(elements);
    return paper && centre ? keepOnPage(paper, centre, { x: dx, y: dy }) : { x: dx, y: dy };
  };
  /**
   * New elements moved as one so their centre is on a page in a notebook: to the nearest page edge, or with `toView` to
   * the middle of the page in view (for a paste from elsewhere).
   */
  const placeOnPage = (elements: WhiteboardElement[], toView = false): WhiteboardElement[] => {
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
    frame.current = requestAnimationFrame(() => measureWhiteboard('whiteboard.paint', () => {
      frame.current = 0;
      const context = canvas.current?.getContext('2d');
      // Hand the finished stroke to the scene in one frame, including when a new contact precedes this repaint.
      const inkId = gesture.current?.ink ? gesture.current.id : undefined;
      if (!inkId) live.current?.clear();
      if (!context) return;
      const erase = erasing.current, zoom = documentRef.current.viewport.zoom;
      const eraser = erase?.partial ? { center: erase.last, radius: erase.radius }
        : !erase && eraserHover.current && optionsRef.current.eraserMode === 'partial' ? { center: eraserHover.current, radius: ERASER_SIZES[optionsRef.current.eraserSize ?? 'medium'] / zoom } : undefined;
      paintWhiteboard(context, documentRef.current, {
        ...size.current, ...paintOptions.current, inkId, selected: selected.current, ghosts: erase?.ghostList, eraser, pendingSelection: marqueePreview.current?.boxes,
        selectionColor: getComputedStyle(root.current!).getPropertyValue('--color-brand').trim() || '#5e6ad2',
        marquee: marquee.current ?? regionRef.current?.bounds, lasso: lasso.current, captureSelection: regionRef.current?.kind === 'capture' || gesture.current?.mode === 'capture', captureGhost: captureGhost.current, cropSource: regionRef.current?.source ? elementBounds(regionRef.current.source) : undefined, overrides: overrides.current, bindingHint: bindHint.current,
      });
    }, { elements: documentRef.current.elements.length, overrides: overrides.current.size }));
  }, []);

  const choose = useCallback((ids: Iterable<string>) => {
    selected.current = new Set(ids);
    setSelection([...selected.current]);
    redraw();
  }, [redraw]);

  const emit = useCallback((next: WhiteboardDocument) => { published.current.add(next); onChangeRef.current(next); }, []);

  /** `record` makes the change one undo step: the transaction from the current scene to `next`, so undo touches only this pane's edits. */
  const publish = useCallback((next: WhiteboardDocument, record = false) => {
    if (record) {
      const before = documentRef.current, redo = diffWhiteboardContent(before, next);
      if (hasWhiteboardChanges(redo)) {
        const undo = diffWhiteboardContent(next, before);
        const entry: HistoryEntry = { undo, redo };
        if (removes(redo)) entry.undoOrder = before.elements.map(element => element.id);
        if (removes(undo)) entry.redoOrder = next.elements.map(element => element.id);
        past.current = [...past.current.slice(-99), entry];
        future.current = [];
      }
    }
    traceJump(documentRef.current.viewport, next.viewport, 'publish', false);
    documentRef.current = next;
    emit(next);
    setRevision(value => value + 1);
    redraw();
  }, [emit, redraw]);

  /** Bound arrows follow edits to their targets during the preview. */
  const previewChanges = (changes: ElementChanges) => {
    for (const [id, element] of changes) overrides.current.set(id, element);
    for (const [id, element] of followBindings(documentRef.current.elements, overrides.current)) overrides.current.set(id, element);
  };
  /** Every committed element edit goes through here, bound arrows following; returns whether the scene changed. One undo step. */
  const commitChanges = (changes: ElementChanges, above?: StackedAdditions) => {
    const current = documentRef.current;
    const elements = applyElementChanges(current.elements, followBindings(current.elements, changes), above);
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
    if (!redo && activeStroke()) { cancelGesture(); return; }
    if (gesture.current || touchViewport.current.active) return;
    const entry = (redo ? future : past).current.pop();
    if (!entry) return;
    const transaction = redo ? entry.redo : entry.undo;
    let next: WhiteboardDocument;
    try { next = restoreLayerOrder(applyWhiteboardTransaction(documentRef.current, transaction), transaction, redo ? entry.redoOrder : entry.undoOrder); }
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
  const setView = useCallback((requested: WhiteboardViewport, cause: CameraCause = 'other') => {
    const viewport = clampView(requested);
    traceJump(documentRef.current.viewport, viewport, cause, viewport !== requested);
    documentRef.current = { ...documentRef.current, viewport };
    refreshInk();
    optionsRef.current.onViewportChange?.(viewport);
    if (optionsRef.current.cameraKey && !cameraSave.current) cameraSave.current = setTimeout(storeCamera, 400);
  }, []);
  /** Stops a glide where it is; returns whether one was running. */
  const haltGlide = useCallback((cause = 'other') => {
    if (!glideFrame.current) return false;
    cancelAnimationFrame(glideFrame.current); glideFrame.current = 0;
    traceGlideEnd(`halted:${cause}`);
    return true;
  }, []);
  /** Every other camera move ends a glide; this one saves or re-renders the camera itself. */
  const updateViewport = useCallback((viewport: WhiteboardViewport, save = true, cause: CameraCause = 'other') => {
    haltGlide(cause);
    setView(viewport, cause);
    redraw();
    setRevision(value => value + 1);
    if (save && !optionsRef.current.onViewportChange) {
      if (viewSave.current) clearTimeout(viewSave.current);
      viewSave.current = setTimeout(() => { viewSave.current = null; emit(documentRef.current); }, 180);
    }
  }, [emit, haltGlide, redraw, setView]);
  /** A pan or two-finger navigation has ended: save the camera in the document, or only re-render when the host keeps it. */
  const settleView = () => {
    if (optionsRef.current.onViewportChange) setRevision(value => value + 1);
    else publish(documentRef.current);
  };
  const stopGlide = (cause: string) => { if (haltGlide(cause)) settleView(); };
  /**
   * A touch or pen pan released while moving glides on, decelerating (`momentum.ts`), and settles once it stops; an axis
   * the pages stop loses its speed so the glide ends at their edge. A mouse drag, a cancelled contact (often a palm the
   * system rejected) or a release held still settles at once. `time` is the release event's (`eventTime`).
   */
  const endPan = (glides: boolean, time: number, source: string) => {
    const measured = panVelocity.current.release(time), released = glides ? measured : null;
    const trace = panVelocity.current.lastRelease;
    traceWhiteboard('camera.pan', {
      source, glides, verdict: glides ? trace?.verdict ?? 'none' : 'not-allowed', travel: trace?.travel ?? 0, spanMs: trace?.spanMs ?? 0,
      vx: released ? Math.round(released.x * 100) / 100 : 0, vy: released ? Math.round(released.y * 100) / 100 : 0,
      handlingLagMs: Math.round(performance.now() - time), samples: trace?.samples ?? '',
    });
    if (!released) { settleView(); return; }
    let velocity = released, last = performance.now();
    glideStats.current = { start: last, from: documentRef.current.viewport, source };
    const step = (time: number) => {
      const { dx, dy, velocity: next } = glide(velocity, Math.max(0, time - last));
      last = time;
      const before = documentRef.current.viewport;
      setView({ ...before, x: before.x + dx, y: before.y + dy }, 'glide');
      const after = documentRef.current.viewport;
      velocity = { x: Math.abs(after.x - before.x - dx) > 0.01 ? 0 : next.x, y: Math.abs(after.y - before.y - dy) > 0.01 ? 0 : next.y };
      redraw();
      if (gliding(velocity)) glideFrame.current = requestAnimationFrame(step);
      else { glideFrame.current = 0; traceGlideEnd(velocity.x || velocity.y ? 'rest' : 'pinned'); settleView(); }
    };
    glideFrame.current = requestAnimationFrame(step);
  };

  /**
   * A document from outside this pane (another pane, a collaborator, a recovery copy) becomes the new base. The camera,
   * the gesture in progress, undo history, the text being written and the selection of shapes that still exist stay.
   */
  const adoptExternal = (incoming: WhiteboardDocument) => {
    const local = documentRef.current;
    if (sameContent(local, incoming)) return;
    const base = { ...incoming, viewport: local.viewport };
    documentRef.current = base;
    // New paper (a page added or removed elsewhere) can leave the camera off the pages.
    if (clampView(base.viewport) !== base.viewport) setView(base.viewport, 'external');
    else if (base.paper !== local.paper) refreshInk();
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
    // What an erase was taking that is gone elsewhere stops being a ghost; a stroke being cut that changed keeps their version.
    const erase = erasing.current;
    if (erase?.ghosts.size) {
      const byId = new Map(base.elements.map(element => [element.id, element]));
      for (const id of [...erase.ghosts.keys()]) {
        const cut = erase.cuts.get(id);
        if (!byId.has(id) || (cut && !sameValue(byId.get(id), cut.source))) dropCut(erase, id);
      }
    }
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
    if (!element) return;
    // The canvas fills the drawing area beside the editor's bars; its CSS box is that area's size.
    const resize = () => {
      const rect = element.getBoundingClientRect();
      const pixelRatio = window.devicePixelRatio || 1;
      if (Math.round(rect.width) !== Math.round(size.current.width) || Math.round(rect.height) !== Math.round(size.current.height)) {
        traceWhiteboard('whiteboard.resize', { fromWidth: Math.round(size.current.width), fromHeight: Math.round(size.current.height), width: Math.round(rect.width), height: Math.round(rect.height) });
      }
      size.current = { width: rect.width, height: rect.height, pixelRatio };
      const width = Math.round(rect.width * pixelRatio), height = Math.round(rect.height * pixelRatio);
      for (const layer of [element, inkCanvas.current, tailCanvas.current]) {
        if (layer) { layer.width = width; layer.height = height; }
      }
      // The first real size is when this device's saved camera comes back, which also stands in for a notebook's first fit.
      const saved = !restored.current && rect.width > 0 && optionsRef.current.cameraKey ? loadCamera(optionsRef.current.cameraKey) : null;
      if (rect.width > 0) restored.current = true;
      if (saved) { fitted.current = true; setView(saved, 'restore'); setRevision(value => value + 1); }
      const paper = boundPaper(), { viewport } = documentRef.current;
      if (paper && !fitted.current && rect.width > 0) {
        fitted.current = true;
        if (viewport.x === 0 && viewport.y === 0 && viewport.zoom === 1) { setView(pageView(paper, rect), 'first-fit'); setRevision(value => value + 1); }
      }
      // A notebook's saved camera, or one a resize has left off the pages, comes back onto them.
      const settled = documentRef.current.viewport;
      if (rect.width > 0 && clampView(settled) !== settled) { setView(settled, 'resize'); setRevision(value => value + 1); }
      else refreshInk();
      redraw();
    };
    const observer = new ResizeObserver(resize);
    observer.observe(element);
    // Moving between displays can change DPR without changing the canvas's CSS box.
    let resolution: MediaQueryList | undefined;
    const watchResolution = () => {
      resolution = window.matchMedia?.(`(resolution: ${window.devicePixelRatio || 1}dppx)`);
      resolution?.addEventListener('change', ratioChanged, { once: true });
    };
    const ratioChanged = () => { resize(); watchResolution(); };
    watchResolution();
    resize();
    return () => {
      resolution?.removeEventListener('change', ratioChanged);
      observer.disconnect();
      cancelAnimationFrame(frame.current);
      frame.current = 0;
      cancelAnimationFrame(navFrame.current); navFrame.current = 0;
      if (haltGlide('unmount') && !optionsRef.current.onViewportChange) emit(documentRef.current);
      if (viewSave.current) { clearTimeout(viewSave.current); viewSave.current = null; emit(documentRef.current); }
    };
  }, [emit, haltGlide, redraw]);
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
      if (event.ctrlKey || event.metaKey) updateViewport(zoomAt(viewport, screen, viewport.zoom * Math.exp(-event.deltaY * 0.01), zoomRange()), true, 'wheel');
      else updateViewport({ ...viewport, x: viewport.x - event.deltaX, y: viewport.y - event.deltaY }, true, 'wheel');
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
  const startText = (point: Point, element?: WhiteboardElement) => {
    choose([]);
    const template: WhiteboardElement = element ?? { id: newId(), type: 'text', x: point.x, y: point.y, width: 0, height: 0, seed: newSeed(), ...newElementStyle('text', style) };
    openDraft({ kind: 'text', element: template, text: element?.text ?? '', existing: Boolean(element) });
  };
  /** The shape keeps its selection while its label is written, so the inspector shows its text options. */
  const startLabel = (shape: WhiteboardElement) => {
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
  const startWriting = (point: Point, element: WhiteboardElement) => element.type === 'text' ? startText(point, element) : startLabel(element);
  const clearCaptureHold = () => {
    if (captureHoldTimer.current !== null) clearTimeout(captureHoldTimer.current);
    captureHoldTimer.current = null; setCaptureHold(null);
  };
  useEffect(() => () => { if (captureHoldTimer.current !== null) clearTimeout(captureHoldTimer.current); }, []);
  const updateRegion = (next: WhiteboardRegion | null) => {
    clearCaptureHold();
    regionRef.current = next; setRegion(next); setRegionCopyState('idle'); setRegionError(null); marquee.current = next?.bounds; redraw();
  };
  const cancelRegion = () => { cancelGesture(); updateRegion(null); setTool('select'); root.current?.focus({ preventScroll: true }); };
  const startCrop = (element?: WhiteboardElement) => {
    const source = element ?? documentRef.current.elements.find(item => selected.current.size === 1 && selected.current.has(item.id));
    if (!source || source.type !== 'image' || isLocked(source) || gesture.current) return;
    choose([]); setTool('crop'); root.current?.focus({ preventScroll: true });
    updateRegion({ kind: 'crop', source, snapshot: documentRef.current, bounds: elementBounds(source) });
  };
  const completeRegion = (expected: WhiteboardRegion, image: WhiteboardImageSource, at?: Point) => {
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
  const duplicateCapture = async (expected: WhiteboardRegion, at: Point) => {
    setRegionCopyState('duplicating'); setRegionError(null);
    try {
      const result = await rasterizeRegion({ ...expected, snapshot: documentRef.current }, paintOptions.current.notebook);
      completeRegion(expected, { dataUrl: await asDataUrl(result.blob), width: result.width, height: result.height, name: `${kindTitle(options.kind ?? 'whiteboard')} capture.png` }, at);
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
  useEffect(() => { traceWhiteboard('tool.committed', { tool }); }, [tool]);
  const chooseTool = (next: WhiteboardTool) => {
    traceWhiteboard('tool.request', { previous: tool, next, gesture: gesture.current?.mode ?? 'none' });
    if (next !== tool && activeStroke()) cancelGesture();
    if (next === 'crop') { startCrop(); return; }
    if (regionRef.current) { cancelGesture(); updateRegion(null); }
    if (next === 'eraser' && tool !== 'eraser') previousTool.current = tool;
    if (next !== 'eraser' && eraserHover.current) { eraserHover.current = null; redraw(); }
    setTool(next); bindHint.current = null; if (next !== 'select' && next !== 'lasso') choose([]);
  };
  /** Binds an arrow end at the pointer (⌘/Ctrl places it freely) and shows the target it binds to. */
  const bindEnd = (arrow: WhiteboardElement, end: 'start' | 'end', point: Point, event: { metaKey: boolean; ctrlKey: boolean }) => {
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
  const dropGesture = () => {
    traceWhiteboard('gesture.drop', { gesture: gesture.current?.mode ?? 'none' });
    if (activeStroke()) setRevision(value => value + 1);
    live.current?.clear();
    clearCaptureHold(); captureGhost.current = undefined; gesture.current = null; overrides.current.clear(); marquee.current = undefined; lasso.current = undefined; bindHint.current = null;
    marqueePreview.current = null; erasing.current = null; eraseTargets.current = null;
  };
  /** Commits the preview unchanged so lifting the pointer preserves the ink already drawn; a cut stroke's pieces stay where it was. */
  const commitGesture = () => commitChanges(new Map(overrides.current), erasing.current?.pieces);
  const dropCut = (erase: EraseState, id: string) => {
    for (const piece of erase.pieces.get(id) ?? []) overrides.current.delete(piece);
    overrides.current.delete(id);
    erase.cuts.delete(id); erase.pieces.delete(id); erase.ghosts.delete(id);
    erase.ghostList = [...erase.ghosts.values()].flat();
  };
  /** The whole-stroke eraser at `point`: the topmost element it reaches that this erase has not already taken. */
  const eraseWhole = (erase: EraseState, elements: readonly WhiteboardElement[], point: Point) => {
    const source = reachable(elements, point);
    let cached = eraseTargets.current;
    if (cached?.owner !== erase || cached.source !== source || cached.taken !== erase.ghosts.size) {
      cached = eraseTargets.current = { owner: erase, source, taken: erase.ghosts.size, targets: source.filter(element => erasable(element) && !erase.ghosts.has(element.id)) };
    }
    const id = hitTest(cached.targets, point, erase.radius), element = id && cached.targets.find(item => item.id === id);
    if (!element) return false;
    erase.ghosts.set(element.id, [element]);
    previewChanges(new Map([[element.id, null]]));
    return true;
  };
  /** The partial eraser swept from `from` to `to`: strokes are cut where it passes, anything else it touches goes whole. */
  const erasePartial = (erase: EraseState, elements: readonly WhiteboardElement[], from: Point, to: Point) => {
    const r = erase.radius, paper = boundPaper(), zoom = documentRef.current.viewport.zoom;
    const area = { x: Math.min(from.x, to.x) - r, y: Math.min(from.y, to.y) - r, width: Math.abs(to.x - from.x) + r * 2, height: Math.abs(to.y - from.y) + r * 2 };
    const changes = new Map<string, WhiteboardElement | null>();
    for (const element of visibleElements(reachable(elements, to), area)) {
      if (!erasable(element)) continue;
      if (element.type !== 'freedraw') {
        if (!erase.ghosts.has(element.id) && sweepTouches(element, from, to, r)) { erase.ghosts.set(element.id, [element]); changes.set(element.id, null); }
        continue;
      }
      const cut = erase.cuts.get(element.id) ?? startCut(element, r / 4, paper ? elementPage(paper, element) : undefined);
      if (!cutAlong(cut, from, to, r)) continue;
      erase.cuts.set(element.id, cut);
      const { kept, removed } = cutPieces(cut, newId, SHORTEST_PIECE / zoom);
      for (const piece of erase.pieces.get(element.id) ?? []) overrides.current.delete(piece);
      changes.set(element.id, kept[0] ?? null);
      for (const piece of kept.slice(1)) changes.set(piece.id, piece);
      erase.pieces.set(element.id, kept.slice(1).map(piece => piece.id));
      erase.ghosts.set(element.id, removed);
    }
    if (changes.size) previewChanges(changes);
    return changes.size > 0;
  };
  /** Carries the erase through each pointer sample in turn, so a fast sweep between samples is still covered. */
  const eraseThrough = (elements: readonly WhiteboardElement[], points: readonly Point[]) => {
    const erase = erasing.current;
    if (!erase) return;
    let changed = false;
    for (const point of points) {
      changed = (erase.partial ? erasePartial(erase, elements, erase.last, point) : eraseWhole(erase, elements, point)) || changed;
      erase.last = point;
    }
    if (changed) erase.ghostList = [...erase.ghosts.values()].flat();
  };
  const startErase = (elements: readonly WhiteboardElement[], point: Point) => {
    const partial = optionsRef.current.eraserMode === 'partial', zoom = documentRef.current.viewport.zoom;
    const radius = (partial ? ERASER_SIZES[optionsRef.current.eraserSize ?? 'medium'] : STROKE_ERASER_TOLERANCE) / zoom;
    erasing.current = { partial, radius, last: point, cuts: new Map(), pieces: new Map(), ghosts: new Map(), ghostList: [] };
    eraserHover.current = null;
    eraseThrough(elements, [point]);
  };
  /** The selection the box `bounds` leaves on release; the live preview asks the same. */
  const boxResult = (active: Gesture, bounds: Bounds) =>
    boxSelection(active.original.elements, bounds, boundPaper(), selected.current, { add: !!active.shift, toggle: active.moved ? undefined : active.id });
  const previewBox = (active: Gesture, bounds: Bounds) => {
    const ids = boxResult(active, bounds), paper = boundPaper();
    const boxes = active.original.elements.flatMap(element => {
      const shown = ids.has(element.id) && !selected.current.has(element.id) && boxBounds(element, paper);
      return shown ? [shown] : [];
    });
    marqueePreview.current = { ids, boxes };
  };
  const pointerDown = (event: PointerEvent<HTMLCanvasElement>, recoveringContact = false) => {
    traceWhiteboard('gesture.down', { tool, pointerId: event.pointerId, pointerType: event.pointerType, contactWidth: Math.round(event.width || 0), contactHeight: Math.round(event.height || 0), recoveringContact, gesture: gesture.current?.mode ?? 'none', penSeen: penSeen.current, touches: touchViewport.current.count });
    if (!recoveringContact && event.button !== 0 && event.button !== 1) return;
    if (!recoveringContact && refusedPointer.current === event.pointerId) refusedPointer.current = null;
    // A contact during a glide stops it; a finger then holds the page, so lifting it does nothing and dragging scrolls on.
    const caught = glideFrame.current !== 0;
    stopGlide(`${event.pointerType}-down`);
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
      if (gesture.current?.pointerType === 'touch' || touchViewport.current.count) { dropGesture(); dropNavigation(); }
    }
    if (event.pointerType === 'touch' && !textDraft) {
      // Palm contacts are ignored while the pen owns a stroke.
      if (gesture.current?.pointerType === 'pen') { event.preventDefault(); return; }
      capturePointer(event);
      if (touchViewport.current.down(event.pointerId, coordinates(event).screen, documentRef.current.viewport)) {
        // A second finger turns the first finger's gesture into navigation, without committing its preview.
        event.preventDefault();
        const center = touchViewport.current.center() ?? { x: documentRef.current.viewport.x, y: documentRef.current.viewport.y };
        panVelocity.current.start(eventTime(event), { ...documentRef.current.viewport, ...center });
        dropGesture(); redraw(); return;
      }
    } else if (touchViewport.current.active) { event.preventDefault(); return; }
    if (gesture.current || textDraft) { traceWhiteboard('input.ignored', { reason: textDraft ? 'text-draft' : 'active-gesture', gesture: gesture.current?.mode ?? 'none' }); return; }
    event.preventDefault();
    // WebKit can paste at a contenteditable's retained caret despite the canvas having DOM focus.
    globalThis.window?.getSelection()?.removeAllRanges();
    root.current?.focus({ preventScroll: true });
    capturePointer(event);
    const { point, screen } = coordinates(event);
    const current = documentRef.current;
    const initial: Gesture = { mode: 'draw', start: point, screen, original: current, moved: false, pointerId: event.pointerId, pointerType: event.pointerType, shift: event.shiftKey };
    overrides.current.clear();
    panVelocity.current.start(eventTime(event), current.viewport);
    const region = regionRef.current;
    const inRegion = !!region && point.x >= region.bounds.x - 16 / current.viewport.zoom && point.x <= region.bounds.x + region.bounds.width + 16 / current.viewport.zoom && point.y >= region.bounds.y - 16 / current.viewport.zoom && point.y <= region.bounds.y + region.bounds.height + 16 / current.viewport.zoom;
    const outsideCrop = tool === 'crop' && region?.kind === 'crop' && regionHandleAt(region.bounds, point, current.viewport.zoom, event.pointerType) === undefined && (point.x < region.bounds.x || point.y < region.bounds.y || point.x > region.bounds.x + region.bounds.width || point.y > region.bounds.y + region.bounds.height);
    if (tool === 'hand' || space || event.button === 1 || (caught && event.pointerType === 'touch')) initial.mode = 'pan';
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
      startErase(current.elements, point);
      choose([]);
    } else if (tool === 'select' || tool === 'lasso') {
      // The lasso moves and resizes what it has selected as Select does; anywhere else it starts a new loop.
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
      else if (tool === 'lasso' && !(id && selected.current.has(id))) { initial.mode = 'lasso'; lasso.current = [point]; if (!event.shiftKey) choose([]); }
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
        // Ink keeps to its presets: selecting a stroke changes neither a pen nor the look of the next shape.
        if (found && found.type !== 'freedraw') setStyle(current => adoptElementStyle(current, found));
      } else { initial.mode = 'marquee'; if (!event.shiftKey) choose([]); }
    } else {
      if (offPage(point)) { refusedPointer.current = event.pointerId; releasePointer(event); return; }
      const type = toolType(tool);
      let element: WhiteboardElement = {
        id: newId(), type, x: point.x, y: point.y, width: 0, height: 0,
        ...newElementStyle(type, toolStyle(tool)), seed: newSeed(),
        ...(['freedraw', 'line', 'arrow'].includes(type) ? { points: [{ x: 0, y: 0 }, { x: 0, y: 0 }] } : {}),
      };
      if (tool === 'arrow') element = bindEnd(element, 'start', point, event);
      initial.id = element.id;
      previewChanges(new Map([[element.id, element]]));
      if (type === 'freedraw') initial.ink = beginInk(element);
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
      const tracked = touchViewport.current.update(event.pointerId, coordinates(event).screen);
      if (touchViewport.current.active) {
        event.preventDefault();
        if (tracked) { navTime.current = eventTime(event); if (!navFrame.current) navFrame.current = requestAnimationFrame(applyNavigation); }
        return;
      }
    }
    const active = gesture.current;
    if (!active) {
      if (tool === 'eraser') {
        if (event.pointerType === 'touch' || optionsRef.current.eraserMode !== 'partial') return;
        eraserHover.current = coordinates(event).point; redraw();
        return;
      }
      if ((tool === 'crop' || tool === 'capture') && regionRef.current) {
        const { point } = coordinates(event), region = regionRef.current;
        const handle = regionHandleAt(region.bounds, point, documentRef.current.viewport.zoom, event.pointerType, tool === 'capture');
        const inside = point.x >= region.bounds.x && point.x <= region.bounds.x + region.bounds.width && point.y >= region.bounds.y && point.y <= region.bounds.y + region.bounds.height;
        setResizeCursor(handle === undefined ? inside ? 'move' : 'default' : ['nwse-resize', 'nesw-resize', 'nwse-resize', 'nesw-resize', 'ns-resize', 'ew-resize', 'ns-resize', 'ew-resize'][handle]);
        return;
      }
      if (tool === 'select' || tool === 'lasso') {
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
    const rect = canvas.current!.getBoundingClientRect(), viewport = documentRef.current.viewport;
    const screen = { x: event.clientX - rect.left, y: event.clientY - rect.top }, point = screenToWorld(screen, viewport);
    const distance = Math.hypot(screen.x - active.screen.x, screen.y - active.screen.y);
    const selectionGesture = ['marquee', 'lasso', 'move', 'resize', 'point', 'segment', 'crop-confirm'].includes(active.mode) || (active.mode === 'capture' && !!active.bounds);
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
      setView({ ...viewport, x: viewport.x + screen.x - active.screen.x, y: viewport.y + screen.y - active.screen.y }, 'pan');
      panVelocity.current.add(eventTime(event), documentRef.current.viewport);
    } else if (active.mode === 'draw' && active.id) {
      const element = overrides.current.get(active.id)!;
      let width = point.x - active.start.x, height = point.y - active.start.y;
      if (element.type === 'freedraw') {
        // The gesture owns this array until commit. Only the element wrapper changes, for the renderer's path caches.
        const points = element.points!, count = points.length;
        const append = (position: Point) => {
          const x = position.x - active.start.x, y = position.y - active.start.y, last = points[points.length - 1];
          if (x !== last.x || y !== last.y) points.push({ x, y });
        };
        for (const sample of event.nativeEvent?.getCoalescedEvents?.() ?? []) {
          append(screenToWorld({ x: sample.clientX - rect.left, y: sample.clientY - rect.top }, viewport));
        }
        append(point);
        if (points.length === count) return;
        const next = { ...element, width, height };
        overrides.current.set(element.id, next);
        if (active.ink) active.ink.draw(next, boundPaper());
        else redraw();
        return;
      }
      if (event.shiftKey) {
        if (element.type === 'line' || element.type === 'arrow') {
          const angle = Math.round(Math.atan2(height, width) / (Math.PI / 4)) * Math.PI / 4;
          const length = Math.hypot(width, height); width = Math.cos(angle) * length; height = Math.sin(angle) * length;
        } else { const side = Math.max(Math.abs(width), Math.abs(height)); width = Math.sign(width || 1) * side; height = Math.sign(height || 1) * side; }
      }
      const points = element.points ? [{ x: 0, y: 0 }, { x: width, y: height }] : undefined;
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
    } else if (active.mode === 'marquee') {
      marquee.current = normalize(active.start, point);
      previewBox(active, marquee.current);
    } else if (active.mode === 'lasso') {
      // Safari batches pen samples into one event; the coalesced ones keep the loop's curve.
      const coalesced = event.nativeEvent?.getCoalescedEvents?.() ?? [];
      for (const sample of [...coalesced, event]) lasso.current?.push(coordinates(sample).point);
    } else if (active.mode === 'erase') {
      // Safari batches pen samples into one event; each coalesced one is a step of the sweep.
      const coalesced = event.nativeEvent?.getCoalescedEvents?.() ?? [];
      eraseThrough(active.original.elements, [...coalesced, event].map(sample => coordinates(sample).point));
    }
    redraw();
  };

  /** Moves the camera to where both fingers are now, and samples their midpoint for the release speed. */
  const applyNavigation = () => {
    navFrame.current = 0;
    const viewport = touchViewport.current.view(zoomRange()), center = touchViewport.current.center();
    if (!viewport || !center) return;
    updateViewport(viewport, false, 'two-finger');
    panVelocity.current.add(navTime.current, { x: center.x, y: center.y, zoom: documentRef.current.viewport.zoom });
  };
  const flushNavigation = () => { if (navFrame.current) { cancelAnimationFrame(navFrame.current); applyNavigation(); } };
  const dropNavigation = () => { cancelAnimationFrame(navFrame.current); navFrame.current = 0; touchViewport.current.reset(); };
  /** Ends a finger that is part of two-finger navigation; returns whether it was. A cancelled one never glides. */
  const endTouch = (event: PointerEvent<HTMLCanvasElement>, cancelled = false) => {
    if (event.pointerType !== 'touch' || !touchViewport.current.has(event.pointerId)) return false;
    flushNavigation();
    const navigating = touchViewport.current.active;
    touchViewport.current.up(event.pointerId, documentRef.current.viewport);
    if (!navigating) return false;
    event.preventDefault();
    releasePointer(event);
    if (!touchViewport.current.count) endPan(!cancelled, eventTime(event), cancelled ? 'two-finger-cancel' : 'two-finger');
    return true;
  };
  const pointerUp = (event: PointerEvent<HTMLCanvasElement>) => {
    traceWhiteboard('gesture.up', { tool, pointerId: event.pointerId, gesture: gesture.current?.mode ?? 'none', gesturePointerId: gesture.current?.pointerId ?? -1 });
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
        const next: WhiteboardRegion = { kind: active.mode, snapshot: active.mode === 'crop' ? regionRef.current!.snapshot : documentRef.current, source: active.mode === 'crop' ? regionRef.current?.source : undefined, bounds };
        updateRegion(next);
        if (active.mode === 'crop' && active.pointerType !== 'mouse') void applyCrop(next);
      } else marquee.current = regionRef.current?.bounds;
      redraw(); return;
    }
    if (active.mode === 'pan') endPan(active.pointerType !== 'mouse', eventTime(event), `${active.pointerType}-pan`);
    else if (active.mode === 'write' && !active.moved) {
      const target = writableAt(active.start);
      if (target) startWriting(active.start, target); else if (!offPage(active.start)) startText(active.start);
    } else if (active.mode === 'marquee' && marquee.current) choose(boxResult(active, marquee.current));
    else if (active.mode === 'lasso') {
      const ids = active.shift ? new Set(selected.current) : new Set<string>();
      if (active.moved && lasso.current) for (const id of lassoSelection(active.original.elements, lasso.current)) ids.add(id);
      choose(ids);
    }
    else if (overrides.current.size && (active.moved || active.mode === 'erase' || (active.mode === 'draw' && overrides.current.get(active.id!)?.type === 'freedraw'))) {
      // A freedraw tap is a dot, so it commits without moving.
      commitGesture();
      if (active.mode === 'draw' && active.id && !INK_TOOLS.has(tool)) { choose([active.id]); setTool('select'); }
    }
    if (active.mode === 'erase' && optionsRef.current.eraserAutoReturn) chooseTool(previousTool.current);
    overrides.current.clear(); marquee.current = undefined; lasso.current = undefined; marqueePreview.current = null; erasing.current = null; eraseTargets.current = null;
    setRevision(value => value + 1);
    redraw();
  };
  /** Escape or a cancelled pointer: the preview is dropped and a pan returns to where it started. Never switches tools. */
  const cancelGesture = () => {
    stopGlide('cancel');
    dropNavigation();
    const active = gesture.current;
    if (active?.mode === 'pan') setView(active.original.viewport, 'cancel');
    if (active && activeStroke()) refusedPointer.current = active.pointerId;
    dropGesture(); marquee.current = regionRef.current?.bounds; redraw();
  };
  const pointerCancel = (event: PointerEvent<HTMLCanvasElement>) => {
    traceWhiteboard('gesture.cancel', { tool, pointerId: event.pointerId, gesture: gesture.current?.mode ?? 'none' });
    // Only the contact that owns the gesture cancels it; a palm's cancel leaves the pen's stroke alone.
    if (!endTouch(event, true) && gesture.current?.pointerId === event.pointerId) cancelGesture();
    if (refusedPointer.current === event.pointerId) refusedPointer.current = null;
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
  const insertImages = (images: readonly WhiteboardImageSource[], at?: Point) => {
    if (gesture.current || !images.length) return;
    flushDraft();
    if (regionRef.current) cancelRegion();
    const elements = placeImagesIn(documentRef.current, paintOptions.current.notebook, images, size.current, at);
    commitChanges(new Map(elements.map(element => [element.id, element])));
    choose(elements.map(element => element.id));
    setTool('select');
  };
  /** Drops a ready-made figure (`templates.ts`) at the centre of the view, on a page in a notebook, and selects it. */
  const insertTemplate = (kind: WhiteboardTemplate) => {
    if (gesture.current || touchViewport.current.active) return;
    flushDraft();
    if (regionRef.current) cancelRegion();
    const centre = screenToWorld({ x: size.current.width / 2, y: size.current.height / 2 }, documentRef.current.viewport);
    const elements = placeOnPage(buildTemplate(kind, centre, style), true);
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
    const ink = draft || types.length ? null : inkTool(tool);
    // With an ink tool and nothing selected the change is to its preset in use; otherwise to the shared defaults.
    if (ink) changePresets(current => patchPreset(current, ink, patch));
    else setStyle(current => patchDrawingStyle(current, patch, types.length ? types : NON_DRAWING.has(tool) ? [] : [toolType(tool)]));
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
    if (paper) updateViewport(pageView(paper, size.current, index, documentRef.current.viewport.zoom), true, 'page');
  };
  /**
   * A notebook page operation as one undo step, through the same transaction as a stroke so it reaches shared devices,
   * then the view moves to page `focus` (of the result). Selected elements the operation removed leave the selection,
   * and a capture or crop in progress ends, since it holds the old layout.
   */
  const editPages = (change: (document: WhiteboardDocument) => WhiteboardDocument, focus: (next: WhiteboardDocument) => number) => {
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
  const pageCount = (document: WhiteboardDocument) => document.paper!.pages.length;
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
    applyTemplateToAll: (index: number) => editPages(current => {
      const page = boundPaper()?.pages[index];
      return page ? setAllPageTemplates(current, templateOf(page)) : current;
    }, () => index),
    setSize: (index: number, size: PageSize) => editPages(current => setPageSize(current, index, size), () => index),
    /** PDF pages, each on a new page of its own size, after page `index`. */
    insertPdfPages: (index: number, sizes: readonly PageSize[], images: readonly WhiteboardImportedImage[]) =>
      editPages(current => insertPdfPages(current, index + 1, sizes, images), () => index + 1),
  };
  const fit = () => {
    const paper = boundPaper();
    if (paper) { updateViewport(pageView(paper, size.current, currentPageIndex()), true, 'fit'); return; }
    const bounds = selectionBounds(documentRef.current.elements);
    if (!bounds) { updateViewport({ x: 0, y: 0, zoom: 1 }, true, 'fit'); return; }
    const zoom = Math.min(2, Math.max(WHITEBOARD_ZOOM_LIMITS.min, Math.min((size.current.width - 160) / Math.max(bounds.width, 1), (size.current.height - 180) / Math.max(bounds.height, 1))));
    updateViewport({ x: (size.current.width - bounds.width * zoom) / 2 - bounds.x * zoom, y: (size.current.height - bounds.height * zoom) / 2 - bounds.y * zoom, zoom }, true, 'fit');
  };
  const zoomTo = (zoom: number) => updateViewport(zoomAt(documentRef.current.viewport, { x: size.current.width / 2, y: size.current.height / 2 }, zoom, zoomRange()), true, 'zoom');
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
      const elements = placeOnPage(cloneWhiteboardElements(source));
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
      if (key === 'g' && event.shiftKey) { event.preventDefault(); setGrid(value => !value); }
      else if (TOOL_KEYS[key]) { event.preventDefault(); chooseTool(TOOL_KEYS[key]); }
      else if (TEMPLATE_KEYS[key]) { event.preventDefault(); insertTemplate(TEMPLATE_KEYS[key]); }
      else if (key === 'i' && optionsRef.current.onInsertImage) { event.preventDefault(); optionsRef.current.onInsertImage(); }
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
    const text = serializeWhiteboardClipboard(elements);
    event.clipboardData.setData('text/plain', text);
    try { event.clipboardData.setData(WHITEBOARD_CLIPBOARD_MIME, text); } catch { /* Plain text remains portable where custom MIME is unavailable. */ }
    event.preventDefault(); event.stopPropagation();
    lastPaste.current = { text, count: 0 };
    if (cut) deleteSelected();
  };
  const paste = (event: ClipboardEvent<HTMLDivElement>) => {
    if ((event.target as HTMLElement).closest(EDITABLE) || gesture.current) return;
    const text = event.clipboardData.getData(WHITEBOARD_CLIPBOARD_MIME) || event.clipboardData.getData('text/plain');
    const source = parseWhiteboardClipboard(text);
    if (!source?.length) return;
    event.preventDefault(); event.stopPropagation();
    const count = lastPaste.current.text === text ? lastPaste.current.count + 1 : 1;
    lastPaste.current = { text, count };
    const elements = placeOnPage(cloneWhiteboardElements(source, { x: 24 * count, y: 24 * count }), true);
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
    : NON_DRAWING.has(tool) ? [] : [{ type: toolType(tool), label: false }];
  const inspector = { sections: inspectorSections(targets), values: inspectorValues(targets, !textDraft && !selection.length ? toolStyle(tool) : style), colorLabel: colorLabel(targets) };
  const ink = inkTool(tool);
  const zoomPercent = Math.round(documentRef.current.viewport.zoom * 100), range = zoomRange(), paper = boundPaper();
  const layerMoves = (['front', 'forward', 'backward', 'back'] as const).filter(move => canReorder(documentRef.current.elements, selected.current, move));
  return {
    root, canvas, inkLayer, inkCanvas, tailCanvas, documentRef, tool, setTool: chooseTool, inspector, changeStyle, previewStyle, grid, setGrid, selection,
    region, startCrop, cancelRegion, completeRegion, copyRegion, duplicateRegion, captureHold, regionCopyState, regionError,
    canCrop: selection.length === 1 && documentRef.current.elements.some(item => selected.current.has(item.id) && item.type === 'image' && !isLocked(item)),
    textDraft, commitText, editText: (text: string) => { draftValue.current = text; },
    pointerDown: (event: PointerEvent<HTMLCanvasElement>) => measureWhiteboard('handler.pointerdown', () => pointerDown(event)),
    pointerMove: (event: PointerEvent<HTMLCanvasElement>) => measureWhiteboard('handler.pointermove', () => pointerMove(event), { tool, gesture: gesture.current?.mode ?? 'none' }),
    pointerUp: (event: PointerEvent<HTMLCanvasElement>) => measureWhiteboard('handler.pointerup', () => pointerUp(event)), pointerCancel, cancelGesture,
    pointerLeave: () => {
      setResizeCursor(null);
      if (!gesture.current && (bindHint.current || eraserHover.current)) { bindHint.current = null; eraserHover.current = null; redraw(); }
    }, doubleClick, keyDown, copy, cut: (event: ClipboardEvent<HTMLDivElement>) => copy(event, true), paste,
    keyUp: (event: KeyboardEvent<HTMLDivElement>) => { if (event.key === ' ') setSpace(false); },
    blur: () => setSpace(false), cursor: space || tool === 'hand' ? 'grab' : tool === 'select' || tool === 'crop' || (tool === 'capture' && region) ? resizeCursor ?? 'default' : tool === 'lasso' ? resizeCursor ?? 'crosshair' : tool === 'text' ? 'text' : 'crosshair',
    /** What releasing the selection box being dragged would select, as its preview paints it; empty without one. */
    previewSelection: () => [...marqueePreview.current?.ids ?? []],
    history, canUndo: past.current.length > 0 || !!activeStroke(), canRedo: future.current.length > 0, historyError, clearHistoryError: () => setHistoryError(null),
    deleteSelected, insertImages, insertTemplate, reorder, layerMoves, fit, zoom, resetZoom: () => zoomTo(1), redraw,
    /** The pen's or highlighter's presets while it is chosen with nothing selected or being written; otherwise null. */
    inkPresets: ink && !selection.length && !textDraft ? { ...toolPresets(presets, ink), select: (at: number) => changePresets(current => selectPreset(current, ink, at)) } : null,
    zoomPercent, canZoomOut: zoomPercent > Math.round(range.min * 100), canZoomIn: zoomPercent < Math.round(range.max * 100), notebook,
    /** A notebook's page controls and the page at the centre of the view; null on a whiteboard. */
    pages: paper ? {
      paper, count: paper.pages.length, index: currentPageIndex(), go: goToPage, add: addPage, remove: removePage, ...pageControls,
    } : null,
  };
}
