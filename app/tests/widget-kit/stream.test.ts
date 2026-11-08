import { afterAll, describe, expect, test } from "bun:test";

// The runtime's timers and flag live on `window`; tests run without a DOM.
const globals = globalThis as Record<string, unknown>;
const added = (["window", "requestAnimationFrame", "cancelAnimationFrame"] as const).filter((k) => !(k in globals));
const stubs: Record<string, unknown> = {
  window: globalThis,
  requestAnimationFrame: (fn: (t: number) => void) => setTimeout(() => fn(performance.now()), 16),
  cancelAnimationFrame: (h: ReturnType<typeof setTimeout>) => clearTimeout(h),
};
for (const k of added) globals[k] = stubs[k];
afterAll(() => {
  for (const k of added) delete globals[k];
});

const { isHostMessage } = await import("../../widget-kit/runtime/host");
const { request } = await import("../../widget-kit/ask/pending");
const { isStreaming, setStreaming } = await import("../../widget-kit/runtime/mode");
const { PREVIEW_WORK_MS, previewScope, retire, scopeHandles } = await import("../../widget-kit/runtime/preview");
const { QUIET_FLAG, STILL_WRITING } = await import("../../widget-kit/protocol");

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

describe("the module message", () => {
  test("is taken only in its exact shape", () => {
    expect(isHostMessage({ type: "oculus-host", kind: "module", id: 1, code: "x", final: false })).toBe(true);
    expect(isHostMessage({ type: "oculus-host", kind: "module", id: 7, code: "", final: true })).toBe(true);
    for (const bad of [
      { type: "oculus-host", kind: "module", id: 0, code: "x", final: false },
      { type: "oculus-host", kind: "module", id: 1.5, code: "x", final: false },
      { type: "oculus-host", kind: "module", id: "1", code: "x", final: false },
      { type: "oculus-host", kind: "module", id: 1, code: 2, final: false },
      { type: "oculus-host", kind: "module", id: 1, code: "x" },
      { type: "oculus-widget", kind: "module", id: 1, code: "x", final: false },
    ]) {
      expect(isHostMessage(bad)).toBe(false);
    }
  });
});

describe("the frame's streaming gate", () => {
  test("refuses requests while streaming, before anything is posted, and quiets the guard", async () => {
    const posted: unknown[] = [];
    setStreaming(true);
    expect(isStreaming()).toBe(true);
    expect((window as unknown as Record<string, unknown>)[QUIET_FLAG]).toBe(true);
    await expect(request({ kind: "send", text: "hi" }, (m) => posted.push(m))).rejects.toThrow(STILL_WRITING);
    await expect(request({ kind: "ask", prompt: "p", schema: {} }, (m) => posted.push(m))).rejects.toThrow(STILL_WRITING);
    expect(posted).toEqual([]);
    setStreaming(false);
    expect((window as unknown as Record<string, unknown>)[QUIET_FLAG]).toBe(false);
    void request({ kind: "send", text: "hi" }, (m) => posted.push(m));
    expect(posted.length).toBe(1);
  });
});

describe("a preview's timers", () => {
  test("a newer program retires an older preview's interval and frames", async () => {
    let ticks = 0;
    const old = previewScope(101);
    old.setInterval(() => ticks++, 5); // a half-written effect: no cleanup
    old.requestAnimationFrame(() => ticks++);
    await sleep(30);
    expect(ticks).toBeGreaterThan(0);
    expect(scopeHandles(101)).toBeGreaterThan(0);
    retire(102);
    const at = ticks;
    await sleep(40);
    expect(ticks).toBe(at);
    expect(scopeHandles(101)).toBe(0);
    // A retired scope schedules nothing more.
    expect(old.setTimeout(() => ticks++, 0)).toBe(0);
  });

  test("a preview's own cancels work, and a newer scope survives retiring older ones", async () => {
    const a = previewScope(201);
    const b = previewScope(202);
    let fired = 0;
    const h = a.setTimeout(() => fired++, 5);
    a.clearTimeout(h);
    b.setTimeout(() => fired++, 5);
    retire(202);
    await sleep(20);
    expect(fired).toBe(1);
  });

  test("a preview stops once its callbacks have spent the budget", async () => {
    const s = previewScope(301);
    let runs = 0;
    s.setInterval(() => {
      runs++;
      const until = performance.now() + PREVIEW_WORK_MS / 4;
      while (performance.now() < until) {
        // A busy callback, as a runaway effect would be.
      }
    }, 1);
    await sleep(PREVIEW_WORK_MS * 2);
    expect(runs).toBeLessThanOrEqual(5);
    expect(scopeHandles(301)).toBe(0);
  });
});
