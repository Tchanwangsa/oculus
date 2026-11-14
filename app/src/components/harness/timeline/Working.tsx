import { useEffect, useState } from "react";
import { elapsedText } from "@/lib/harness";

/** "Working for 8s", ticking; "Working" with no start. Its own component, so
 *  the second hand re-renders this line and nothing else. Steady text: the
 *  run's live step is what shimmers. */
export function Working({ since }: { since: number | null }) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (since == null) return;
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [since]);
  return <>{since == null ? "Working" : `Working for ${elapsedText(Math.max(0, now - since))}`}</>;
}
