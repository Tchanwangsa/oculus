import { expect, test } from "bun:test";
import { artifactWidgetKey, createWidgetStore, entryState, widgetKey } from "@/stores/chat/widgetStore";

function storage(initial = "{}") {
  let value = initial;
  return { getItem: () => value, setItem: (_key: string, next: string) => { value = next; } };
}

test("widget progress survives a fresh store and isolates thread, row and fence", () => {
  const disk = storage();
  const store = createWidgetStore(disk);
  const states = [
    [widgetKey(1, 10, 0), { "$mass": 5, "progress:0": { value: { flipped: true, known: 2 } } }],
    [widgetKey(1, 10, 1), { "$mass": 9 }],
    [widgetKey(1, 11, 0), { "$mass": 12 }],
    [widgetKey(2, 10, 0), { "$mass": 13 }],
  ] as const;
  for (const [key, state] of states) store.getState().save(key, "program", state);
  const reloaded = createWidgetStore(disk).getState().entries;
  for (const [key, state] of states) expect(reloaded[key]).toEqual({ source: "program", state });
});

test("rewind prunes only removed rows; delete prunes only its thread", () => {
  const disk = storage();
  const store = createWidgetStore(disk).getState();
  for (const [thread, row] of [[1, 10], [1, 11], [1, -1], [2, 20]]) store.save(widgetKey(thread, row, 0), "program", {});
  store.prune(1, 11);
  expect(Object.keys(createWidgetStore(disk).getState().entries)).toEqual(["1:10:0", "2:20:0"]);
  store.prune(1);
  expect(Object.keys(createWidgetStore(disk).getState().entries)).toEqual(["2:20:0"]);
});

test("corrupt storage is ignored and failed writes retain in-memory progress", () => {
  expect(createWidgetStore(storage("{")).getState().entries).toEqual({});
  expect(createWidgetStore(storage('{"bad":null,"bad2":{"source":"x","state":[]}}')).getState().entries).toEqual({});
  const store = createWidgetStore({ getItem: () => { throw Error("denied"); }, setItem: () => { throw Error("full"); } });
  store.getState().save("1:10:0", "program", { "$x": 2 });
  expect(store.getState().entries["1:10:0"].state).toEqual({ "$x": 2 });
});

test("an artifact's entry survives thread pruning and goes with drop", () => {
  const disk = storage();
  const store = createWidgetStore(disk).getState();
  store.save(widgetKey(1, 10, 0), "program", {});
  store.setAsk(artifactWidgetKey(7), "program", { allowed: true, used: 3 });
  store.prune(1);
  expect(Object.keys(createWidgetStore(disk).getState().entries)).toEqual(["artifact:7"]);
  store.drop(artifactWidgetKey(7));
  store.drop(artifactWidgetKey(8));
  expect(createWidgetStore(disk).getState().entries).toEqual({});
});

test("entryState hands over the state for that source and never the grant", () => {
  const entry = { source: "program", state: { angle: 2 }, ask: { allowed: true, used: 4 } };
  expect(entryState(entry, "program")).toEqual({ angle: 2 });
  expect(entryState(entry, "other")).toEqual({});
  expect(entryState(undefined, "program")).toEqual({});
});
