import { expect, test } from "bun:test";
import { LatestWriter } from "@/lib/companion/drafts";

test("coalesced callers wait for their version or a newer completed write", async () => {
  const writes: number[] = [];
  const releases: (() => void)[] = [];
  const writer = new LatestWriter<number>(async value => {
    writes.push(value);
    await new Promise<void>(resolve => releases.push(resolve));
    return true;
  }, () => {});
  let first = false, replaced = false, latest = false;
  const a = writer.store(1).then(() => { first = true; });
  const b = writer.store(2).then(() => { replaced = true; });
  const c = writer.store(3).then(() => { latest = true; });
  expect(writes).toEqual([1]);
  releases.shift()!();
  await a;
  expect(first).toBe(true);
  expect(replaced).toBe(false);
  expect(latest).toBe(false);
  expect(writes).toEqual([1, 3]);
  releases.shift()!();
  await Promise.all([b, c]);
  expect(replaced).toBe(true);
  expect(latest).toBe(true);
});

test("a failed write settles callers and the writer accepts a later save", async () => {
  const finished: boolean[] = [];
  const writer = new LatestWriter<number>(async value => {
    if (value === 1) throw new Error("Quota exceeded");
    return true;
  }, kept => finished.push(kept));
  await writer.store(1);
  await writer.store(2);
  expect(finished).toEqual([false, true]);
});
