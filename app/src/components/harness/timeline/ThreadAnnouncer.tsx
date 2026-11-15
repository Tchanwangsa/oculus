import { useEffect, useRef, useState } from "react";
import { createAnnouncer, lastTurnEnd, spokenStatus } from "@/lib/harness";
import { useHarnessStore } from "@/stores/chat/harnessStore";

/**
 * One polite live region per view of a thread, for screen readers: it says
 * "Working", "Done", "Failed", "Stopped" (and, once the bridges raise them,
 * "Needs approval" and "Needs your input") when the open thread's status
 * changes — never on mount or a thread switch, and never a message or a step.
 * A plain `aria-live="polite"` region, not `role="status"`.
 */
export function ThreadAnnouncer({ threadId }: { threadId: number }) {
  const [text, setText] = useState("");
  const running = useHarnessStore((s) => s.live[threadId]?.running ?? false);
  const lastRun = useHarnessStore((s) => lastTurnEnd(s.live[threadId]?.ended, s.runs[threadId]));
  // Until the thread's runs are read, its status is unknown, not news.
  const loaded = useHarnessStore((s) => threadId in s.runs);
  const announcer = useRef<ReturnType<typeof createAnnouncer> | null>(null);
  if (!announcer.current) announcer.current = createAnnouncer(setText);

  useEffect(() => {
    announcer.current?.reset();
  }, [threadId]);
  useEffect(() => {
    if (loaded) announcer.current?.update(spokenStatus({ running, approval: false, input: false, lastRun }));
  }, [threadId, loaded, running, lastRun]);
  useEffect(() => () => announcer.current?.dispose(), []);

  return (
    <div aria-live="polite" aria-atomic="true" className="sr-only">
      {text}
    </div>
  );
}
