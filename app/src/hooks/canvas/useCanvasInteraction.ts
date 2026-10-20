import { useCallback, useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent, type PointerEvent, type MouseEvent, type ClipboardEvent } from 'react';
import { LABELLED_TYPES, type CanvasDocument, type CanvasElement, type CanvasTool, type CanvasViewport, type Point } from '@/lib/canvas/model';
import { boundsContain, elementBounds, screenToWorld, selectionBounds, zoomAt, type Bounds } from '@/lib/canvas/geometry';
import { hitTest, simplifyPoints } from '@/lib/canvas/engine';
import { paintCanvas, pointEditTarget, resizeHandles, elementResizeHandles, selectedBounds } from '@/lib/canvas/render';
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
import { preserveImageAspectRatio, resizeElements } from '@/lib/canvas/resize';
import { placeCanvasImages, type CanvasImageSource } from '@/lib/canvas/mediaPlacement';
import { TouchViewportGesture } from '@/lib/canvas/touchViewport';
import { applyCanvasTransaction, diffCanvasContent, hasCanvasChanges, sameValue, type CanvasTransaction } from '@/lib/canvas/collaboration';
import { secureId } from '@/lib/canvas/ids';
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
}
type Gesture = {
  /** `write` is a touch tap with the Text tool, which opens the overlay on release so a second finger can still start navigating. */
  mode: 'pan' | 'draw' | 'move' | 'resize' | 'point' | 'segment' | 'marquee' | 'erase' | 'write';
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
const NON_DRAWING = new Set<CanvasTool>(['select', 'hand', 'eraser']);
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
  const paintOptions = useRef({ grid, editingId: textDraft?.element.id });
  paintOptions.current = { grid, editingId: textDraft?.element.id };

  const redraw = useCallback(() => {
    if (frame.current) return;
    frame.current = requestAnimationFrame(() => {
      frame.current = 0;
      const context = canvas.current?.getContext('2d');
      if (!context) return;
      paintCanvas(context, documentRef.current, {
        ...size.current, ...paintOptions.current, selected: selected.current,
        selectionColor: getComputedStyle(root.current!).getPropertyValue('--color-brand').trim() || '#5e6ad2',
        marquee: marquee.current, overrides: overrides.current, bindingHint: bindHint.current,
      });
    });
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

  /** Moves the camera without a render, for every pointer sample of a pan. */
  const setView = useCallback((viewport: CanvasViewport) => {
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
      if (event.ctrlKey || event.metaKey) updateViewport(zoomAt(viewport, screen, Math.max(0.1, Math.min(8, viewport.zoom * Math.exp(-event.deltaY * 0.01)))));
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
    const id = hitTest(current.elements, point, tolerance);
    const hit = current.elements.find(element => element.id === id);
    if (hit && (hit.type === 'text' || LABELLED_TYPES.has(hit.type))) return hit;
    return labelTargetAt(current.elements, point, tolerance);
  };
  const startWriting = (point: Point, element: CanvasElement) => element.type === 'text' ? startText(point, element) : startLabel(element);
  const chooseTool = (next: CanvasTool) => {
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
  const dropGesture = () => { gesture.current = null; overrides.current.clear(); marquee.current = undefined; bindHint.current = null; };
  /** Commits the gesture's previews: a freedraw stroke is simplified first. Returns whether the scene changed. */
  const commitGesture = (active: Gesture) => {
    const changes = new Map(overrides.current);
    if (active.mode === 'draw' && active.id) {
      const element = changes.get(active.id);
      if (element?.type === 'freedraw' && element.points) changes.set(element.id, { ...element, points: simplifyPoints(element.points, 0.5 / documentRef.current.viewport.zoom) });
    }
    return commitChanges(changes);
  };
  const eraseAt = (elements: CanvasElement[], point: Point) => {
    const erasable = elements.filter(element => element.type !== 'image' && overrides.current.get(element.id) !== null);
    const id = hitTest(erasable, point, 10 / documentRef.current.viewport.zoom);
    if (id) previewChanges(new Map([[id, null]]));
  };
  const pointerDown = (event: PointerEvent<HTMLCanvasElement>, recoveringContact = false) => {
    if (!recoveringContact && event.button !== 0 && event.button !== 1) return;
    if (event.pointerType === 'pen') {
      penSeen.current = true;
      // A fresh pen down proves the last contact ended even when WebKit dropped its up: keep that stroke's ink as its own stroke.
      const previous = gesture.current;
      if (previous?.pointerType === 'pen') {
        const element = previous.mode === 'draw' && previous.id ? overrides.current.get(previous.id) : undefined;
        if (element?.type === 'freedraw' && element.points) { gesture.current = null; commitGesture(previous); }
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
    if (gesture.current || textDraft) return;
    event.preventDefault();
    root.current?.focus({ preventScroll: true });
    capturePointer(event);
    const { point, screen } = coordinates(event);
    const current = documentRef.current;
    const initial: Gesture = { mode: 'draw', start: point, screen, original: current, moved: false, pointerId: event.pointerId, pointerType: event.pointerType, shift: event.shiftKey };
    overrides.current.clear();
    if (tool === 'hand' || space || event.button === 1 || (event.pointerType === 'touch' && penSeen.current)) initial.mode = 'pan';
    else if (tool === 'text') {
      if (event.pointerType === 'touch') initial.mode = 'write';
      else {
        const target = writableAt(point);
        if (target) startWriting(point, target); else startText(point);
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
      const id = hitTest(current.elements, point, 6 / current.viewport.zoom);
      if (editing && pointHandle) {
        initial.id = editing.id;
        if (pointHandle.kind === 'segment') Object.assign(initial, { mode: 'segment', base: editing, index: pointHandle.segment });
        else { initial.mode = 'point'; Object.assign(initial, pointDrag(editing, pointHandle)); }
      } else if (handle >= 0 && bounds) { initial.mode = 'resize'; initial.bounds = bounds; initial.handle = handle; }
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
    redraw();
  };

  const pointerMove = (event: PointerEvent<HTMLCanvasElement>) => {
    // A pen move with the contact button down whose pointerdown never arrived starts the stroke here. Pencil hover
    // can report pressure, so only `buttons` counts.
    if ((!gesture.current || gesture.current.pointerType === 'touch') && event.pointerType === 'pen' && (event.buttons & 1) !== 0 && !textDraft) pointerDown(event, true);
    if (event.pointerType === 'touch') {
      const viewport = touchViewport.current.move(event.pointerId, coordinates(event).screen);
      if (touchViewport.current.active) { event.preventDefault(); if (viewport) updateViewport(viewport, false); return; }
    }
    const active = gesture.current;
    if (!active) {
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
    active.moved ||= Math.hypot(screen.x - active.screen.x, screen.y - active.screen.y) > 2;
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
      previewChanges(moveElements(active.original.elements, selected.current, dx, dy));
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
      const opposite = resizeHandles(original)[(active.handle + 2) % 4];
      previewChanges(resizeElements(active.original.elements, selected.current, original, preserveImageAspectRatio(active.original.elements, selected.current, original, point, opposite, event.shiftKey)));
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
    if (endTouch(event)) return;
    const active = gesture.current;
    if (!active || active.pointerId !== event.pointerId) return;
    // A quick down and up may have no move between them; the release is the stroke's last sample.
    pointerMove(event);
    gesture.current = null; bindHint.current = null;
    releasePointer(event);
    if (active.mode === 'pan') settleView();
    else if (active.mode === 'write' && !active.moved) {
      const target = writableAt(active.start);
      if (target) startWriting(active.start, target); else startText(active.start);
    } else if (active.mode === 'marquee' && marquee.current) {
      const bounds = marquee.current;
      const ids = active.shift ? new Set(selected.current) : new Set<string>();
      if (!active.moved && active.id) {
        if (ids.has(active.id)) ids.delete(active.id); else ids.add(active.id);
      } else {
        for (const element of active.original.elements) if (boundsContain(bounds, elementBounds(element))) ids.add(element.id);
      }
      choose(ids);
    } else if (overrides.current.size && (active.moved || active.mode === 'erase' || (active.mode === 'draw' && overrides.current.get(active.id!)?.type === 'freedraw'))) {
      // A freedraw tap is a dot, so it commits without moving.
      commitGesture(active);
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
    dropGesture(); redraw();
  };
  const pointerCancel = (event: PointerEvent<HTMLCanvasElement>) => {
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
  const insertImages = (images: readonly CanvasImageSource[]) => {
    if (gesture.current || !images.length) return;
    flushDraft();
    const elements = placeCanvasImages(images, documentRef.current.viewport, size.current);
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
  const fit = () => {
    const bounds = selectionBounds(documentRef.current.elements);
    if (!bounds) { updateViewport({ x: 0, y: 0, zoom: 1 }); return; }
    const zoom = Math.min(2, Math.max(0.1, Math.min((size.current.width - 160) / Math.max(bounds.width, 1), (size.current.height - 180) / Math.max(bounds.height, 1))));
    updateViewport({ x: (size.current.width - bounds.width * zoom) / 2 - bounds.x * zoom, y: (size.current.height - bounds.height * zoom) / 2 - bounds.y * zoom, zoom });
  };
  const zoom = (factor: number) => updateViewport(zoomAt(documentRef.current.viewport, { x: size.current.width / 2, y: size.current.height / 2 }, Math.max(0.1, Math.min(8, documentRef.current.viewport.zoom * factor))));
  const keyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if ((event.target as HTMLElement).closest('input,textarea,[contenteditable=true]')) return;
    const key = event.key.toLowerCase(), mod = event.metaKey || event.ctrlKey;
    if (key === 'escape') { cancelGesture(); choose([]); setTool('select'); }
    else if (key === ' ') { event.preventDefault(); setSpace(true); }
    else if (mod && key === 'z') { event.preventDefault(); history(event.shiftKey); }
    else if (mod && key === 'y') { event.preventDefault(); history(true); }
    else if (mod && key === 'a') { event.preventDefault(); choose(documentRef.current.elements.map(element => element.id)); }
    else if (mod && key === 'd') {
      event.preventDefault();
      const source = documentRef.current.elements.filter(element => selected.current.has(element.id));
      if (!source.length) return;
      const elements = cloneCanvasElements(source);
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
      commitChanges(moveElements(documentRef.current.elements, selected.current, dx, dy));
    } else if (!mod && !event.altKey) {
      const tools: Record<string, CanvasTool> = { v: 'select', h: 'hand', r: 'rectangle', o: 'ellipse', d: 'diamond', l: 'line', a: 'arrow', p: 'freedraw', t: 'text', e: 'eraser' };
      if (tools[key]) { event.preventDefault(); chooseTool(tools[key]); }
      else if (key === 'g') setGrid(value => !value);
      else if (key === '1') fit();
      else if (key === '0') updateViewport(zoomAt(documentRef.current.viewport, { x: size.current.width / 2, y: size.current.height / 2 }, 1));
      else if (key === '+' || key === '=') zoom(1.2);
      else if (key === '-') zoom(1 / 1.2);
    }
    if (event.defaultPrevented) event.stopPropagation();
  };
  const copy = (event: ClipboardEvent<HTMLDivElement>, cut = false) => {
    if ((event.target as HTMLElement).closest(EDITABLE) || gesture.current) return;
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
    const elements = cloneCanvasElements(source, { x: 24 * count, y: 24 * count });
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
    const element = writableAt(point);
    if (element) startWriting(point, element);
  };
  void revision;
  const targets: InspectorTarget[] = textDraft ? [inspectorTarget(textDraft.element, textDraft.kind === 'label')]
    : selection.length ? documentRef.current.elements.filter(element => selected.current.has(element.id)).map(element => inspectorTarget(element))
    : NON_DRAWING.has(tool) ? [] : [{ type: tool as CanvasElement['type'], label: false }];
  const inspector = { sections: inspectorSections(targets), values: inspectorValues(targets, style), colorLabel: colorLabel(targets) };
  const layerMoves = (['front', 'forward', 'backward', 'back'] as const).filter(move => canReorder(documentRef.current.elements, selected.current, move));
  return {
    root, canvas, documentRef, tool, setTool: chooseTool, inspector, changeStyle, previewStyle, grid, setGrid, selection,
    textDraft, commitText, editText: (text: string) => { draftValue.current = text; },
    pointerDown: (event: PointerEvent<HTMLCanvasElement>) => pointerDown(event), pointerMove, pointerUp, pointerCancel, cancelGesture,
    pointerLeave: () => { if (!gesture.current && bindHint.current) { bindHint.current = null; redraw(); } }, doubleClick, keyDown, copy, cut: (event: ClipboardEvent<HTMLDivElement>) => copy(event, true), paste,
    keyUp: (event: KeyboardEvent<HTMLDivElement>) => { if (event.key === ' ') setSpace(false); },
    blur: () => setSpace(false), cursor: space || tool === 'hand' ? 'grab' : tool === 'select' ? 'default' : tool === 'text' ? 'text' : 'crosshair',
    history, canUndo: past.current.length > 0, canRedo: future.current.length > 0, historyError, clearHistoryError: () => setHistoryError(null),
    deleteSelected, insertImages, reorder, layerMoves, fit, zoom, resetZoom: () => updateViewport(zoomAt(documentRef.current.viewport, { x: size.current.width / 2, y: size.current.height / 2 }, 1)), redraw,
    zoomPercent: Math.round(documentRef.current.viewport.zoom * 100),
  };
}
