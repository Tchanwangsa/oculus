import { useEffect, useRef, useState } from 'react';
import { watchNativeFileDrop } from '@/lib/ui/nativeFileDrop';

/** Finder drops are webview events, intercepted before React's onDrop. */
export function useFileDrop(ref: React.RefObject<HTMLElement | null>, onDrop: (paths: string[]) => void) {
  const [over, setOver] = useState(false);
  const latest = useRef(onDrop);
  latest.current = onDrop;
  useEffect(() => {
    if (!ref.current) return;
    try { return watchNativeFileDrop(ref.current, (_, at) => setOver(!!at), paths => latest.current(paths), 8); }
    catch { /* A plain browser has no native file events. */ }
  }, [ref]);
  return over;
}
