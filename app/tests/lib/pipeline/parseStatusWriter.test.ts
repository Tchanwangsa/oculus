import { describe, expect, test } from "bun:test";
import { createParseStatusWriter } from "@/lib/pipeline/parseStatusWriter";

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((done) => { resolve = done; });
  return { promise, resolve };
}

describe("parse status persistence", () => {
  test("coalesces overlapping progress writes and preserves transition order", async () => {
    const gate = deferred();
    const writes: string[] = [];
    const writer = createParseStatusWriter(async (_, __, status) => {
      writes.push(status);
      if (status === "running") await gate.promise;
      return true;
    });
    const pending = [
      writer.write(1, "a.pdf", "running"),
      writer.write(1, "a.pdf", "running"),
      writer.write(1, "a.pdf", "quality"),
    ];
    await Promise.resolve();
    expect(writes).toEqual(["running"]);
    gate.resolve();
    await Promise.all(pending);
    expect(writes).toEqual(["running", "quality"]);
  });

  test("missing rows and failed writes allow the next heartbeat to retry", async () => {
    let calls = 0;
    const writer = createParseStatusWriter(async () => {
      calls++;
      if (calls === 1) return false;
      if (calls === 2) throw new Error("database unavailable");
      return true;
    });
    await writer.write(1, "a.pdf", "running");
    await expect(writer.write(1, "a.pdf", "running")).rejects.toThrow("database unavailable");
    await writer.write(1, "a.pdf", "running");
    await writer.write(1, "a.pdf", "running");
    expect(calls).toBe(3);
  });

  test("a scrape reset runs between the old heartbeat and the new parse", async () => {
    const gate = deferred();
    const writes: string[] = [];
    const writer = createParseStatusWriter(async (_, __, status) => {
      writes.push(status);
      return true;
    });
    await writer.write(1, "a.pdf", "running");
    const reset = writer.mutate(1, "a.pdf", async () => {
      writes.push("reset");
      await gate.promise;
    });
    const resumed = writer.write(1, "a.pdf", "running");
    await Promise.resolve();
    gate.resolve();
    await Promise.all([reset, resumed]);
    expect(writes).toEqual(["running", "reset", "running"]);
  });

  test("finished files release the cache and other files persist independently", async () => {
    const gate = deferred();
    const writes: string[] = [];
    const writer = createParseStatusWriter(async (_, path, status) => {
      writes.push(`${path}:${status}`);
      if (path === "a.pdf") await gate.promise;
      return true;
    });
    const blocked = writer.write(1, "a.pdf", "running");
    await writer.write(1, "b.pdf", "quality");
    await writer.mutate(1, "b.pdf", async () => {});
    await writer.write(1, "b.pdf", "quality");
    expect(writes).toEqual(["a.pdf:running", "b.pdf:quality", "b.pdf:quality"]);
    gate.resolve();
    await blocked;
  });
});
