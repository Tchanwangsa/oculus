import { expect, test } from "bun:test";
import { isRevisionDue, subjectRevisionDue } from "@/lib/revision/due";
import type { RevisionItem } from "@/lib/revision/types";
import { date, iso, sqlite } from "./fixtures";

test("revision due uses the review queue's active, due-now rule and filters subject", () => {
  const item = (extra: Partial<RevisionItem> = {}) => ({ status: "active", due: iso(), subject_id: 1, ...extra }) as RevisionItem;
  const items = [item(), item({ due: sqlite() }), item({ subject_id: 2 }), item({ subject_id: null }),
    item({ status: "draft" }), item({ status: "suspended" }), item({ due: iso(7) }), item({ due: "bad" })];
  expect(items.filter((row) => isRevisionDue(row, date().getTime()))).toHaveLength(4);
  expect(subjectRevisionDue(items, 1, date().getTime())).toBe(2);
});
