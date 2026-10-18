import { useCallback, useEffect, useRef, useState, type KeyboardEvent, type PointerEvent, type MouseEvent, type ClipboardEvent } from 'react';
import { LABELLED_TYPES, type CanvasDocument, type CanvasElement, type CanvasTool, type Point } from '@/lib/canvas/model';
import { boundsIntersect, elementBounds, screenToWorld, selectionBounds, zoomAt, type Bounds } from '@/lib/canvas/geometry';
import { hitTest, simplifyPoints } from '@/lib/canvas/engine';
import { paintCanvas, resizeHandles, elementResizeHandles, selectedBounds } from '@/lib/canvas/render';
import { EDITABLE, onEdit } from '@/lib/menu/editRouting';
import { onBackForward } from '@/lib/menu/backForwardRouting';
import { CANVAS_CLIPBOARD_MIME, serializeCanvasClipboard, parseCanvasClipboard, cloneCanvasElements } from '@/lib/canvas/renderClipboard';
import { applyElementChanges, type ElementChanges } from '@/lib/canvas/changes';
import { canReorder, reorderElements, type LayerMove } from '@/lib/canvas/layers';
import { bindArrowEnd, findBindingSnap, followBindings, moveElements, sameSnap, type BindingSnap } from '@/lib/canvas/bindings';
import { commitTextEdit, growToFitLabel, labelTargetAt, stylePatchChanges } from '@/lib/canvas/labels';
import {
  adoptElementStyle, colorLabel, DEFAULT_DRAWING_STYLE, inspectorSections, inspectorTarget, inspectorValues, newElementStyle, patchDrawingStyle,
  patchElementStyle, type DrawingStyle, type InspectorTarget, type StylePatch,
} from '@/lib/canvas/style';

/** `text` writes standalone text (`element` is the existing one or a template for a new one); `label` writes into a shape. */
export interface TextDraft { kind: 'text' | 'label'; element: CanvasElement; text: string; existing: boolean }
type Gesture = {
  mode: 'pan' | 'draw' | 'move' | 'resize' | 'marquee' | 'erase';
  start: Point;
  screen: Point;
  original: CanvasDocument;
  moved: boolean;
  pointerId: number;
  id?: string;
  handle?: number;
  endpoint?: number;
  bounds?: Bounds;
  shift?: boolean;
};
const normalize = (a: Point, b: Point): Bounds => ({ x: Math.min(a.x, b.x), y: Math.min(a.y, b.y), width: Math.abs(a.x - b.x), height: Math.abs(a.y - b.y) });
const newId = () => crypto.randomUUID();
const newSeed = () => Math.floor(Math.random() * 100000);
const NON_DRAWING = new Set<CanvasTool>(['select', 'hand', 'eraser']);

export function useCanvasInteraction(document: CanvasDocument, onChange: (document: CanvasDocument) => void) {
  const root = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const documentRef = useRef(document);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;
  const [tool, setTool] = useState<CanvasTool>('select');
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
  const overrides = useRef(new Map<string, CanvasElement | null>());
  const marquee = useRef<Bounds | undefined>(undefined);
  const bindHint = useRef<BindingSnap | null>(null);
  const size = useRef({ width: 0, height: 0, pixelRatio: 1 });
  const frame = useRef(0);
  const lastPaste = useRef({ text: '', count: 0 });
  const past = useRef<CanvasDocument[]>([]);
  const future = useRef<CanvasDocument[]>([]);
  const viewSave = useRef<ReturnType<typeof setTimeout> | null>(null);
  const options = useRef({ grid, editingId: textDraft?.element.id });
  options.current = { grid, editingId: textDraft?.element.id };

  const redraw = useCallback(() => {
    if (frame.current) return;
    frame.current = requestAnimationFrame(() => {
      frame.current = 0;
      const context = canvas.current?.getContext('2d');
      if (!context) return;
      paintCanvas(context, documentRef.current, {
        ...size.current, ...options.current, selected: selected.current,
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

  const publish = useCallback((next: CanvasDocument, before?: CanvasDocument) => {
    if (before) {
      past.current = [...past.current.slice(-99), before];
      future.current = [];
    }
    documentRef.current = next;
    onChangeRef.current(next);
    setRevision(value => value + 1);
    redraw();
  }, [redraw]);

  /** Every gesture preview goes through here, so bound arrows follow the shapes being previewed. */
  const previewChanges = (changes: ElementChanges) => {
    for (const [id, element] of changes) overrides.current.set(id, element);
    for (const [id, element] of followBindings(documentRef.current.elements, overrides.current)) overrides.current.set(id, element);
  };
  /** Every committed element edit goes through here, bound arrows following; returns whether the scene changed. */
  const commitChanges = (changes: ElementChanges, before: CanvasDocument = documentRef.current) => {
    const current = documentRef.current;
    const elements = applyElementChanges(current.elements, followBindings(current.elements, changes));
    if (elements === current.elements) return false;
    publish({ ...current, elements }, before);
    return true;
  };

  /** Layer order has its own publish: `commitChanges` keeps order. One undo step. */
  const reorder = useCallback((move: LayerMove) => {
    if (gesture.current) return;
    const current = documentRef.current;
    const elements = reorderElements(current.elements, selected.current, move);
    if (elements !== current.elements) publish({ ...current, elements }, current);
  }, [publish]);

  const history = useCallback((redo = false) => {
    if (gesture.current) return;
    const stack = redo ? future : past;
    const next = stack.current.pop();
    if (!next) return;
    (redo ? past : future).current.push(documentRef.current);
    choose([]);
    publish({ ...next, viewport: documentRef.current.viewport });
  }, [choose, publish]);

  const updateViewport = useCallback((viewport: CanvasDocument['viewport'], save = true) => {
    documentRef.current = { ...documentRef.current, viewport };
    redraw();
    setRevision(value => value + 1);
    if (save) {
      if (viewSave.current) clearTimeout(viewSave.current);
      viewSave.current = setTimeout(() => { viewSave.current = null; onChangeRef.current(documentRef.current); }, 180);
    }
  }, [redraw]);

  useEffect(() => {
    if (documentRef.current !== document) {
      documentRef.current = document;
      gesture.current = null;
      overrides.current.clear();
      marquee.current = undefined;
      past.current = [];
      future.current = [];
      selected.current = new Set();
      setSelection([]);
      draftRef.current = null; setTextDraft(null);
      if (viewSave.current) { clearTimeout(viewSave.current); viewSave.current = null; }
      setRevision(value => value + 1);
    }
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
      if (viewSave.current) { clearTimeout(viewSave.current); viewSave.current = null; onChangeRef.current(documentRef.current); }
    };
  }, [redraw]);
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
      if (gesture.current) return;
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
  const chooseTool = (next: CanvasTool) => { setTool(next); bindHint.current = null; if (next !== 'select') choose([]); };
  /** Binds an arrow end at the pointer (⌘/Ctrl places it freely) and shows the target it binds to. */
  const bindEnd = (arrow: CanvasElement, end: 'start' | 'end', point: Point, event: { metaKey: boolean; ctrlKey: boolean }) => {
    const bound = bindArrowEnd(arrow, end, point, documentRef.current.elements, documentRef.current.viewport.zoom, { live: overrides.current, free: event.metaKey || event.ctrlKey });
    bindHint.current = bound.snap;
    return bound.arrow;
  };
  const pointerDown = (event: PointerEvent<HTMLCanvasElement>) => {
    if (event.button !== 0 && event.button !== 1) return;
    if (gesture.current || textDraft) return;
    event.preventDefault();
    root.current?.focus({ preventScroll: true });
    event.currentTarget.setPointerCapture(event.pointerId);
    const { point, screen } = coordinates(event);
    const current = documentRef.current;
    const initial: Gesture = { mode: 'draw', start: point, screen, original: current, moved: false, pointerId: event.pointerId, shift: event.shiftKey };
    overrides.current.clear();
    if (tool === 'hand' || space || event.button === 1) initial.mode = 'pan';
    else if (tool === 'text') {
      const target = writableAt(point);
      if (target) startWriting(point, target); else startText(point);
      return;
    } else if (tool === 'eraser') {
      initial.mode = 'erase';
      const id = hitTest(current.elements, point, 10 / current.viewport.zoom);
      if (id) previewChanges(new Map([[id, null]]));
      choose([]);
    } else if (tool === 'select') {
      const bounds = selectedBounds(current, selected.current);
      const selectedElements = current.elements.filter(element => selected.current.has(element.id));
      const handle = bounds ? elementResizeHandles(selectedElements, bounds).findIndex(p => Math.hypot(p.x - point.x, p.y - point.y) < 10 / current.viewport.zoom) : -1;
      const id = hitTest(current.elements, point, 6 / current.viewport.zoom);
      if (handle >= 0 && bounds) { initial.mode = 'resize'; initial.bounds = bounds; initial.handle = handle;
        if (selectedElements.length === 1 && ['line', 'arrow'].includes(selectedElements[0].type)) { initial.id = selectedElements[0].id; initial.endpoint = handle; } }
      else if (id) {
        initial.mode = 'move';
        if (event.shiftKey) {
          const ids = new Set(selected.current);
          if (ids.has(id)) ids.delete(id); else ids.add(id);
          choose(ids);
        } else if (!selected.current.has(id)) choose([id]);
        const found = current.elements.find(element => element.id === id);
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
      documentRef.current = { ...documentRef.current, viewport: { ...viewport, x: viewport.x + screen.x - active.screen.x, y: viewport.y + screen.y - active.screen.y } };
    } else if (active.mode === 'draw' && active.id) {
      const element = overrides.current.get(active.id)!;
      let width = point.x - active.start.x, height = point.y - active.start.y;
      if (event.shiftKey && element.type !== 'freedraw') {
        if (element.type === 'line' || element.type === 'arrow') {
          const angle = Math.round(Math.atan2(height, width) / (Math.PI / 4)) * Math.PI / 4;
          const length = Math.hypot(width, height); width = Math.cos(angle) * length; height = Math.sin(angle) * length;
        } else { const side = Math.max(Math.abs(width), Math.abs(height)); width = Math.sign(width || 1) * side; height = Math.sign(height || 1) * side; }
      }
      const points = element.type === 'freedraw'
        ? [...(element.points ?? []), { x: width, y: height }]
        : element.points ? [{ x: 0, y: 0 }, { x: width, y: height }] : undefined;
      // x/y are restated because a bound start has moved the previewed arrow off the pointer-down point.
      const next = { ...element, x: active.start.x, y: active.start.y, width, height, points };
      previewChanges(new Map([[element.id, next.type === 'arrow' ? bindEnd(next, 'end', point, event) : next]]));
    } else if (active.mode === 'move') {
      let dx = point.x - active.start.x, dy = point.y - active.start.y;
      if (event.shiftKey) { if (Math.abs(dx) > Math.abs(dy)) dy = 0; else dx = 0; }
      previewChanges(moveElements(active.original.elements, selected.current, dx, dy));
    } else if (active.mode === 'resize' && active.endpoint !== undefined && active.id) {
      const element = active.original.elements.find(item => item.id === active.id)!;
      const points = element.points?.length ? element.points : [{ x: 0, y: 0 }, { x: element.width, y: element.height }];
      const start = active.endpoint === 0 ? point : { x: element.x + points[0].x, y: element.y + points[0].y };
      const end = active.endpoint === 1 ? point : { x: element.x + points[points.length - 1].x, y: element.y + points[points.length - 1].y };
      const next = { ...element, x: start.x, y: start.y, width: end.x - start.x, height: end.y - start.y, points: [{ x: 0, y: 0 }, { x: end.x - start.x, y: end.y - start.y }] };
      previewChanges(new Map([[element.id, next.type === 'arrow' ? bindEnd(next, active.endpoint === 0 ? 'start' : 'end', point, event) : next]]));
    } else if (active.mode === 'resize' && active.bounds && active.handle !== undefined) {
      const original = active.bounds;
      const opposite = resizeHandles(original)[(active.handle + 2) % 4];
      const next = normalize(point, opposite);
      const sx = next.width / Math.max(original.width, 1), sy = next.height / Math.max(original.height, 1);
      const changes = new Map<string, CanvasElement>();
      for (const element of active.original.elements) {
        if (!selected.current.has(element.id)) continue;
        // Text scales its font; a shape keeps its label's size, rewraps it and grows if the label no longer fits.
        const resized: CanvasElement = {
          ...element, x: next.x + (element.x - original.x) * sx, y: next.y + (element.y - original.y) * sy,
          width: element.width * (element.type === 'text' ? sy : sx), height: element.height * sy,
          points: element.points?.map(p => ({ x: p.x * sx, y: p.y * sy })),
          ...(element.type === 'text' ? { fontSize: Math.max(8, (element.fontSize ?? 24) * sy) } : {}),
        };
        changes.set(element.id, growToFitLabel(resized));
      }
      previewChanges(changes);
    } else if (active.mode === 'marquee') marquee.current = normalize(active.start, point);
    else if (active.mode === 'erase') {
      const id = hitTest(active.original.elements, point, 10 / documentRef.current.viewport.zoom);
      if (id) previewChanges(new Map([[id, null]]));
    }
    redraw();
  };

  const pointerUp = (event: PointerEvent<HTMLCanvasElement>) => {
    const active = gesture.current;
    if (!active || active.pointerId !== event.pointerId) return;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
    gesture.current = null; bindHint.current = null;
    if (active.mode === 'pan') publish(documentRef.current);
    else if (active.mode === 'marquee' && marquee.current) {
      const bounds = marquee.current;
      const ids = active.shift ? new Set(selected.current) : new Set<string>();
      for (const element of active.original.elements) if (boundsIntersect(elementBounds(element), bounds)) ids.add(element.id);
      choose(ids);
    } else if (overrides.current.size && (active.moved || active.mode === 'erase')) {
      const changes = new Map(overrides.current);
      if (active.mode === 'draw' && active.id) {
        const element = changes.get(active.id);
        if (element?.type === 'freedraw' && element.points) changes.set(element.id, { ...element, points: simplifyPoints(element.points, 0.5 / documentRef.current.viewport.zoom) });
      }
      commitChanges(changes, active.original);
      if (active.mode === 'draw' && active.id && tool !== 'freedraw') { choose([active.id]); setTool('select'); }
    }
    overrides.current.clear(); marquee.current = undefined;
    setRevision(value => value + 1);
    redraw();
  };
  const cancelGesture = () => {
    if (gesture.current) documentRef.current = gesture.current.original;
    gesture.current = null; overrides.current.clear(); marquee.current = undefined; bindHint.current = null; redraw();
  };
  const deleteSelected = () => {
    if (!selected.current.size) return;
    if (draftRef.current) { draftRef.current = null; setTextDraft(null); }
    commitChanges(new Map([...selected.current].map(id => [id, null])));
    choose([]);
  };
  const commitText = (text: string) => {
    const draft = draftRef.current;
    if (!draft) return;
    draftRef.current = null;
    const current = documentRef.current;
    const base = current.elements.find(item => item.id === draft.element.id) ?? draft.element;
    const element = commitTextEdit(base, draft.kind, text);
    if (draft.kind === 'text' || current.elements.includes(base)) commitChanges(new Map([[base.id, element]]), current);
    setTextDraft(null); setTool('select'); choose(element ? [element.id] : []);
    requestAnimationFrame(() => root.current?.focus({ preventScroll: true }));
  };
  const flushDraft = () => { if (draftRef.current) commitText(draftValue.current); };
  const selectedTypes = () => documentRef.current.elements.filter(element => selected.current.has(element.id)).map(element => element.type);
  /** While writing, a style change restyles the draft (and its shape or text, if they exist) and the overlay follows; commit re-measures. */
  const restyleDraft = (draft: TextDraft, patch: StylePatch) => {
    const element = patchElementStyle(draft.element, patch);
    if (element === draft.element) return;
    if (draft.existing) commitChanges(new Map([[element.id, element]]));
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
    flushDraft();
    overrides.current.clear();
    previewChanges(stylePatchChanges(documentRef.current.elements, selected.current, patch));
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
    cancelText: () => { draftRef.current = null; setTextDraft(null); redraw(); requestAnimationFrame(() => root.current?.focus({ preventScroll: true })); },
    pointerDown, pointerMove, pointerUp, cancelGesture, pointerLeave: () => { if (!gesture.current && bindHint.current) { bindHint.current = null; redraw(); } }, doubleClick, keyDown, copy, cut: (event: ClipboardEvent<HTMLDivElement>) => copy(event, true), paste,
    keyUp: (event: KeyboardEvent<HTMLDivElement>) => { if (event.key === ' ') setSpace(false); },
    blur: () => setSpace(false), cursor: space || tool === 'hand' ? 'grab' : tool === 'select' ? 'default' : tool === 'text' ? 'text' : 'crosshair',
    history, canUndo: past.current.length > 0, canRedo: future.current.length > 0,
    deleteSelected, reorder, layerMoves, fit, zoom, resetZoom: () => updateViewport(zoomAt(documentRef.current.viewport, { x: size.current.width / 2, y: size.current.height / 2 }, 1)), redraw,
    zoomPercent: Math.round(documentRef.current.viewport.zoom * 100),
  };
}
