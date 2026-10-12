import { describe, expect, test } from "bun:test";

import { createWorkspaceSaver, type SaveState } from "@/lib/ed/workspaceSaver";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

const tick = (ms = 0) => new Promise((resolve) => setTimeout(resolve, ms));

/** A Rust side whose saves resolve only when the test says so. */
function fakeRust() {
  const disk: Record<string, string> = {};
  const writes: { path: string; text: string; done: ReturnType<typeof deferred<"ed" | "local">> }[] = [];
  const saver = createWorkspaceSaver({
    delay: 20,
    write: async (path, text) => {
      const done = deferred<"ed" | "local">();
      writes.push({ path, text, done });
      const where = await done.promise;
      disk[path] = text;
      return where;
    },
  });
  const statuses: SaveState["state"][] = [];
  saver.subscribe({ status: (s) => statuses.push(s.state) });
  return { saver, disk, writes, statuses };
}

describe("workspace saver", () => {
  test("an edit is written once after typing stops", async () => {
    const { saver, writes } = fakeRust();
    saver.loaded("a.py", "");
    saver.edit("a.py", "x");
    saver.edit("a.py", "xy");
    expect(writes.length).toBe(0);
    await tick(40);
    expect(writes.map((w) => w.text)).toEqual(["xy"]);
  });

  test("an edit to a file not yet read is ignored", async () => {
    const { saver, writes } = fakeRust();
    saver.edit("a.py", "x");
    await saver.flush();
    expect(writes.length).toBe(0);
    expect(saver.dirty()).toBe(false);
  });

  test("one write at a time, looping until every file matches", async () => {
    const { saver, writes, disk, statuses } = fakeRust();
    saver.loaded("a.py", "");
    saver.loaded("b.py", "");
    saver.edit("a.py", "a");
    const first = saver.flush();
    saver.edit("a.py", "ab");
    saver.edit("b.py", "b");
    expect(saver.flush()).toBe(first);
    expect(writes.map((w) => w.text)).toEqual(["a"]);

    writes[0].done.resolve("ed");
    await tick();
    expect(writes.map((w) => `${w.path}:${w.text}`)).toEqual(["a.py:a", "a.py:ab"]);
    writes[1].done.resolve("ed");
    await tick();
    expect(writes.map((w) => `${w.path}:${w.text}`)).toEqual(["a.py:a", "a.py:ab", "b.py:b"]);
    writes[2].done.resolve("local");
    await first;
    expect(disk).toEqual({ "a.py": "ab", "b.py": "b" });
    expect(saver.dirty()).toBe(false);
    expect(statuses.at(-1)).toBe("saved");
    expect(saver.status).toEqual({ state: "saved", where: "local" });
  });

  test("a rejected save shows and waits for the next edit or flush", async () => {
    const { saver, writes } = fakeRust();
    saver.loaded("a.py", "");
    saver.edit("a.py", "x");
    const run = saver.flush();
    writes[0].done.reject(new Error("upload failed"));
    await run;
    expect(saver.status).toEqual({ state: "error", message: "Error: upload failed" });
    expect(saver.dirty("a.py")).toBe(true);
    await tick(40);
    expect(writes.length).toBe(1);

    const again = saver.flush();
    writes[1].done.resolve("ed");
    await again;
    expect(saver.status).toEqual({ state: "saved", where: "ed" });
  });

  test("another client's text is taken only when nothing is unsaved", async () => {
    const { saver, writes } = fakeRust();
    const seen: string[] = [];
    saver.subscribe({ text: (path, text) => seen.push(`${path}:${text}`) });
    saver.loaded("a.py", "old");
    expect(saver.external("a.py", "theirs")).toBe(true);
    expect(saver.text("a.py")).toBe("theirs");
    expect(saver.dirty("a.py")).toBe(false);

    saver.edit("a.py", "mine");
    expect(saver.external("a.py", "theirs again")).toBe(false);
    expect(saver.text("a.py")).toBe("mine");
    expect(seen).toEqual(["a.py:theirs", "a.py:mine"]);

    const run = saver.flush();
    writes[0].done.resolve("ed");
    await run;
    expect(saver.external("a.py", "later")).toBe(true);
    expect(saver.external("b.py", "never read")).toBe(false);
  });

  test("a read that lands after an edit keeps the edit", () => {
    const { saver } = fakeRust();
    saver.loaded("a.py", "disk");
    saver.edit("a.py", "typed");
    expect(saver.loaded("a.py", "disk")).toBe("typed");
  });

  test("forgetting drops clean buffers but never unsaved ones or those kept", () => {
    const { saver } = fakeRust();
    saver.loaded("a.py", "a");
    saver.loaded("b.py", "b");
    saver.loaded("c.py", "c");
    saver.edit("b.py", "b2");
    saver.forgetClean((p) => p === "c.py");
    expect(saver.text("a.py")).toBeUndefined();
    expect(saver.text("b.py")).toBe("b2");
    expect(saver.text("c.py")).toBe("c");
  });
});
