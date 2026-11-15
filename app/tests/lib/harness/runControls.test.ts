import { describe, expect, test } from "bun:test";
import { commandError, moveQueued, queueHeld, queuedLabel, resumeLabel, runControls, type RunControlInputs } from "@/lib/harness";

const idle: RunControlInputs = {
  running: false,
  liveRun: null,
  stopSent: false,
  held: false,
  hasDraft: false,
  ready: false,
  writing: false,
};

describe("runControls: Stop", () => {
  test("idle: no Stop", () => {
    expect(runControls(idle).stop).toBeNull();
  });

  test("running: Stop generation, naming the run on screen", () => {
    const { stop } = runControls({ ...idle, running: true, liveRun: { id: 7, stopping: false } });
    expect(stop).toEqual({ label: "Stop generation", stopping: false, runId: 7 });
  });

  test("a run marked stopping reads Stopping and takes no click", () => {
    const { stop } = runControls({ ...idle, running: true, liveRun: { id: 7, stopping: true } });
    expect(stop).toEqual({ label: "Stopping", stopping: true, runId: 7 });
  });

  test("a stop on its way reads Stopping before Rust marks the run", () => {
    const { stop } = runControls({ ...idle, running: true, liveRun: { id: 7, stopping: false }, stopSent: true });
    expect(stop?.label).toBe("Stopping");
    expect(stop?.stopping).toBe(true);
  });

  test("a turn the CLI started itself: Stop names no run", () => {
    const { stop } = runControls({ ...idle, running: true });
    expect(stop).toEqual({ label: "Stop generation", stopping: false, runId: null });
    const sent = runControls({ ...idle, running: true, stopSent: true }).stop;
    expect(sent).toEqual({ label: "Stopping", stopping: true, runId: null });
  });
});

describe("runControls: the main button", () => {
  test("idle with a draft sends", () => {
    expect(runControls({ ...idle, hasDraft: true, ready: true }).send).toEqual({
      kind: "send",
      label: "Send message",
      disabled: false,
    });
  });

  test("idle with nothing typed: a disabled send", () => {
    expect(runControls(idle).send).toEqual({ kind: "send", label: "Send message", disabled: true });
  });

  test("still writing attachments: disabled", () => {
    expect(runControls({ ...idle, hasDraft: true, ready: true, writing: true }).send?.disabled).toBe(true);
  });

  test("running with a draft queues it; with none there is only Stop", () => {
    const busy = { ...idle, running: true, liveRun: { id: 7, stopping: false } };
    expect(runControls({ ...busy, hasDraft: true, ready: true }).send).toEqual({
      kind: "queue",
      label: "Queue message",
      disabled: false,
    });
    expect(runControls(busy).send).toBeNull();
  });

  test("a held queue and an empty box: Resume thread", () => {
    expect(runControls({ ...idle, held: true }).send).toEqual({
      kind: "resume",
      label: "Resume thread",
      disabled: false,
    });
  });

  test("a held queue and a draft: the draft is sent after the waiting messages", () => {
    expect(runControls({ ...idle, held: true, hasDraft: true, ready: true }).send).toEqual({
      kind: "send",
      label: "Send after the waiting messages",
      disabled: false,
    });
  });

  test("a held queue while a turn is still open shows Stop, not Resume", () => {
    const c = runControls({ ...idle, held: true, running: true, liveRun: { id: 7, stopping: true } });
    expect(c.stop?.label).toBe("Stopping");
    expect(c.send).toBeNull();
  });
});

test("Resume's label: Resume thread with the box empty, else Resume", () => {
  expect(resumeLabel(true)).toBe("Resume thread");
  expect(resumeLabel(false)).toBe("Resume");
});

test("queueHeld: any waiting message held", () => {
  expect(queueHeld(undefined)).toBe(false);
  expect(queueHeld([{ id: "1", text: "a", held: false }])).toBe(false);
  expect(queueHeld([{ id: "1", text: "a", held: false }, { id: "2", text: "b", held: true }])).toBe(true);
});

test("commandError keeps Rust's words and never says nothing", () => {
  expect(commandError("the queue is a thread's")).toBe("the queue is a thread's");
  expect(commandError(new Error("gone"))).toBe("gone");
  expect(commandError(undefined)).toBe("Something went wrong");
  expect(commandError("  ")).toBe("Something went wrong");
});

test("a waiting message says whether it is held", () => {
  expect(queuedLabel({ held: true })).toBe("Held");
  expect(queuedLabel({ held: false })).toBe("Waiting");
});

describe("moveQueued", () => {
  const ids = ["a", "b", "c"];
  test("earlier and later swap with the neighbour", () => {
    expect(moveQueued(ids, "b", -1)).toEqual(["b", "a", "c"]);
    expect(moveQueued(ids, "b", 1)).toEqual(["a", "c", "b"]);
    expect(moveQueued(ids, "a", 1)).toEqual(["b", "a", "c"]);
  });
  test("past either end, or an id that went out, moves nothing", () => {
    expect(moveQueued(ids, "a", -1)).toBeNull();
    expect(moveQueued(ids, "c", 1)).toBeNull();
    expect(moveQueued(ids, "z", 1)).toBeNull();
  });
  test("the list it was given is left as it was", () => {
    moveQueued(ids, "b", 1);
    expect(ids).toEqual(["a", "b", "c"]);
  });
});
