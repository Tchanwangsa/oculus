import { expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { SubjectPicker } from "@/components/harness/composer/SubjectPicker";
import { subjectOption, subjectOptions } from "@/components/harness/composer/subjectOptions";
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

test("the chip shows the short code, or General", () => {
  const html = (value: "general" | number) =>
    renderToStaticMarkup(<SubjectPicker subjects={subjects} value={value} onChange={() => {}} />);
  expect(html("general")).toContain(">General<");
  const chosen = html(1);
  expect(chosen).toContain(">MAST20004<");
  expect(chosen).not.toContain("MAST20004_2026_SM2");
  expect(chosen).toContain('aria-label="Subject: Probability"');
});

test("a scope names its row; an unchosen or deleted one names none", () => {
  expect(subjectOption(subjects, "general")?.label).toBe("General");
  expect(subjectOption(subjects, 3)?.label).toBe("MAST10007");
  expect(subjectOption(subjects, null)).toBeNull();
  expect(subjectOption(subjects, 99)).toBeNull();
});

test("the headline asks for a subject, then asks the question in it", async () => {
  const { DraftHero } = await import("@/components/harness/composer/DraftHero");
  const html = (scope: "general" | number | null) =>
    renderToStaticMarkup(<DraftHero subjects={subjects} scope={scope} onChoose={() => {}} />);
  const text = (scope: "general" | number | null) => html(scope).replace(/<[^>]+>/g, " ").replace(/\s+/g, " ");
  expect(text(null)).toContain("Choose a subject to start");
  expect(text(null)).not.toContain("What should we work on");
  expect(text(2)).toContain("What should we work on in COMP30027 ?");
  expect(text("general")).toContain("What should we work on in General ?");
  // The pill is a button that opens the dialog, named by the full subject.
  expect(html(1)).toContain('aria-label="Subject: Probability"');
  expect(html(1)).toContain('aria-haspopup="dialog"');
});

test("the new-thread dialog lists General and this term, then past subjects, and marks the choice", async () => {
  const { SubjectChoices } = await import("@/components/harness/threads/NewThreadDialog");
  const html = renderToStaticMarkup(<SubjectChoices subjects={subjects} value={2} onPick={() => {}} />);
  const text = html.replace(/<[^>]+>/g, " ").replace(/\s+/g, " ");
  expect(text).toContain("New thread in General The whole library MAST20004 Probability COMP30027 Machine Learning");
  expect(text).toContain("Past subjects PHYC10003 Physics 1 MAST10007 Linear Algebra");
  expect(html).not.toContain("MAST20004_2026_SM2");
  // One row is the choice, and the highlight opens on it.
  expect(html.match(/aria-selected="true"/g)?.length).toBe(1);
  expect(html).toMatch(/aria-selected="true"[^>]*data-index="2"/);
  expect(html).toMatch(/data-index="2"[^>]*class="[^"]*bg-accent/);
});
