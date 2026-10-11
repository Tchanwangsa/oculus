import { useEffect, useState } from "react";
import { useTabActive } from "@/components/tabs/TabContext";

/** The current time, re-rendered once a minute so "now" lines and past-event
 *  styling do not go stale in a window left open. */
export function useNow(intervalMs = 60_000): Date {
  const active = useTabActive();
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    if (!active) return;
    setNow(new Date());
    const id = setInterval(() => setNow(new Date()), intervalMs);
    return () => clearInterval(id);
  }, [intervalMs, active]);
  return now;
}
