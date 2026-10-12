import { describe, expect, test } from "bun:test";
import type { DbFile } from "@/lib/db";
import { filterEdThreads, markHits, matchSnippet, parseEdThread, queryWords, threadTitle, type ThreadFilters } from "@/lib/ed/threads";

const file = { id: 1, filename: "0046-question-4-a2.md" } as DbFile;
const markdown = "# Question 4 A2\n\n**#46 · question · unresolved · Assignments / A2**  \n**By:** Student · 2026-10-12 15:42\n\n---\n\nHow do I calculate the period?\n\n## Replies\nUse the eigenvalues of the rotation matrix.";
const filters = (overrides: Partial<ThreadFilters> = {}): ThreadFilters => ({
  query: "", types: new Set(), statuses: new Set(), categories: new Set(), ...overrides,
});

describe("Ed board search and filters", () => {
  test("keeps the original title and puts the unpadded number into it", () => {
    const thread = parseEdThread(markdown, file);
    expect(threadTitle(thread)).toBe("#46: Question 4 A2");
    expect(thread.category).toBe("Assignments / A2");
    expect(thread.resolved).toBe(false);
    expect(thread.posted?.getHours()).toBe(15);
  });

  test("matches title, body and reply keywords together, regardless of case or order", () => {
    const thread = parseEdThread(markdown, file);
    expect(filterEdThreads([thread], filters({ query: "  EIGENVALUES   period A2 " }))).toEqual([thread]);
    expect(filterEdThreads([thread], filters({ query: "#46" }))).toEqual([thread]);
    expect(filterEdThreads([thread], filters({ query: "period absent" }))).toEqual([]);
  });

  test("combines filters, and answer-status selections exclude non-questions", () => {
    const question = parseEdThread(markdown, file);
    const announcement = parseEdThread("# Notice\n\n**#45 · announcement · General**\n\n---\n\nPeriod extended", { ...file, id: 2 });
    const all = [question, announcement];
    expect(filterEdThreads(all, filters({ types: new Set(["question", "announcement"]) }))).toEqual(all);
    expect(filterEdThreads(all, filters({ query: "period", statuses: new Set(["unresolved"]), categories: new Set(["Assignments / A2"]) }))).toEqual([question]);
    expect(filterEdThreads(all, filters({ types: new Set(["announcement"]), statuses: new Set(["unresolved"]) }))).toEqual([]);
  });

  test("front matter supplies the metadata and the meta line is the fallback", () => {
    const front = [
      "---",
      "ed_thread_id: 98765",
      "number: 47",
      'type: "question"',
      "answered: true",
      'category: "Assignments"',
      'subcategory: "A2"',
      "subsubcategory: null",
      'category_color: "#9b59b6"',
      'url: "https://edstem.org/xx/courses/1/discussion/98765"',
      "---",
      "",
    ].join("\n");
    const thread = parseEdThread(front + markdown, file);
    expect(threadTitle(thread)).toBe("#47: Question 4 A2");
    expect(thread.kind).toBe("question");
    expect(thread.resolved).toBe(true);
    expect(thread.category).toBe("Assignments / A2");
    expect(thread.categoryColor).toBe("#9b59b6");
    expect(thread.url).toBe("https://edstem.org/xx/courses/1/discussion/98765");
    expect(thread.posted?.getHours()).toBe(15);
    expect(filterEdThreads([thread], filters({ query: "category_color" }))).toEqual([]);

    const old = parseEdThread(markdown, { ...file, source_url: "https://edstem.org/xx/courses/1/discussion/98765" });
    expect(old.categoryColor).toBeNull();
    expect(old.url).toBe("https://edstem.org/xx/courses/1/discussion/98765");
    expect(parseEdThread(markdown, { ...file, source_url: "https://canvas.example.org/x" }).url).toBeNull();
  });

  test("older and missing scrapes retain usable titles and unknown status", () => {
    const older = parseEdThread("# Old title\n\n**#46 · question · General**\n\n---\n\nBody", file);
    expect(older.resolved).toBeNull();
    expect(filterEdThreads([older], filters({ statuses: new Set(["unknown"]) }))).toEqual([older]);
    const missing = parseEdThread("", file);
    expect(threadTitle(missing)).toBe("#46: Question 4 A2");
    expect(filterEdThreads([missing], filters({ query: "question" }))).toEqual([missing]);
  });
});

describe("Ed board match snippets", () => {
  const thread = parseEdThread(markdown, file);

  test("the snippet is the prose around the first hit, markdown dropped", () => {
    const parts = matchSnippet(thread, queryWords("EIGENVALUES"))!;
    expect(parts.map((p) => p.text).join("")).toContain("Use the eigenvalues of the rotation matrix.");
    expect(parts.filter((p) => p.hit).map((p) => p.text)).toEqual(["eigenvalues"]);
  });

  test("a keyword only in the title gives no snippet but is marked there", () => {
    expect(matchSnippet(thread, queryWords("A2"))).toBeNull();
    expect(markHits("Question 4 A2", ["a2"])).toEqual([
      { text: "Question 4 ", hit: false },
      { text: "A2", hit: true },
    ]);
  });
});
