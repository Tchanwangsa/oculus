import type { AgentQuestion, HarnessRequest, QuestionAnswer, QuestionPayload } from "./types";
import { isAnswerable } from "./requests";

/** Only question requests use this payload. Secrets never arrive from Rust. */
export function questionPayload(request: HarnessRequest): QuestionPayload | null {
  if (request.kind !== "question" || !Array.isArray(request.payload.questions)) return null;
  return request.payload as unknown as QuestionPayload;
}

export function waitingQuestions(requests: Record<number, HarnessRequest> | undefined): HarnessRequest[] {
  return Object.values(requests ?? {}).filter((r) => r.kind === "question" && isAnswerable(r)).sort((a, b) => a.id - b.id);
}

/** The same shape Respond validates; single choice takes selection or text. */
export function questionAnswer(question: AgentQuestion, draft: { selected: string[]; text: string; dismissed: boolean }): QuestionAnswer {
  return { questionId: question.id, answers: draft.dismissed ? [] : draft.text.trim() ? question.multi ? [...draft.selected, draft.text] : [draft.text] : draft.selected, dismissed: draft.dismissed };
}
