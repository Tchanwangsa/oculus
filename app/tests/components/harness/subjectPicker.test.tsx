import { expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { SubjectPicker } from "@/components/harness/composer/SubjectPicker";
import { subjectOptions } from "@/components/harness/composer/subjectOptions";
import type { Subject } from "@/lib/db";

const subject = (id: number, code: string, name: string, is_current: boolean, term_sort: string | null = null): Subject => ({
  id, code, name, is_current, term_sort, term_name: null, workflow_state: "available", selected: true,
  last_synced_at: null, created_at: "",
});

const subjects = [
  subject(1, "MAST20004_2026_SM2", "Probability (MAST20004_2026_SM2)", true),
  subject(2, "COMP30027_2026_SM2", "Machine Learning", true),
  subject(3, "MAST10007_2025_SM1", "Linear Algebra", false, "2025-03"),
  subject(4, "PHYC10003_2025_SM2", "Physics 1", false, "2025-07"),
  subject(5, "ARTS1000", "Arts and Ideas", false, null),
];

test("General comes first, then this term's subjects, then past terms newest first", () => {
  const { listed, past } = subjectOptions(subjects, "");
  expect(listed.map((o) => o.scope)).toEqual(["general", 1, 2]);
  expect(past.map((o) => o.scope)).toEqual([4, 3, 5]);
});

test("rows show the short code and carry the full name", () => {
  const [, probability] = subjectOptions(subjects, "").listed;
  expect(probability.label).toBe("MAST20004");
  expect(probability.title).toBe("Probability");
});

test("a search matches the code or the name, every word, any case, past terms included", () => {
  const pick = (q: string) => {
    const { listed, past } = subjectOptions(subjects, q);
    return [...listed, ...past].map((o) => o.scope);
  };
  expect(pick("mast")).toEqual([1, 3]);
  expect(pick("  LINEAR  ")).toEqual([3]);
  expect(pick("machine learn")).toEqual([2]);
  expect(pick("learning machine")).toEqual([2]);
  expect(pick("gen")).toEqual(["general"]);
  expect(pick("2026")).toEqual([1, 2]);
  expect(pick("zzz")).toEqual([]);
});

test("the heading trigger asks for a choice until one is made; the chip shows General", () => {
  const html = (value: "general" | number | null, variant: "heading" | "chip") =>
    renderToStaticMarkup(<SubjectPicker subjects={subjects} value={value} onChange={() => {}} variant={variant} />);
  expect(html(null, "heading")).toContain("Choose a subject");
  expect(html("general", "heading")).toContain(">General<");
  const chosen = html(1, "heading");
  expect(chosen).toContain(">MAST20004<");
  expect(chosen).not.toContain("MAST20004_2026_SM2");
  expect(chosen).toContain('aria-label="Subject: Probability"');
  expect(chosen).toContain('title="Probability"');
  expect(html(null, "chip")).toContain(">General<");
});
