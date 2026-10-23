import { expect, test } from "bun:test";
import { createProposalStore } from "@/stores/chat/proposalStore";
import type { Proposal, ProposalApi, ProposalState } from "@/lib/harness/proposals";

const row = (patch: Partial<Proposal> = {}): Proposal => ({ id: "10.0", item_id: 10, block: 0, thread_id: 1, kind: "NewTasks", status: "pending", state: { project: null, items: [{ title: "Original" }] }, told: null, updated_at: "2026-10-24T00:00:00Z", made: [], ...patch });
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>((done) => { resolve = done; }); return { promise, resolve }; }
const api = (patch: Partial<ProposalApi> = {}): ProposalApi => ({
  open: async () => row(), edit: async (_id, state) => row({ state }), apply: async (_id, state) => row({ state, status: "created" }),
  discard: async () => row({ status: "discarded" }), undo: async () => row({ status: "undone" }), start: async () => row({ status: "created" }), ...patch,
});

test("a decision waits for in-flight edits and applies the latest shared draft once", async () => {
  const first = deferred<Proposal>();
  const started = deferred<void>();
  const calls: ProposalState[] = [];
  const store = createProposalStore(api({ edit: async () => { started.resolve(); return first.promise; }, apply: async (_id, state) => { calls.push(state); return row({ state, status: "created" }); } }), 60_000);
  store.getState().receive(row());
  store.getState().edit("10.0", { project: null, items: [{ title: "First" }] });
  const flush = store.getState().flush("10.0");
  await started.promise;
  store.getState().edit("10.0", { project: null, items: [{ title: "Latest", included: false }] });
  const apply = store.getState().act("10.0", "apply");
  const duplicate = store.getState().act("10.0", "apply");
  first.resolve(row({ state: { project: null, items: [{ title: "First" }] } }));
  await Promise.all([flush, apply, duplicate]);
  expect(calls).toEqual([{ project: null, items: [{ title: "Latest", included: false }] }]);
  expect(store.getState().rows["10.0"].status).toBe("created");
  expect(store.getState().drafts["10.0"]).toBeUndefined();
});

test("decisions from another view clear a local draft and late pending events cannot rearm Create", () => {
  const store = createProposalStore(api(), 60_000);
  store.getState().receive(row());
  store.getState().edit("10.0", { items: [] });
  store.getState().receive(row({ status: "created" }));
  store.getState().receive(row());
  expect(store.getState().rows["10.0"].status).toBe("created");
  expect(store.getState().drafts).toEqual({});
});

test("prune during open, edit or apply never restores a removed card", async () => {
  for (const action of ["open", "edit", "apply"] as const) {
    const pending = deferred<Proposal>();
    const store = createProposalStore(api({ [action]: async () => pending.promise }), 60_000);
    let work: Promise<void>;
    if (action === "open") work = store.getState().open(1, 10, 0, "NewTasks", {});
    else {
      store.getState().receive(row());
      if (action === "edit") { store.getState().edit("10.0", { items: [] }); work = store.getState().flush("10.0"); }
      else work = store.getState().act("10.0", "apply");
      await Promise.resolve();
    }
    store.getState().prune(1, 10);
    pending.resolve(row({ status: action === "apply" ? "created" : "pending" }));
    await work;
    expect(store.getState().rows).toEqual({});
    expect(store.getState().drafts).toEqual({});
  }
});

test("failed apply leaves the pending card and edited state available with an inline error", async () => {
  const store = createProposalStore(api({ apply: async () => { throw new Error("changed since: due"); } }), 60_000);
  store.getState().receive(row());
  const state = { project: null, items: [{ title: "My edits" }] };
  store.getState().edit("10.0", state);
  await store.getState().act("10.0", "apply");
  expect(store.getState().rows["10.0"].status).toBe("pending");
  expect(store.getState().drafts["10.0"]).toEqual(state);
  expect(store.getState().errors["10.0"]).toContain("changed since");
  expect(store.getState().busy["10.0"]).toBe(false);
});

test("late created responses cannot reverse Undo and running jobs cannot replace their completion", () => {
  const store = createProposalStore(api());
  store.getState().receive(row({ status: "created" }));
  store.getState().receive(row({ status: "undone" }));
  store.getState().receive(row({ status: "created" }));
  expect(store.getState().rows["10.0"].status).toBe("undone");
  for (const phase of ["complete", "error"]) {
    const id = phase === "complete" ? "11.0" : "12.0";
    store.getState().receive(row({ id, status: "created", state: { job: { phase } } }));
    store.getState().receive(row({ id, status: "created", state: { job: { phase: "running" } } }));
    expect(store.getState().rows[id].state.job).toEqual({ phase });
  }
});

test("thread flush also waits for an Undo already in flight", async () => {
  const response = deferred<Proposal>(), started = deferred<void>();
  const store = createProposalStore(api({ undo: async () => { started.resolve(); return response.promise; } }));
  store.getState().receive(row({ status: "created" }));
  const undo = store.getState().act("10.0", "undo");
  await started.promise;
  let flushed = false;
  const flush = store.getState().flushThread(1).then(() => { flushed = true; });
  await Promise.resolve();
  expect(flushed).toBe(false);
  response.resolve(row({ status: "undone" }));
  await Promise.all([undo, flush]);
  expect(store.getState().rows["10.0"].status).toBe("undone");
});
