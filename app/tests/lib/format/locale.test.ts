import { afterAll, expect, test } from "bun:test";
import { displayLocale, displayTag, pinDisplayLocale } from "@/lib/format/locale";
import { startOfWeek } from "@/lib/planning/calendar/dates";

afterAll(() => pinDisplayLocale(undefined));

const afternoon = new Date(2026, 9, 12, 14, 5);

test("a Thai-region locale formats a Gregorian year, not a Buddhist-era one", () => {
  pinDisplayLocale("en-TH");
  const text = afternoon.toLocaleDateString(displayLocale, { month: "long", year: "numeric" });
  expect(text).toContain("2026");
  expect(text).not.toContain("2569");
});

test("the clock is 12-hour in a locale whose own clock is 24-hour", () => {
  for (const locale of ["en-TH", "en-GB", "de-DE"]) {
    pinDisplayLocale(locale);
    const time = afternoon.toLocaleTimeString(displayLocale, { hour: "numeric", minute: "2-digit" });
    expect(time).toMatch(/^2:05\s?[ap]\.?m\.?$/i);
  }
});

test("the calendar and clock join a locale's existing extensions", () => {
  expect(displayTag("th-TH-u-nu-thai")).toBe("th-TH-u-ca-gregory-hc-h12-nu-thai");
  expect(displayTag("en-AU")).toBe("en-AU-u-ca-gregory-hc-h12");
});

test("weeks start on Monday whatever the locale", () => {
  // Sunday 11 October 2026 belongs to the week from Monday 5 October.
  expect(startOfWeek(new Date(2026, 9, 11)).getDate()).toBe(5);
  expect(startOfWeek(new Date(2026, 9, 12)).getDate()).toBe(12);
});
