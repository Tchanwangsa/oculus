import { afterAll, beforeEach, expect, mock, test } from "bun:test";
import type { HarnessEnvelope, HarnessRequest } from "@/lib/harness";

let answer: (command: string, args: Record<string, unknown>) => Promise<unknown> = async () => {
  throw new Error("Tauri is not available");
};
mock.module("@tauri-apps/api/core", () => ({ invoke: (command: string, args: Record<string, unknown>) => answer(command, args) }));
afterAll(() => {
  answer = async () => { throw new Error("Tauri is not available"); };
});

const { useHarnessStore } = await import("@/stores/chat/harnessStore");

const THREAD = 9;
const request = (patch: Partial<HarnessRequest> = {}): HarnessRequest => ({
  id: 3, thread_id: THREAD, run_id: 1, item_id: 12, provider: "codex", kind: "approval", action: "command",
  payload: { command: "touch out.txt" },
  options: [{ id: "accept", label: "Approve", decision: "accept" }, { id: "decline", label: "Decline", decision: "decline" }],
  status: "pending", responder: "live", reason: null, response: null, delivery: "none",
  created_at: "2026-11-16 00:00:00", resolved_at: null, ...patch,
});
const told = (revision: number, itemId: number | null, event: HarnessEnvelope["event"]): HarnessEnvelope => ({
  threadId: THREAD, provider: "codex", itemId, revision, event,
});

beforeEach(() => {
  useHarnessStore.getState().removed(THREAD);
  answer = async () => ({ revision: 1, items: [], runs: [], queued: [], requests: [] });
});

test("a request's row holds its id and its state follows every update", async () => {
  await useHarnessStore.getState().load(THREAD);
  const { fold } = useHarnessStore.getState();
  fold(told(2, 12, { type: "request_opened", request: request() }));
  const rows = useHarnessStore.getState().items[THREAD];
  expect(rows.map((r) => [r.id, r.kind, r.ref_id])).toEqual([[12, "request", "3"]]);
  expect(useHarnessStore.getState().requests[THREAD][3].status).toBe("pending");
  fold(told(3, null, { type: "request_updated", request: request({ status: "resolved", response: "accept", delivery: "sending" }) }));
  fold(told(4, null, { type: "request_updated", request: request({ status: "resolved", response: "accept", delivery: "acknowledged" }) }));
  expect(useHarnessStore.getState().requests[THREAD][3].delivery).toBe("acknowledged");
  expect(useHarnessStore.getState().items[THREAD].length).toBe(1);
});

test("a reload reads the requests and a rewind takes those whose rows went", async () => {
  answer = async () => ({
    revision: 5, items: [], runs: [], queued: [],
    requests: [request(), request({ id: 4, item_id: 20, status: "expired", reason: "The app was closed." })],
  });
  await useHarnessStore.getState().load(THREAD);
  expect(Object.keys(useHarnessStore.getState().requests[THREAD])).toEqual(["3", "4"]);
  useHarnessStore.getState().fold(told(6, null, { type: "rewound", from_item_id: 15, context: true }));
  expect(Object.keys(useHarnessStore.getState().requests[THREAD])).toEqual(["3"]);
});

test("the chat's permission mode follows its event", () => {
  useHarnessStore.setState({ threads: [{ id: THREAD, provider: "codex" } as never] });
  useHarnessStore.getState().fold(told(2, null, { type: "permission_mode", mode: "auto" }));
  expect(useHarnessStore.getState().threads[0].permission_mode).toBe("auto");
});
