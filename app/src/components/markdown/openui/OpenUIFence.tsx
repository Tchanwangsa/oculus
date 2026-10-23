import { lazy, Suspense, useEffect, useState, type ReactNode } from "react";

const OpenUIBlock = lazy(() => import("./OpenUIBlock"));

/** As `Mermaid`'s: a streaming fence settles before it is parsed, so the
 *  timeline never parses per delta, only once per pause in the stream. */
const SETTLE_MS = 150;

/**
 * A ```openui fence. Reached from `MD_COMPONENTS.pre`. Shows the fence as
 * code until its source stops changing, then hands it to the lazily loaded
 * `OpenUIBlock`, which draws it or keeps showing the code. A fence still
 * streaming usually fails to parse (cut mid-string or with a reference not
 * yet defined), so it stays code until the program is whole.
 */
export function OpenUIFence({ source, children, blockIndex = 0 }: { source: string; children: ReactNode; blockIndex?: number }) {
  const [settled, setSettled] = useState<string | null>(null);
  useEffect(() => {
    const timer = setTimeout(() => setSettled(source), SETTLE_MS);
    return () => clearTimeout(timer);
  }, [source]);
  if (settled === null) return <>{children}</>;
  return (
    <Suspense fallback={children}>
      {/* Keyed so a fence that failed mid-stream gets a fresh try once more
          of it arrives. */}
      <OpenUIBlock key={settled} source={settled} fallback={children} blockIndex={blockIndex} />
    </Suspense>
  );
}

/** The fence tags that mean OpenUI Lang: ours, and the library's own. */
export const OPENUI_LANGS = /(^|\s)language-openui(-lang)?(\s|$)/;
