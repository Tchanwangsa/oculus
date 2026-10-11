import { describe, expect, test } from "bun:test";
import { readStringSet } from "@/hooks/ui/useStoredState";

describe("stored group preferences", () => {
  test("rejects invalid JSON and non-array preferences", () => {
    for (const value of [null, "broken", "null", "{}"]) expect([...readStringSet(value)]).toEqual([]);
  });
  test("drops malformed entries and duplicates without changing a saved order", () => {
    expect([...readStringSet('["2", null, 4, "general", "2", false]')]).toEqual(["2", "general"]);
  });
});
