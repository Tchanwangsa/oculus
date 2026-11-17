import { expect, test } from "bun:test";
import {
  parseCitation,
  fullCitation,
  citationText,
} from "@/lib/citations/parse";
import { resolvedNow } from "@/lib/citations/resolve";
import { lecturePagePath } from "@/lib/lectures/pagePath";
test("lecture time survives parsing, resolution and copying", () => {
  const shape = parseCitation("courses/X/Course/Recordings/2026-08-04-intro#t=723.5")!;
  expect(shape).toMatchObject({
    kind: "path",
    path: "courses/X/Course/Recordings/2026-08-04-intro",
    at: 723.5,
  });
  expect(resolvedNow(shape)).toMatchObject({ at: 723.5 });
  expect(citationText(fullCitation("courses/X/Course/Recordings/2026-08-04-intro#t=723.5")!)).toBe(
    "courses/X/Course/Recordings/2026-08-04-intro#t=723.5",
  );
});
test("lecture time routes preserve zero and omit invalid values", () => {
  const lecture = { id: "recording-1", subject_id: 1, title: "Calculus" };
  expect(lecturePagePath(lecture, 0)).toContain("&at=0");
  expect(lecturePagePath(lecture, 723)).toContain("&at=723");
  expect(lecturePagePath(lecture, -1)).not.toContain("&at=");
  expect(lecturePagePath(lecture, NaN)).not.toContain("&at=");
});
