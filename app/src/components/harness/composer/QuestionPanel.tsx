import { useEffect, useRef } from "react";
import { CheckSquare, Circle, Question, RadioButton, Square } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { useTabActive, useTabId } from "@/components/tabs/TabContext";
import { activePane } from "@/stores/shell/tabStore";
import { digitOption, typingTarget, type AgentQuestion, type AnswerDraft } from "@/lib/harness";
import { cn } from "@/lib/utils";
import type { QuestionComposer } from "./useQuestionComposer";

/**
 * An agent's question, on top of the composer: one question at a time with
 * its "n/m" count and header, radio rows (single choice) or checkbox rows
 * (multiple), each with its description. Digits 1–9 pick while focus is not
 * where text is typed; a typed answer goes in the composer below. Rust's
 * refusal is said inside the panel, never in a toast. The flow is
 * `useQuestionComposer`'s; this only draws it.
 */
export function QuestionPanel({ flow }: { flow: QuestionComposer }) {
  const root = useRef<HTMLDivElement>(null);
  const active = useTabActive();
  const paneId = useTabId();
  const latest = useRef(flow);
  latest.current = flow;

  // Window-wide, as T3's, but only in the focused pane of the active tab and
  // only when focus is on nothing in particular or inside this panel, so a
  // second pane's panel or a field elsewhere never takes the digit.
  useEffect(() => {
    if (!active) return;
    const onKey = (e: globalThis.KeyboardEvent) => {
      if (e.defaultPrevented || e.metaKey || e.ctrlKey || e.altKey || e.repeat) return;
      if (!paneTakesDigit(paneId, activePane()?.id)) return;
      const target = e.target as HTMLElement | null;
      if (typingTarget(target)) return;
      const focus = document.activeElement;
      const here = focus == null || focus === document.body || !!root.current?.contains(focus);
      if (!here) return;
      const f = latest.current;
      const label = digitOption(e.key, f.question);
      if (!label) return;
      e.preventDefault();
      f.pick(label);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [active, paneId]);

  return (
    <QuestionPanelView
      rootRef={root}
      question={flow.question}
      draft={flow.draft}
      index={flow.index}
      count={flow.questions.length}
      ready={flow.ready}
      busy={flow.busy}
      error={flow.error}
      moved={flow.moved}
      primaryLabel={flow.primaryLabel}
      onPick={flow.pick}
      onBack={flow.back}
      onPrimary={flow.primary}
      onDismiss={flow.dismiss}
    />
  );
}

/** Whether this pane may take a digit: only the focused pane does, so two
 *  split panes with waiting questions never both answer. Outside any pane
 *  (id 0) the panel always may. */
export function paneTakesDigit(paneId: number, focusedPaneId: number | undefined): boolean {
  return paneId === 0 || focusedPaneId === paneId;
}

/** The panel as drawn, given its state (the WebKit checks render it alone). */
export function QuestionPanelView({
  rootRef,
  question,
  draft,
  index,
  count,
  ready,
  busy,
  error,
  moved,
  primaryLabel,
  onPick,
  onBack,
  onPrimary,
  onDismiss,
}: {
  rootRef?: React.Ref<HTMLDivElement>;
  question: AgentQuestion;
  draft: AnswerDraft;
  index: number;
  count: number;
  ready: boolean;
  busy: boolean;
  error: string | null;
  moved: boolean;
  primaryLabel: string;
  onPick: (label: string) => void;
  onBack: () => void;
  onPrimary: () => void;
  onDismiss: () => void;
}) {
  const typed = question.allowsTypedAnswer && draft.text.trim().length > 0;
  return (
    <div
      ref={rootRef}
      data-question-panel={question.id}
      className="flex flex-col gap-2 rounded-xl border border-border bg-card px-3 py-2.5"
    >
      <div className="flex min-w-0 items-center gap-2">
        <Question size={14} weight="fill" className="shrink-0 text-brand" aria-hidden />
        {question.header && (
          <span className="min-w-0 truncate text-[11px] font-medium text-muted-foreground" title={question.header}>
            {question.header}
          </span>
        )}
        {count > 1 && (
          <span className="ml-auto shrink-0 text-[11px] tabular-nums text-muted-foreground">
            {index + 1}/{count}
          </span>
        )}
      </div>
      <p data-selectable className="break-words text-[13px] leading-snug text-foreground">
        {question.text}
      </p>
      {question.options.length > 0 && (
        <div
          role={question.multi ? "group" : "radiogroup"}
          aria-label={question.text}
          className="-mx-1.5 flex flex-col"
        >
          {question.options.map((option, i) => {
            const on = draft.selected.includes(option.label) && !draft.dismissed;
            const Mark = question.multi ? (on ? CheckSquare : Square) : on ? RadioButton : Circle;
            return (
              <button
                key={option.label}
                type="button"
                role={question.multi ? "checkbox" : "radio"}
                aria-checked={on}
                disabled={busy}
                onClick={() => onPick(option.label)}
                className="flex w-full cursor-default items-start gap-2 rounded-lg px-1.5 py-1.5 text-left outline-none transition-colors hover:bg-accent focus-visible:bg-accent disabled:opacity-60"
              >
                <Mark
                  size={14}
                  weight={on ? "fill" : "regular"}
                  className={cn("mt-px shrink-0", on ? "text-brand" : "text-muted-foreground")}
                  aria-hidden
                />
                <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                  <span className="break-words text-xs text-foreground">{option.label}</span>
                  {option.description && (
                    <span className="break-words text-[11px] leading-relaxed text-muted-foreground">
                      {option.description}
                    </span>
                  )}
                </span>
                {i < 9 && (
                  <kbd className="mt-px shrink-0 rounded border border-border bg-surface px-1 py-0.5 text-[10px] leading-none tabular-nums text-muted-foreground">
                    {i + 1}
                  </kbd>
                )}
              </button>
            );
          })}
        </div>
      )}
      {typed && !question.multi && (
        <p className="text-[11px] text-muted-foreground">Your answer is what you typed below.</p>
      )}
      {moved && (
        <p className="text-[11px] text-muted-foreground">What you typed was moved to your message draft.</p>
      )}
      {error && (
        <p role="alert" className="break-words text-[11px] leading-relaxed text-destructive">
          {error}
        </p>
      )}
      <div className="flex flex-wrap items-center gap-x-2 gap-y-1.5">
        <button
          type="button"
          disabled={busy}
          onClick={onDismiss}
          className="cursor-pointer text-[11px] text-muted-foreground underline-offset-2 hover:text-foreground hover:underline disabled:cursor-default"
        >
          Dismiss question without answering
        </button>
        <div className="ml-auto flex shrink-0 items-center gap-1.5">
          {index > 0 && (
            <Button size="xs" variant="ghost" disabled={busy} onClick={onBack} className="will-change-[opacity]">
              Back
            </Button>
          )}
          <Button size="xs" disabled={!ready || busy} onClick={onPrimary} className="will-change-[opacity]">
            {primaryLabel}
          </Button>
        </div>
      </div>
    </div>
  );
}
