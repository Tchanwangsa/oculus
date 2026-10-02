import { useEffect, useRef, useState, type RefObject } from "react";
import { SendControls } from "@/components/harness/SendControls";
import { MentionInput, type MentionInputHandle } from "@/components/harness/MentionInput";
import { MentionMenu } from "@/components/harness/MentionMenu";
import { useMentionMenu } from "@/components/harness/useMentionMenu";
import { ModelPicker } from "@/components/harness/ModelPicker";
import { UsageMeter } from "@/components/harness/UsageMeter";
import { SubjectSelect } from "@/components/harness/SubjectSelect";
import { type Subject } from "@/lib/db";
import {
  defaultSelection,
  providerLabel,
  type Provider,
  type RateWindow,
  type ThreadUsage,
} from "@/lib/harness";
import { useProviderModels } from "@/hooks/useProviderModels";
import { useAttachments } from "@/hooks/useAttachments";
import { AttachmentStrip } from "@/components/harness/AttachmentStrip";
import { useDraftStore } from "@/stores/draftStore";
import { signInState, useSignInStatus } from "@/hooks/useSignInStatus";
import { SignInDialog, useSignIn } from "@/components/harness/SignInDialog";
import { cn } from "@/lib/utils";

/**
 * The chat box: text, then model picker / usage / send+stop. Enter sends,
 * Shift+Enter breaks. While a turn runs a send is queued by Rust (`Queue` in
 * `app/src-tauri/src/harness/mod.rs`); Stop hands the queue back as `restore`.
 * Scope is chosen above the box, only while the thread is new. `@` mentions
 * insert library paths (drawn as chips by `MentionInput`), never file content.
 * Unsent text lives in `draftStore` under `draftKey`, so it outlives the box.
 */
export function Composer({
  draftKey,
  provider,
  model,
  reasoning,
  providerLocked,
  subjects,
  subjectId,
  onSubject,
  subjectLocked,
  running,
  usage,
  rateLimits,
  restore,
  onRestored,
  onProvider,
  onModel,
  onReasoning,
  onSend,
  onStop,
  autoFocus,
  dropRef,
  onDropping,
}: {
  /** Whose draft this box shows (`draftKey` in `stores/draftStore.ts`). */
  draftKey: string;
  provider: Provider;
  model: string | null;
  /** Reasoning effort for the next turn; null leaves the flag off. */
  reasoning: string | null;
  /** An open thread keeps its provider; only a new one can pick. */
  providerLocked: boolean;
  subjects: Subject[];
  /** null is the general thread — the whole library. */
  subjectId: number | null;
  onSubject: (id: number | null) => void;
  subjectLocked: boolean;
  running: boolean;
  usage: ThreadUsage | null;
  rateLimits: RateWindow[];
  /** What Stop dropped from the queue; `n` makes the same text twice two restores. */
  restore?: { text: string; n: number } | null;
  onRestored?: () => void;
  onProvider: (p: Provider) => void;
  onModel: (m: string | null) => void;
  onReasoning: (level: string | null) => void;
  onSend: (text: string) => void;
  onStop: () => void;
  autoFocus?: boolean;
  /** A wider drop target than the box (the page around it), which then draws
   *  its own overlay from `onDropping`. */
  dropRef?: RefObject<HTMLElement | null>;
  onDropping?: (over: boolean) => void;
}) {
  /** The serialized message (chips as paths); emptiness checks read this. */
  const text = useDraftStore((s) => s.drafts[draftKey] ?? "");
  const setDraft = useDraftStore((s) => s.setDraft);
  const ref = useRef<MentionInputHandle>(null);
  const mentions = useMentionMenu({ subjectId, input: ref });
  /** The drop target for attachments: the whole box, not just the editor. */
  const wrapRef = useRef<HTMLDivElement>(null);
  /** Pasted/dropped pictures, in memory until send writes them to the library. */
  const att = useAttachments(dropRef ?? wrapRef);
  useEffect(() => onDropping?.(att.dropping), [att.dropping, onDropping]);

  // Restored text goes before anything typed since, because it was typed first.
  useEffect(() => {
    if (!restore) return;
    ref.current?.prepend(restore.text);
    onRestored?.();
  }, [restore, onRestored]);

  // Only the selected provider's CLI is asked for its models.
  const { providers: pickerProviders } = useProviderModels(provider);

  // Warn only on a measured `out` (never `unknown`), and never disable sending:
  // the status is a cached read the student can change from a terminal.
  const { statuses, recheck } = useSignInStatus();
  const signedOut = signInState(statuses, provider) === "out";
  const [signInOpen, setSignInOpen] = useState(false);
  const signIn = useSignIn(recheck);

  // A turn always names an explicit model + level: fill an empty selection as
  // soon as the list arrives, rather than sending with none.
  const active = pickerProviders.find((p) => p.id === provider);
  useEffect(() => {
    if (model || !active || active.loading || active.models.length === 0) return;
    const pick = defaultSelection(active.models);
    if (!pick.model) return;
    onModel(pick.model);
    onReasoning(pick.reasoning);
  }, [model, active, onModel, onReasoning]);

  const ready = text.trim().length > 0 || att.items.length > 0;

  /** Send or queue. Pictures are written first; if that fails the box is kept as is. */
  const send = async () => {
    const message = await att.prepare(text);
    if (message == null) return;

    ref.current?.clear();
    mentions.close();
    // Rust decides send-now vs queue.
    onSend(message);
  };

  return (
    <div ref={wrapRef} className="flex flex-col gap-1.5">
      {!subjectLocked && (
        <div className="flex items-center px-0.5">
          <SubjectSelect
            subjects={subjects}
            value={subjectId}
            onChange={onSubject}
            className="h-6 max-w-[200px] rounded-full border-border/70 bg-card px-2 text-[11px] text-muted-foreground shadow-none hover:bg-accent hover:text-foreground"
          />
        </div>
      )}

      {/* Portals out and positions itself at the caret. */}
      <MentionMenu {...mentions.menu} />

      {att.error && (
        <div className="px-0.5 text-[11px] text-destructive">{att.error}</div>
      )}

      {signedOut && (
        <div className="flex items-center gap-1.5 px-0.5 text-[11px] text-muted-foreground">
          <span>{providerLabel(provider)} is signed out.</span>
          <button
            type="button"
            onClick={() => setSignInOpen(true)}
            className="cursor-pointer text-brand underline-offset-2 hover:underline"
          >
            Sign in
          </button>
        </div>
      )}

      {signInOpen && (
        <SignInDialog
          provider={provider}
          run={signIn.run?.provider === provider ? signIn.run : null}
          onStart={() => signIn.start(provider)}
          onCode={(code) => signIn.submitCode(code)}
          onCancel={() => signIn.cancel()}
          onClose={() => {
            if (signIn.run?.result) signIn.clear();
            setSignInOpen(false);
          }}
        />
      )}

      <div
        className={cn(
          "flex flex-col gap-4 rounded-xl border border-border bg-card px-3 py-3 shadow-sm transition-[border-color,box-shadow] focus-within:border-ring focus-within:ring-[3px] focus-within:ring-ring/25",
          att.dropping && !dropRef && "border-brand ring-[3px] ring-brand/25",
        )}
      >
        <div className="flex flex-col gap-2">
          <AttachmentStrip items={att.items} onDetach={att.detach} />
          {/* Keyed: the editor reads its text once, at mount. */}
          <MentionInput
            key={draftKey}
            ref={ref}
            initialText={text}
            autoFocus={autoFocus}
            onEdit={(next, caret) => {
              setDraft(draftKey, next);
              mentions.track(next, caret);
            }}
            onFiles={att.attach}
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
            placeholder={
              running ? "Working… your next message waits its turn" : "What would you like to work on?"
            }
          />
        </div>
        <div className="flex items-center gap-1">
          <ModelPicker
            className="-ml-1.5"
            providers={pickerProviders}
            provider={provider}
            providerLocked={providerLocked}
            model={model}
            reasoning={reasoning}
            onProvider={onProvider}
            onModel={onModel}
            onReasoning={onReasoning}
          />
          <div className="flex-1" />
          <UsageMeter usage={usage} rateLimits={rateLimits} />
          <SendControls running={running} ready={ready} writing={att.writing} onSend={() => void send()} onStop={onStop} />
        </div>
      </div>
    </div>
  );
}
