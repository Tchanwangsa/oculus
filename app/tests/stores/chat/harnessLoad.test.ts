import { afterAll, beforeEach, expect, mock, test } from "bun:test";
import type { HarnessEnvelope, HarnessItem, HarnessSnapshot } from "@/lib/harness";

let answer: (command: string, args: Record<string, unknown>) => Promise<unknown> = async () => {
  throw new Error("Tauri is not available");
};
mock.module("@tauri-apps/api/core", () => ({ invoke: (command: string, args: Record<string, unknown>) => answer(command, args) }));
afterAll(() => {
  // Module mocks outlive the file; leave later files an invoke that fails as it does without Tauri.
  answer = async () => { throw new Error("Tauri is not available"); };
});

const { useHarnessStore } = await import("@/stores/chat/harnessStore");

const THREAD = 7;
const row = (id: number, kind: HarnessItem["kind"], content: string): HarnessItem => ({
  id, thread_id: THREAD, kind, ref_id: null, content, meta: null, created_at: "2026-11-15 00:00:00", run_id: null,
});
const told = (revision: number, itemId: number | null, event: HarnessEnvelope["event"]): HarnessEnvelope => ({
  threadId: THREAD, provider: "codex", itemId, revision, event,
});
const ids = () => useHarnessStore.getState().items[THREAD].map((i) => i.id);

/** A `harness_load` that answers when the test says. */
function pendingLoad() {
  let resolve!: (snapshot: HarnessSnapshot) => void;
  const read = new Promise<HarnessSnapshot>((r) => (resolve = r));
  answer = async (command, args) => {
    expect(command).toBe("harness_load");
    expect(args.threadId).toBe(THREAD);
    return read;
  };
  return resolve;
}

beforeEach(() => {
  useHarnessStore.getState().removed(THREAD);
  useHarnessStore.setState({ threads: [], items: {}, runs: {}, live: {}, holds: {}, queued: {}, contextDrift: {} });
});

test("a reload keeps what was written during its read and takes nothing twice", async () => {
  const resolve = pendingLoad();
  const loading = useHarnessStore.getState().load(THREAD);
  const apply = useHarnessStore.getState().apply;
  // Told while the read is out. The snapshot, at revision 7, holds the first three.
  apply(told(5, 11, { type: "assistant_message", text: "first answer" }));
  apply(told(6, null, { type: "rewound", from_item_id: 12, context: true }));
  apply(told(7, 13, { type: "user_message", text: "asked again" }));
  apply(told(8, 14, { type: "assistant_message", text: "second answer" }));
  resolve({
    revision: 7,
    items: [row(10, "user", "asked"), row(11, "assistant", "first answer"), row(13, "user", "asked again")],
    runs: [],
    queued: [{ id: "41", text: "waiting", held: false }],
  });
  await loading;

  // 11 and 13 once each; the rewind, already in the snapshot, did not take
  // 13 (written after it, with a higher id); 14 came after the snapshot.
  expect(ids()).toEqual([10, 11, 13, 14]);
  const state = useHarnessStore.getState();
  expect(state.queued[THREAD]).toEqual([{ id: "41", text: "waiting", held: false }]);
  expect(state.live[THREAD].running).toBe(true);
});

test("an envelope the snapshot already holds changes no rows but still ends the turn", async () => {
  const resolve = pendingLoad();
  const loading = useHarnessStore.getState().load(THREAD);
  resolve({ revision: 9, items: [row(10, "user", "asked"), row(11, "assistant", "answer")], runs: [], queued: [] });
  await loading;
  const apply = useHarnessStore.getState().apply;
  apply(told(9, 10, { type: "user_message", text: "asked" }));
  expect(useHarnessStore.getState().live[THREAD].running).toBe(true);
  apply(told(8, null, { type: "rewound", from_item_id: 11, context: true }));
  apply(told(9, null, { type: "turn_finished", status: "completed" }));
  expect(ids()).toEqual([10, 11]);
  expect(useHarnessStore.getState().live[THREAD].running).toBe(false);

  // A later one is new.
  apply(told(10, 12, { type: "assistant_message", text: "more" }));
  expect(ids()).toEqual([10, 11, 12]);
});

test("the queue is replaced whole, and a list the snapshot already holds is not put back", async () => {
  const resolve = pendingLoad();
  const loading = useHarnessStore.getState().load(THREAD);
  const apply = useHarnessStore.getState().apply;
  const waiting = (id: string, text: string) => ({ id, text, held: false });
  // Told while the read is out: one message joined (in the snapshot), then went out (after it).
  apply(told(4, null, { type: "queue", messages: [waiting("41", "first"), waiting("42", "second")] }));
  apply(told(6, null, { type: "queue", messages: [waiting("42", "second")] }));
  resolve({ revision: 5, items: [], runs: [], queued: [waiting("41", "first"), waiting("42", "second, edited")] });
  await loading;
  expect(useHarnessStore.getState().queued[THREAD]).toEqual([waiting("42", "second")]);

  apply(told(5, null, { type: "queue", messages: [waiting("41", "first")] }));
  expect(useHarnessStore.getState().queued[THREAD]).toEqual([waiting("42", "second")]);
  apply(told(7, null, { type: "queue", messages: [] }));
  expect(useHarnessStore.getState().queued[THREAD]).toEqual([]);
});

test("a failed read leaves an empty thread that the stream still fills", async () => {
  answer = async () => { throw new Error("database is locked"); };
  await useHarnessStore.getState().load(THREAD);
  expect(ids()).toEqual([]);
  useHarnessStore.getState().apply(told(3, 11, { type: "assistant_message", text: "answer" }));
  expect(ids()).toEqual([11]);
});

test("a stop marks its run as stopping, and a held queue does not keep the thread working", async () => {
  const resolve = pendingLoad();
  const loading = useHarnessStore.getState().load(THREAD);
  resolve({
    revision: 3,
    items: [row(10, "user", "asked")],
    runs: [{ id: 5, execution_status: "running", delivery_state: "unknown", answer_item_id: null, started_at: "2026-11-15 00:00:00", ended_at: null }],
    queued: [{ id: "6", text: "waiting", held: false }],
  });
  await loading;
  const apply = useHarnessStore.getState().apply;
  apply(told(4, null, { type: "turn_started" }));
  apply(told(4, null, { type: "stop_requested", run_id: 5 }));
  expect(useHarnessStore.getState().runs[THREAD][5].stopping).toBe(true);
  apply(told(4, null, { type: "stop_unresponsive", run_id: 5, others: 2 }));
  expect(useHarnessStore.getState().runs[THREAD][5].unresponsive).toEqual({ others: 2 });
  // The stop held the queue before the turn ended.
  apply(told(5, null, { type: "queue", messages: [{ id: "6", text: "waiting", held: true }] }));
  apply(told(6, null, { type: "turn_finished", status: "interrupted" }));
  const state = useHarnessStore.getState();
  expect(state.live[THREAD].running).toBe(false);
  expect(state.queued[THREAD]).toEqual([{ id: "6", text: "waiting", held: true }]);
});

test("a reload mid-answer shows the text streamed so far, once, and goes on from there", async () => {
  const resolve = pendingLoad();
  const loading = useHarnessStore.getState().load(THREAD);
  const apply = useHarnessStore.getState().apply;
  const delta = (revision: number, text: string) => told(revision, null, { type: "assistant_delta", text });
  // Told while the read is out. The draft, written at revision 5, holds the first.
  apply(delta(4, "Hel"));
  apply(delta(5, "lo"));
  resolve({
    revision: 5,
    items: [row(10, "user", "asked")],
    runs: [{ id: 5, execution_status: "running", delivery_state: "unknown", answer_item_id: null, started_at: "2026-11-15 00:00:00", ended_at: null }],
    queued: [],
    draft: { run_id: 5, text: "Hel" },
  });
  await loading;
  const live = () => {
    useHarnessStore.getState().flushLive();
    return useHarnessStore.getState().live[THREAD];
  };
  expect(live().streaming).toBe("Hello");
  expect(live().running).toBe(true);

  apply(delta(5, " there"));
  // One the draft already held, delivered late, adds nothing.
  apply(delta(4, "Hel"));
  expect(live().streaming).toBe("Hello there");

  apply(told(6, 11, { type: "assistant_message", text: "Hello there" }));
  expect(live().streaming).toBe("");
  expect(ids()).toEqual([10, 11]);
});

test("a message that became a row during the read leaves no streamed copy of itself", async () => {
  const resolve = pendingLoad();
  const loading = useHarnessStore.getState().load(THREAD);
  const apply = useHarnessStore.getState().apply;
  apply(told(4, null, { type: "assistant_delta", text: "Done" }));
  apply(told(6, 11, { type: "assistant_message", text: "Done." }));
  apply(told(6, null, { type: "assistant_delta", text: "Next" }));
  // Read before the message landed: its text was still the draft.
  resolve({ revision: 5, items: [row(10, "user", "asked")], runs: [], queued: [], draft: { run_id: 5, text: "Done" } });
  await loading;
  useHarnessStore.getState().flushLive();
  expect(useHarnessStore.getState().live[THREAD].streaming).toBe("Next");
  expect(ids()).toEqual([10, 11]);
});
