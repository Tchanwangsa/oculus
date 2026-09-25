import { useEffect, useState } from "react";

/** The current time, re-rendered once a minute so "now" lines and past-event
 *  styling do not go stale in a window left open. */
export function useNow(intervalMs = 60_000): Date {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), intervalMs);
    return () => clearInterval(id);
  }, [intervalMs]);
  return now;
}
