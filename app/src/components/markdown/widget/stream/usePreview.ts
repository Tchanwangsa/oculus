import { useEffect, useRef, useState } from "react";
import { transpilePreview } from "../transpile";
import { PreviewScheduler } from "./schedule";

export interface Preview {
  js: string;
  pending: string[];
}

/**
 * The newest prefix of a streaming fence that transpiled, or null before one
 * has. Only while `active`; a prefix that fails to transpile is dropped, so
 * the frame keeps the last one that did.
 */
export function usePreview(source: string, active: boolean): Preview | null {
  const [preview, setPreview] = useState<Preview | null>(null);
  const latest = useRef(source);
  latest.current = source;
  const scheduler = useRef<PreviewScheduler | null>(null);

  useEffect(() => {
    if (!active) return;
    let alive = true;
    const s = new PreviewScheduler(
      () => latest.current,
      async (candidate) => {
        const out = await transpilePreview(candidate);
        if (alive && out.js != null) setPreview({ js: out.js, pending: out.pending });
      },
    );
    scheduler.current = s;
    s.poke();
    return () => {
      alive = false;
      s.dispose();
      scheduler.current = null;
    };
  }, [active]);

  useEffect(() => {
    scheduler.current?.poke();
  }, [source]);

  return active ? preview : null;
}
