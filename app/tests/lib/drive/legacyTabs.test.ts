import { expect, test } from "bun:test";
import { legacyTabTargets, resolveLegacyTab } from "@/lib/drive/legacyTabs";

const disk = (folders: string[]) => async (rel: string) => rel === "" || folders.includes(rel);

test("only the old sub-tab names, in their exact case, are old sub-tabs", () => {
  expect(legacyTabTargets("downloads")).toEqual(["Course/Files", "Course"]);
  expect(legacyTabTargets("media")).toEqual(["Course/Media", "Course"]);
  for (const own of ["uploads", "drive", "documents", "canvases", "notebooks"]) expect(legacyTabTargets(own)).toEqual([""]);
  expect(legacyTabTargets("Media")).toBeNull();
  expect(legacyTabTargets("Course/Files")).toBeNull();
  expect(legacyTabTargets("")).toBeNull();
  expect(legacyTabTargets("constructor")).toBeNull();
});

test("an old sub-tab goes to the first of its folders that exists", async () => {
  expect(await resolveLegacyTab("downloads", disk(["Course", "Course/Files"]))).toBe("Course/Files");
  expect(await resolveLegacyTab("media", disk(["Course", "Course/Files"]))).toBe("Course");
  expect(await resolveLegacyTab("media", disk([]))).toBe("");
  expect(await resolveLegacyTab("uploads", disk([]))).toBe("");
});

test("a student's folder by an old sub-tab's name is theirs, and other paths are left alone", async () => {
  expect(await resolveLegacyTab("uploads", disk(["uploads"]))).toBeNull();
  expect(await resolveLegacyTab("downloads", disk(["downloads", "Course/Files"]))).toBeNull();
  let asked = 0;
  const counting = async () => { asked += 1; return true; };
  expect(await resolveLegacyTab("Week 3", counting)).toBeNull();
  expect(asked).toBe(0);
});
