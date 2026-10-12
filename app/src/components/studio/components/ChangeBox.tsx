import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { ArrowUp, CircleNotch } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import type { Applied } from "@/hooks/studio/useLibraryChange";
import { chatHref } from "@/lib/harness";
import { cn } from "@/lib/utils";

/**
 * "Ask for a change": one box that sends the request to the agent in a Chat
 * thread (the first one starts it, the rest continue it). While its turn runs
 * a spinner takes the button's place; after it, the component is read again
 * and the result is said plainly, with Undo when an earlier version exists
 * and a way to the thread.
 */
export function ChangeBox({
  name,
  working,
  applied,
  error,
  threadId,
  onSend,
  onUndo,
}: {
  name: string;
  /** The component the running turn is changing, if any. */
  working: string | null;
  applied: Applied | null;
  error: string | null;
  threadId: number | null;
  onSend: (request: string) => Promise<boolean>;
  onUndo: () => Promise<void>;
}) {
  const navigate = useNavigate();
  const [text, setText] = useState("");
  const [sending, setSending] = useState(false);
  const busy = working != null || sending;
  const ready = text.trim().length > 0 && !busy;

  const send = async () => {
    if (!ready) return;
    const request = text;
    setSending(true);
    setText("");
    const ok = await onSend(request);
    // A send that failed gives the words back.
    if (!ok) setText(request);
    setSending(false);
  };
  const open = threadId == null ? null : chatHref(threadId);
  const mine = applied?.name === name ? applied : null;

  return (
    <div className="space-y-2.5">
      <div className="flex items-end gap-2 rounded-2xl border border-input bg-card p-1.5 pl-3.5 shadow-xs focus-within:border-ring focus-within:ring-[3px] focus-within:ring-ring/25">
        <Textarea
          aria-label={`Ask for a change to ${name}`}
          value={text}
          disabled={busy}
          rows={1}
          placeholder={`Describe a change to ${name}`}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
              e.preventDefault();
              void send();
            }
          }}
          className="max-h-40 min-h-8 flex-1 resize-none rounded-none border-0 bg-transparent px-0 py-1.5 shadow-none focus-visible:ring-0 dark:bg-transparent"
        />
        <Button type="button" size="icon-sm" aria-label="Send" disabled={!ready} onClick={() => void send()}>
          {busy ? <CircleNotch size={14} className="animate-spin" /> : <ArrowUp size={14} weight="bold" />}
        </Button>
      </div>

      {working != null && (
        <p className="text-[12.5px] text-muted-foreground">
          {working === name ? "The agent is changing it" : `The agent is changing ${working}`}. Its steps are in the thread.
        </p>
      )}
      {error && (
        <p data-selectable className="text-[12.5px] leading-snug text-destructive">
          {error}
        </p>
      )}
      {mine && working == null && <Outcome applied={mine} />}
      <div className={cn("flex items-center gap-2", !(mine?.undoId || open) && "hidden")}>
        {mine?.undoId && (
          <Button type="button" variant="outline" size="sm" onClick={() => void onUndo()}>
            Undo
          </Button>
        )}
        {open && (
          <Button type="button" variant="ghost" size="sm" data-tab-href={open} onClick={() => navigate(open)}>
            Open thread
          </Button>
        )}
      </div>
    </div>
  );
}

function Outcome({ applied }: { applied: Applied }) {
  const text = applied.undone
    ? "Restored the earlier version."
    : !applied.changed
      ? "The turn ended without changing it. The thread shows why."
      : applied.undoId
        ? "Changed. The earlier version is kept."
        : "Changed. There was no earlier version at this level to go back to.";
  return <p className="text-[12.5px] text-muted-foreground">{text}</p>;
}
