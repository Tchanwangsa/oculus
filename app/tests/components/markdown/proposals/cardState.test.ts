import { describe, expect, test } from "bun:test";
import { cardState, type CardInputs } from "@/components/markdown/openui/proposals/cardState";
import { excludeStale, initialProposal } from "@/components/markdown/openui/proposals/model";
import { createProposalStore } from "@/stores/chat/proposalStore";
import type { Proposal, ProposalApi } from "@/lib/harness/proposals";

const at = (patch: Partial<CardInputs>) => cardState({ streaming: false, id: "10.0", busy: false, ...patch });

describe("card state machine", () => {
  test("each step from streaming to a decision has one phase", () => {
    expect(at({ streaming: true, id: null }).phase).toBe("streaming");
    expect(at({ id: null }).phase).toBe("uncommitted");
    expect(at({}).phase).toBe("loading");
    expect(at({ status: "pending" }).phase).toBe("pending");
    expect(at({ status: "pending", busy: true }).phase).toBe("applying");
    expect(at({ status: "created" }).phase).toBe("created");
    expect(at({ status: "created", busy: true }).phase).toBe("applying");
    expect(at({ status: "undone" }).phase).toBe("undone");
  });

  test("a streaming fence wins over a committed row", () => {
    expect(at({ streaming: true, status: "pending" }).phase).toBe("streaming");
  });

  test("only a pending card, or one whose Apply was refused, takes input", () => {
    const editable = (patch: Partial<CardInputs>) => at(patch).editable;
    expect(editable({ status: "pending" })).toBe(true);
    expect(editable({ status: "pending", error: "project 12 no longer exists" })).toBe(true);
    for (const patch of [{ streaming: true }, { id: null }, {}, { error: "database is locked" }, { status: "pending" as const, busy: true }, { status: "created" as const }, { status: "undone" as const }]) expect(editable(patch)).toBe(false);
  });

  test("failures say what failed", () => {
    expect(at({ error: "database is locked" })).toMatchObject({ phase: "failed", failure: "load" });
    expect(at({ status: "pending", error: "refused" })).toMatchObject({ phase: "failed", failure: "apply" });
    expect(at({ status: "created", job: "error" })).toMatchObject({ phase: "failed", failure: "job" });
    // An Undo that fails leaves the card created, with the error beside it.
    expect(at({ status: "created", error: "undo refused" }).phase).toBe("created");
  });

  test("a job's run is the applying phase and its end is created", () => {
    expect(at({ status: "created", job: "running" }).phase).toBe("applying");
    expect(at({ status: "created", job: "complete" }).phase).toBe("created");
  });

  test("discarded and superseded collapse; nothing else does", () => {
    expect(at({ status: "discarded" }).collapsed).toBe(true);
    expect(at({ status: "superseded" }).collapsed).toBe(true);
    expect(at({ status: "discarded", busy: true }).phase).toBe("discarded");
    expect(at({ status: "undone" }).collapsed).toBe(false);
  });
});

describe("a stale change beside an unchanged one", () => {
  // Task 46 was renamed in the app after the proposal; task 43 was not touched.
  const state = initialProposal("TaskChanges", { changes: [
    { id: 46, from: { title: "Write project retrospective", due: "2026-11-14" }, to: { title: "Write retrospective", due: "2026-11-16" } },
    { id: 43, from: { estimate: 60 }, to: { estimate: 90 } },
  ] });
  const live = { "46": { id: 46, title: "Retro notes (renamed in app)", due: "2026-11-14" }, "43": { id: 43, title: "Individual contribution report", estimate: 60 } };

  test("only the renamed task starts excluded, and the other row keeps its identity", () => {
    const values = state.changes as Record<string, unknown>[];
    const next = excludeStale(values, live);
    expect(next[0]).toMatchObject({ id: 46, included: false });
    expect(next[1]).toBe(values[1]);
    expect(excludeStale(next, live)).toBe(next);
  });

  test("saving that exclusion leaves the card error-free", async () => {
    const row = (patch: Partial<Proposal> = {}): Proposal => ({ id: "804.0", item_id: 804, block: 0, thread_id: 1, kind: "TaskChanges", status: "pending", state, told: null, updated_at: "", made: [], ...patch });
    const api: ProposalApi = { open: async () => row(), edit: async (_id, next) => row({ state: next }), apply: async (_id, next) => row({ state: next, status: "created" }), discard: async () => row({ status: "discarded" }), undo: async () => row({ status: "undone" }), start: async () => row() };
    const store = createProposalStore(api, 0);
    store.getState().receive(row());
    const next = { ...state, changes: excludeStale(state.changes as Record<string, unknown>[], live) };
    store.getState().edit("804.0", next);
    await store.getState().flush("804.0");
    expect(store.getState().errors["804.0"]).toBeUndefined();
    expect(store.getState().rows["804.0"].state).toEqual(next);
    await store.getState().act("804.0", "apply");
    expect(store.getState().errors["804.0"]).toBeUndefined();
    expect(store.getState().rows["804.0"].status).toBe("created");
  });
});

test("a busy or unready card locks its body; a running job does not", () => {
  const locked = (patch: Partial<CardInputs>) => cardState({ streaming: false, id: "10.0", busy: false, ...patch }).locked;
  expect(locked({ streaming: true })).toBe(true);
  expect(locked({ id: null })).toBe(true);
  expect(locked({})).toBe(true);
  expect(locked({ error: "database is locked" })).toBe(true);
  expect(locked({ status: "pending", busy: true })).toBe(true);
  expect(locked({ status: "created", job: "running" })).toBe(false);
  expect(locked({ status: "pending" })).toBe(false);
  expect(locked({ status: "created" })).toBe(false);
});
