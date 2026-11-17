import { expect, test } from "bun:test";
import { deriveComingUp } from "@/hooks/data/overview/comingUp";
import { parseTaskDoc } from "@/pages/subject/content/assignments/grouping";
import { date, event, file, iso, task } from "./fixtures";

test("deadlines merge tasks and Canvas from now, independent of the selected week, capped at five", () => {
  const rows = [event(iso(7), 1, { kind: "due" }), event(iso(4), 1, { kind: "due" }), event(iso(7), 2, { kind: "due" })];
  const tasks = [task(), task({ id: 2, due_at: iso(6) }), task({ id: 3, due_at: "bad" }),
    task({ id: 4, project_subject_id: 2 }), task({ id: 5, done_at: iso(6) }),
    ...[8, 9, 10, 11, 12].map((day) => task({ id: day, due_at: iso(day) }))];
  const result = deriveComingUp(1, rows, tasks, [], date());
  expect(result).toHaveLength(5);
  expect(result[0].id).toBe("task:2");
  expect(result[0].date.getTime()).toBe(date().getTime());
  expect(result.map((item) => item.kind)).toContain("due");
  expect(result.every((item) => item.date >= date())).toBe(true);
  expect(result[1].date.getTime()).toBe(date(7).getTime());
});

test("submitted assignments match the source link even when the file has no Canvas id", () => {
  const url = "https://canvas.example.org/courses/1/assignments/17";
  const row = event(iso(7), 1, { id: "assignment_17", kind: "due", title: "Renamed assignment", url });
  const doc = parseTaskDoc(`# Assignment\n\n**Status:** graded\n[Open in Canvas](${url})\n`, file({ category: "assignment" }));
  expect(deriveComingUp(1, [row], [], [doc], date())).toEqual([]);
  expect(deriveComingUp(1, [row], [], [{ ...doc, submitted: false }], date())[0].href).toContain("/subjects/1/file?");
  expect(deriveComingUp(1, [row], [], [{ ...doc, file: file({ subject_id: 2 }) }], date())).toHaveLength(1);
});

test("quiz shells match only a unique title and due minute when ids differ", () => {
  const due = date(7).toISOString().slice(0, 16).replace("T", " ");
  const doc = parseTaskDoc(`# Quiz\n\n**Due:** ${due} UTC\n**Status:** submitted\n`, file({ category: "quiz", canvas_id: 5 }));
  const row = event(iso(7), 1, { id: "assignment_17", kind: "due", title: "Quiz" });
  expect(deriveComingUp(1, [row], [], [doc], date())).toEqual([]);
  expect(deriveComingUp(1, [row], [], [doc, { ...doc, submitted: false }], date())).toHaveLength(1);
});
