import { useEffect, useRef, useState, type RefObject } from "react";
import { Camera, CameraSlash } from "@phosphor-icons/react";

import { useDictation } from "@/components/harness/dictation/useDictation";
import { DictationBar, DictationMic, DictationFeedback } from "@/components/harness/dictation/DictationControls";
import { SendControls } from "@/components/harness/composer/SendControls";
import { MentionInput, type MentionInputHandle } from "@/components/harness/mentions/MentionInput";
import { MentionMenu } from "@/components/harness/mentions/MentionMenu";
import { useMentionMenu } from "@/components/harness/mentions/useMentionMenu";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { ModelPicker, type PickerProvider } from "@/components/harness/composer/ModelPicker";
import { UsageMeter } from "@/components/harness/composer/UsageMeter";
import { AttachmentStrip } from "@/components/harness/attachments/AttachmentStrip";
import { useAttachments } from "@/hooks/agents/useAttachments";
import type { Provider, RateWindow, ThreadUsage } from "@/lib/harness";
import { fmtClockSecs } from "@/lib/lectures/media";
import { cn } from "@/lib/utils";
import { useDraftStore } from "@/stores/chat/draftStore";
import { useTabActive } from "@/components/tabs/TabContext";

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
  const active = useTabActive();
  const [at, setAt] = useState(() => atRef.current);
  useEffect(() => {
    if (!active) return;
    setAt(atRef.current);
    const t = setInterval(() => setAt(atRef.current), 1000);
    return () => clearInterval(t);
  }, [atRef, active]);

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
          ? "Includes the current frame, the last minute of transcript and the chapter"
          : "Sending without the moment"}
      </TooltipContent>
    </Tooltip>
  );
}

/**
 * The dock's composer — a sibling of `components/harness/composer/Composer.tsx`, not a
 * variant: the lecture fixes the subject and the agent already has the
 * recording folder (`docs/harness.md`), so there is no scope picker, and it adds
 * the moment instead. `@` is the page composer's, fixed to the lecture's
 * subject (`MentionInput`, `useMentionMenu`). Attachments are shared outright
 * (`useAttachments`, `AttachmentStrip`).
 *
 * Enter sends, Shift+Enter breaks a line. While a turn runs a send is queued by
 * Rust, so stop and send sit side by side. Unsent text lives in `draftStore`,
 * and so do long pastes, held as cards in the attachment strip.
 */
export function LectureChatComposer({
  draftKey,
  subjectId,
  providers,
  provider,
  providerLocked,
  usage,
  rateLimits,
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
  dropRef,
  onDropping,
}: {
  /** Whose draft this box shows (`draftKey` in `stores/chat/draftStore.ts`). */
  draftKey: string;
  /** The lecture's subject, which `@` searches. */
  subjectId: number;
  providers: PickerProvider[];
  provider: Provider;
  /** An open thread keeps its agent; only a new one can pick. */
  providerLocked: boolean;
  usage: ThreadUsage | null;
  rateLimits: RateWindow[];
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
  /** A wider drop target than the box (the panel around it), which then
   *  draws its own overlay from `onDropping`. */
  dropRef?: RefObject<HTMLElement | null>;
  onDropping?: (over: boolean) => void;
}) {
  const text = useDraftStore((s) => s.drafts[draftKey] ?? "");
  const setDraft = useDraftStore((s) => s.setDraft);
  const ref = useRef<MentionInputHandle>(null);
  const mentions = useMentionMenu({ subjectId, input: ref });
  /** The drop target: the box and its attachment strip. */
  const wrapRef = useRef<HTMLDivElement>(null);
  /** Pictures pasted or dropped in, written on send. */
  const att = useAttachments(dropRef ?? wrapRef, draftKey, {
    notAPicture: "Only images can be attached — type @ for a course file.",
  });
  useEffect(() => onDropping?.(att.dropping), [att.dropping, onDropping]);

  /** Whether there is a message at all: words, pastes, pictures. */
  const ready = text.trim().length > 0 || att.items.length > 0 || att.texts.length > 0;

  // Put back before anything already typed, because it was typed first; its
  // pasted blocks go back to being cards.
  const takeBackRef = useRef(att.takeBack);
  takeBackRef.current = att.takeBack;
  useEffect(() => {
    if (!restore) return;
    const rest = takeBackRef.current(restore.text);
    if (rest) ref.current?.prepend(rest);
  }, [restore]);

  /** A text card's words back into the box, after what is typed. */
  const putBack = (pasted: string) => ref.current?.append(pasted);

  const dictation = useDictation(draftKey, putBack);

  // Pictures are written before the message is assembled; if that fails the
  // box is kept as it was.
  const send = async () => {
    if (dictation.busy) return;
    const message = await att.prepare(text);
    if (message == null) return;

    ref.current?.clear();
    mentions.close();
    // Rust decides whether this sends now or queues.
    onSend(message);
  };

  return (
    <div ref={wrapRef} className="flex flex-col gap-1.5">
      {/* Portals out and positions itself at the caret. */}
      <MentionMenu {...mentions.menu} />
      <DictationFeedback dictation={dictation} />
      {att.error && <div className="px-0.5 text-[11px] text-destructive">{att.error}</div>}

      <div
        className={cn(
          "flex flex-col gap-2 rounded-lg border border-border bg-card px-2 py-2 shadow-sm transition-[border-color,box-shadow] focus-within:border-ring focus-within:ring-[3px] focus-within:ring-ring/25",
          // Drag-over styling in focus's vocabulary, like the page composer.
          att.dropping && !dropRef && "border-brand ring-[3px] ring-brand/25",
        )}
      >
        <AttachmentStrip
          items={att.items}
          onDetach={att.detach}
          texts={att.texts}
          onEditText={att.editText}
          onDetachText={att.detachText}
          onPutBack={putBack}
          subjectId={subjectId}
          compact
        />
        {/* Keyed: the editor reads its text once, at mount. */}
        <MentionInput
          key={draftKey}
          ref={ref}
          initialText={text}
          onEdit={(next, caret) => {
            setDraft(draftKey, next);
            mentions.track(next, caret);
          }}
          onFiles={att.attach}
          onPasteText={att.attachText}
          onBlur={mentions.close}
          onKeyDown={(e) => {
            // The menu's claimed keys come back prevented, so a pick never sends.
            mentions.keyDown(e);
            if (e.defaultPrevented) return;
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              void send();
            }
          }}
          placeholder={running ? "Working…" : "Ask about this lecture"}
        />
        {dictation.busy ? <DictationBar dictation={dictation} /> : <div className="flex items-center gap-1">
          <ModelPicker
            className="-ml-1 min-w-0"
            providers={providers}
            provider={provider}
            providerLocked={providerLocked}
            model={model}
            reasoning={reasoning}
            onProvider={onProvider}
            onModel={onModel}
            onReasoning={onReasoning}
          />
          <div className="ml-auto flex shrink-0 items-center gap-1">
            <UsageMeter usage={usage} rateLimits={rateLimits} />
            <MomentChip atRef={atRef} on={moment} onToggle={() => onMoment(!moment)} />
            <DictationMic dictation={dictation} />
            <SendControls running={running} ready={ready} writing={att.writing}
              onSend={() => void send()} onStop={onStop} />
          </div>
        </div>}
      </div>
    </div>
  );
}
