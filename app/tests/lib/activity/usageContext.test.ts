import { describe, expect, test } from "bun:test";
import { sameContext, usageContext } from "@/lib/activity/usageContext";

describe("usage context", () => {
  test.each([
    ["/", "other", null],
    ["/new", "other", null],
    ["/chat?thread=5", "chat", null],
    ["/calendar", "planning", null],
    ["/projects/3", "planning", null],
    ["/tasks/9", "planning", null],
    ["/browse/4", "browser", null],
    ["/subjects/12", "course", 12],
    ["/subjects/12/discussion", "course", 12],
    ["/subjects/12/lecture?id=abc", "lecture", 12],
    ["/subjects/12/file?path=x/downloads/a.pdf", "file", 12],
    ["/subjects/12/file?path=x/documents/n.md", "document", 12],
    ["/subjects/12/projects", "planning", 12],
  ] as const)("%s is %s", (path, kind, subjectId) => {
    expect(usageContext(path)).toEqual({ kind, subjectId });
  });

  test("same context needs the same kind and subject", () => {
    const lecture = usageContext("/subjects/12/lecture?id=a");
    expect(sameContext(null, lecture)).toBe(false);
    expect(sameContext(lecture, usageContext("/subjects/12/lecture?id=b"))).toBe(true);
    expect(sameContext(lecture, usageContext("/subjects/13/lecture?id=a"))).toBe(false);
    expect(sameContext(lecture, usageContext("/subjects/12"))).toBe(false);
  });
});
