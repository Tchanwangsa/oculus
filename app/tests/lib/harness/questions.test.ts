import { expect, test } from "bun:test";
import { questionAnswer, waitingQuestions } from "@/lib/harness/questions";
import { waitingRequests } from "@/lib/harness/requests";
import type { AgentQuestion, HarnessRequest } from "@/lib/harness/types";
const q: AgentQuestion = { id: "native", text: "Choose?", header: "Choice", options: [{ label: "Red", description: "Warm" }], multi: false, allowsTypedAnswer: true, secret: false };

test("answer values retain native ids and typed single/multi and dismissal semantics", () => {
  expect(questionAnswer(q, { selected: ["Red"], text: "typed", dismissed: false })).toEqual({ questionId: "native", answers: ["typed"], dismissed: false });
  expect(questionAnswer({ ...q, multi: true }, { selected: ["Red"], text: "typed", dismissed: false }).answers).toEqual(["Red", "typed"]);
  expect(questionAnswer(q, { selected: ["Red"], text: "typed", dismissed: true }).answers).toEqual([]);
});

test("oldest pending question is selected across live and message routes, separately from approvals", () => {
  const row = (id: number, kind: "approval" | "question", responder: "live" | "message", status = "pending") => ({ id, kind, responder, status }) as HarnessRequest;
  const requests = { 1: row(1, "approval", "live"), 2: row(2, "question", "message"), 3: row(3, "question", "live"), 4: row(4, "question", "live", "resolved") };
  expect(waitingQuestions(requests).map((r) => r.id)).toEqual([2, 3]);
  expect(waitingRequests(requests).map((r) => r.id)).toEqual([1]);
});
