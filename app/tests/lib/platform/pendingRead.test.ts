import { describe, expect, test } from "bun:test";
import { createPendingReader } from "@/lib/platform/pendingRead";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

describe("concurrent subject reads", () => {
  test("shares pending reads by subject and reads again after settlement", async () => {
    const reads: { id: number; result: ReturnType<typeof deferred<string>> }[] = [];
    const read = createPendingReader((id: number) => {
      const result = deferred<string>();
      reads.push({ id, result });
      return result.promise;
    });
    const first = read(1);
    expect(read(1)).toBe(first);
    const other = read(2);
    expect(other).not.toBe(first);
    expect(reads.map((r) => r.id)).toEqual([1, 2]);
    reads[0].result.resolve("subject one");
    reads[1].result.resolve("subject two");
    expect(await first).toBe("subject one");
    expect(await other).toBe("subject two");
    const refresh = read(1);
    expect(reads.map((r) => r.id)).toEqual([1, 2, 1]);
    reads[2].result.resolve("new rows");
    expect(await refresh).toBe("new rows");
  });

  test("a post-write refresh supersedes an older read without its cleanup losing the new request", async () => {
    const results = [deferred<string>(), deferred<string>()];
    let calls = 0;
    const read = createPendingReader((_id: number) => results[calls++].promise);
    const old = read(1);
    const fresh = read(1, true);
    expect(fresh).not.toBe(old);
    expect(read(1)).toBe(fresh);
    results[0].resolve("before write");
    await old;
    expect(read(1)).toBe(fresh);
    expect(calls).toBe(2);
    results[1].resolve("after write");
    expect(await fresh).toBe("after write");
  });

  test("failed reads are shared and the next mount retries", async () => {
    const failed = deferred<string>();
    let calls = 0;
    const read = createPendingReader((_id: null) => ++calls === 1 ? failed.promise : Promise.resolve("recovered"));
    const first = read(null);
    const second = read(null);
    const settled = Promise.allSettled([first, second]);
    failed.reject(new Error("database unavailable"));
    expect((await settled).map((r) => r.status)).toEqual(["rejected", "rejected"]);
    expect(calls).toBe(1);
    expect(await read(null)).toBe("recovered");
    expect(calls).toBe(2);
  });
});
