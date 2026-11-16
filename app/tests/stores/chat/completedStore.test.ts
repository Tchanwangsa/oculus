import { afterAll, beforeEach, expect, mock, spyOn, test } from "bun:test";
import * as tauriEvent from "@tauri-apps/api/event";
import * as db from "@/lib/db";
import type { HarnessThread } from "@/lib/harness";

const invoked: { cmd: string; args: Record<string, unknown> }[] = [];
let answer = async (cmd: string, args: Record<string, unknown>): Promise<unknown> => void invoked.push({ cmd, args });
mock.module("@tauri-apps/api/core", () => ({ invoke: (cmd: string, args: Record<string, unknown>) => answer(cmd, args) }));
let changed: (() => void) | null = null;
let listens = 0;
let unlistens = 0;
const realListen = tauriEvent.listen;
type Listen = (name: string, handler: () => void) => Promise<() => void>;
let listen: Listen = async (name, handler) => {
  expect(name).toBe("harness-threads-changed");
  listens++;
  changed = handler;
  return () => void unlistens++;
};
mock.module("@tauri-apps/api/event", () => ({ ...tauriEvent, listen: (name: string, handler: () => void) => listen(name, handler) }));

const thread = (id: number, patch: Partial<HarnessThread> = {}): HarnessThread =>
  ({ id, title: `t${id}`, subject_id: id % 2 ? 1 : null, provider: "claude", status: "idle", updated_at: "2026-11-01 00:00:00", ...patch }) as HarnessThread;

// 40 completed threads, newest first; odd ids in subject 1, even ones General.
let completed = Array.from({ length: 40 }, (_, i) => thread(40 - i, { completed_at: "2026-11-02 00:00:00" }));
let open: HarnessThread[] = [];
const reads: string[] = [];
const select = async (sql: string, values: unknown[] = []) => {
  reads.push(sql);
  if (sql.startsWith("SELECT subject_id, COUNT(*)")) {
    return [{ subject_id: null, count: completed.filter((t) => t.subject_id == null).length }, { subject_id: 1, count: completed.filter((t) => t.subject_id === 1).length }];
  }
  if (sql.includes("completed_at IS NOT NULL")) {
    const scoped = sql.includes("subject_id IS");
    if (sql.startsWith("SELECT COUNT(*)")) return [{ count: completed.filter((t) => !scoped || t.subject_id === values[0]).length }];
    const [limit, offset, subject] = values as [number, number, number | null];
    return completed.filter((t) => !scoped || t.subject_id === subject).slice(offset, offset + limit);
  }
  if (sql.includes("completed_at IS NULL")) return open;
  if (sql.includes("WHERE t.id IN")) return completed.filter((t) => (values as number[]).includes(t.id));
  throw new Error(`unexpected read: ${sql}`);
};
const getDb = spyOn(db, "getDb").mockResolvedValue({ select } as unknown as Awaited<ReturnType<typeof db.getDb>>);
afterAll(() => {
  getDb.mockRestore();
  // Module mocks outlive the file; leave later files Tauri's absence and the real listen.
  answer = async () => { throw new Error("Tauri is not available"); };
  listen = realListen as unknown as Listen;
});

const { useCompletedStore, COMPLETED_FIRST } = await import("@/stores/chat/completedStore");
const { useHarnessStore } = await import("@/stores/chat/harnessStore");
const { watchThreadsChanged, threadsChanged } = await import("@/stores/chat/threadsChanged");
const ids = (threads: HarnessThread[] | undefined) => threads?.map((t) => t.id) ?? [];

beforeEach(() => {
  completed = Array.from({ length: 40 }, (_, i) => thread(40 - i, { completed_at: "2026-11-02 00:00:00" }));
  open = [];
  invoked.length = 0;
  reads.length = 0;
  useCompletedStore.setState({ counts: {}, all: null, bySubject: {} });
  useHarnessStore.setState({ threads: [], holds: {}, threadsLoaded: false });
});

test("the Status fold reads 10, then 25 more each time, to the end", async () => {
  const s = useCompletedStore.getState();
  await s.loadAll();
  expect(COMPLETED_FIRST).toBe(10);
  expect(ids(useCompletedStore.getState().all?.threads)).toEqual([40, 39, 38, 37, 36, 35, 34, 33, 32, 31]);
  expect(useCompletedStore.getState().all?.total).toBe(40);
  await s.moreAll();
  expect(useCompletedStore.getState().all?.threads).toHaveLength(35);
  expect(useCompletedStore.getState().all?.nextOffset).toBe(35);
  await s.moreAll();
  expect(useCompletedStore.getState().all?.threads).toHaveLength(40);
  expect(useCompletedStore.getState().all?.nextOffset).toBeNull();
  const before = reads.length;
  await s.moreAll();
  expect(reads.length).toBe(before);
});

test("counts key by subject id and General; a subject fold pages its own read", async () => {
  const s = useCompletedStore.getState();
  await s.loadCounts();
  expect(useCompletedStore.getState().counts).toEqual({ general: 20, "1": 20 });
  await s.loadSubject(null);
  const general = useCompletedStore.getState().bySubject.general;
  expect(general.page.threads.every((t) => t.subject_id == null)).toBe(true);
  expect(general.page.threads).toHaveLength(10);
  await s.moreSubject(null);
  expect(useCompletedStore.getState().bySubject.general.page.threads).toHaveLength(20);
  expect(useCompletedStore.getState().bySubject["1"]).toBeUndefined();
});

test("refresh re-reads every fold at the length it shows", async () => {
  const s = useCompletedStore.getState();
  await s.loadAll();
  await s.moreAll();
  await s.loadSubject(1);
  // One thread reopened elsewhere.
  completed = completed.filter((t) => t.id !== 39);
  await s.refresh();
  const st = useCompletedStore.getState();
  expect(st.all?.threads).toHaveLength(35);
  expect(ids(st.all?.threads)).not.toContain(39);
  expect(st.all?.total).toBe(39);
  expect(ids(st.bySubject["1"].page.threads)).not.toContain(39);
  expect(st.counts["1"]).toBe(19);
});

test("Complete and Reopen send their commands with the thread id", async () => {
  await useCompletedStore.getState().complete(7);
  await useCompletedStore.getState().reopen(9);
  expect(invoked.map((c) => [c.cmd, c.args.threadId])).toEqual([
    ["harness_complete_thread", 7],
    ["harness_reopen_thread", 9],
  ]);
  expect(invoked.every((c) => typeof c.args.commandId === "string")).toBe(true);
});

test("harness-threads-changed re-reads the open list and the folds, one listener for every column", async () => {
  const offA = watchThreadsChanged();
  const offB = watchThreadsChanged();
  await Promise.resolve();
  expect(listens).toBe(1);
  await useCompletedStore.getState().loadAll();
  open = [thread(1)];
  completed = completed.filter((t) => t.id !== 40);
  reads.length = 0;
  changed?.();
  await new Promise((r) => setTimeout(r, 0));
  expect(ids(useHarnessStore.getState().threads)).toEqual([1]);
  expect(useCompletedStore.getState().all?.threads[0].id).toBe(39);
  offA();
  expect(unlistens).toBe(0);
  offB();
  await Promise.resolve();
  expect(unlistens).toBe(1);
});

test("a held thread that completed stays in the store, marked completed", async () => {
  open = [thread(1)];
  useHarnessStore.setState({ holds: { 40: 1 } });
  await threadsChanged();
  const threads = useHarnessStore.getState().threads;
  expect(ids(threads)).toEqual([1, 40]);
  expect(threads[1].completed_at).toBeTruthy();
  // Not held any more: the next read drops it.
  useHarnessStore.setState({ holds: {} });
  await useHarnessStore.getState().loadThreads();
  expect(ids(useHarnessStore.getState().threads)).toEqual([1]);
});

test("keep lists a completed thread once", () => {
  const t = thread(40, { completed_at: "2026-11-02 00:00:00" });
  useHarnessStore.getState().keep(t);
  useHarnessStore.getState().keep(t);
  expect(ids(useHarnessStore.getState().threads)).toEqual([40]);
});
