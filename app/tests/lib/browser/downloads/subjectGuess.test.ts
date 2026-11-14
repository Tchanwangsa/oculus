import { describe, expect, test } from "bun:test";
import { courseIdOf, guessSubject, preselectSubject } from "@/lib/browser/downloads/subjectGuess";

const subjects = [
  { id: 101, code: "COMP10001_2026_SM2" },
  { id: 202, code: "MAST20004_2026_SM2" },
];
const canvas = "https://canvas.example.edu";

describe("courseIdOf", () => {
  test("reads the id from a course page at any depth", () => {
    expect(courseIdOf("https://canvas.example.edu/courses/202/files/9/download?download_frd=1")).toBe(202);
    expect(courseIdOf("https://canvas.example.edu/courses/101")).toBe(101);
    expect(courseIdOf("https://lms.other-school.example/courses/7/pages/x")).toBe(7);
  });

  test("ignores paths that only look like one, and non-URLs", () => {
    expect(courseIdOf("https://canvas.example.edu/courses/abc/")).toBeNull();
    expect(courseIdOf("https://canvas.example.edu/courses/12abc")).toBeNull();
    expect(courseIdOf("https://canvas.example.edu/files/9")).toBeNull();
    expect(courseIdOf("https://example.com/?next=/courses/101/")).toBeNull();
    expect(courseIdOf("not a url")).toBeNull();
  });
});

describe("guessSubject", () => {
  test("a course page on the institution's Canvas names its subject", () => {
    expect(guessSubject(`${canvas}/courses/202/modules`, subjects, canvas)).toBe("MAST20004_2026_SM2");
  });

  test("another site's /courses/ path names nothing", () => {
    expect(guessSubject("https://mooc.example.org/courses/202/", subjects, canvas)).toBeNull();
  });

  test("before a profile exists any host is read", () => {
    expect(guessSubject("https://lms.example.org/courses/101/", subjects, null)).toBe("COMP10001_2026_SM2");
  });

  test("a course that is not one of the student's subjects, or no URL, names nothing", () => {
    expect(guessSubject(`${canvas}/courses/999/`, subjects, canvas)).toBeNull();
    expect(guessSubject(undefined, subjects, canvas)).toBeNull();
  });
});

describe("preselectSubject", () => {
  test("the guess wins over the last subject", () => {
    expect(preselectSubject("MAST20004_2026_SM2", "COMP10001_2026_SM2", subjects)).toBe("MAST20004_2026_SM2");
  });

  test("with no guess, the last subject if it still exists", () => {
    expect(preselectSubject(null, "COMP10001_2026_SM2", subjects)).toBe("COMP10001_2026_SM2");
    expect(preselectSubject(null, "GONE_2025_SM1", subjects)).toBeNull();
    expect(preselectSubject(null, null, subjects)).toBeNull();
  });
});
