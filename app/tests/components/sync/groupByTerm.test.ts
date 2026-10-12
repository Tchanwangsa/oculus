import { expect, test } from "bun:test";
import { groupByTerm } from "@/components/sync/groupByTerm";
import type { Subject } from "@/lib/db";

function subject(id: number, term_name: string | null): Subject {
  return {
    id,
    code: `S${id}`,
    name: `Subject ${id}`,
    term_name,
    term_sort: null,
    is_current: false,
    workflow_state: "available",
    selected: false,
    last_synced_at: null,
    created_at: "",
  };
}

test("term groups keep the newest-first order, integer-like names included", () => {
  const groups = groupByTerm([
    subject(1, "2026"),
    subject(2, "Fall 2025"),
    subject(3, "2025"),
    subject(4, "2026"),
    subject(5, null),
  ]);
  expect([...groups.keys()]).toEqual(["2026", "Fall 2025", "2025", "Unknown term"]);
  expect(groups.get("2026")?.map((s) => s.id)).toEqual([1, 4]);
});
