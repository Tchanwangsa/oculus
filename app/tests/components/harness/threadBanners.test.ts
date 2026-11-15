import { describe, expect, test } from "bun:test";
import { heldDescription, threadBannerItems, unresponsiveDescription, type ThreadBannerInputs } from "@/components/harness/composer/ThreadBanners";
import { STOP_PATIENCE_MS, type HarnessRun } from "@/lib/harness";

const base: ThreadBannerInputs = { items: [], windows: [], dismissed: [], onDismiss: () => {}, now: 0 };
const resume = (patch: Partial<NonNullable<ThreadBannerInputs["resume"]>> = {}) => ({
  onClick: () => {},
  pending: false,
  error: null,
  ...patch,
});
const held = [
  { id: "1", text: "first", held: true },
  { id: "2", text: "second", held: true },
];

describe("threadBannerItems: a held queue", () => {
  test("an idle chat with held messages offers Resume", () => {
    const [b] = threadBannerItems({ ...base, queued: held, resume: resume() });
    expect(b.id).toBe("held");
    expect(b.title).toBe("2 messages waiting");
    expect(b.actions?.map((a) => [a.label, a.primary])).toEqual([["Resume", true]]);
  });

  test("one message is one message", () => {
    const [b] = threadBannerItems({ ...base, queued: [held[0]], resume: resume() });
    expect(b.title).toBe("1 message waiting");
  });

  test("after a failed reply it says so", () => {
    const [b] = threadBannerItems({ ...base, queued: held, lastRun: "failed", resume: resume() });
    expect(b.description).toBe(heldDescription("failed"));
    expect(b.description).toStartWith("The last reply failed");
  });

  test("a stop and a restart cannot be told apart, so they share the words", () => {
    for (const lastRun of ["interrupted", "completed", null] as const) {
      const [b] = threadBannerItems({ ...base, queued: held, lastRun, resume: resume() });
      expect(b.description).toStartWith("Held after a stop or a restart.");
    }
  });

  test("nothing while a turn is open, nothing for messages that are not held", () => {
    expect(threadBannerItems({ ...base, queued: held, running: true, resume: resume() })).toEqual([]);
    const waiting = held.map((q) => ({ ...q, held: false }));
    expect(threadBannerItems({ ...base, queued: waiting, resume: resume() })).toEqual([]);
  });

  test("no Resume to offer, no notice", () => {
    expect(threadBannerItems({ ...base, queued: held })).toEqual([]);
  });

  test("while Resume is out its button is disabled; a failure is said in the card", () => {
    const [out] = threadBannerItems({ ...base, queued: held, resume: resume({ pending: true }) });
    expect(out.actions?.[0].disabled).toBe(true);
    const [failed] = threadBannerItems({ ...base, queued: held, resume: resume({ error: "no such thread" }) });
    expect(failed.description).toBe("Couldn't resume: no such thread");
    expect(failed.tone).toBe("danger");
    expect(failed.actions?.[0].disabled).toBe(false);
  });

  test("clicking Resume calls it", () => {
    let n = 0;
    threadBannerItems({ ...base, queued: held, resume: resume({ onClick: () => n++ }) })[0].actions?.[0].onClick();
    expect(n).toBe(1);
  });
});

describe("threadBannerItems: a stop the agent ignores", () => {
  const run = (patch: Partial<HarnessRun>): HarnessRun => ({
    id: 7, status: "running", delivery: "unknown", answer_item_id: null, started_ms: 0, ended_ms: null, ...patch,
  });
  const told = (others: number) => run({ stopping: true, stop_requested_ms: 0, unresponsive: { others } });
  const force = (patch: Partial<NonNullable<ThreadBannerInputs["forceStop"]>> = {}) => ({
    onClick: () => {},
    pending: false,
    error: null,
    ...patch,
  });

  test("once Rust says so, the notice offers Force stop for the run", () => {
    const stopped: number[] = [];
    const [b] = threadBannerItems({ ...base, running: true, liveRun: told(0), forceStop: force({ onClick: (id) => stopped.push(id) }) });
    expect(b.id).toBe("unresponsive:7");
    expect(b.title).toBe("Agent not responding to Stop");
    expect(b.actions?.[0].label).toBe("Force stop");
    b.actions?.[0].onClick();
    expect(stopped).toEqual([7]);
  });

  test("on a shared server the button says what else it stops", () => {
    const one = threadBannerItems({ ...base, running: true, liveRun: told(1), sharedProcess: true, forceStop: force() });
    expect(one[0].actions?.[0].label).toBe("Force stop — also stops 1 other chat");
    const many = threadBannerItems({ ...base, running: true, liveRun: told(3), sharedProcess: true, forceStop: force() });
    expect(many[0].actions?.[0].label).toBe("Force stop — also stops 3 other chats");
    expect(many[0].description).toContain("3 other chats");
  });

  test("after a reload the clock decides, from when the stop was asked", () => {
    const asked = run({ stopping: true, stop_requested_ms: 1000 });
    const early = threadBannerItems({ ...base, now: 1000 + STOP_PATIENCE_MS - 1, running: true, liveRun: asked, forceStop: force() });
    expect(early).toEqual([]);
    const [late] = threadBannerItems({ ...base, now: 1000 + STOP_PATIENCE_MS, running: true, liveRun: asked, forceStop: force() });
    expect(late.title).toBe("Agent not responding to Stop");
  });

  test("with no count told, a shared server's other chats are named without a number", () => {
    const asked = run({ stopping: true, stop_requested_ms: 0 });
    const [b] = threadBannerItems({ ...base, now: STOP_PATIENCE_MS, running: true, liveRun: asked, sharedProcess: true, forceStop: force() });
    expect(b.actions?.[0].label).toBe("Force stop");
    expect(b.description).toBe(unresponsiveDescription(null, true));
    expect(b.description).toContain("Other chats on the same server stop too.");
  });

  test("a run not stopping, or no Force stop to offer, has no notice", () => {
    expect(threadBannerItems({ ...base, running: true, liveRun: run({}), forceStop: force() })).toEqual([]);
    expect(threadBannerItems({ ...base, running: true, liveRun: told(0) })).toEqual([]);
  });

  test("while it is out the button rests; a failure is said in the card", () => {
    const [out] = threadBannerItems({ ...base, running: true, liveRun: told(0), forceStop: force({ pending: true }) });
    expect(out.actions?.[0].disabled).toBe(true);
    const [failed] = threadBannerItems({ ...base, running: true, liveRun: told(0), forceStop: force({ error: "no server" }) });
    expect(failed.description).toBe("Couldn't force stop: no server");
    expect(failed.tone).toBe("danger");
  });

  test("it comes first in the lane", () => {
    const limited = [
      { id: 1, thread_id: 1, kind: "user" as const, ref_id: null, content: "x", meta: null, created_at: "2026-11-15 10:00:00", run_id: 7 },
      { id: 2, thread_id: 1, kind: "error" as const, ref_id: null, content: "x", meta: JSON.stringify({ limit: true }), created_at: "2026-11-15 10:00:00", run_id: 7 },
    ];
    const ids = threadBannerItems({ ...base, items: limited, running: true, liveRun: told(0), forceStop: force() }).map((b) => b.id);
    expect(ids[0]).toBe("unresponsive:7");
  });
});
