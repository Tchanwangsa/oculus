import { describe, expect, test } from "bun:test";

import { markDialogCopy, markPrompts } from "@/lib/ed/markPrompts";

const now = Date.parse("2026-10-13T10:00:00Z");
const base = { now, dueAt: null, confirmSubmit: false, attemptsRemaining: null };

describe("mark prompts", () => {
  test("nothing to ask marks straight away", () => {
    expect(markPrompts(base)).toEqual([]);
    expect(markPrompts({ ...base, attemptsRemaining: 3 })).toEqual([]);
    expect(markPrompts({ ...base, dueAt: "2026-10-14T00:00:00Z" })).toEqual([]);
  });

  test("past the due date asks about a late submission", () => {
    expect(markPrompts({ ...base, dueAt: "2026-10-12T23:59:00Z" })).toEqual(["late"]);
  });

  test("an unreadable due date is not late", () => {
    expect(markPrompts({ ...base, dueAt: "soon" })).toEqual([]);
  });

  test("the last attempt and confirm_submit each ask, most serious first", () => {
    expect(markPrompts({ ...base, attemptsRemaining: 1 })).toEqual(["lastAttempt"]);
    expect(markPrompts({ ...base, confirmSubmit: true })).toEqual(["confirm"]);
    expect(
      markPrompts({ now, dueAt: "2026-10-01T00:00:00Z", confirmSubmit: true, attemptsRemaining: 1 }),
    ).toEqual(["late", "lastAttempt", "confirm"]);
  });
});

describe("mark dialog copy", () => {
  test("titled by the most serious prompt", () => {
    expect(markDialogCopy(["confirm"], null).title).toBe("Submit for marking?");
    expect(markDialogCopy(["lastAttempt", "confirm"], null).title).toBe("Use your last attempt?");
    const late = markDialogCopy(["late", "lastAttempt"], "Mon 12 Oct, 11:59 pm");
    expect(late.title).toBe("Submit late?");
    expect(late.confirmLabel).toBe("Submit late");
    expect(late.lines[0]).toContain("Mon 12 Oct, 11:59 pm");
    expect(late.lines).toContain("This is your last attempt.");
  });
});
