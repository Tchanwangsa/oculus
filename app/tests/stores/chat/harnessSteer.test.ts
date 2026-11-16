import { afterAll, beforeEach, expect, mock, test } from "bun:test";
import type { HarnessEnvelope } from "@/lib/harness";

const priorStorage = globalThis.localStorage;
let storage = new Map<string, string>();

let answer: (command: string, args: Record<string, unknown>) => Promise<unknown> = async () => {
  throw new Error("Tauri is not available");
};
// The rest of the module stays real: later test files import it too.
const tauri = { ...(await import("@tauri-apps/api/core")) };
mock.module("@tauri-apps/api/core", () => ({ ...tauri, invoke: (command: string, args: Record<string, unknown>) => answer(command, args) }));
afterAll(() => {
  Object.assign(globalThis, { localStorage: priorStorage });
  answer = async () => { throw new Error("Tauri is not available"); };
});

const { useHarnessStore } = await import("@/stores/chat/harnessStore");

const THREAD = 11;
const told = (revision: number, itemId: number | null, event: HarnessEnvelope["event"], runId = 1): HarnessEnvelope => ({
  threadId: THREAD, provider: "codex", itemId, runId, revision, event,
});
const meta = (id: number) => JSON.parse(useHarnessStore.getState().items[THREAD].find((r) => r.id === id)?.meta ?? "null");

beforeEach(() => {
  storage = new Map();
  Object.assign(globalThis, { localStorage: { getItem: (key: string) => storage.get(key) ?? null, setItem: (key: string, value: string) => { storage.set(key, value); } } });
  useHarnessStore.getState().removed(THREAD);
  answer = async () => ({ revision: 1, items: [], runs: [], queued: [], requests: [] });
});

test("a steer is a row of the running run whose delivery follows every update", async () => {
  await useHarnessStore.getState().load(THREAD);
  const { fold } = useHarnessStore.getState();
  fold(told(2, 5, { type: "user_message", text: "question" }));
  fold(told(3, 7, { type: "steer", text: "in metres", message: 2, client: "c-1", delivery: "sending" }));
  const row = useHarnessStore.getState().items[THREAD].find((r) => r.id === 7);
  expect([row?.kind, row?.run_id, row?.content]).toEqual(["steer", 1, "in metres"]);
  expect(meta(7)).toEqual({ message: 2, client: "c-1", delivery: "sending" });
  fold(told(4, null, { type: "steer_updated", item_id: 7, message: 2, delivery: "written" }));
  fold(told(5, null, { type: "steer_updated", item_id: 7, message: 2, delivery: "delivered" }));
  expect(meta(7).delivery).toBe("delivered");
  // A steer never ends the run: it is still running.
  expect(useHarnessStore.getState().runs[THREAD][1].status).toBe("running");
});

test("a steer that missed its turn leaves the timeline for the queue", async () => {
  await useHarnessStore.getState().load(THREAD);
  const { fold } = useHarnessStore.getState();
  fold(told(2, 5, { type: "user_message", text: "question" }));
  fold(told(3, 7, { type: "steer", text: "late", message: 2, client: "c-1", delivery: "sending" }));
  fold(told(4, null, { type: "steer_updated", item_id: 7, message: 2, delivery: "requeued" }));
  fold(told(4, null, { type: "queue", messages: [{ id: "2", text: "late", held: false }] }));
  expect(useHarnessStore.getState().items[THREAD].map((r) => r.id)).toEqual([5]);
  expect(useHarnessStore.getState().queued[THREAD]).toEqual([{ id: "2", text: "late", held: false }]);
});

test("a restarted attempt folds the old attempt's replies as work and keeps the run going", async () => {
  await useHarnessStore.getState().load(THREAD);
  const { fold } = useHarnessStore.getState();
  fold(told(2, 5, { type: "user_message", text: "question" }));
  fold(told(3, 6, { type: "assistant_message", text: "feet" }));
  fold(told(4, 7, { type: "steer", text: "metres", message: 2, client: "c-1", delivery: "restart" }));
  fold(told(5, null, { type: "steer_updated", item_id: 7, message: 2, delivery: "delivered" }));
  fold(told(5, null, { type: "attempt_restarted", run_id: 1, attempt_id: 9, superseded: 8, work_item_ids: [6] }));
  const rows = useHarnessStore.getState().items[THREAD];
  expect(rows.find((r) => r.id === 6)?.display_role).toBe("commentary");
  expect(useHarnessStore.getState().live[THREAD].running).toBe(true);
});
