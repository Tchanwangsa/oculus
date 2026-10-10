import { expect, test } from "bun:test";
import { sqliteUtcToMs } from "@/lib/format/format";

test("shared timestamp conversion treats SQLite UTC and explicit ISO zones consistently", () => {
  expect(sqliteUtcToMs("2026-10-02 09:10:00")).toBe(sqliteUtcToMs("2026-10-02T09:10:00Z"));
  expect(sqliteUtcToMs("2026-10-02T19:10:00+10:00")).toBe(sqliteUtcToMs("2026-10-02T09:10:00Z"));
  expect(sqliteUtcToMs("invalid")).toBeUndefined();
});
