import { expect, test } from "bun:test";
import { deriveYours } from "@/hooks/data/overview/yours";
import { loadSubjectThreads } from "@/hooks/data/overview/rows";
import { date, file, first, second, sqlite, strip, thread } from "./fixtures";

test("Yours includes creation and edits in their own weeks using UTC local-row stamps", () => {
  const files = ["document", "whiteboard", "notebook", "upload", "file"].map((category, id) => file({ id, category, content_changed_at: sqlite(20) }));
  expect(deriveYours(1, strip, first, files, [], [])).toHaveLength(4);
  expect(deriveYours(1, strip, second, files, [], []).every((item) => item.date.getTime() === date(20).getTime())).toBe(true);
  const items = deriveYours(1, strip, first, files, [thread({ updated_at: sqlite(20) })], [thread(), thread({ id: 2, subject_id: 2 })]);
  expect(items.filter((item) => item.kind === "chat")).toHaveLength(1);
  expect(items.find((item) => item.kind === "chat")?.date).toEqual(date());
  expect(deriveYours(1, strip, second, [], [thread({ updated_at: sqlite(20) })], [thread()])[0].date).toEqual(date(20));
  expect(deriveYours(1, { ...strip, weeks: [], current: -1 }, null, files, [thread()], [])).toEqual([]);
});

test("a subject's chats load page by page and deduplicate moving pages", async () => {
  const offsets: number[] = [];
  const rows = await loadSubjectThreads(1, async (subjectId, offset = 0) => {
    expect(subjectId).toBe(1);
    offsets.push(offset);
    return offset === 0 ? { threads: [thread()], nextOffset: 1 }
      : { threads: [thread(), thread({ id: 2 })], nextOffset: null };
  });
  expect(offsets).toEqual([0, 1]);
  expect(rows.map((row) => row.id)).toEqual([1, 2]);
});
