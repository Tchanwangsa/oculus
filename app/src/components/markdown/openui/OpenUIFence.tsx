import { lazy, Suspense, useEffect, useRef, useState, type ReactNode } from "react";
import { useWidgetRow } from "./state";
import { OpenUIFailure, OpenUISkeleton } from "./Streaming";

const OpenUIBlock = lazy(() => import("./OpenUIBlock"));

/** Coalesce deltas without postponing the next frame when another arrives. */
export function OpenUIFence({ source, children, blockIndex = 0, closed = false }: { source: string; children: ReactNode; blockIndex?: number; closed?: boolean }) {
  const row = useWidgetRow();
  const isStreaming = !row || row.row <= 0 || !closed;
  const [drawn, setDrawn] = useState(source);
  const latest = useRef(source);
  latest.current = source;
  const frame = useRef<number | null>(null);
  useEffect(() => {
    if (frame.current === null) frame.current = requestAnimationFrame(() => {
      frame.current = null;
      setDrawn(latest.current);
    });
  }, [source]);
  useEffect(() => () => {
    if (frame.current !== null) cancelAnimationFrame(frame.current);
    frame.current = null;
  }, []);
  // A close/commit flushes the final delta before strict validation.
  return (
    <Suspense fallback={<OpenUISkeleton />}>
      <OpenUIBlock source={isStreaming ? drawn : source} isStreaming={isStreaming}
        fallback={<OpenUIFailure source={source} code={children} />} blockIndex={blockIndex} />
    </Suspense>
  );
}

/** The fence tags that mean OpenUI Lang: ours, and the library's own. */
export const OPENUI_LANGS = /(^|\s)language-openui(-lang)?(\s|$)/;
