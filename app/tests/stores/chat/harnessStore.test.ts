import { beforeEach, expect, test } from "bun:test";
import { anyRunning, useHarnessStore } from "@/stores/chat/harnessStore";
import type { HarnessEnvelope, HarnessItem, HarnessThread } from "@/lib/harness";

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
    holds: { 1: 3, 2: 1 }, queued: { 1: [{ id: "q1", text: "queued" }], 2: [] },
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
