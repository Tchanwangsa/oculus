import { describe, expect, test } from "bun:test";
import { SubjectSelectionWrites } from "../src/lib/subjectSelection";

function deferred() {
  let resolve!: () => void;
  let reject!: (reason: Error) => void;
  const promise = new Promise<void>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

describe("shared subject selection writes", () => {
  test("pending edits protect reads, then stop masking committed external choices", async () => {
    const gate = deferred();
    const writes = new SubjectSelectionWrites(() => gate.promise);
    const beforeEdit = writes.revision;
    const save = writes.set(1, true);
    expect(writes.reconcile([{ id: 1, selected: false }], beforeEdit)).toBeNull();
    const beforeCommit = writes.revision;
    expect([...writes.reconcile([{ id: 1, selected: false }], beforeCommit)!]).toEqual([1]);
    gate.resolve();
    await save;
    expect(writes.reconcile([{ id: 1, selected: false }], beforeCommit)).toBeNull();
    // A fresh read from another tab/CLI is authoritative after this edit lands.
    expect([...writes.reconcile([{ id: 1, selected: false }], writes.revision)!]).toEqual([]);
    expect([...writes.reconcile([{ id: 2, selected: true }], writes.revision)!]).toEqual([2]);
  });

  test("rapid toggles from multiple tabs commit in order and keep the latest pending choice", async () => {
    const first = deferred();
    const second = deferred();
    const calls: boolean[] = [];
    const writes = new SubjectSelectionWrites(async (_, selected) => {
      calls.push(selected);
      await (calls.length === 1 ? first.promise : second.promise);
    });
    const saveFirst = writes.set(1, true);
    const saveSecond = writes.set(1, false);
    await Promise.resolve();
    await Promise.resolve();
    expect(calls).toEqual([true]);
    expect(writes.selected(1, true)).toBe(false);
    first.resolve();
    await saveFirst;
    expect(writes.selected(1, true)).toBe(false);
    second.resolve();
    await saveSecond;
    expect(calls).toEqual([true, false]);
    expect([...writes.reconcile([{ id: 1, selected: false }], writes.revision)!]).toEqual([]);
  });

  test("a failed older write cannot revert a newer edit or block later writes", async () => {
    const first = deferred();
    const second = deferred();
    let calls = 0;
    const writes = new SubjectSelectionWrites(() => ++calls === 1 ? first.promise : second.promise);
    const failed = writes.set(1, true).catch((reason: Error) => reason.message);
    const saved = writes.set(1, false);
    first.reject(new Error("temporarily locked"));
    expect(await failed).toBe("temporarily locked");
    expect(writes.selected(1, true)).toBe(false);
    second.resolve();
    await saved;
    await writes.drain();
    expect(calls).toBe(2);
  });

  test("sync waits for edits appended while draining and exposes a final failed write", async () => {
    const first = deferred();
    const second = deferred();
    let calls = 0;
    const writes = new SubjectSelectionWrites(() => ++calls === 1 ? first.promise : second.promise);
    void writes.set(1, true);
    let finished = false;
    const drained = writes.drain().then(() => { finished = true; });
    const saved = writes.set(2, false);
    first.resolve();
    await Promise.resolve();
    expect(finished).toBe(false);
    second.resolve();
    await saved;
    await drained;
    expect(finished).toBe(true);

    const broken = new SubjectSelectionWrites(async () => { throw new Error("read only"); });
    await expect(broken.set(1, true)).rejects.toThrow("read only");
    await expect(broken.drain()).rejects.toThrow("read only");
    expect([...broken.reconcile([{ id: 1, selected: false }], broken.revision)!]).toEqual([]);
  });
});
