import { describe, expect, test } from "bun:test";
import { filterToken, matchesFilterDraft, resolveFilter, withFilter } from "@/lib/search/filters";
import type { Subject } from "@/lib/db";

const subject = (id: number, code: string, current = true) => ({
  id, code, name: `Subject (${code})`, is_current: current,
}) as Subject;
const subjects = [subject(2, "COMP30022_2026_SM2"), subject(1, "COMP30026_2026_SM2")];

describe("palette filters", () => {
  test("exact short subject codes select the current term and ambiguous kinds remain drafts", () => {
    const older = subject(9, "COMP30026_2025_SM2", false);
    expect(resolveFilter({ key: "in", value: "comp30026" }, [older, ...subjects]))
      .toEqual({ key: "in", subject: subjects[1] });
    expect(resolveFilter({ key: "type", value: "p" }, subjects)).toBeNull();
    expect(resolveFilter({ key: "type", value: "document" }, subjects))
      .toEqual({ key: "type", kind: "note" });
  });
  test("filter tokens retain preceding query and replacing a kind retains the subject", () => {
    const token = filterToken("proof TYPE:le")!;
    expect(token).toMatchObject({ key: "type", value: "le" });
    expect("proof TYPE:le".slice(0, token.start)).toBe("proof ");
    expect(withFilter([{ key: "in", subject: subjects[0] }, { key: "type", kind: "file" }],
      { key: "type", kind: "lecture" })).toEqual([
        { key: "in", subject: subjects[0] }, { key: "type", kind: "lecture" },
      ]);
  });
  test("a switched draft refuses preceding-key results and ordinary navigation until it resolves", () => {
    const draft = { key: "type" as const, value: "le" };
    expect(matchesFilterDraft(draft, { key: "in", subject: subjects[0] })).toBe(false);
    expect(matchesFilterDraft(draft, null)).toBe(false);
    expect(matchesFilterDraft(draft, { key: "type", kind: "lecture" })).toBe(true);
    expect(matchesFilterDraft(null, null)).toBe(true);
    expect(matchesFilterDraft({ key: "type", value: "fi" }, { key: "type", kind: "lecture" })).toBe(false);
    expect(matchesFilterDraft({ key: "type", value: "fi" }, { key: "type", kind: "file" })).toBe(true);
    expect(matchesFilterDraft({ key: "in", value: "comp30026" }, { key: "in", subject: subjects[0] })).toBe(false);
    expect(matchesFilterDraft({ key: "in", value: "comp30026" }, { key: "in", subject: subjects[1] })).toBe(true);
  });
});
