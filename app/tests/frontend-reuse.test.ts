import { describe, expect, test } from "bun:test";
import { groupBySubject } from "../src/lib/subjectGroups";
import { readStringSet } from "../src/hooks/useStoredState";
import { libraryLinkHref, libraryLinkTarget } from "../src/lib/libraryLinks";
import { filterToken, matchesFilterDraft, resolveFilter, withFilter } from "../src/lib/searchFilters";
import { sqliteUtcToMs } from "../src/lib/format";
import type { DbFile, Subject } from "../src/lib/db";
import { messageAt, parseErrorMeta, parsePermissionMeta, parseToolMeta, type HarnessItem } from "../src/lib/harness";

const subject = (id: number, code: string, current = true) => ({
  id, code, name: `Subject (${code})`, is_current: current,
}) as Subject;
const subjects = [subject(2, "COMP30022_2026_SM2"), subject(1, "COMP30026_2026_SM2")];

describe("shared subject grouping", () => {
  test("keeps first appearance and row order while combining deleted subjects with unscoped rows", () => {
    const items = [
      { id: 1, subject_id: 1 }, { id: 2, subject_id: 999 },
      { id: 3, subject_id: 2 }, { id: 4, subject_id: null }, { id: 5, subject_id: 1 },
    ];
    const groups = groupBySubject(items, subjects, { key: "general", label: "General" });
    expect(groups.map((g) => g.key)).toEqual(["1", "general", "2"]);
    expect(groups.map((g) => g.items.map((x) => x.id))).toEqual([[1, 5], [2, 4], [3]]);
    expect(groups[0]).toMatchObject({ label: "COMP30026", title: "Subject", subjectId: 1 });
    expect(groups[1]).toMatchObject({ label: "General", subjectId: null });
    expect(groups[0].items[0]).toBe(items[0]);
  });

  test("lets projects name their unscoped group Personal and leaves empty groups to the caller", () => {
    expect(groupBySubject([], subjects, { key: "personal", label: "Personal" })).toEqual([]);
    expect(groupBySubject([{ subject_id: null }], subjects, { key: "personal", label: "Personal" })[0])
      .toMatchObject({ key: "personal", label: "Personal", title: "Not scoped to a subject" });
  });
});

describe("stored group preferences", () => {
  test("rejects invalid JSON and non-array preferences", () => {
    for (const value of [null, "broken", "null", "{}"]) expect([...readStringSet(value)]).toEqual([]);
  });
  test("drops malformed entries and duplicates without changing a saved order", () => {
    expect([...readStringSet('["2", null, 4, "general", "2", false]')]).toEqual(["2", "general"]);
  });
});

describe("inherited note links and palette filters", () => {
  test("same-subject note links round-trip spaces, parentheses, percent and Unicode", () => {
    const path = "courses/COMP30026_2026_SM2/uploads/Notes (week 2) 100% λ.pdf";
    const file = { relative_path: path } as DbFile;
    const href = libraryLinkHref("courses/COMP30026_2026_SM2/documents/My note.md", path)!;
    expect(href).not.toContain(" ");
    expect(href).not.toContain("(");
    expect(libraryLinkTarget(href, [file])).toBe(file);
    expect(libraryLinkHref("courses/COMP30022_2026_SM2/documents/n.md", path)).toBeNull();
  });
  test("renamed Canvas pages resolve by canonical source URL and malformed percentages do not throw", () => {
    const file = { relative_path: "courses/X/pages/new-title.md", source_url: "https://canvas.test/courses/1/pages/original-slug" } as DbFile;
    expect(libraryLinkTarget(file.source_url!, [file])).toBe(file);
    expect(libraryLinkTarget("../pages/stray%name.md", [])).toBeUndefined();
  });
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

test("shared timestamp conversion treats SQLite UTC and explicit ISO zones consistently", () => {
  expect(sqliteUtcToMs("2026-10-02 09:10:00")).toBe(sqliteUtcToMs("2026-10-02T09:10:00Z"));
  expect(sqliteUtcToMs("2026-10-02T19:10:00+10:00")).toBe(sqliteUtcToMs("2026-10-02T09:10:00Z"));
  expect(sqliteUtcToMs("invalid")).toBeUndefined();
});


describe("timeline metadata boundaries", () => {
  const row = (meta: string | null) => ({ meta }) as HarnessItem;
  test("tool fields are narrowed without discarding provider-specific input", () => {
    const input = { CommandLine: "oculus read file.pdf" };
    const item = row(JSON.stringify({ kind: "bash", name: "run", input, ok: false, output: "failed" }));
    expect(parseToolMeta(item)).toMatchObject({ kind: "bash", name: "run", input, ok: false, output: "failed" });
    expect(parseToolMeta(item)).toBe(parseToolMeta(item));
    expect(parseToolMeta(row(JSON.stringify({ kind: "future", name: {}, input, ok: "yes", output: {} }))))
      .toEqual({ input });
  });
  test("replacing a committed row invalidates its metadata cache", () => {
    const item = row('{"kind":"read","ok":null}');
    expect(parseToolMeta(item).ok).toBeNull();
    expect(parseToolMeta({ ...item, meta: '{"kind":"read","ok":true}' }).ok).toBe(true);
  });
  test("auth, lecture moments and permission strings retain their own validation", () => {
    const item = row('{"auth":"codex","at":220,"tool":"Bash","action":"command","target":"ls","rule":"command(ls)"}');
    expect(parseErrorMeta(item)).toEqual({ auth: "codex" });
    expect(messageAt(item)).toBe(220);
    expect(parsePermissionMeta(item)).toEqual({ tool: "Bash", action: "command", target: "ls", rule: "command(ls)" });
    expect(parseErrorMeta(row('{"auth":"future"}'))).toEqual({});
    expect(messageAt(row('{"at":"220"}'))).toBeNull();
    expect(parsePermissionMeta(row('{"tool":12,"rule":{}}')).rule).toBeNull();
  });
});
