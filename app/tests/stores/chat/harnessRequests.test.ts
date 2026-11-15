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
  fold(told(2, 12, { type: "request_opened", request: request(), waiting: 1 }));
  const rows = useHarnessStore.getState().items[THREAD];
  expect(rows.map((r) => [r.id, r.kind, r.ref_id])).toEqual([[12, "request", "3"]]);
  expect(useHarnessStore.getState().requests[THREAD][3].status).toBe("pending");
  fold(told(3, null, { type: "request_updated", request: request({ status: "resolved", response: "accept", delivery: "sending" }), waiting: 0 }));
  fold(told(4, null, { type: "request_updated", request: request({ status: "resolved", response: "accept", delivery: "acknowledged" }), waiting: 0 }));
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

test("a listed thread's waiting count follows each request event, opened or not, without reordering the list", () => {
  const updated_at = "2026-11-16 00:00:00";
  useHarnessStore.setState({ threads: [{ id: THREAD, provider: "codex", pending_requests: 0, updated_at } as never] });
  const { fold } = useHarnessStore.getState();
  const thread = () => useHarnessStore.getState().threads[0];
  fold(told(2, 12, { type: "request_opened", request: request(), waiting: 1 }));
  fold(told(3, 13, { type: "request_opened", request: request({ id: 4, item_id: 13 }), waiting: 2 }));
  expect(thread().pending_requests).toBe(2);
  expect(THREAD in useHarnessStore.getState().items).toBe(false);
  fold(told(4, null, { type: "request_updated", request: request({ status: "resolved", response: "accept", delivery: "sending" }), waiting: 1 }));
  expect(thread().pending_requests).toBe(1);
  fold(told(5, null, { type: "request_updated", request: request({ id: 4, status: "cancelled", responder: "not_resumable" }), waiting: 0 }));
  expect(thread().pending_requests).toBe(0);
  expect(thread().updated_at).toBe(updated_at);
});

test("the chat's permission mode follows its event", () => {
  useHarnessStore.setState({ threads: [{ id: THREAD, provider: "codex" } as never] });
  useHarnessStore.getState().fold(told(2, null, { type: "permission_mode", mode: "auto" }));
  expect(useHarnessStore.getState().threads[0].permission_mode).toBe("auto");
});

test("a reviewer's decision lands on its tool row's meta", async () => {
  answer = async () => ({
    revision: 1, runs: [], queued: [], requests: [],
    items: [{ id: 5, thread_id: THREAD, kind: "tool", ref_id: "exec-1", content: "touch x", meta: JSON.stringify({ kind: "bash", ok: null }), created_at: "", run_id: 1 }],
  });
  await useHarnessStore.getState().load(THREAD);
  const review = { status: "denied" as const, risk: "medium", rationale: "too broad" };
  useHarnessStore.getState().fold(told(2, 5, { type: "tool_reviewed", id: "exec-1", review }));
  const meta = JSON.parse(useHarnessStore.getState().items[THREAD][0].meta ?? "{}");
  expect(meta.review).toEqual(review);
  expect(meta.kind).toBe("bash");
});

test("a reload restores a live request as one the composer answers, and an ended one as settled", async () => {
  const { requestActions, requestState, waitingRequests } = await import("@/lib/harness");
  const row = { id: 12, thread_id: THREAD, kind: "request" as const, ref_id: "3", content: null, meta: null, created_at: "", run_id: 1 };
  answer = async () => ({
    revision: 7, items: [row], runs: [], queued: [],
    requests: [request(), request({ id: 4, item_id: 20, status: "expired", responder: "not_resumable", reason: "The app was closed." })],
  });
  await useHarnessStore.getState().load(THREAD);
  const s = useHarnessStore.getState();
  expect(s.items[THREAD][0].ref_id).toBe("3");
  const live = s.requests[THREAD][3];
  expect(requestState(live).kind).toBe("waiting");
  expect(requestActions(live).approve?.id).toBe("accept");
  expect(waitingRequests(s.requests[THREAD]).map((r) => r.id)).toEqual([3]);
  const ended = s.requests[THREAD][4];
  expect(requestState(ended)).toMatchObject({ kind: "expired", notes: ["The app was closed."] });
  expect(requestActions(ended).approve).toBeNull();
});
