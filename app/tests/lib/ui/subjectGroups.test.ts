import { describe, expect, test } from "bun:test";
import { groupBySubject } from "@/lib/ui/subjectGroups";
import type { Subject } from "@/lib/db";

const subject = (id: number, code: string, current = true) => ({
  id, code, name: `Subject (${code})`, is_current: current,
}) as Subject;
const subjects = [subject(2, "COMP30022_2026_SM2"), subject(1, "COMP30026_2026_SM2")];

describe("shared subject grouping", () => {
  test("keeps first appearance and row order while combining deleted subjects with unscoped rows", () => {
    const items = [
      { id: 1, subject_id: 1 }, { id: 2, subject_id: 999 },
      { id: 3, subject_id: 2 }, { id: 4, subject_id: null }, { id: 5, subject_id: 1 },
    ];
    const groups = groupBySubject(items, subjects, { key: "general", label: "General" });
    expect(groups.map((g) => g.key)).toEqual(["1", "general", "2"]);
    expect(groups.map((g) => g.items.map((x) => x.id))).toEqual([[1, 5], [2, 4], [3]]);
    expect(groups[0]).toMatchObject({ label: "COMP30026", title: "Subject", subjectId: 1 });
    expect(groups[1]).toMatchObject({ label: "General", subjectId: null });
    expect(groups[0].items[0]).toBe(items[0]);
  });

  test("lets projects name their unscoped group Personal and leaves empty groups to the caller", () => {
    expect(groupBySubject([], subjects, { key: "personal", label: "Personal" })).toEqual([]);
    expect(groupBySubject([{ subject_id: null }], subjects, { key: "personal", label: "Personal" })[0])
      .toMatchObject({ key: "personal", label: "Personal", title: "Not scoped to a subject" });
  });
});
