import { describe, expect, test } from "bun:test";
import { request, settle, waitingCount, type Outgoing } from "../../widget-kit/ask/pending";
import { hostMessage, isHostMessage } from "../../widget-kit/runtime/host";

const reply = (id: string, result: { value: unknown } | { error: string }) =>
  "error" in result
    ? ({ type: "oculus-host", kind: "reply", id, ok: false, error: result.error } as const)
    : ({ type: "oculus-host", kind: "reply", id, ok: true, value: result.value } as const);

function capture() {
  const posted: (Outgoing & { id: string })[] = [];
  return { posted, post: (m: Outgoing & { id: string }) => posted.push(m) };
}

describe("oculus/ask's pending requests", () => {
  test("each request gets its own id and settles by it, in any order", async () => {
    const c = capture();
    const first = request({ kind: "ask", prompt: "p", schema: { type: "integer" } }, c.post);
    const second = request({ kind: "send", text: "hi" }, c.post);
    const [a, b] = c.posted;
    expect(a).toMatchObject({ kind: "ask", prompt: "p", schema: { type: "integer" } });
    expect(b).toMatchObject({ kind: "send", text: "hi" });
    expect(a.id).not.toBe(b.id);
    expect(waitingCount()).toBe(2);
    expect(settle(reply(b.id, { value: null }))).toBe(true);
    expect(settle(reply(a.id, { value: 42 }))).toBe(true);
    expect(await first).toBe(42);
    expect(await second).toBeNull();
    expect(waitingCount()).toBe(0);
  });

  test("an error reply rejects with its reason; an unknown or repeated id is ignored", async () => {
    const c = capture();
    const p = request({ kind: "ask", prompt: "p", schema: {} }, c.post);
    const id = c.posted[0].id;
    expect(settle(reply("r999999", { value: 1 }))).toBe(false);
    expect(settle(reply(id, { error: "A request is already running." }))).toBe(true);
    await expect(p).rejects.toThrow("A request is already running.");
    expect(settle(reply(id, { value: 1 }))).toBe(false);
  });
});

describe("the runtime's host messages", () => {
  const parentWindow = {};
  test("a reply counts only from the parent and only in its shapes", () => {
    const ok = reply("r1", { value: { a: 1 } });
    expect(hostMessage({ source: parentWindow, data: ok }, parentWindow)).toEqual(ok);
    expect(hostMessage({ source: {}, data: ok }, parentWindow)).toBeNull();
    expect(isHostMessage(reply("r1", { error: "no" }))).toBe(true);
    expect(isHostMessage(reply("r1", { value: undefined }))).toBe(true);
    for (const bad of [
      { type: "oculus-host", kind: "reply", id: "r1", ok: true },
      { type: "oculus-host", kind: "reply", id: "r1", ok: false, error: 3 },
      { type: "oculus-host", kind: "reply", id: "r1", ok: "yes", value: 1 },
      { type: "oculus-host", kind: "reply", id: 1, ok: true, value: 1 },
      { type: "oculus-host", kind: "reply", id: "bad id", ok: true, value: 1 },
      { type: "oculus-widget", kind: "reply", id: "r1", ok: true, value: 1 },
    ]) {
      expect(isHostMessage(bad)).toBe(false);
    }
  });
});

describe("ask and send check their arguments in the frame", () => {
  test("a wrong argument rejects before anything waits", async () => {
    const { ask, send } = await import("../../widget-kit/ask");
    const before = waitingCount();
    const cycle: Record<string, unknown> = {};
    cycle.self = cycle;
    await expect(ask(3 as unknown as string, {})).rejects.toThrow("the prompt is a string");
    await expect(ask("p", [] as unknown as object)).rejects.toThrow("JSON Schema object");
    await expect(ask("p", cycle)).rejects.toThrow("the schema isn't JSON");
    await expect(send(null as unknown as string)).rejects.toThrow("the text is a string");
    expect(waitingCount()).toBe(before);
  });
});
