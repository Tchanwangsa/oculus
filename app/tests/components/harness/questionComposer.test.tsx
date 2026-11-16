import { afterAll, beforeEach, expect, mock, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import type { AgentQuestion, HarnessEnvelope, HarnessRequest } from "@/lib/harness";

const priorStorage = globalThis.localStorage;
let storage = new Map<string, string>();
const calls: { command: string; args: Record<string, unknown> }[] = [];
let answer: (command: string, args: Record<string, unknown>) => Promise<unknown> = async () => null;
mock.module("@tauri-apps/api/core", () => ({
  invoke: (command: string, args: Record<string, unknown>) => {
    calls.push({ command, args });
    return answer(command, args);
  },
}));
afterAll(() => {
  Object.assign(globalThis, { localStorage: priorStorage });
  Object.assign(initial, initialCopy);
});

const { useHarnessStore } = await import("@/stores/chat/harnessStore");
// A server render reads a zustand store's initial state (the object it was
// created with); the probe copies the live state onto it before each render.
const initial = useHarnessStore.getInitialState();
const initialCopy = { ...initial };
const { useDraftStore } = await import("@/stores/chat/draftStore");
const { useQuestionComposer } = await import("@/components/harness/composer/useQuestionComposer");
const { QuestionPanelView } = await import("@/components/harness/composer/QuestionPanel");
type Flow = NonNullable<ReturnType<typeof useQuestionComposer>>;

const THREAD = 41;
const DRAFT = "thread:41";
const colour: AgentQuestion = {
  id: "colour", header: "Colour", text: "Which colour?",
  options: [{ label: "Red", description: "Warm" }, { label: "Blue", description: "Cool" }],
  multi: false, allowsTypedAnswer: true, secret: false,
};
const size: AgentQuestion = { ...colour, id: "size", header: "Size", text: "Which size?", options: [{ label: "Small", description: "" }], allowsTypedAnswer: false };
const request = (patch: Partial<HarnessRequest> = {}): HarnessRequest => ({
  id: 5, thread_id: THREAD, run_id: 1, item_id: 50, provider: "claude", kind: "question", action: "question",
  payload: { questions: [colour] }, options: [], status: "pending", responder: "live", reason: null,
  response: null, answers: null, delivery: "none", created_at: "2026-11-16 00:00:00", resolved_at: null, ...patch,
});
const open = (r: HarnessRequest, revision = 2) => {
  const event: HarnessEnvelope = { threadId: THREAD, provider: "claude", itemId: r.item_id, revision, event: { type: "request_opened", request: r, waiting: 1, waiting_questions: 1 } };
  useHarnessStore.getState().fold(event);
};

/** The hook's current value, read by rendering it once on the server. */
function flow(): Flow | null {
  let out: Flow | null = null;
  function Probe() {
    out = useQuestionComposer(THREAD, DRAFT);
    return null;
  }
  Object.assign(initial, useHarnessStore.getState());
  renderToStaticMarkup(<Probe />);
  return out;
}
const draftOf = (question: string) => useHarnessStore.getState().questionDrafts[THREAD]?.[5]?.[question];
const responds = () => calls.filter((c) => c.command === "harness_respond").map((c) => [c.args.requestId, c.args.optionId, c.args.answers]);

beforeEach(async () => {
  storage = new Map();
  Object.assign(globalThis, { localStorage: { getItem: (k: string) => storage.get(k) ?? null, setItem: (k: string, v: string) => { storage.set(k, v); }, removeItem: (k: string) => { storage.delete(k); } } });
  useHarnessStore.getState().removed(THREAD);
  useDraftStore.getState().setDraft(DRAFT, "");
  calls.length = 0;
  answer = async (command) => (command === "harness_load" ? { revision: 1, items: [], runs: [], queued: [], requests: [] } : null);
  await useHarnessStore.getState().load(THREAD);
});

test("no question, no flow; approvals are not questions", () => {
  expect(flow()).toBeNull();
  open(request({ kind: "approval", action: "command", payload: { command: "ls" } }));
  expect(flow()).toBeNull();
});

test("a pick is a draft in the store, so it survives a chat switch and a reload", () => {
  open(request());
  const f = flow()!;
  expect(f.question.id).toBe("colour");
  expect(f.placeholder).toBe("Type your answer");
  expect(f.primaryLabel).toBe("Send answer");
  expect(f.ready).toBe(false);
  f.pick("Blue");
  expect(draftOf("colour")).toEqual({ selected: ["Blue"], text: "", dismissed: false });
  expect(JSON.parse(storage.get("oculus.questionDrafts")!)[THREAD][5].colour.selected).toEqual(["Blue"]);
  expect(flow()!.ready).toBe(true);
});

test("typing is the answer; a later pick moves the typed text to the message draft", () => {
  open(request());
  useDraftStore.getState().setDraft(DRAFT, "half a message");
  flow()!.type("Teal");
  expect(draftOf("colour")).toEqual({ selected: [], text: "Teal", dismissed: false });
  flow()!.pick("Red");
  expect(draftOf("colour")).toEqual({ selected: ["Red"], text: "", dismissed: false });
  expect(useDraftStore.getState().drafts[DRAFT]).toBe("half a message\n\nTeal");
});

test("Enter submits the answer with the question's native key, never a message", async () => {
  open(request());
  flow()!.type("Teal");
  let prevented = false;
  flow()!.keyDown({ key: "Enter", shiftKey: false, nativeEvent: { isComposing: false }, preventDefault: () => { prevented = true; } } as never);
  await Bun.sleep(0);
  expect(prevented).toBe(true);
  expect(responds()).toEqual([[5, "answer", [{ questionId: "colour", answers: ["Teal"], dismissed: false }]]]);
  expect(calls.some((c) => c.command === "harness_send")).toBe(false);
});

test("Shift+Enter and an unanswered question send nothing", async () => {
  open(request());
  let prevented = false;
  const key = (shiftKey: boolean) => ({ key: "Enter", shiftKey, nativeEvent: { isComposing: false }, preventDefault: () => { prevented = true; } }) as never;
  flow()!.keyDown(key(true));
  expect(prevented).toBe(false);
  flow()!.keyDown(key(false));
  await Bun.sleep(0);
  expect(responds()).toEqual([]);
});

test("a choice-only question ignores typing and says to choose", () => {
  open(request({ payload: { questions: [size] } }));
  const f = flow()!;
  expect(f.placeholder).toBe("Choose an option above");
  f.type("Large");
  expect(draftOf("size")).toBeUndefined();
});

test("two questions: the panel reopens at the first unanswered, and every answer goes once", async () => {
  open(request({ payload: { questions: [colour, size] } }));
  expect(flow()!.primaryLabel).toBe("Next");
  flow()!.pick("Red");
  // A fresh mount (a chat switch, a reload) opens where the student left off.
  const back = flow()!;
  expect(back.index).toBe(1);
  expect(back.primaryLabel).toBe("Send answers");
  back.pick("Small");
  flow()!.primary();
  await Bun.sleep(0);
  expect(responds()).toEqual([[5, "answer", [
    { questionId: "colour", answers: ["Red"], dismissed: false },
    { questionId: "size", answers: ["Small"], dismissed: false },
  ]]]);
});

test("dismissing the only question dismisses the request", async () => {
  open(request());
  flow()!.dismiss();
  await Bun.sleep(0);
  expect(responds()).toEqual([[5, "dismiss", []]]);
});

test("dismissing one of two marks it and the last answer sends both", async () => {
  open(request({ payload: { questions: [colour, size] } }));
  flow()!.dismiss();
  expect(draftOf("colour")?.dismissed).toBe(true);
  const f = flow()!;
  expect(f.index).toBe(1);
  f.pick("Small");
  flow()!.primary();
  await Bun.sleep(0);
  expect(responds()).toEqual([[5, "answer", [
    { questionId: "colour", answers: [], dismissed: true },
    { questionId: "size", answers: ["Small"], dismissed: false },
  ]]]);
});

test("a question whose session is gone answers in a new message", () => {
  open(request({ responder: "message", reason: "The agent's process ended." }));
  expect(flow()!.primaryLabel).toBe("Answer in a new message");
});

test("the oldest waiting question is the one answered", () => {
  open(request({ id: 9, item_id: 90 }));
  open(request({ id: 5, item_id: 50, payload: { questions: [size] } }), 3);
  expect(flow()!.request.id).toBe(5);
});

test("the panel draws radios with digit caps, the count and the refusal", () => {
  const html = renderToStaticMarkup(
    <QuestionPanelView
      question={{ ...colour, multi: true }}
      draft={{ selected: ["Blue"], text: "", dismissed: false }}
      index={0}
      count={2}
      ready
      busy={false}
      error="This question was already answered."
      moved={false}
      primaryLabel="Next"
      onPick={() => {}}
      onBack={() => {}}
      onPrimary={() => {}}
      onDismiss={() => {}}
    />,
  );
  expect(html).toContain('role="group"');
  expect(html).toContain('role="checkbox" aria-checked="false"');
  expect(html).toContain('role="checkbox" aria-checked="true"');
  expect(html).toContain(">1</kbd>");
  expect(html).toContain(">2</kbd>");
  expect(html).toContain(">1/2</span>");
  expect(html).toContain("This question was already answered.");
  expect(html).toContain("Dismiss question without answering");
  expect(html).not.toContain(">Back<");
  const single = renderToStaticMarkup(
    <QuestionPanelView question={colour} draft={{ selected: [], text: "", dismissed: false }} index={1} count={2} ready={false}
      busy={false} error={null} moved primaryLabel="Answer in a new message" onPick={() => {}} onBack={() => {}} onPrimary={() => {}} onDismiss={() => {}} />,
  );
  expect(single).toContain('role="radiogroup"');
  expect(single).toContain(">Back<");
  expect(single).toContain("Answer in a new message");
  expect(single).toContain("What you typed was moved to your message draft.");
});
