import { ChatCircleText, ChatCircleSlash, Question } from "@phosphor-icons/react";
import { questionPayload, questionRowState, type AgentQuestion, type HarnessRequest } from "@/lib/harness";
import { RequestNotes, SettledRow } from "./SettledRow";

/**
 * A question request's row. Waiting, it is a card with what the agent
 * asked; the answer is given in the composer's `QuestionPanel`. Answered,
 * every question stays readable beside its answer, or "Dismissed". A
 * dismissed, expired or cancelled request is one quiet line opening to the
 * questions. Like an approval, it sits outside the run's work fold.
 */
export function QuestionView({ request }: { request: HarnessRequest }) {
  const state = questionRowState(request);
  const questions = questionPayload(request)?.questions ?? [];

  if (state.kind === "waiting") {
    return (
      <div
        data-request={request.id}
        data-request-state="waiting"
        className="flex items-start gap-2.5 rounded-xl border border-border bg-card px-3 py-2.5"
      >
        <Question size={14} weight="fill" className="mt-0.5 shrink-0 text-brand" aria-hidden />
        <div className="flex min-w-0 flex-1 flex-col gap-2">
          <div className="text-xs text-foreground">{state.title}</div>
          {questions.map((q) => (
            <Asked key={q.id} question={q} />
          ))}
          {/* The run's fold or the live tail says "Waiting for your answer". */}
          <RequestNotes notes={state.notes} warn={state.warn} />
        </div>
      </div>
    );
  }

  if (state.kind === "answered") {
    return (
      <div data-request={request.id} data-request-state="answered" className="flex min-w-0 items-start gap-1.5 px-2 py-0.5">
        <ChatCircleText size={14} className="mt-[3px] shrink-0 text-muted-foreground" aria-hidden />
        <div className="flex min-w-0 flex-1 flex-col gap-1.5 text-xs leading-5">
          <span className="text-muted-foreground">{state.title}</span>
          {state.entries.map((entry) => (
            <div key={entry.question.id} className="flex min-w-0 flex-col">
              <span data-selectable className="break-words text-muted-foreground">
                {entry.question.text}
              </span>
              {entry.dismissed ? (
                <span className="text-muted-foreground">Dismissed</span>
              ) : (
                <span data-selectable className="break-words font-medium text-foreground">
                  {entry.answers.join(", ")}
                </span>
              )}
            </div>
          ))}
          <RequestNotes notes={state.notes} warn={state.warn} />
        </div>
      </div>
    );
  }

  return (
    <SettledRow
      icon={ChatCircleSlash}
      state={state.kind}
      label={state.label}
      subject={questions[0]?.text ?? "A question"}
      title={state.title}
      notes={state.notes}
      warn={state.warn}
    >
      {questions.map((q) => (
        <Asked key={q.id} question={q} />
      ))}
    </SettledRow>
  );
}

/** One question as asked: its header, its text and the options offered. */
function Asked({ question }: { question: AgentQuestion }) {
  return (
    <div className="flex min-w-0 flex-col gap-0.5">
      {question.header && <span className="text-[11px] text-muted-foreground">{question.header}</span>}
      <p data-selectable className="break-words text-[12.5px] leading-snug text-foreground">
        {question.text}
      </p>
      {question.options.length > 0 && (
        <p className="break-words text-[11px] leading-relaxed text-muted-foreground">
          {question.options.map((o) => o.label).join(" · ")}
          {question.multi && " — choose any"}
        </p>
      )}
    </div>
  );
}
