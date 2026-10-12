import { expect, test } from "bun:test";
import { displayCode, sqliteUtcToMs } from "@/lib/format/format";

test("shared timestamp conversion treats SQLite UTC and explicit ISO zones consistently", () => {
  expect(sqliteUtcToMs("2026-10-02 09:10:00")).toBe(sqliteUtcToMs("2026-10-02T09:10:00Z"));
  expect(sqliteUtcToMs("2026-10-02T19:10:00+10:00")).toBe(sqliteUtcToMs("2026-10-02T09:10:00Z"));
  expect(sqliteUtcToMs("invalid")).toBeUndefined();
});

test("a code loses only a year-led term suffix", () => {
  expect(displayCode("MULT20015_2026_SM2")).toBe("MULT20015");
  expect(displayCode("CS_101_2026_FA")).toBe("CS_101");
  expect(displayCode("CS_101")).toBe("CS_101");
  expect(displayCode("CS 101-001")).toBe("CS 101-001");
  expect(displayCode("INFO30006")).toBe("INFO30006");
});
