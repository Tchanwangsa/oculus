import { describe, expect, test } from "bun:test";
import { createAnnouncer, lastTurnEnd, SETTLE_MS, spokenStatus, type HarnessRun, type Spoken, type Timers } from "@/lib/harness";

/** Hand-driven timers: `tick(ms)` fires what is due. */
function fakeTimers() {
  let now = 0;
  let next = 1;
  const due = new Map<number, { at: number; fn: () => void }>();
  const timers: Timers = {
    set: (fn, ms) => {
      const id = next++;
      due.set(id, { at: now + ms, fn });
      return id;
    },
    clear: (id) => void due.delete(id as number),
  };
  const tick = (ms: number) => {
    now += ms;
    for (const [id, t] of [...due].sort((a, b) => a[1].at - b[1].at)) {
      if (t.at <= now) {
        due.delete(id);
        t.fn();
      }
    }
  };
  return { timers, tick };
}

function setup() {
  const said: string[] = [];
  const { timers, tick } = fakeTimers();
  const a = createAnnouncer((t) => said.push(t), timers);
  const step = (s: Spoken | null) => {
    a.update(s);
    tick(SETTLE_MS);
  };
  return { a, said, tick, step };
}

describe("createAnnouncer", () => {
  test("says nothing about the status it starts with", () => {
    const { said, step } = setup();
    step("done");
    expect(said).toEqual([]);
  });

  test("a run starting and finishing: Working, then Done", () => {
    const { said, step } = setup();
    step(null);
    step("working");
    step("done");
    expect(said).toEqual(["Working", "Done"]);
  });

  test("a stop and a failure have their own words", () => {
    const { said, step } = setup();
    step("done");
    step("working");
    step("stopped");
    step("working");
    step("failed");
    expect(said).toEqual(["Working", "Stopped", "Working", "Failed"]);
  });

  test("approval and input", () => {
    const { said, step } = setup();
    step("working");
    step("approval");
    step("input");
    expect(said).toEqual(["Needs approval", "Needs your input"]);
  });

  test("a flicker shorter than the settle time is not said", () => {
    const { a, said, tick } = setup();
    a.update("done");
    a.update("working");
    tick(SETTLE_MS / 3);
    a.update("done");
    tick(SETTLE_MS);
    expect(said).toEqual([]);
  });

  test("only the status that holds is said", () => {
    const { a, said, tick } = setup();
    a.update(null);
    a.update("working");
    tick(SETTLE_MS - 1);
    expect(said).toEqual([]);
    a.update("done");
    tick(SETTLE_MS);
    expect(said).toEqual(["Done"]);
  });

  test("a thread switch clears the region and is not news", () => {
    const { a, said, step } = setup();
    step("done");
    step("working");
    a.reset();
    step("failed");
    expect(said).toEqual(["Working", ""]);
    step("working");
    expect(said).toEqual(["Working", "", "Working"]);
  });

  test("going quiet says nothing", () => {
    const { said, step } = setup();
    step("done");
    step(null);
    expect(said).toEqual([]);
  });
});

describe("spokenStatus", () => {
  const base = { running: false, approval: false, input: false, lastRun: null };
  test("follows the sidebar's order, a stop after it", () => {
    expect(spokenStatus({ ...base, running: true, lastRun: "failed" })).toBe("working");
    expect(spokenStatus({ ...base, lastRun: "completed" })).toBe("done");
    expect(spokenStatus({ ...base, lastRun: "failed" })).toBe("failed");
    expect(spokenStatus({ ...base, lastRun: "interrupted" })).toBe("stopped");
    expect(spokenStatus({ ...base, approval: true, running: true })).toBe("approval");
    expect(spokenStatus(base)).toBeNull();
  });
});

describe("lastTurnEnd", () => {
  const runs: Record<number, HarnessRun> = {
    2: { id: 2, status: "completed", delivery: "delivered", answer_item_id: 1, started_ms: 0, ended_ms: 1 },
  };
  test("the last turn heard ending wins: a stopped CLI turn has no run of its own", () => {
    expect(lastTurnEnd("interrupted", runs)).toBe("interrupted");
  });
  test("with none heard (a fresh view), the newest run says", () => {
    expect(lastTurnEnd(undefined, runs)).toBe("completed");
    expect(lastTurnEnd(undefined, undefined)).toBeNull();
  });
});
