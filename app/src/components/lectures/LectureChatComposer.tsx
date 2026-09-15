import { useEffect, useLayoutEffect, useRef, useState, type RefObject } from "react";
import { Camera, CameraSlash, PaperPlaneTilt, Stop } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { ModelPicker, type PickerProvider } from "@/components/harness/ModelPicker";
import type { Provider } from "@/lib/harness";
import { fmtTime } from "@/lib/lectures";
import { cn } from "@/lib/utils";

/** The empty box is one line tall; both halves of the autosize measure against
 *  this, so it has to stay the textarea's own line-height. */
const LINE_H = 16;

/** About six lines. The dock is short as well as narrow — a box that grew to
 *  ten would be most of the panel. */
const MAX_H = 96;

/**
 * The moment's timestamp, ticking itself.
 *
 * It reads the playhead out of a **ref** and re-renders only itself, once a
 * second. The player re-renders four times a second on `timeupdate` and
 * `TranscriptPanel` is memoised against exactly that — a `currentTime` prop
 * anywhere in the dock's props bag would throw the memo away on every one of
 * those frames and put the virtualised transcript back in the path of a
 * decoding video. Same shape as `Running`'s elapsed clock in `ChaptersPanel`.
 */
function MomentChip({
  atRef,
  on,
  onToggle,
}: {
  atRef: RefObject<number>;
  on: boolean;
  onToggle: () => void;
}) {
  const [at, setAt] = useState(() => atRef.current);
  useEffect(() => {
    const t = setInterval(() => setAt(atRef.current), 1000);
    return () => clearInterval(t);
  }, [atRef]);

  const Icon = on ? Camera : CameraSlash;
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          aria-pressed={on}
          onClick={onToggle}
          className={cn(
            "flex h-6 shrink-0 cursor-pointer items-center gap-1 rounded-full px-2 text-[11px] tabular-nums transition-colors",
            on
              ? "bg-brand/12 text-brand hover:bg-brand/20"
              : "text-muted-foreground hover:bg-accent hover:text-foreground",
          )}
        >
          <Icon size={12} className="shrink-0" />
          {fmtTime(at)}
        </button>
      </TooltipTrigger>
      <TooltipContent>
        {on
          ? "Sending the moment: this second, the frame, the last minute of transcript and the chapter"
          : "Sending the message on its own"}
      </TooltipContent>
    </Tooltip>
  );
}

/**
 * The dock's composer.
 *
 * **A sibling of `app/src/components/harness/Composer.tsx`, not a variant of
 * it.** The two boxes answer different questions. That one opens a
 * conversation and so carries the choices that are made once — the subject
 * scope above the box, the `@` menu that turns a filename into a library path
 * — and it has the room for them. This one is 300px wide beside a playing
 * video, and both of those are already answered: the lecture fixes the
 * subject, and the agent has been handed the recording folder in its
 * instructions (`docs/harness.md`), so a file is typed as a path. What it adds
 * instead is the moment, which the page composer has no playhead to build.
 * Threading a `compact` flag and four "not here" props through one component
 * would have made the shared one harder to read than both of them are apart.
 *
 * Enter sends, Shift+Enter breaks a line — bb's keys, and the same ones next
 * door. While a turn runs the box still takes a message: Rust queues it, so
 * stop and send sit side by side exactly as they do on the page.
 */
export function LectureChatComposer({
  providers,
  provider,
  providerLocked,
  model,
  reasoning,
  running,
  atRef,
  moment,
  onMoment,
  restore,
  onProvider,
  onModel,
  onReasoning,
  onSend,
  onStop,
}: {
  providers: PickerProvider[];
  provider: Provider;
  /** An open thread keeps its agent; only a new one can pick. */
  providerLocked: boolean;
  model: string | null;
  reasoning: string | null;
  running: boolean;
  /** The playhead, written by the player. Its identity never changes. */
  atRef: RefObject<number>;
  /** Whether the next message carries the moment. */
  moment: boolean;
  onMoment: (on: boolean) => void;
  /** Words stop dropped out of the queue, handed back to be typed over. The
   *  counter is what is watched: the same text twice is still two restores. */
  restore?: { text: string; n: number } | null;
  onProvider: (p: Provider) => void;
  onModel: (m: string | null) => void;
  onReasoning: (level: string | null) => void;
  onSend: (text: string) => void;
  onStop: () => void;
}) {
  const [text, setText] = useState("");
  const ref = useRef<HTMLTextAreaElement>(null);

  // Put back before anything already typed, because it was typed first.
  useEffect(() => {
    if (!restore) return;
    setText((t) => [restore.text, t].filter(Boolean).join("\n\n"));
    ref.current?.focus();
  }, [restore]);

  // The box grows to its content. A layout effect rather than the change
  // handler: at the moment a handler runs the textarea still holds the old
  // string, and measuring it then sizes the box to the wrong text.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.style.height = `${LINE_H}px`;
    el.style.height = `${Math.min(el.scrollHeight, MAX_H)}px`;
  }, [text]);

  const send = () => {
    const t = text.trim();
    if (!t) return;
    setText("");
    // Whether this goes out now or waits behind the running turn is Rust's
    // call, not this box's: it owns the queue and the order.
    onSend(t);
  };

  return (
    <div className="flex flex-col gap-2 rounded-lg border border-border bg-card px-2 py-2 shadow-sm transition-[border-color,box-shadow] focus-within:border-ring focus-within:ring-[3px] focus-within:ring-ring/25">
      <Textarea
        ref={ref}
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            send();
          }
        }}
        rows={1}
        placeholder={running ? "Working…" : "Ask about this lecture"}
        className="min-h-[16px] w-full resize-none rounded-none border-0 bg-transparent p-0 text-[12px]! leading-[16px] shadow-none focus-visible:border-0 focus-visible:ring-0 dark:bg-transparent"
        style={{ height: `${LINE_H}px`, maxHeight: MAX_H }}
      />
      <div className="flex items-center gap-1">
        <ModelPicker
          className="-ml-1 min-w-0 flex-1 justify-start"
          providers={providers}
          provider={provider}
          providerLocked={providerLocked}
          model={model}
          reasoning={reasoning}
          onProvider={onProvider}
          onModel={onModel}
          onReasoning={onReasoning}
        />
        <MomentChip atRef={atRef} on={moment} onToggle={() => onMoment(!moment)} />
        {running && (
          <Button size="icon-xs" variant="ghost" className="shrink-0" aria-label="Stop" onClick={onStop}>
            <Stop weight="fill" />
          </Button>
        )}
        {(!running || text.trim()) && (
          <Button
            size="icon-xs"
            disabled={!text.trim()}
            onClick={send}
            className="shrink-0"
            aria-label={running ? "Queue" : "Send"}
          >
            <PaperPlaneTilt />
          </Button>
        )}
      </div>
    </div>
  );
}
