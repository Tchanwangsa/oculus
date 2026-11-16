import { useEffect, useState } from "react";
import { getPendingQuestions, getThreadPreviews, type HarnessThread } from "@/lib/harness";

export interface ThreadLines {
  /** Each thread's newest message, either side's. */
  messages: Map<number, string>;
  /** Each thread's waiting question, for the Question row. */
  questions: Map<number, string>;
}

const EMPTY: ThreadLines = { messages: new Map(), questions: new Map() };

/**
 * The listed threads' third lines, one read for the list. Re-read when a
 * listed thread changes (`updated_at`, or a question opening or closing,
 * which does not touch `updated_at`), never per streamed token.
 */
export function useThreadPreviews(threads: HarnessThread[]): ThreadLines {
  const [lines, setLines] = useState<ThreadLines>(EMPTY);
  const key = threads.map((t) => `${t.id}:${t.updated_at}:${t.pending_questions ?? 0}`).join(",");
  useEffect(() => {
    const ids = threads.map((t) => t.id);
    const asking = threads.filter((t) => (t.pending_questions ?? 0) > 0).map((t) => t.id);
    let live = true;
    Promise.all([getThreadPreviews(ids), getPendingQuestions(asking)])
      .then(([messages, questions]) => live && setLines({ messages, questions }))
      .catch((e) => console.error("thread previews failed", e));
    return () => {
      live = false;
    };
    // `key` stands for `threads`, whose identity changes on unrelated writes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  return lines;
}
