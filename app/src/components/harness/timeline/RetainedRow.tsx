import { useCallback, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { cn } from "@/lib/utils";

/** The portal target stays the same when `useRowVisibility` detaches it, so
 *  off-screen rows keep their React state without participating in layout. */
export function RetainedRow({ children, className, questionId }: {
  children: ReactNode;
  className: string;
  questionId?: number;
}) {
  const [host] = useState(() => {
    const el = document.createElement("div");
    el.className = "min-w-0";
    el.setAttribute("data-row-content", "");
    return el;
  });
  const [ready, setReady] = useState(false);
  const attach = useCallback((row: HTMLDivElement | null) => {
    if (!row) return;
    row.appendChild(host);
    // Mount portal children into a connected host, before their layout effects.
    setReady(true);
  }, [host]);
  return (
    <div
      ref={attach}
      className={cn(className, questionId != null && "thread-map-question-row")}
      data-question-id={questionId}
    >
      {ready && createPortal(children, host)}
    </div>
  );
}
