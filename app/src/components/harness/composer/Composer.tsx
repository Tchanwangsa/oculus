import { useEffect, useRef, useState, type ReactNode, type RefObject } from "react";
import { SendControls } from "@/components/harness/composer/SendControls";
import { MentionInput, type MentionInputHandle } from "@/components/harness/mentions/MentionInput";
import { ComposerMenu } from "@/components/harness/mentions/ComposerMenu";
import { useComposerTools } from "@/components/harness/composer/useComposerTools";
import { usePromptHistory } from "@/hooks/agents/usePromptHistory";
import { notePickedSkills } from "@/lib/harness/skills";
import { ModelPicker } from "@/components/harness/composer/ModelPicker";
import { UsageMeter } from "@/components/harness/composer/UsageMeter";
import { SubjectPicker } from "@/components/harness/composer/SubjectPicker";
import { ThreadBanners } from "@/components/harness/composer/ThreadBanners";
import { canSend, enterAction, UNSCOPED_PLACEHOLDER } from "@/components/harness/composer/sendGate";
import { type Subject } from "@/lib/db";
import {
  defaultSelection,
  providerLabel,
  scopeSubject,
  type Provider,
  type RateWindow,
  type ThreadUsage,
} from "@/lib/harness";
import { useProviderModels } from "@/hooks/agents/useProviderModels";
import { useAttachments } from "@/hooks/agents/useAttachments";
import { AttachmentStrip } from "@/components/harness/attachments/AttachmentStrip";
import { useDraftStore } from "@/stores/chat/draftStore";
import { signInState, useSignInStatus } from "@/hooks/agents/useSignInStatus";
import { SignInDialog, useSignIn } from "@/components/harness/SignInDialog";
import { useDictation } from "@/components/harness/dictation/useDictation";
import { DictationBar, DictationMic, DictationFeedback } from "@/components/harness/dictation/DictationControls";
import { cn } from "@/lib/utils";

/** The empty box's hint, naming its two menus. */
export const COMPOSER_PLACEHOLDER = "Ask anything. @ files, / skills";

/**
 * The chat box: text, then model picker / usage / send+stop. Enter sends,
 * Shift+Enter breaks. While a turn runs a send waits as a queued run
 * (`harness/store/queue.rs`); Stop holds that queue. A rewind hands its
 * question back as `restore`.
 * Scope is chosen above the box, only while the thread is new; a new Chat
 * thread chooses it in the headline (`DraftHero`) and, while it has none,
 * the box is `unscoped`: it keeps collecting but will not send. `@` mentions
 * insert library paths (drawn as chips by `MentionInput`), never file content.
 * Unsent text lives in `draftStore` under `draftKey`, so it outlives the box;
 * so do long pastes, held as cards in the attachment strip.
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
  unscoped = false,
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
  leading,
  placeholder,
  threadId = null,
}: {
  /** Whose draft this box shows (`draftKey` in `stores/chat/draftStore.ts`). */
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
  /** Given where the box picks its own scope (a chip above it, Studio's New
   *  video); Chat's new thread picks it in the headline instead. */
  onSubject?: (id: number | null) => void;
  subjectLocked: boolean;
  /** A new Chat thread with no subject or General chosen yet: Enter and the
   *  send button do nothing, and the placeholder says why. */
  unscoped?: boolean;
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
  /** Chips after the subject's, above the box (Studio's style chip). */
  leading?: ReactNode;
  /** The empty box's hint; a running turn's own wins. */
  placeholder?: string;
  /** The open thread, whose notices (`ThreadBanners`) sit above the box. */
  threadId?: number | null;
}) {
  /** The serialized message (chips as paths); emptiness checks read this. */
  const text = useDraftStore((s) => s.drafts[draftKey] ?? "");
  const setDraft = useDraftStore((s) => s.setDraft);
  const ref = useRef<MentionInputHandle>(null);
  const dictation = useDictation(draftKey, (transcript) => ref.current?.append(transcript));
  const tools = useComposerTools({ subjectId, input: ref });
  const mentions = tools.menu;
  /** The drop target for attachments: the whole box, not just the editor. */
  const wrapRef = useRef<HTMLDivElement>(null);
  /** Pasted/dropped pictures, in memory until send writes them to the library;
   *  long pastes as text cards. */
  const att = useAttachments(dropRef ?? wrapRef, draftKey);
  useEffect(() => onDropping?.(att.dropping), [att.dropping, onDropping]);
  const history = usePromptHistory({ threadId, input: ref, text });

  // Restored text goes before anything typed since, because it was typed first;
  // its pasted blocks go back to being cards.
  const takeBackRef = useRef(att.takeBack);
  takeBackRef.current = att.takeBack;
  useEffect(() => {
    if (!restore) return;
    const rest = takeBackRef.current(restore.text);
    if (rest) ref.current?.prepend(rest);
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

  const ready = text.trim().length > 0 || att.items.length > 0 || att.texts.length > 0;

  /** Send or queue. Pictures are written first; if that fails the box is kept as is. */
  const send = async () => {
    if (dictation.busy || unscoped) return;
    const message = await att.prepare(text);
    if (message == null) return;

    notePickedSkills(message, ref.current?.pickedSkills() ?? []);
    ref.current?.clear();
    mentions.close();
    // Rust decides send-now vs queue.
    onSend(message);
  };

  return (
    <div ref={wrapRef} className="flex flex-col gap-1.5">
      {((!subjectLocked && onSubject) || leading) && (
        <div className="flex items-center gap-1.5 px-0.5">
          {!subjectLocked && onSubject && (
            <SubjectPicker
              variant="chip"
              subjects={subjects}
              value={subjectId ?? "general"}
              onChange={(scope) => onSubject(scopeSubject(scope))}
            />
          )}
          {leading}
        </div>
      )}

      {/* Portals out and positions itself at the caret. */}
      <ComposerMenu {...mentions.menu} />

      <DictationFeedback dictation={dictation} />

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

      <ThreadBanners threadId={threadId} rateLimits={rateLimits} />

      <div
        className={cn(
          "flex flex-col gap-4 rounded-xl border border-border bg-card px-3 py-3 shadow-sm transition-[border-color,box-shadow] focus-within:border-ring focus-within:ring-[3px] focus-within:ring-ring/25",
          att.dropping && !dropRef && "border-brand ring-[3px] ring-brand/25",
        )}
      >
        <div className="flex flex-col gap-2">
          <AttachmentStrip
            items={att.items}
            onDetach={att.detach}
            texts={att.texts}
            onEditText={att.editText}
            onDetachText={att.detachText}
            onPutBack={(t) => ref.current?.append(t)}
            subjectId={subjectId}
          />
          {/* Keyed: the editor reads its text once, at mount. */}
          <MentionInput
            key={draftKey}
            ref={ref}
            initialText={text}
            skills={tools.skillNames}
            autoFocus={autoFocus}
            onEdit={(next, caret, masked) => {
              setDraft(draftKey, next);
              mentions.track(next, caret, masked);
            }}
            onFiles={att.attach}
            onPasteText={att.attachText}
            onBlur={mentions.close}
            onKeyDown={(e) => {
              // The menu's claimed keys come back prevented, so a pick never sends.
              mentions.keyDown(e);
              if (e.defaultPrevented) return;
              history.keyDown(e);
              if (e.defaultPrevented) return;
              const action = enterAction(e, unscoped);
              if (action) e.preventDefault();
              if (action === "send") void send();
            }}
            placeholder={
              running
                ? "Working… your next message waits its turn"
                : unscoped
                  ? UNSCOPED_PLACEHOLDER
                  : (placeholder ?? COMPOSER_PLACEHOLDER)
            }
          />
        </div>
        {dictation.busy ? <DictationBar dictation={dictation} /> : <div className="flex items-center gap-1">
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
            open={tools.modelOpen}
            onOpenChange={tools.setModelOpen}
          />
          <div className="flex-1" />
          <UsageMeter usage={usage} rateLimits={rateLimits} />
          <DictationMic dictation={dictation} />
          <SendControls running={running} ready={canSend({ ready, unscoped })} writing={att.writing} onSend={() => void send()} onStop={onStop} />
        </div>}
      </div>
    </div>
  );
}
