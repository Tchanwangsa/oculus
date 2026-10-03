import { describe, expect, test } from "bun:test";
import { createCourseFileDataLoader } from "../src/lib/courseFiles";
import type { DbFile } from "../src/lib/db";

const file = (id: number, overrides: Partial<DbFile> = {}) => ({
  id, relative_path: `courses/X/modules/${id}.md`, scraped_at: "2026-10-03 10:00:00",
  last_accessed_at: null, ...overrides,
}) as DbFile;

describe("scraped metadata reads", () => {
  test("unchanged row snapshots share reads and parsed identities across refreshes", async () => {
    let reads = 0;
    const load = createCourseFileDataLoader((markdown, row) => ({ markdown, row }), async () => {
      reads++;
      return "# Header";
    });
    const rows = [file(1), file(2)];
    const first = await load(rows);
    const next = await load(rows.map((row) => ({ ...row })));
    expect(reads).toBe(2);
    expect(next).toBe(first);
    expect(next[0]).toBe(first[0]);
    expect(next[1]).toBe(first[1]);

    const opened = { ...rows[0], last_accessed_at: "2026-10-03 10:10:00" };
    const refreshed = await load([opened, rows[1]]);
    expect(reads).toBe(3);
    expect(refreshed[0].row).toBe(opened);
    expect(refreshed[1]).toBe(first[1]);
  });

  test("pending reads are shared, updated scrapes are re-read, and result order follows the current list", async () => {
    const callbacks: ((value: string) => void)[] = [];
    const load = createCourseFileDataLoader((markdown, row) => ({ markdown, id: row.id }),
      () => new Promise<string>((resolve) => callbacks.push(resolve)));
    const rows = [file(1), file(2)];
    const first = load(rows);
    const second = load([rows[1], { ...rows[0] }]);
    expect(callbacks).toHaveLength(2);
    callbacks[0]("one");
    callbacks[1]("two");
    expect((await second).map((r) => r.id)).toEqual([2, 1]);
    expect((await second)[1]).toBe((await first)[0]);

    const updated = load([{ ...rows[0], scraped_at: "2026-10-03 11:00:00" }]);
    expect(callbacks).toHaveLength(3);
    callbacks[2]("changed");
    expect((await updated)[0].markdown).toBe("changed");
  });

  test("removed files leave the cache and failed reads retry on the next refresh", async () => {
    let reads = 0;
    const load = createCourseFileDataLoader((markdown) => markdown, async () => {
      if (++reads === 1) throw new Error("not ready");
      return "ready";
    });
    const rows = [file(1)];
    expect(await load(rows)).toEqual([""]);
    expect(await load(rows)).toEqual(["ready"]);
    await load([]);
    expect(await load(rows)).toEqual(["ready"]);
    expect(reads).toBe(3);
  });
});
