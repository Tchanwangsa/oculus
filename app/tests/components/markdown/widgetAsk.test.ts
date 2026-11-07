import { describe, expect, test } from "bun:test";
import { ALLOWED, DENIED, gate, spend } from "@/components/markdown/widget/ask/gate";
import { ASK_CAP, MAX_PROMPT, MAX_SCHEMA, MAX_TEXT, REFUSED } from "@/components/markdown/widget/ask/limits";
import type { WidgetRequest } from "@/components/markdown/widget/messages";
import { createWidgetStore } from "@/stores/chat/widgetStore";

const ask = (prompt = "Steps for x^2?", schema: Record<string, unknown> = { type: "object" }): WidgetRequest => ({
  type: "oculus-widget",
  kind: "ask",
  id: "r1",
  prompt,
  schema,
});
const send = (text = "Explain step 2"): WidgetRequest => ({ type: "oculus-widget", kind: "send", id: "r2", text });
const now = (over: Partial<Parameters<typeof gate>[1]> = {}) => ({ grant: ALLOWED, busy: false, canSend: true, ...over });

describe("the host's gate", () => {
  test("the first request waits for the student's Allow", () => {
    expect(gate(ask(), now({ grant: undefined }))).toEqual({ hold: true });
    expect(gate(send(), now({ grant: undefined }))).toEqual({ hold: true });
  });

  test("allowed runs; denied is refused and stays refused", () => {
    expect(gate(ask(), now())).toEqual({ run: true });
    expect(gate(send(), now())).toEqual({ run: true });
    expect(gate(ask(), now({ grant: DENIED }))).toEqual({ refuse: REFUSED.denied });
    expect(gate(send(), now({ grant: DENIED }))).toEqual({ refuse: REFUSED.denied });
    expect(REFUSED.denied).toBe("The student didn't allow this widget to use a model.");
  });

  test("asks and sends share one cap per allowance", () => {
    let grant = ALLOWED;
    for (let i = 0; i < ASK_CAP; i++) {
      expect(gate(i % 2 ? send() : ask(), now({ grant }))).toEqual({ run: true });
      grant = spend(grant);
    }
    expect(grant.used).toBe(20);
    expect(gate(ask(), now({ grant }))).toEqual({ refuse: "This widget has used its 20 requests.", capped: true });
    expect(gate(send(), now({ grant }))).toEqual({ refuse: REFUSED.capped, capped: true });
    // Allow 20 more is a fresh allowance.
    expect(gate(ask(), now({ grant: ALLOWED }))).toEqual({ run: true });
  });

  test("one request at a time, before anything else", () => {
    expect(gate(ask(), now({ busy: true }))).toEqual({ refuse: "A request is already running." });
    expect(gate(send(), now({ busy: true, grant: undefined }))).toEqual({ refuse: REFUSED.busy });
    expect(gate(ask(), now({ busy: true, grant: DENIED }))).toEqual({ refuse: REFUSED.busy });
  });

  test("send needs a thread that listens; ask doesn't", () => {
    expect(gate(send(), now({ canSend: false }))).toEqual({ refuse: REFUSED.noChat });
    // Refused before the student is asked to allow anything.
    expect(gate(send(), now({ canSend: false, grant: undefined }))).toEqual({ refuse: REFUSED.noChat });
    expect(gate(ask(), now({ canSend: false }))).toEqual({ run: true });
  });

  test("sizes are refused with a reason, before the Allow", () => {
    const g = now({ grant: undefined });
    expect(gate(ask("x".repeat(MAX_PROMPT)), g)).toEqual({ hold: true });
    expect(gate(ask("x".repeat(MAX_PROMPT + 1)), g)).toEqual({ refuse: "The prompt is over 20,000 characters." });
    // Characters, not UTF-16 units.
    expect(gate(ask("𝑥".repeat(MAX_PROMPT)), g)).toEqual({ hold: true });
    expect(gate(ask("  "), g)).toEqual({ refuse: REFUSED.emptyPrompt });
    const big = { type: "string", description: "d".repeat(MAX_SCHEMA) };
    expect(gate(ask("q", big), g)).toEqual({ refuse: "The schema is over 8,000 characters of JSON." });
    expect(gate(send("y".repeat(MAX_TEXT)), g)).toEqual({ hold: true });
    expect(gate(send("y".repeat(MAX_TEXT + 1)), g)).toEqual({ refuse: "The message is over 4,000 characters." });
    expect(gate(send(""), g)).toEqual({ refuse: REFUSED.emptyText });
  });
});

describe("the grant in widgetStore", () => {
  const disk = () => {
    let value = "{}";
    return { getItem: () => value, setItem: (_k: string, next: string) => { value = next; } };
  };

  test("is kept with the entry, survives state saves and a reload, and a new source starts fresh", () => {
    const storage = disk();
    const store = createWidgetStore(storage).getState();
    store.save("1:2:0", "v1", { a: 1 });
    store.setAsk("1:2:0", "v1", { allowed: true, used: 3 });
    store.save("1:2:0", "v1", { a: 2 });
    const reloaded = createWidgetStore(storage).getState().entries["1:2:0"];
    expect(reloaded).toEqual({ source: "v1", state: { a: 2 }, ask: { allowed: true, used: 3 } });
    store.save("1:2:0", "v2", { a: 3 });
    expect(createWidgetStore(storage).getState().entries["1:2:0"]).toEqual({ source: "v2", state: { a: 3 } });
    // A grant set for a new source drops the old source's state.
    store.setAsk("1:2:0", "v3", { allowed: false, used: 0 });
    expect(createWidgetStore(storage).getState().entries["1:2:0"]).toEqual({ source: "v3", state: {}, ask: { allowed: false, used: 0 } });
  });

  test("a malformed grant is dropped, its entry kept", () => {
    const storage = disk();
    storage.setItem("", JSON.stringify({ "1:2:0": { source: "s", state: { a: 1 }, ask: { allowed: "yes", used: -1 } } }));
    expect(createWidgetStore(storage).getState().entries).toEqual({ "1:2:0": { source: "s", state: { a: 1 } } });
  });
});
