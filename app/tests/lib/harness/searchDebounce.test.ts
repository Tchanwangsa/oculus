import { expect, test } from "bun:test";
import { debouncedSearch, SEARCH_DEBOUNCE_MS } from "@/lib/harness";

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

test("the default wait is about 150 ms", () => {
  expect(SEARCH_DEBOUNCE_MS).toBe(150);
});

test("searches once for the last keystroke after the wait", async () => {
  const asked: string[] = [];
  const got: (string[] | null)[] = [];
  const s = debouncedSearch(async (q) => (asked.push(q), [q]), (r) => got.push(r), 20);
  s.set("e");
  s.set("ei");
  s.set("eigen");
  await wait(5);
  expect(asked).toEqual([]);
  await wait(40);
  expect(asked).toEqual(["eigen"]);
  expect(got).toEqual([["eigen"]]);
});

test("clearing reports null at once and drops the pending search", async () => {
  const asked: string[] = [];
  const got: (string[] | null)[] = [];
  const s = debouncedSearch(async (q) => (asked.push(q), [q]), (r) => got.push(r), 20);
  s.set("eigen");
  s.set("  ");
  expect(got).toEqual([null]);
  await wait(40);
  expect(asked).toEqual([]);
});

test("an older answer arriving late never overwrites a newer one", async () => {
  const got: string[] = [];
  const slow = (q: string) => wait(q === "a" ? 60 : 1).then(() => q);
  const s = debouncedSearch(slow, (r) => r && got.push(r), 5);
  s.set("a");
  await wait(15);
  s.set("ab");
  await wait(100);
  expect(got).toEqual(["ab"]);
});

test("dispose cancels the pending search and any answer on its way", async () => {
  const got: unknown[] = [];
  const s = debouncedSearch(async (q) => q, (r) => got.push(r), 10);
  s.set("x");
  s.dispose();
  await wait(30);
  expect(got).toEqual([]);
});
