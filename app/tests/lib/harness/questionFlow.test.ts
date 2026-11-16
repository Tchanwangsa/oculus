import { describe, expect, test } from "bun:test";
import {
  EMPTY_DRAFT,
  digitOption,
  firstOpenQuestion,
  pickOption,
  questionPlaceholder,
  questionPrimaryLabel,
  questionReady,
  questionRowState,
  questionSubmission,
  runWaitingOn,
  typeAnswer,
  typingTarget,
} from "@/lib/harness/questions";
import type { AgentQuestion, HarnessRequest } from "@/lib/harness/types";

const colour: AgentQuestion = {
  id: "colour",
  header: "Colour",
  text: "Which colour?",
  options: [
    { label: "Red", description: "Warm" },
    { label: "Blue", description: "Cool" },
    { label: "Green", description: "" },
  ],
  multi: false,
  allowsTypedAnswer: true,
  secret: false,
};
const toppings: AgentQuestion = { ...colour, id: "toppings", header: "Toppings", text: "Which toppings?", multi: true };
const strict: AgentQuestion = { ...colour, id: "strict", allowsTypedAnswer: false };

const request = (patch: Partial<HarnessRequest> = {}): HarnessRequest => ({
  id: 7, thread_id: 1, run_id: 2, item_id: 30, provider: "codex", kind: "question", action: "question",
  payload: { questions: [colour, toppings] }, options: [], status: "pending", responder: "live", reason: null,
  response: null, answers: null, delivery: "none", created_at: "2026-11-16 00:00:00", resolved_at: null, ...patch,
});

describe("digit keys", () => {
  test("1–9 name the options in order, and nothing past the list", () => {
    expect(digitOption("1", colour)).toBe("Red");
    expect(digitOption("3", colour)).toBe("Green");
    expect(digitOption("4", colour)).toBeNull();
    expect(digitOption("0", colour)).toBeNull();
    expect(digitOption("a", colour)).toBeNull();
    expect(digitOption("12", colour)).toBeNull();
  });

  test("are ignored where text is typed", () => {
    expect(typingTarget({ tagName: "input" })).toBe(true);
    expect(typingTarget({ tagName: "TEXTAREA" })).toBe(true);
    expect(typingTarget({ tagName: "SELECT" })).toBe(true);
    expect(typingTarget({ tagName: "DIV", isContentEditable: true })).toBe(true);
    expect(typingTarget({ tagName: "BUTTON", isContentEditable: false })).toBe(false);
    expect(typingTarget({ tagName: "BODY" })).toBe(false);
    expect(typingTarget(null)).toBe(false);
  });
});

describe("picking", () => {
  test("single choice replaces the pick, advances and hands back typed text", () => {
    const first = pickOption(colour, EMPTY_DRAFT, "Red");
    expect(first).toEqual({ draft: { selected: ["Red"], text: "", dismissed: false }, moved: null, advance: true });
    const typed = pickOption(colour, { selected: [], text: "Teal ", dismissed: false }, "Blue");
    expect(typed.draft.selected).toEqual(["Blue"]);
    expect(typed.draft.text).toBe("");
    expect(typed.moved).toBe("Teal ");
    expect(pickOption(colour, { selected: [], text: "   ", dismissed: false }, "Blue").moved).toBeNull();
  });

  test("multiple choice toggles in option order, keeps typed text and never advances", () => {
    let d = pickOption(toppings, { ...EMPTY_DRAFT, text: "olives" }, "Green").draft;
    d = pickOption(toppings, d, "Red").draft;
    expect(d.selected).toEqual(["Red", "Green"]);
    expect(d.text).toBe("olives");
    const off = pickOption(toppings, d, "Red");
    expect(off.draft.selected).toEqual(["Green"]);
    expect(off.advance).toBe(false);
    expect(off.moved).toBeNull();
  });

  test("a pick or typing undoes a dismissal", () => {
    const dismissed = { ...EMPTY_DRAFT, dismissed: true };
    expect(pickOption(colour, dismissed, "Red").draft.dismissed).toBe(false);
    expect(typeAnswer(colour, dismissed, "x").dismissed).toBe(false);
  });

  test("typing on single choice clears the pick; on multiple it sits beside it", () => {
    expect(typeAnswer(colour, { selected: ["Red"], text: "", dismissed: false }, "Teal").selected).toEqual([]);
    expect(typeAnswer(colour, { selected: ["Red"], text: "x", dismissed: false }, "").selected).toEqual(["Red"]);
    expect(typeAnswer(toppings, { selected: ["Red"], text: "", dismissed: false }, "olives").selected).toEqual(["Red"]);
  });
});

describe("readiness and where the panel opens", () => {
  test("ready with a pick, a typed answer it allows, or a dismissal", () => {
    expect(questionReady(colour, undefined)).toBe(false);
    expect(questionReady(colour, EMPTY_DRAFT)).toBe(false);
    expect(questionReady(colour, { ...EMPTY_DRAFT, selected: ["Red"] })).toBe(true);
    expect(questionReady(colour, { ...EMPTY_DRAFT, text: " teal " })).toBe(true);
    expect(questionReady(strict, { ...EMPTY_DRAFT, text: "teal" })).toBe(false);
    expect(questionReady(colour, { ...EMPTY_DRAFT, dismissed: true })).toBe(true);
  });

  test("opens at the first unanswered question, so a reload comes back to it", () => {
    expect(firstOpenQuestion([colour, toppings], {})).toBe(0);
    expect(firstOpenQuestion([colour, toppings], { colour: { ...EMPTY_DRAFT, selected: ["Red"] } })).toBe(1);
    expect(firstOpenQuestion([colour, toppings], {
      colour: { ...EMPTY_DRAFT, selected: ["Red"] },
      toppings: { ...EMPTY_DRAFT, selected: ["Blue"] },
    })).toBe(1);
    expect(firstOpenQuestion([], undefined)).toBe(0);
  });
});

describe("submission", () => {
  test("nothing goes until every question is ready", () => {
    expect(questionSubmission([colour, toppings], { colour: { ...EMPTY_DRAFT, selected: ["Red"] } })).toBeNull();
  });

  test("every question once, in provider order, by native id", () => {
    const out = questionSubmission([colour, toppings], {
      toppings: { selected: ["Red", "Blue"], text: "olives", dismissed: false },
      colour: { selected: [], text: "Teal", dismissed: false },
    });
    expect(out).toEqual({
      option: "answer",
      answers: [
        { questionId: "colour", answers: ["Teal"], dismissed: false },
        { questionId: "toppings", answers: ["Red", "Blue", "olives"], dismissed: false },
      ],
    });
  });

  test("one dismissed question goes as an empty dismissed entry; all dismissed dismiss the request", () => {
    const some = questionSubmission([colour, toppings], {
      colour: { ...EMPTY_DRAFT, dismissed: true, text: "kept" },
      toppings: { ...EMPTY_DRAFT, selected: ["Green"] },
    });
    expect(some?.answers[0]).toEqual({ questionId: "colour", answers: [], dismissed: true });
    const all = questionSubmission([colour, toppings], {
      colour: { ...EMPTY_DRAFT, dismissed: true },
      toppings: { ...EMPTY_DRAFT, dismissed: true },
    });
    expect(all).toEqual({ option: "dismiss", answers: [] });
  });

  test("a choice-only question never sends typed text", () => {
    const out = questionSubmission([strict], { strict: { selected: ["Red"], text: "ignored", dismissed: false } });
    expect(out?.answers).toEqual([{ questionId: "strict", answers: ["Red"], dismissed: false }]);
  });
});

describe("words", () => {
  test("the main button: Next, then the answer, or a new message once the session is gone", () => {
    expect(questionPrimaryLabel(0, 2, "live")).toBe("Next");
    expect(questionPrimaryLabel(0, 2, "message")).toBe("Next");
    expect(questionPrimaryLabel(1, 2, "live")).toBe("Send answers");
    expect(questionPrimaryLabel(0, 1, "live")).toBe("Send answer");
    expect(questionPrimaryLabel(0, 1, "message")).toBe("Answer in a new message");
    expect(questionPrimaryLabel(1, 2, "message")).toBe("Answer in a new message");
  });

  test("the composer's hint says whether typing answers", () => {
    expect(questionPlaceholder(colour)).toBe("Type your answer");
    expect(questionPlaceholder(strict)).toBe("Choose an option above");
  });

  test("a run waits for approval before it waits for an answer", () => {
    const approval = { ...request({ id: 1, kind: "approval", action: "command" }) };
    expect(runWaitingOn({ 7: request() })).toBe("answer");
    expect(runWaitingOn({ 7: request(), 1: approval })).toBe("approval");
    expect(runWaitingOn({ 7: request() }, 3)).toBeNull();
    expect(runWaitingOn({ 7: request({ status: "resolved" }) })).toBeNull();
    expect(runWaitingOn({ 7: request({ responder: "message", run_id: null }) })).toBe("answer");
    expect(runWaitingOn(undefined)).toBeNull();
  });
});

describe("the timeline row", () => {
  test("waiting says who asks and how many; a lost session says its answer becomes a message", () => {
    const live = questionRowState(request());
    expect(live.kind).toBe("waiting");
    expect(live.title).toBe("Codex asks 2 questions");
    expect(live.label).toBe("Waiting for your answer");
    expect(live.notes).toEqual([]);
    const lost = questionRowState(request({ responder: "message", reason: "The app was closed.", payload: { questions: [colour] } }));
    expect(lost.title).toBe("Codex asks");
    expect(lost.notes).toEqual(["The app was closed. Your answer goes to Codex as a new message."]);
    const failed = questionRowState(request({ delivery: "failed" }));
    expect(failed.warn).toBe(true);
    expect(failed.notes).toEqual(["Your answer did not reach Codex. Answer again."]);
  });

  test("answered keeps each question beside its answer, or Dismissed", () => {
    const state = questionRowState(request({
      status: "resolved", response: "answer", delivery: "sending",
      answers: [
        { questionId: "colour", answers: [], dismissed: true },
        { questionId: "toppings", answers: ["Red", "olives"], dismissed: false },
      ],
    }));
    expect(state.kind).toBe("answered");
    expect(state.entries.map((e) => [e.question.id, e.answers, e.dismissed])).toEqual([
      ["colour", [], true],
      ["toppings", ["Red", "olives"], false],
    ]);
    expect(state.notes).toEqual(["Sending your answer to Codex…"]);
  });

  test("a whole dismissal, an expiry and a cancellation are one quiet line", () => {
    expect(questionRowState(request({ status: "resolved", response: "dismiss" })).label).toBe("Dismissed");
    const expired = questionRowState(request({ status: "expired", reason: "The agent withdrew this request." }));
    expect([expired.kind, expired.label, expired.notes]).toEqual(["expired", "Expired", ["The agent withdrew this request."]]);
    expect(questionRowState(request({ status: "cancelled", reason: null })).notes).toEqual(["It was cancelled."]);
    const unsure = questionRowState(request({ status: "resolved", response: "answer", delivery: "uncertain", answers: [] }));
    expect(unsure.warn).toBe(true);
  });
});
