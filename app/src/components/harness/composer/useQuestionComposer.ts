import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { useShallow } from "zustand/react/shallow";
import {
  EMPTY_DRAFT,
  firstOpenQuestion,
  harnessRespond,
  pickOption,
  questionPayload,
  questionPlaceholder,
  questionPrimaryLabel,
  questionReady,
  questionSubmission,
  typeAnswer,
  waitingQuestions,
  type AgentQuestion,
  type AnswerDraft,
  type HarnessRequest,
  type QuestionAnswer,
} from "@/lib/harness";
import { useDraftStore } from "@/stores/chat/draftStore";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { useRunCommand } from "./useRunCommand";

/** How long a single choice shows as picked before the next question. */
export const ADVANCE_MS = 200;

const NO_DRAFTS: Record<string, AnswerDraft> = {};

/** The oldest waiting question and everything the panel and the composer
 *  need to answer it; null when none waits. */
export interface QuestionComposer {
  request: HarnessRequest;
  questions: AgentQuestion[];
  index: number;
  question: AgentQuestion;
  draft: AnswerDraft;
  /** The current question has an answer or a dismissal. */
  ready: boolean;
  /** The editor's key: a question's typed answer is its own editor, so the
   *  message draft under `draftKey` is never touched. */
  editorKey: string;
  placeholder: string;
  primaryLabel: string;
  busy: boolean;
  error: string | null;
  /** Typed text a single choice handed back to the message draft. */
  moved: boolean;
  pick: (label: string) => void;
  type: (text: string) => void;
  back: () => void;
  primary: () => void;
  dismiss: () => void;
  /** The editor's Enter: the answer, never a message. */
  keyDown: (e: KeyboardEvent<HTMLElement>) => void;
}

/**
 * The question flow behind `QuestionPanel` and both composers (Chat and the
 * lecture dock): one question at a time, the oldest waiting request first.
 * Drafts live in `harnessStore.questionDrafts`, so a chat switch or reload
 * returns to the same step. One `harnessRespond` sends the whole request; a
 * refusal stays in `error` for the panel to say.
 */
export function useQuestionComposer(threadId: number | null, draftKey: string): QuestionComposer | null {
  const waiting = useHarnessStore(useShallow((s) => (threadId == null ? [] : waitingQuestions(s.requests[threadId]))));
  // Answered here, until its `request_updated` replaces the object (a failed
  // delivery comes back pending, to answer again).
  const [answered, setAnswered] = useState<readonly HarnessRequest[]>([]);
  const request = waiting.find((r) => !answered.includes(r)) ?? null;
  const questions = (request && questionPayload(request)?.questions) || [];
  const drafts = useHarnessStore((s) =>
    threadId == null || !request ? NO_DRAFTS : (s.questionDrafts[threadId]?.[request.id] ?? NO_DRAFTS),
  );
  const setQuestionDraft = useHarnessStore((s) => s.setQuestionDraft);

  const [step, setStep] = useState<{ request: number; index: number } | null>(null);
  const [nonce, setNonce] = useState(0);
  const [moved, setMoved] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const send = useRunCommand();
  const { clear } = send;

  const requestId = request?.id ?? null;
  const opening = firstOpenQuestion(questions, drafts);
  // A new request opens at its first unanswered question, and its own error.
  useEffect(() => {
    if (requestId == null) return;
    setStep({ request: requestId, index: opening });
    setMoved(false);
    clear();
    // `opening` is read once per request: picks must not move the step.
  }, [requestId, clear]);
  useEffect(() => () => {
    if (timer.current) clearTimeout(timer.current);
  }, [requestId]);

  const respond = (target: HarnessRequest, option: "answer" | "dismiss", answers: QuestionAnswer[]) =>
    void send.run(async () => {
      await harnessRespond(target.thread_id, target.id, option, answers);
      setAnswered((s) => [...s.filter((r) => waiting.includes(r)), target]);
    });

  if (!request || threadId == null || questions.length === 0) return null;
  const count = questions.length;
  const index = Math.min(step?.request === request.id ? step.index : opening, count - 1);
  const question = questions[index];
  const draft = drafts[question.id] ?? EMPTY_DRAFT;
  const ready = questionReady(question, draft);
  const goTo = (i: number) => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = null;
    setStep({ request: request.id, index: Math.max(0, Math.min(i, count - 1)) });
  };
  const save = (q: AgentQuestion, next: AnswerDraft) => setQuestionDraft(threadId, request.id, q.id, next);

  const submit = (all: Record<string, AnswerDraft>) => {
    const out = questionSubmission(questions, all);
    if (!out) {
      goTo(firstOpenQuestion(questions, all));
      return;
    }
    respond(request, out.option, out.answers);
  };

  const primary = () => {
    if (send.pending || !ready) return;
    if (index < count - 1) goTo(index + 1);
    else submit(drafts);
  };

  return {
    request,
    questions,
    index,
    question,
    draft,
    ready,
    editorKey: `question:${request.id}:${question.id}:${nonce}`,
    placeholder: questionPlaceholder(question),
    primaryLabel: questionPrimaryLabel(index, count, request.responder),
    busy: send.pending,
    error: send.error,
    moved,
    pick: (label) => {
      if (send.pending) return;
      const picked = pickOption(question, draft, label);
      save(question, picked.draft);
      if (picked.moved) {
        // T3's rule: a typed answer is not thrown away; it becomes the message.
        const store = useDraftStore.getState();
        const existing = store.drafts[draftKey] ?? "";
        store.setDraft(draftKey, existing.trim() ? `${existing}\n\n${picked.moved}` : picked.moved);
        setMoved(true);
        setNonce((n) => n + 1);
      }
      if (picked.advance && index < count - 1) {
        if (timer.current) clearTimeout(timer.current);
        const at = request.id;
        timer.current = setTimeout(() => {
          timer.current = null;
          setStep((s) => (s && s.request === at && s.index === index ? { request: at, index: index + 1 } : s));
        }, ADVANCE_MS);
      }
    },
    type: (text) => {
      if (!question.allowsTypedAnswer) return;
      save(question, typeAnswer(question, draft, text));
    },
    back: () => goTo(index - 1),
    primary,
    dismiss: () => {
      if (send.pending) return;
      if (count === 1) {
        respond(request, "dismiss", []);
        return;
      }
      const next = { ...draft, dismissed: true };
      save(question, next);
      if (index < count - 1) goTo(index + 1);
      else submit({ ...drafts, [question.id]: next });
    },
    keyDown: (e) => {
      if (e.key !== "Enter" || e.shiftKey || e.nativeEvent.isComposing) return;
      e.preventDefault();
      primary();
    },
  };
}
