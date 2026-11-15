import { beforeEach, expect, test } from "bun:test";
import { anyRunning, useHarnessStore } from "@/stores/chat/harnessStore";
import { lastTurnEnd, spokenStatus, type HarnessEnvelope, type HarnessItem, type HarnessThread } from "@/lib/harness";

const idle = { running: false, streaming: "", thinking: "", toolOutput: {} };
beforeEach(() => {
  useHarnessStore.getState().flushLive();
  useHarnessStore.setState({ threads: [], items: {}, live: {}, holds: {}, queued: {}, contextDrift: {} });
});

test("deleting a thread releases retained streams and the global busy flag", () => {
  const surviving = { ...idle, streaming: "another thread" };
  const survivingRows = [{ id: 20, thread_id: 2 }] as HarnessItem[];
  useHarnessStore.setState({
    threads: [{ id: 1 }, { id: 2 }] as HarnessThread[],
    items: { 1: [{ id: 10 }] as HarnessItem[], 2: survivingRows },
    live: { 1: { ...idle, running: true, toolOutput: { tool: "large streamed output" } }, 2: surviving },
    holds: { 1: 3, 2: 1 }, queued: { 1: [{ id: "q1", text: "queued", held: false }], 2: [] },
    contextDrift: { 1: true, 2: true },
  });
  expect(anyRunning(useHarnessStore.getState().live)).toBe(true);
  useHarnessStore.getState().removed(1);
  const state = useHarnessStore.getState();
  expect(state.threads.map((t) => t.id)).toEqual([2]);
  expect(state.items).toEqual({ 2: survivingRows });
  expect(state.items[2]).toBe(survivingRows);
  expect(state.live).toEqual({ 2: surviving });
  expect(state.live[2]).toBe(surviving);
  expect(anyRunning(state.live)).toBe(false);
  expect(state.holds).toEqual({ 2: 1 });
  expect(state.queued).toEqual({ 2: [] });
  expect(state.contextDrift).toEqual({ 2: true });
});

test("a buffered delta cannot recreate a deleted thread on the next flush", () => {
  useHarnessStore.getState().apply({
    threadId: 1, provider: "claude", event: { type: "assistant_delta", text: "pending" },
  } as HarnessEnvelope);
  useHarnessStore.getState().removed(1);
  useHarnessStore.getState().flushLive();
  expect(useHarnessStore.getState().live[1]).toBeUndefined();
});

test("a turn the agent starts itself leaves a row and runs until it finishes", () => {
  useHarnessStore.setState({
    threads: [{ id: 1, status: "idle" }] as HarnessThread[],
    items: { 1: [] },
  });
  const apply = useHarnessStore.getState().apply;
  apply({
    threadId: 1, provider: "claude", itemId: 30, event: { type: "unprompted_turn", after: ["Say pong"] },
  });
  let state = useHarnessStore.getState();
  expect(state.items[1].map((i) => [i.id, i.kind, i.meta])).toEqual([
    [30, "unprompted", JSON.stringify({ after: ["Say pong"] })],
  ]);
  expect(state.live[1].running).toBe(true);
  expect(state.threads[0].status).toBe("running");

  apply({ threadId: 1, provider: "claude", itemId: null, event: { type: "turn_finished", status: "completed" } });
  state = useHarnessStore.getState();
  expect(state.live[1].running).toBe(false);
  expect(state.threads[0].status).toBe("idle");
});

/** What the open thread's announcer would say now. */
function spoken(threadId: number) {
  const s = useHarnessStore.getState();
  return spokenStatus({
    running: s.live[threadId]?.running ?? false,
    approval: false,
    input: false,
    lastRun: lastTurnEnd(s.live[threadId]?.ended, s.runs[threadId]),
  });
}

test("stopping a turn the agent started itself is said as Stopped, not as the run before it ended", () => {
  useHarnessStore.setState({
    threads: [{ id: 1, status: "idle" }] as HarnessThread[],
    items: { 1: [] },
    runs: { 1: { 4: { id: 4, status: "failed", delivery: "incomplete", answer_item_id: null, started_ms: 0, ended_ms: 1 } } },
  });
  const apply = useHarnessStore.getState().apply;
  apply({ threadId: 1, provider: "claude", itemId: 31, event: { type: "unprompted_turn", after: ["Agent"] } });
  expect(spoken(1)).toBe("working");
  apply({ threadId: 1, provider: "claude", itemId: 32, event: { type: "turn_finished", status: "interrupted" } });
  expect(useHarnessStore.getState().live[1].ended).toBe("interrupted");
  expect(spoken(1)).toBe("stopped");
});

test("a stopped run whose queue is held ends idle and Stopped, not Working", () => {
  useHarnessStore.setState({
    threads: [{ id: 1, status: "running" }] as HarnessThread[],
    items: { 1: [] },
    live: { 1: { ...idle, running: true } },
    runs: { 1: { 5: { id: 5, status: "running", delivery: "unknown", answer_item_id: null, started_ms: 0, ended_ms: null } } },
  });
  const apply = useHarnessStore.getState().apply;
  apply({ threadId: 1, provider: "claude", itemId: null, event: { type: "stop_requested", run_id: 5 } });
  // Still open while it stops: Working is true until the turn ends.
  expect(useHarnessStore.getState().runs[1][5].stopping).toBe(true);
  expect(spoken(1)).toBe("working");
  apply({ threadId: 1, provider: "claude", itemId: null, event: { type: "queue", messages: [{ id: "8", text: "next", held: true }] } });
  apply({
    threadId: 1, provider: "claude", itemId: null, runId: 5,
    event: { type: "run_settled", answer_item_id: null, work_item_ids: [], delivery: "incomplete", status: "interrupted" },
  } as HarnessEnvelope);
  apply({ threadId: 1, provider: "claude", itemId: 33, event: { type: "turn_finished", status: "interrupted" } });
  const state = useHarnessStore.getState();
  expect(state.live[1].running).toBe(false);
  expect(state.threads[0].status).toBe("idle");
  expect(spoken(1)).toBe("stopped");
});
