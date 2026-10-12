import { afterAll, expect, test } from "bun:test";
import { displayLocale, gregorian, pinDisplayLocale } from "@/lib/format/locale";

afterAll(() => pinDisplayLocale(undefined));

test("a Thai-region locale formats a Gregorian year, not a Buddhist-era one", () => {
  pinDisplayLocale("en-TH");
  const text = new Date(2026, 9, 12).toLocaleDateString(displayLocale, {
    month: "long",
    year: "numeric",
  });
  expect(text).toContain("2026");
  expect(text).not.toContain("2569");
});

test("the calendar joins a locale's existing extensions", () => {
  expect(gregorian("th-TH-u-nu-thai")).toBe("th-TH-u-ca-gregory-nu-thai");
  expect(gregorian("en-AU")).toBe("en-AU-u-ca-gregory");
});
