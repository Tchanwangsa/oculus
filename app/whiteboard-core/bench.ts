// Geometry throughput only: this does not measure rendering or input latency.
// From app/: bun canvas-core/bench.ts (build with bun run canvas first).
import { initSync } from './pkg/oculus_canvas_core';
import { hitTest, loadCanvasEngine, visibleElements } from '../src/lib/canvas/engine';
import { boundsIntersect, elementBounds, elementHitTest } from '../src/lib/canvas/geometry';
import type { CanvasElement } from '../src/lib/canvas/model';

initSync({ module: await Bun.file(new URL('./pkg/oculus_canvas_core_bg.wasm', import.meta.url)).arrayBuffer() });
await loadCanvasEngine();
const elements: CanvasElement[] = Array.from({ length: 10000 }, (_, i) => ({ id: String(i), type: 'rectangle', x: i % 100 * 150, y: Math.floor(i / 100) * 150, width: 100, height: 100, strokeWidth: 2, stroke: '#111', fill: 'transparent', style: 'clean', seed: i }));
const queries = Array.from({ length: 2000 }, (_, i) => ({ x: (i * 139) % 15000, y: (i * 313) % 15000 }));
const areas = queries.slice(0, 500).map(p => ({ ...p, width: 900, height: 700 }));
function measure(label: string, run: () => void) {
  run();
  const timings: number[] = [];
  for (let i = 0; i < 7; i++) { const start = performance.now(); run(); timings.push(performance.now() - start); }
  timings.sort((a, b) => a - b);
  console.log(`${label}: ${timings[3].toFixed(2)} ms median of 7 batches`);
}
const start = performance.now(); hitTest(elements, queries[0]);
console.log(`10,000 shapes: first bridge transfer + BVH + query ${(performance.now() - start).toFixed(2)} ms`);
measure('Rust retained 2,000 hit queries', () => { for (const q of queries) hitTest(elements, q, 4); });
measure('JS reference 2,000 hit queries', () => { for (const q of queries) for (let i = elements.length - 1; i >= 0; i--) if (elementHitTest(elements[i], q, 4)) break; });
measure('Rust retained 500 culling queries', () => { for (const area of areas) visibleElements(elements, area); });
measure('JS reference 500 culling queries', () => { for (const area of areas) elements.filter(el => { const b = elementBounds(el); return boundsIntersect({ x: b.x - 3, y: b.y - 3, width: b.width + 6, height: b.height + 6 }, area); }); });
