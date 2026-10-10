import { expect, test } from "bun:test";
import { displayCode } from "../src/lib/format";

test("a code loses only a year-led term suffix", () => {
  expect(displayCode("MULT20015_2026_SM2")).toBe("MULT20015");
  expect(displayCode("CS_101_2026_FA")).toBe("CS_101");
  expect(displayCode("CS_101")).toBe("CS_101");
  expect(displayCode("CS 101-001")).toBe("CS 101-001");
  expect(displayCode("INFO30006")).toBe("INFO30006");
});
