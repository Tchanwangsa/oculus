import { expect, test } from "bun:test";
import { createProposalVersionStore } from "@/stores/chat/proposalVersionStore";

function storage() {
  const entries = new Map<string, string>();
  return { getItem: (key: string) => entries.get(key) ?? null, setItem: (key: string, value: string) => { entries.set(key, value); } };
}

test("switching versions parks edits separately and restores them after reload", () => {
  const disk = storage(), store = createProposalVersionStore(disk);
  const editedV2 = { notes: [{ title: "My V2", body: "" }] }, editedV1 = { notes: [{ title: "My V1" }] };
  expect(store.getState().select("20.0", 1, 20, "10.0", editedV2)).toBeUndefined();
  expect(store.getState().select("20.0", 1, 20, "20.0", editedV1)).toEqual(editedV2);
  const reloaded = createProposalVersionStore(disk);
  expect(reloaded.getState().entries["20.0"].selected).toBe("20.0");
  expect(reloaded.getState().select("20.0", 1, 20, "10.0", editedV2)).toEqual(editedV1);
});

test("reset clears only the selected version's parked edits", () => {
  const store = createProposalVersionStore();
  store.getState().select("20.0", 1, 20, "10.0", { notes: ["V2 edits"] });
  store.getState().select("20.0", 1, 20, "20.0", { notes: ["V1 edits"] });
  store.getState().reset("20.0", 1, 20, "20.0");
  expect(store.getState().entries["20.0"].drafts).toEqual({ "10.0": { notes: ["V1 edits"] } });
  store.getState().select("20.0", 1, 20, "10.0", { notes: ["V2 original"] });
  expect(store.getState().select("20.0", 1, 20, "20.0", { notes: ["V1 edits"] })).toEqual({ notes: ["V2 original"] });
});

test("rewind and delete prune parked versions only for the removed replies", () => {
  const disk = storage(), store = createProposalVersionStore(disk);
  for (const [id, thread, item] of [["10.0", 1, 10], ["20.0", 1, 20], ["30.0", 2, 30]] as const) store.getState().select(id, thread, item, id, {});
  store.getState().prune(1, 20);
  expect(Object.keys(store.getState().entries)).toEqual(["10.0", "30.0"]);
  store.getState().prune(1);
  expect(Object.keys(createProposalVersionStore(disk).getState().entries)).toEqual(["30.0"]);
});

test("malformed or unavailable local storage leaves a usable workspace", () => {
  for (const raw of ['null', '[]', '{"20.0":null}', '{"20.0":{"selected":"10.0"}}', 'bad json']) {
    expect(createProposalVersionStore({ getItem: () => raw, setItem() {} }).getState().entries).toEqual({});
  }
  const store = createProposalVersionStore({ getItem() { throw Error("unavailable"); }, setItem() { throw Error("full"); } });
  store.getState().select("20.0", 1, 20, "10.0", { notes: [] });
  expect(store.getState().entries["20.0"].selected).toBe("10.0");
});
