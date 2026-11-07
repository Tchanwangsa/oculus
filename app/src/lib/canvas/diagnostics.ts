/** Optional companion tracing; the desktop pays only the disabled-sink check. */
export type TraceFields = Record<string, string | number | boolean | null>;
export interface CanvasTraceSink {
  event(name: string, fields: TraceFields): void;
  begin(name: string, fields: TraceFields): () => void;
}
let sink: CanvasTraceSink | null = null;
export function installCanvasTrace(next: CanvasTraceSink): () => void {
  sink = next;
  return () => { if (sink === next) sink = null; };
}
export function traceCanvas(name: string, fields: TraceFields = {}): void {
  try { sink?.event(name, fields); } catch { /* tracing must never break input */ }
}
function begin(name: string, fields: TraceFields): () => void {
  let finish: (() => void) | undefined;
  try { finish = sink?.begin(name, fields); } catch { /* receiver unavailable */ }
  return () => { try { finish?.(); } catch { /* tracing must never break input */ } };
}
export function measureCanvas<T>(name: string, run: () => T, fields: TraceFields = {}): T {
  if (!sink) return run();
  const finish = begin(name, fields);
  try { return run(); } finally { finish(); }
}
export async function measureCanvasAsync<T>(name: string, run: () => T | PromiseLike<T>): Promise<T> {
  if (!sink) return await run();
  const finish = begin(name, { asynchronous: true });
  try { return await run(); } finally { finish(); }
}
