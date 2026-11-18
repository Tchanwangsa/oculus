/** Optional companion tracing; the desktop pays only the empty-sink check. */
export type TraceFields = Record<string, string | number | boolean | null>;
export interface WhiteboardTraceSink {
  event(name: string, fields: TraceFields): void;
  /** Spans are timed only by sinks that want them (the diagnostics log); the on-screen overlay has none. */
  begin?(name: string, fields: TraceFields): () => void;
}
/** Both the diagnostics log and the gesture overlay can listen at once. */
const sinks = new Set<WhiteboardTraceSink>();
export function installWhiteboardTrace(next: WhiteboardTraceSink): () => void {
  sinks.add(next);
  return () => { sinks.delete(next); };
}
export function traceWhiteboard(name: string, fields: TraceFields = {}): void {
  for (const sink of sinks) {
    try { sink.event(name, fields); } catch { /* tracing must never break input */ }
  }
}
function begin(name: string, fields: TraceFields): () => void {
  const finishes: (() => void)[] = [];
  for (const sink of sinks) {
    try { const finish = sink.begin?.(name, fields); if (finish) finishes.push(finish); } catch { /* receiver unavailable */ }
  }
  return () => { for (const finish of finishes) { try { finish(); } catch { /* tracing must never break input */ } } };
}
export function measureWhiteboard<T>(name: string, run: () => T, fields: TraceFields = {}): T {
  if (!sinks.size) return run();
  const finish = begin(name, fields);
  try { return run(); } finally { finish(); }
}
export async function measureWhiteboardAsync<T>(name: string, run: () => T | PromiseLike<T>): Promise<T> {
  if (!sinks.size) return await run();
  const finish = begin(name, { asynchronous: true });
  try { return await run(); } finally { finish(); }
}
