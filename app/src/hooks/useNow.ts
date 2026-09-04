import { useEffect, useState } from "react";

/**
 * The current time, re-rendered once a minute.
 *
 * Anything that draws "now" — the week grid's time line, the grey wash over
 * elapsed hours, past-event styling — has to move on its own or it silently
 * goes stale on a window left open all day. A minute is the finest granularity
 * any of those actually show.
 */
export function useNow(intervalMs = 60_000): Date {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), intervalMs);
    return () => clearInterval(id);
  }, [intervalMs]);
  return now;
}
