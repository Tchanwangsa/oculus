import { expect, test } from "bun:test";
import { driveErrorText, parseDriveError, READ_ONLY_TEXT } from "@/lib/drive/commands";

test("a JSON-serialised typed error is parsed", () => {
  const raw = JSON.stringify({ kind: "exists", message: "Exam prep already exists" });
  expect(parseDriveError(raw)).toEqual({ kind: "exists", message: "Exam prep already exists" });
  expect(driveErrorText(raw)).toBe("Exam prep already exists");
});

test("read_only is worded for the student, whatever Rust said", () => {
  const raw = JSON.stringify({ kind: "read_only", message: "courses/X/Course/a.pdf is synced" });
  expect(driveErrorText(raw)).toBe(READ_ONLY_TEXT);
  expect(driveErrorText({ kind: "read_only", message: "" })).toBe(READ_ONLY_TEXT);
});

test("anything else falls back to its raw text", () => {
  expect(parseDriveError("disk full")).toEqual({ kind: null, message: "disk full" });
  expect(parseDriveError(new Error("boom"))).toEqual({ kind: null, message: "boom" });
  expect(parseDriveError('{"kind":"weird","message":"x"}').kind).toBeNull();
  expect(parseDriveError("[1,2]").message).toBe("[1,2]");
  expect(driveErrorText("")).toBe("Something went wrong");
});
