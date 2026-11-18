import { expect, test } from "bun:test";
import { createJobQueue, PictureCache, Skipped, stampKey } from "@/components/drive/thumbs/pictureCache";

const tick = () => new Promise((resolve) => setTimeout(resolve, 5));

test("a changed file gets a new key", () => {
  const file = { path: "a/b.canvas", size: 10, modified: 1 };
  expect(stampKey(file)).not.toBe(stampKey({ ...file, modified: 2 }));
  expect(stampKey(file)).not.toBe(stampKey({ ...file, size: 11 }));
});

test("the least recently used picture is evicted, and only once", async () => {
  const evicted: string[] = [];
  const cache = new PictureCache<string>(2, (value) => evicted.push(value));
  const live = () => true;
  await cache.load("a", async () => "A", live);
  await cache.load("b", async () => "B", live);
  await cache.load("a", async () => "never", live);
  await cache.load("c", async () => "C", live);
  expect(evicted).toEqual(["B"]);
  expect(await cache.load("a", async () => "again", live)).toBe("A");
});

test("concurrent asks share one load, which runs while any asker is live", async () => {
  const cache = new PictureCache<string>(4);
  const queue = createJobQueue(1);
  let runs = 0;
  let firstLive = true;
  const make = (live: () => boolean) => queue(async () => { runs += 1; return "pic"; }, live);
  const block = queue(() => tick().then(() => "blocker"), () => true);
  const first = cache.load("k", make, () => firstLive);
  const second = cache.load("k", make, () => true);
  firstLive = false;
  await block;
  expect(await first).toBe("pic");
  expect(await second).toBe("pic");
  expect(runs).toBe(1);
});

test("a job whose askers all left is skipped, not cached, and runs newest first", async () => {
  const cache = new PictureCache<string>(4);
  const queue = createJobQueue(1);
  const order: string[] = [];
  const block = queue(() => tick().then(() => "blocker"), () => true);
  let goneLive = true;
  const gone = cache.load("gone", (live) => queue(async () => { order.push("gone"); return "x"; }, live), () => goneLive);
  const older = queue(async () => { order.push("older"); }, () => true);
  const newer = queue(async () => { order.push("newer"); }, () => true);
  goneLive = false;
  await block;
  await Promise.all([older, newer]);
  await expect(gone).rejects.toBeInstanceOf(Skipped);
  expect(order).toEqual(["newer", "older"]);
  expect(await cache.load("gone", async () => "fresh", () => true)).toBe("fresh");
});
