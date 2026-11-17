import { describe, expect, test } from "bun:test";
import { recordingStatus } from "@/lib/weeks";
import { event, lecture } from "./fixtures";

describe("recording timetable status", () => {
  test("Canvas UTC starts match local recording times", () => {
    expect(recordingStatus(lecture("2026-09-21T11:05:00"), [event("2026-09-21T11:00:00")])).toBe("timetabled");
  });

  test("holiday, break and other-stream recordings are untimetabled", () => {
    const classes = [event("2026-09-18T11:00:00"), event("2026-10-09T11:00:00"), event("2026-09-25T11:00:00", 2)];
    for (const date of ["2026-09-25T11:05:00", "2026-10-02T11:05:00", "2026-09-18T14:05:00"]) {
      expect(recordingStatus(lecture(date), classes)).toBe("untimetabled");
    }
  });

  test("classes start within inclusive -15/+30 minutes of the recording", () => {
    const row = lecture("2026-09-21T11:00:00");
    expect(recordingStatus(row, [event("2026-09-21T10:45:00")])).toBe("timetabled");
    expect(recordingStatus(row, [event("2026-09-21T11:30:00")])).toBe("timetabled");
    expect(recordingStatus(row, [event("2026-09-21T10:44:59")])).toBe("untimetabled");
    expect(recordingStatus(row, [event("2026-09-21T11:30:01")])).toBe("untimetabled");
  });

  test("a nearby class across midnight is a different local day", () => {
    expect(recordingStatus(lecture("2026-09-22T00:05:00"), [event("2026-09-21T23:55:00")])).toBe("untimetabled");
  });

  test("no usable subject classes means unknown, not untimetabled", () => {
    expect(recordingStatus(lecture("2026-09-21T11:00:00"), [
      event("2026-09-21T11:00:00", 2),
      event("2026-09-21T11:00:00", 1, { kind: "due" }),
      event("2026-09-21T11:00:00", 1, { all_day: 1 }),
      event("2026-09-21T11:00:00", 1, { start_at: "bad" }),
    ])).toBe("unknown");
    expect(recordingStatus(lecture("bad"), [event("2026-09-21T11:00:00")])).toBe("untimetabled");
  });

  test("matching considers all timed classes, regardless of title preference", () => {
    expect(recordingStatus(lecture("2026-09-22T11:05:00"), [
      event("2026-09-21T11:00:00"), event("2026-09-22T11:00:00", 1, { title: "Workshop" }),
    ])).toBe("timetabled");
  });
});
