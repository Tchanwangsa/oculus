import { useMemo, useRef, type KeyboardEvent, type RefObject } from "react";
import type { MentionInputHandle } from "@/components/harness/mentions/MentionInput";
import type { HarnessItem } from "@/lib/harness";
import { promptHistoryEntries, stepPromptHistory, type PromptHistoryPosition } from "@/lib/harness/promptHistory";
import { useHarnessStore } from "@/stores/chat/harnessStore";

const NONE: HarnessItem[] = [];

/**
 * ArrowUp in an empty composer recalls the open thread's earlier messages,
 * ArrowDown walks forward again (`lib/harness/promptHistory.ts`). Runs after
 * the `@`/`/` menu's keys, which come first while it shows. `text` is the box's
 * serialized message, as the draft holds it.
 */
export function usePromptHistory({
  threadId,
  input,
  text,
}: {
  threadId: number | null;
  input: RefObject<MentionInputHandle | null>;
  text: string;
}) {
  const rows = useHarnessStore((s) => (threadId == null ? NONE : (s.items[threadId] ?? NONE)));
  const entries = useMemo(() => promptHistoryEntries(rows), [rows]);
  const position = useRef<PromptHistoryPosition | null>(null);

  /** Claims ArrowUp/ArrowDown only when it moves through the history. */
  function keyDown(e: KeyboardEvent<HTMLElement>) {
    if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;
    if (e.shiftKey || e.altKey || e.metaKey || e.ctrlKey) return;
    const step = stepPromptHistory({
      direction: e.key === "ArrowUp" ? "backward" : "forward",
      entries,
      position: position.current,
      current: text,
    });
    if (!step) return;
    e.preventDefault();
    position.current = step.position;
    input.current?.replace(step.prompt);
  }

  return { keyDown };
}
