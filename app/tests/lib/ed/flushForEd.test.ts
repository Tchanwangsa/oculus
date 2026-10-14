import { describe, expect, test } from "bun:test";

import { flushForEd } from "@/lib/ed/flushForEd";
import { createWorkspaceSaver } from "@/lib/ed/workspaceSaver";

describe("flush before Run and Mark", () => {
  test("everything written resolves null", async () => {
    const writes: string[] = [];
    const saver = createWorkspaceSaver({ write: async (_p, t) => (writes.push(t), "ed"), delay: 10_000 });
    saver.loaded("a.py", "x");
    saver.edit("a.py", "y");
    expect(await flushForEd(saver)).toBeNull();
    expect(writes).toEqual(["y"]);
  });

  test("a failed save stops it with the reason", async () => {
    const saver = createWorkspaceSaver({
      write: async () => {
        throw new Error("upload refused");
      },
      delay: 10_000,
    });
    saver.loaded("a.py", "x");
    saver.edit("a.py", "y");
    expect(await flushForEd(saver)).toContain("upload refused");
  });
});
