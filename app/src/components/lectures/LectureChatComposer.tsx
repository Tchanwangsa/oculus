import { useEffect, useLayoutEffect, useRef, useState, type RefObject } from "react";
import { Camera, CameraSlash, PaperPlaneTilt, Stop } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { ModelPicker, type PickerProvider } from "@/components/harness/ModelPicker";
import { AttachmentStrip } from "@/components/harness/AttachmentStrip";
import { useAttachments } from "@/hooks/useAttachments";
import { imageFiles, withAttachments } from "@/lib/attachments";
import type { Provider } from "@/lib/harness";
import { fmtClockSecs } from "@/lib/lectures";
import { cn } from "@/lib/utils";

/** The empty box's height; must stay the textarea's own line-height. */
const LINE_H = 16;

/** About six lines: the dock is short as well as narrow. */
const MAX_H = 96;

/**
 * The moment's timestamp. Reads the playhead from a ref and re-renders only
 * itself, once a second, so no `currentTime` prop breaks `TranscriptPanel`'s memo.
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
          {fmtClockSecs(at)}
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
 * The dock's composer — a sibling of `components/harness/Composer.tsx`, not a
 * variant: the lecture fixes the subject and the agent already has the
 * recording folder (`docs/harness.md`), so there is no scope picker or `@` menu,
 * and it adds the moment instead. Attachments are shared outright
 * (`useAttachments`, `AttachmentStrip`).
 *
 * Enter sends, Shift+Enter breaks a line. While a turn runs a send is queued by
 * Rust, so stop and send sit side by side.
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
  /** Words a stop handed back. Watched by `n`: the same text twice is still
   *  two restores. */
  restore?: { text: string; n: number } | null;
  onProvider: (p: Provider) => void;
  onModel: (m: string | null) => void;
  onReasoning: (level: string | null) => void;
  onSend: (text: string) => void;
  onStop: () => void;
}) {
  const [text, setText] = useState("");
  const ref = useRef<HTMLTextAreaElement>(null);
  /** The drop target: the box and its attachment strip. */
  const wrapRef = useRef<HTMLDivElement>(null);
  /** Pictures pasted or dropped in, written on send. No `@` menu here, so the
   *  refusal says to type a path. */
  const att = useAttachments(wrapRef, {
    notAPicture: "Only images can be attached — type a path for a course file.",
  });

  /** Whether there is a message at all: words, pictures, or both. */
  const ready = text.trim().length > 0 || att.items.length > 0;

  // Put back before anything already typed, because it was typed first.
  useEffect(() => {
    if (!restore) return;
    setText((t) => [restore.text, t].filter(Boolean).join("\n\n"));
    ref.current?.focus();
  }, [restore]);

  // Autosize in a layout effect: in the change handler the textarea still holds
  // the old string.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.style.height = `${LINE_H}px`;
    el.style.height = `${Math.min(el.scrollHeight, MAX_H)}px`;
  }, [text]);

  // Pictures are written before the message is assembled; if that fails the
  // box is kept as it was.
  const send = async () => {
    const t = text.trim();
    if ((!t && !att.items.length) || att.writing) return;

    const paths = await att.flush();
    if (!paths) return;

    setText("");
    // Rust decides whether this sends now or queues.
    onSend(withAttachments(t, paths));
  };

  return (
    <div ref={wrapRef} className="flex flex-col gap-1.5">
      {att.error && <div className="px-0.5 text-[11px] text-destructive">{att.error}</div>}

      <div
        className={cn(
          "flex flex-col gap-2 rounded-lg border border-border bg-card px-2 py-2 shadow-sm transition-[border-color,box-shadow] focus-within:border-ring focus-within:ring-[3px] focus-within:ring-ring/25",
          // Drag-over styling in focus's vocabulary, like the page composer.
          att.dropping && "border-brand ring-[3px] ring-brand/25",
        )}
      >
        <AttachmentStrip items={att.items} onDetach={att.detach} compact />
        {/* `overflow-x-hidden`: with classic scrollbars WebKit paints a
            horizontal bar across a one-line field. */}
        <Textarea
          ref={ref}
          value={text}
          onChange={(e) => setText(e.target.value)}
          onPaste={(e) => {
            // A picture on the clipboard wins over its text flavour; plain text
            // falls through to the native paste (keeps undo).
            const pictures = imageFiles(e.clipboardData.files);
            if (!pictures.length) return;
            e.preventDefault();
            att.attach(pictures);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              void send();
            }
          }}
          rows={1}
          placeholder={running ? "Working…" : "Ask about this lecture"}
          className="min-h-[16px] w-full resize-none overflow-x-hidden overflow-y-auto rounded-none border-0 bg-transparent p-0 text-[12px]! leading-[16px] shadow-none focus-visible:border-0 focus-visible:ring-0 dark:bg-transparent"
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
          {(!running || ready) && (
            <Button
              size="icon-xs"
              disabled={!ready || att.writing}
              onClick={() => void send()}
              className="shrink-0"
              aria-label={running ? "Queue" : "Send"}
            >
              <PaperPlaneTilt />
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}
