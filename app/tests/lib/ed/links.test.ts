import { expect, test } from "bun:test";
import { edThreadId, isEdUrl } from "@/lib/ed/links";

test("an Ed URL is decided by its host, in any region", () => {
  expect(isEdUrl("https://edstem.org/xx/courses/1/discussion/2")).toBe(true);
  expect(isEdUrl("https://xx.edstem.org/api/threads/2")).toBe(true);
  expect(isEdUrl("https://notedstem.org/courses/1")).toBe(false);
  expect(isEdUrl("https://canvas.example.org/courses/1")).toBe(false);
  expect(isEdUrl("not a url")).toBe(false);
  expect(isEdUrl(null)).toBe(false);
});

test("a thread id comes only from an Ed discussion URL", () => {
  expect(edThreadId("https://edstem.org/xx/courses/1/discussion/98765")).toBe(98765);
  expect(edThreadId("https://edstem.org/xx/courses/1/lessons/2")).toBeNull();
  expect(edThreadId("https://canvas.example.org/courses/1/discussion/3")).toBeNull();
  expect(edThreadId(null)).toBeNull();
});
