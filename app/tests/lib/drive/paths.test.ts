import { expect, test } from "bun:test";
import {
  foldCode,
  freeName,
  isWithin,
  nameProblem,
  parentRel,
  stemLength,
  subjectForFolder,
  subjectRelOf,
} from "@/lib/drive/paths";
import { COURSES_DIR } from "@/lib/files/libraryLayout";
import type { Subject } from "@/lib/db";

test("a path's subject folder", () => {
  expect(subjectRelOf(`${COURSES_DIR}/COMP30026_2026_SM2/A/b.pdf`)).toBe(`${COURSES_DIR}/COMP30026_2026_SM2`);
  expect(subjectRelOf(`${COURSES_DIR}/COMP30026_2026_SM2`)).toBe(`${COURSES_DIR}/COMP30026_2026_SM2`);
  expect(subjectRelOf(COURSES_DIR)).toBeNull();
  expect(subjectRelOf("")).toBeNull();
  expect(subjectRelOf("studio/videos/x")).toBeNull();
});

test("parents and containment", () => {
  expect(parentRel("a/b/c")).toBe("a/b");
  expect(parentRel("a")).toBe("");
  expect(isWithin("a/b/c", "a/b")).toBe(true);
  expect(isWithin("a/bc", "a/b")).toBe(false);
});

test("a subject is found by its folder as Rust names it", () => {
  const subjects = [{ code: "CS 101" }, { code: "COMP30026_2026_SM2" }] as Subject[];
  expect(subjectForFolder("CS_101", subjects)?.code).toBe("CS 101");
  expect(subjectForFolder("comp30026_2026_sm2", subjects)?.code).toBe("COMP30026_2026_SM2");
  expect(subjectForFolder("MAST20009", subjects)).toBeUndefined();
  expect(foldCode("a-b c")).toBe("A_B_C");
});

test("a new entry's name steps aside from taken ones, ignoring case", () => {
  expect(freeName("Untitled folder", ["Notes"])).toBe("Untitled folder");
  expect(freeName("Untitled folder", ["untitled folder", "Untitled folder 2"])).toBe("Untitled folder 3");
});

test("a rename selects the stem of a file and all of a folder", () => {
  expect(stemLength("notes.final.pdf", false)).toBe("notes.final".length);
  expect(stemLength("README", false)).toBe(6);
  expect(stemLength("v1.2", true)).toBe(4);
});

test("names Rust would refuse are caught first", () => {
  expect(nameProblem("  ")).not.toBeNull();
  expect(nameProblem(".hidden")).not.toBeNull();
  expect(nameProblem("a/b")).not.toBeNull();
  expect(nameProblem("Week 3")).toBeNull();
});
