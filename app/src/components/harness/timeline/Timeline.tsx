import { memo, useMemo, useState } from "react";
import { copyAsMarkdown, dragAsMarkdown } from "@/lib/markdown/selection";
import type { HarnessItem, Provider } from "@/lib/harness";
import { useSignInStatus } from "@/hooks/agents/useSignInStatus";
import { SignInDialog, useSignIn } from "../SignInDialog";
import { Bundle, Item } from "./ItemRows";
import { ContextDrift } from "./Messages";
import { LiveTail, Pending } from "./Pending";
import { buildRows } from "./rows";
import type { PendingActions, QuestionActions } from "./types";

export type { PendingActions, QuestionActions } from "./types";

/**
 * The thread as a list. Messages are the spine; the tool calls and reasoning
 * between them are a *step*, and a finished step of two or more rows folds
 * into one summary row ("Explored 3 files, ran 2 commands"). The step in
 * progress stays unfolded; finished rows are dimmed.
 *
 * **Nothing here re-renders for the turn in flight.** Committed rows never
 * change, so all are memoised; only `LiveTail` and a running `Tool` subscribe
 * to the stream.
 */
export const Timeline = memo(function Timeline({
  items,
  threadId,
  running,
  questions,
  pending,
}: {
  items: HarnessItem[];
  threadId: number | null;
  running: boolean;
  /** Stable for the life of the page — see `Item`. */
  questions?: QuestionActions;
  pending?: PendingActions;
}) {
  // Held here, not in the row, so the dialog survives rows re-rendering; and
  // in `Timeline`, not `ChatPage`, so the lecture dock gets it too.
  const [signIn, setSignIn] = useState<Provider | null>(null);
  const { recheck } = useSignInStatus();
  const run = useSignIn(recheck);

  const rows = useMemo(() => buildRows(items, running), [items, running]);
  // Which question each answer answered, for Retry.
  const asked = useMemo(() => {
    const map = new Map<number, HarnessItem>();
    let last: HarnessItem | undefined;
    for (const i of items) {
      if (i.kind === "user") last = i;
      else if (i.kind === "assistant" && last) map.set(i.id, last);
    }
    return map;
  }, [items]);
  // Only the newest refusal is still a question; the ones above it are what
  // happened. Allowing is idempotent, so this is tidiness, not safety.
  const latestPermission = useMemo(() => {
    for (let i = items.length - 1; i >= 0; i--) if (items[i].kind === "permission") return items[i].id;
    return undefined;
  }, [items]);
  return (
    <div
      className="flex min-w-0 flex-col gap-2"
      onCopy={copyAsMarkdown}
      onDragStart={dragAsMarkdown}
    >
      {rows.map((r) =>
        r.kind === "bundle" ? (
          <Bundle key={r.id} items={r.items} />
        ) : (
          <Item
            key={r.item.id}
            item={r.item}
            dim={r.dim}
            actions={questions}
            asked={asked.get(r.item.id)}
            onSignIn={setSignIn}
            latest={r.item.id === latestPermission}
          />
        ),
      )}
      {threadId != null && <ContextDrift threadId={threadId} />}
      {threadId != null && <LiveTail threadId={threadId} />}
      {threadId != null && pending && <Pending threadId={threadId} actions={pending} />}
      {signIn && (
        <SignInDialog
          provider={signIn}
          run={run.run?.provider === signIn ? run.run : null}
          onStart={() => run.start(signIn)}
          onCode={(code) => run.submitCode(code)}
          onCancel={() => run.cancel()}
          onClose={() => {
            // Clear a finished run; keep one in flight so reopening resumes it.
            if (run.run?.result) run.clear();
            setSignIn(null);
          }}
        />
      )}
    </div>
  );
});
