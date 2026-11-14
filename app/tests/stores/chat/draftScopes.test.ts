import { afterEach, beforeEach, expect, test } from "bun:test";
import { storedScopes, useDraftStore } from "@/stores/chat/draftStore";

const previous = (globalThis as { localStorage?: Storage }).localStorage;
let storage: Map<string, string>;
beforeEach(() => {
  storage = new Map();
  Object.assign(globalThis, {
    localStorage: {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => void storage.set(key, value),
    },
  });
  useDraftStore.setState({ scopes: {} });
});
afterEach(() => {
  Object.assign(globalThis, { localStorage: previous });
});

const subjects = [{ id: 7 }, { id: 9 }];
const state = () => useDraftStore.getState();

test("unchosen, General and a subject round-trip through storage", () => {
  expect(state().scopeFor("new:chat", subjects)).toBeNull();
  state().setScope("new:chat", "general");
  state().setScope("new:studio", 9);
  // What the next launch reads.
  expect(storedScopes()).toEqual({ "new:chat": "general", "new:studio": 9 });
  useDraftStore.setState({ scopes: storedScopes() });
  expect(state().scopeFor("new:chat", subjects)).toBe("general");
  expect(state().scopeFor("new:studio", subjects)).toBe(9);
  expect(state().scopeFor("new:lecture:x", subjects)).toBeNull();
});

test("forgetting a choice makes it unchosen again, not General", () => {
  state().setScope("new:chat", 7);
  state().setScope("new:chat", null);
  expect(state().scopeFor("new:chat", subjects)).toBeNull();
  expect(storedScopes()).toEqual({});
});

test("a deleted subject's id reads as unchosen", () => {
  state().setScope("new:chat", 12);
  expect(state().scopes["new:chat"]).toBe(12);
  expect(state().scopeFor("new:chat", subjects)).toBeNull();
});

test("damaged stored entries are dropped on load", () => {
  storage.set("oculus.chatScopes", JSON.stringify({ a: "general", b: 4, c: null, d: "4", e: -1, f: 1.5, g: "General" }));
  expect(storedScopes()).toEqual({ a: "general", b: 4 });
  storage.set("oculus.chatScopes", "[1]");
  expect(storedScopes()).toEqual({});
});
