import { expect, test } from "bun:test";
import { parseOnboardingRow, type OnboardingRow } from "@/lib/db/onboarding";

test("a well-formed row round-trips", () => {
  const row: OnboardingRow = { completedAt: "2026-10-10T00:00:00.000Z", skipped: ["library", "agent"] };
  expect(parseOnboardingRow(JSON.stringify(row))).toEqual(row);
  expect(parseOnboardingRow(JSON.stringify({ completedAt: null, skipped: [] }))).toEqual({
    completedAt: null,
    skipped: [],
  });
});

test("absent and malformed rows read as absent", () => {
  expect(parseOnboardingRow(null)).toBeNull();
  expect(parseOnboardingRow("")).toBeNull();
  expect(parseOnboardingRow("{not json")).toBeNull();
  expect(parseOnboardingRow("42")).toBeNull();
  expect(parseOnboardingRow(JSON.stringify({ completedAt: 7 }))).toBeNull();
  expect(parseOnboardingRow(JSON.stringify({ skipped: [] }))).toBeNull();
});

test("unknown step ids and a missing list are dropped", () => {
  expect(
    parseOnboardingRow(JSON.stringify({ completedAt: null, skipped: ["canvas", "inbox", 3] })),
  ).toEqual({ completedAt: null, skipped: ["canvas"] });
  expect(parseOnboardingRow(JSON.stringify({ completedAt: null }))).toEqual({
    completedAt: null,
    skipped: [],
  });
});
