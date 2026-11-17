import { expect, test } from "bun:test";
import { driveFolderHref, filesFolderHref, parseDriveRoute, parseFilesRoute } from "@/lib/drive/routes";

test("Drive routes round-trip a folder, a segment at a time", () => {
  expect(driveFolderHref("")).toBe("/drive");
  expect(driveFolderHref("courses")).toBe("/drive");
  const rel = "courses/COMP1/Week 1/a#b?c";
  const href = driveFolderHref(rel);
  expect(href).toBe("/drive/COMP1/Week%201/a%23b%3Fc");
  expect(parseDriveRoute(href)).toBe(rel);
  expect(parseDriveRoute("/drive")).toBe("");
  expect(parseDriveRoute("/drives")).toBeNull();
  // Drive's Recent list is not a folder.
  expect(parseDriveRoute("/drive/recent")).toBeNull();
});

test("Files routes round-trip a folder inside a subject", () => {
  expect(filesFolderHref(3, "")).toBe("/subjects/3/files");
  const href = filesFolderHref(3, "Course/Files/Week 2");
  expect(href).toBe("/subjects/3/files/Course/Files/Week%202");
  expect(parseFilesRoute(href)).toEqual({ subjectId: 3, inner: "Course/Files/Week 2" });
  expect(parseFilesRoute("/subjects/3/files")).toEqual({ subjectId: 3, inner: "" });
  expect(parseFilesRoute("/subjects/3/file?path=x")).toBeNull();
});
