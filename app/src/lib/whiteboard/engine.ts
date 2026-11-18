import { arrowheadOf, type CanvasElement, type Point } from './model';
import { arrowheadSize, arrowPath, boundsIntersect, elementBounds, elementFilled, elementHitTest, simplifyPolyline, type Bounds } from './geometry';
import { linePath, pathEnd } from './arrow/path';

type WasmModule = typeof import('../../../canvas-core/pkg/oculus_canvas_core');
type Scene = InstanceType<WasmModule['CanvasScene']>;
let wasm: WasmModule | undefined;
let loading: Promise<void> | undefined;
let status: 'loading' | 'rust' | 'javascript' = 'loading';
const scenes = new Map<readonly CanvasElement[], Scene>();
const kinds: CanvasElement['type'][] = ['rectangle', 'ellipse', 'diamond', 'line', 'arrow', 'freedraw', 'text', 'image'];
/** Floats per shape; the layout is documented on `CanvasScene::new` in canvas-core. */
const RECORD = 14;

/** An arrow sends its flattened path and each head's angle and size, so Rust tests the heads the JS reference draws. */
function record(el: CanvasElement, start: number, count: number): number[] {
  const base = [kinds.indexOf(el.type), el.x, el.y, el.width, el.height, el.strokeWidth, elementFilled(el) ? 1 : 0, start, count];
  if (el.type !== 'arrow') return [...base, 0, 0, 0, 0, 0];
  const heads = (arrowheadOf(el, 'start') !== 'none' ? 1 : 0) | (arrowheadOf(el, 'end') !== 'none' ? 2 : 0);
  const size = arrowheadSize(linePath(el).length);
  return [...base, heads, pathEnd(el, 'start').angle, size, pathEnd(el, 'end').angle, size];
}

export function getCanvasEngineStatus() { return status; }

export function loadCanvasEngine(): Promise<void> {
  return loading ??= import('../../../canvas-core/pkg/oculus_canvas_core').then(async module => {
    await module.default();
    wasm = module;
    status = 'rust';
  }).catch(error => {
    status = 'javascript';
    console.error('[canvas] Rust engine could not load; using JavaScript geometry.', error);
    throw new Error('Rust canvas engine could not load. Drawing is using JavaScript geometry.');
  });
}

/** Callers replace the array and edited elements, keeping queries allocation-light. */
function sceneFor(elements: readonly CanvasElement[]): Scene | undefined {
  if (!wasm) return;
  const current = scenes.get(elements);
  if (current) return current;
  const records = new Float64Array(elements.length * RECORD);
  const coordinates: number[] = [];
  elements.forEach((el, i) => {
    const start = coordinates.length / 2;
    const points = el.type === 'arrow' ? arrowPath(el) : el.points;
    if (points) for (const p of points) coordinates.push(p.x, p.y);
    records.set(record(el, start, coordinates.length / 2 - start), i * RECORD);
  });
  const next = new wasm.CanvasScene(records, new Float64Array(coordinates));
  scenes.set(elements, next);
  // Undo history can retain many scene arrays. Keep Rust memory independently bounded.
  if (scenes.size > 8) {
    const oldest = scenes.keys().next().value!;
    scenes.get(oldest)!.free(); scenes.delete(oldest);
  }
  return next;
}

export function hitTest(elements: readonly CanvasElement[], point: Point, tolerance = 6): string | null {
  const scene = sceneFor(elements);
  if (scene) {
    const index = scene.hit_test(point.x, point.y, tolerance);
    return index >= 0 ? elements[index].id : null;
  }
  for (let i = elements.length - 1; i >= 0; i--) if (elementHitTest(elements[i], point, tolerance)) return elements[i].id;
  return null;
}

export function visibleElements(elements: readonly CanvasElement[], bounds: Bounds): CanvasElement[] {
  const scene = sceneFor(elements);
  if (scene) return Array.from(scene.visible_indices(bounds.x, bounds.y, bounds.width, bounds.height), i => elements[i]);
  return elements.filter(el => {
    const b = elementBounds(el), pad = el.strokeWidth / 2 + (el.type === 'arrow' ? 16 : 0) + 2;
    return boundsIntersect({ x: b.x - pad, y: b.y - pad, width: b.width + pad * 2, height: b.height + pad * 2 }, bounds);
  });
}

export function simplifyPoints(points: readonly Point[], tolerance = 1): Point[] {
  if (!wasm) return simplifyPolyline(points, tolerance);
  const coordinates = new Float64Array(points.length * 2);
  points.forEach((p, i) => { coordinates[i * 2] = p.x; coordinates[i * 2 + 1] = p.y; });
  const result = wasm.simplify_points(coordinates, tolerance);
  const simplified: Point[] = [];
  for (let i = 0; i < result.length; i += 2) simplified.push({ x: result[i], y: result[i + 1] });
  return simplified;
}
