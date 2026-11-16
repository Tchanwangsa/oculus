import type { AgentQuestion, HarnessRequest, QuestionAnswer, QuestionPayload } from "./types";
import { isAnswerable } from "./requests";
import { providerLabel } from "./registry";

/**
 * What the question panel, the composer and a question's timeline row decide
 * for an agent's question (docs/harness.md "Questions keep their native
 * keys"). Rust validates every submission; these only choose what to show
 * and what to send, so each is testable without a DOM.
 */

/** One question's unsubmitted answer (`harnessStore.questionDrafts`). */
export interface AnswerDraft {
  selected: string[];
  text: string;
  dismissed: boolean;
}

export const EMPTY_DRAFT: AnswerDraft = { selected: [], text: "", dismissed: false };

/** Only question requests use this payload. Secrets never arrive from Rust. */
export function questionPayload(request: HarnessRequest): QuestionPayload | null {
  if (request.kind !== "question" || !Array.isArray(request.payload.questions)) return null;
  return request.payload as unknown as QuestionPayload;
}

export function waitingQuestions(requests: Record<number, HarnessRequest> | undefined): HarnessRequest[] {
  return Object.values(requests ?? {}).filter((r) => r.kind === "question" && isAnswerable(r)).sort((a, b) => a.id - b.id);
}

/** The same shape Respond validates; single choice takes selection or text. */
export function questionAnswer(question: AgentQuestion, draft: AnswerDraft): QuestionAnswer {
  const typed = draft.text.trim();
  const answers = draft.dismissed ? [] : typed ? (question.multi ? [...draft.selected, typed] : [typed]) : draft.selected;
  return { questionId: question.id, answers, dismissed: draft.dismissed };
}

/** Whether a question has what Respond needs: a dismissal, or a value. */
export function questionReady(question: AgentQuestion, draft: AnswerDraft | undefined): boolean {
  if (!draft) return false;
  if (draft.dismissed) return true;
  if (draft.text.trim() && question.allowsTypedAnswer) return true;
  return draft.selected.length > 0;
}

/** Where the panel opens: the first question still to answer, so a chat
 *  switch or a reload comes back to the same step. */
export function firstOpenQuestion(questions: AgentQuestion[], drafts: Record<string, AnswerDraft> | undefined): number {
  const i = questions.findIndex((q) => !questionReady(q, drafts?.[q.id]));
  return i < 0 ? Math.max(0, questions.length - 1) : i;
}

/** An option picked by click or digit. Single choice replaces the selection
 *  and hands back anything typed (`moved`, for the message draft), then
 *  advances; multiple choice toggles and keeps the typed text beside it. */
export function pickOption(
  question: AgentQuestion,
  draft: AnswerDraft,
  label: string,
): { draft: AnswerDraft; moved: string | null; advance: boolean } {
  if (question.multi) {
    const selected = draft.selected.includes(label)
      ? draft.selected.filter((s) => s !== label)
      : question.options.map((o) => o.label).filter((l) => l === label || draft.selected.includes(l));
    return { draft: { ...draft, selected, dismissed: false }, moved: null, advance: false };
  }
  const moved = draft.text.trim() ? draft.text : null;
  return { draft: { selected: [label], text: "", dismissed: false }, moved, advance: true };
}

/** Text typed in the composer while a question waits. On single choice a
 *  typed answer replaces the picked option, so nothing shows as chosen. */
export function typeAnswer(question: AgentQuestion, draft: AnswerDraft, text: string): AnswerDraft {
  const selected = !question.multi && text.trim() ? [] : draft.selected;
  return { ...draft, text, selected, dismissed: false };
}

/** The option a digit key 1–9 names, or null. */
export function digitOption(key: string, question: AgentQuestion): string | null {
  if (!/^[1-9]$/.test(key)) return null;
  return question.options[Number(key) - 1]?.label ?? null;
}

/** Digits pick options only when focus is not somewhere text is typed. */
export function typingTarget(el: { tagName?: string; isContentEditable?: boolean } | null | undefined): boolean {
  if (!el) return false;
  const tag = el.tagName?.toUpperCase();
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || el.isContentEditable === true;
}

/** What the request's answer sends: every question once in provider order;
 *  every one dismissed is the whole request dismissed. Null until each
 *  question is ready. */
export function questionSubmission(
  questions: AgentQuestion[],
  drafts: Record<string, AnswerDraft> | undefined,
): { option: "answer" | "dismiss"; answers: QuestionAnswer[] } | null {
  if (!questions.every((q) => questionReady(q, drafts?.[q.id]))) return null;
  if (questions.every((q) => drafts?.[q.id]?.dismissed)) return { option: "dismiss", answers: [] };
  return {
    option: "answer",
    answers: questions.map((q) => {
      const draft = drafts![q.id];
      // A choice-only question never sends typed text, whatever was kept.
      return questionAnswer(q, q.allowsTypedAnswer ? draft : { ...draft, text: "" });
    }),
  };
}

/** The panel's main button: Next until the last question, then the answer,
 *  which on a lost session goes as a new message. */
export function questionPrimaryLabel(index: number, count: number, responder: HarnessRequest["responder"]): string {
  if (index < count - 1) return "Next";
  if (responder === "message") return "Answer in a new message";
  return count > 1 ? "Send answers" : "Send answer";
}

/** The composer's hint while a question waits. */
export function questionPlaceholder(question: AgentQuestion): string {
  return question.allowsTypedAnswer ? "Type your answer" : "Choose an option above";
}

/** What a run waits on: an approval outranks a question, as the sidebar's
 *  Approval outranks Input. */
export function runWaitingOn(
  requests: Record<number, HarnessRequest> | undefined,
  runId?: number,
): "approval" | "answer" | null {
  let answer = false;
  for (const r of Object.values(requests ?? {})) {
    if (!isAnswerable(r) || (runId !== undefined && r.run_id !== runId)) continue;
    if (r.kind === "approval") return "approval";
    answer = true;
  }
  return answer ? "answer" : null;
}

/** Said in place of a waiting run's clock, steady like it: nothing shimmers
 *  while the student is the one to act. */
export const WAITING_LABEL = { approval: "Waiting for approval", answer: "Waiting for your answer" } as const;

/** One question as its timeline row shows it once answered. */
export interface AnsweredEntry {
  question: AgentQuestion;
  answers: string[];
  dismissed: boolean;
}

/** How a question request stands, for its timeline row. */
export interface QuestionRowState {
  kind: "waiting" | "answered" | "dismissed" | "expired" | "cancelled";
  /** "Codex asks", "Claude Code asks 2 questions". */
  title: string;
  label: string;
  notes: string[];
  warn: boolean;
  entries: AnsweredEntry[];
}

export function questionRowState(request: HarnessRequest): QuestionRowState {
  const who = providerLabel(request.provider);
  const questions = questionPayload(request)?.questions ?? [];
  const title = questions.length > 1 ? `${who} asks ${questions.length} questions` : `${who} asks`;
  const uncertain = request.delivery === "uncertain";
  const uncertainNote = `Delivery uncertain: the connection to ${who} broke while your answer was sent, so it may not have arrived. It was not sent again.`;
  const base = { title, warn: uncertain, entries: [] as AnsweredEntry[] };
  if (request.status === "pending") {
    const notes: string[] = [];
    if (request.responder === "message") {
      notes.push(`${request.reason ?? "The agent that asked has gone."} Your answer goes to ${who} as a new message.`);
    }
    if (request.delivery === "failed") notes.push(`Your answer did not reach ${who}. Answer again.`);
    return { ...base, kind: "waiting", label: WAITING_LABEL.answer, notes, warn: request.delivery === "failed" };
  }
  if (request.status === "expired" || request.status === "cancelled") {
    return {
      ...base,
      kind: request.status,
      label: request.status === "expired" ? "Expired" : "Cancelled",
      notes: [request.reason ?? (request.status === "expired" ? "It can no longer be answered." : "It was cancelled."),
        ...(uncertain ? [uncertainNote] : [])],
    };
  }
  const notes: string[] = [];
  if (request.delivery === "sending") notes.push(`Sending your answer to ${who}…`);
  if (uncertain) notes.push(uncertainNote);
  if (request.response === "dismiss") return { ...base, kind: "dismissed", label: "Dismissed", notes };
  const byId = new Map((request.answers ?? []).map((a) => [a.questionId, a]));
  const entries = questions.map((question) => {
    const a = byId.get(question.id);
    return { question, answers: a?.answers ?? [], dismissed: a ? a.dismissed : true };
  });
  return { ...base, kind: "answered", label: "Answered", notes, entries };
}
