import { expect, test } from "bun:test";
import { isEdUrl } from "@/lib/ed/links";

test("an Ed URL is decided by its host, in any region", () => {
  expect(isEdUrl("https://edstem.org/xx/courses/1/discussion/2")).toBe(true);
  expect(isEdUrl("https://xx.edstem.org/api/threads/2")).toBe(true);
  expect(isEdUrl("https://notedstem.org/courses/1")).toBe(false);
  expect(isEdUrl("https://canvas.example.org/courses/1")).toBe(false);
  expect(isEdUrl("not a url")).toBe(false);
  expect(isEdUrl(null)).toBe(false);
});
