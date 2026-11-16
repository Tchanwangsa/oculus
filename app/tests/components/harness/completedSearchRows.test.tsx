import { describe, expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import type { Subject } from "@/lib/db";
import type { CompletedPage, HarnessThread, ThreadSearchResult } from "@/lib/harness";
import { CompletedList } from "@/components/harness/threads/completed/CompletedList";
import { withCompletedGroups } from "@/components/harness/threads/completed/groups";
import { completedTotal } from "@/components/harness/threads/completed/StatusCompleted";
import { SearchResults } from "@/components/harness/threads/search/SearchResults";

const sqlite = (msAgo: number) => new Date(Date.now() - msAgo).toISOString().slice(0, 19).replace("T", " ");
const thread = (id: number, patch: Partial<HarnessThread> = {}) =>
  ({ id, title: `Thread ${id}`, subject_id: 3, provider: "claude", status: "idle", updated_at: sqlite(4 * 86_400_000), completed_at: sqlite(86_400_000), ...patch }) as HarnessThread;
const subject = { id: 3, code: "MAST10007_2026_SM2", name: "Linear Algebra" } as Subject;
const bySubject = new Map([[3, subject]]);

const list = (page: CompletedPage) =>
  renderToStaticMarkup(
    <CompletedList page={page} bySubject={bySubject} showSubject activeId={null} collapsed={false} onOpen={() => {}} onReopen={() => {}} onMore={() => {}} />,
  );

describe("Completed rows", () => {
  test("one grey line each with its age, Reopen on hover, and the next page's size", () => {
    const html = list({ threads: [thread(1), thread(2)], total: 40, nextOffset: 2 });
    expect(html).toContain("Thread 1");
    expect(html).toContain(">4d<");
    expect(html).toContain('title="Reopen thread"');
    expect(html).toContain("text-muted-foreground");
    expect(html).toMatch(/Show <span class="tabular-nums">25<\/span> more/);
  });

  test("the last page shows how many remain, and none once read to the end", () => {
    expect(list({ threads: [thread(1)], total: 4, nextOffset: 1 })).toMatch(/Show <span class="tabular-nums">3<\/span> more/);
    expect(list({ threads: [thread(1)], total: 1, nextOffset: null })).not.toContain("more");
  });

  test("the Status count sums every subject's", () => {
    expect(completedTotal({ general: 4, "3": 9 })).toBe(13);
    expect(completedTotal({})).toBe(0);
  });

  test("a subject whose threads are all completed still gets a group for its fold", () => {
    const open = [{ key: "3", label: "MAST10007", title: "", subjectId: 3, items: [thread(1)] }];
    const groups = withCompletedGroups(open, [subject, { id: 5, code: "PHYC10001_2026_SM2", name: "Physics" } as Subject], { "3": 2, "5": 4, general: 1, "9": 3 });
    expect(groups.map((g) => [g.key, g.items.length])).toEqual([["3", 1], ["5", 0], ["general", 0]]);
    expect(withCompletedGroups(open, [subject], {})).toBe(open);
  });
});

const result = (id: number, snippet: string, patch: Partial<ThreadSearchResult> = {}) =>
  ({ ...thread(id, { completed_at: null }), snippet, subject_code: subject.code, subject_name: subject.name, score: -1, ...patch }) as ThreadSearchResult;

describe("Search results", () => {
  test("rows show the subject, the hit in bold as a span, and Completed for a completed chat", () => {
    const html = renderToStaticMarkup(
      <SearchResults
        found={{ q: "eigen", results: [result(1, "…the **eigen**values <i>of</i> A…"), result(2, "plain", { completed_at: sqlite(1000) })] }}
        activeId={null}
        onOpen={() => {}}
      />,
    );
    expect(html).toContain(`<span class="tabular-nums">2</span> chats mention “eigen”`);
    expect(html).toContain("MAST10007");
    expect(html).toContain('<span class="font-semibold text-foreground">eigen</span>values');
    // Text, never markup.
    expect(html).toContain("&lt;i&gt;of&lt;/i&gt;");
    expect(html).toContain("Completed · 4d");
    expect(html.match(/Completed ·/g)).toHaveLength(1);
  });

  test("no results is one muted line", () => {
    const html = renderToStaticMarkup(<SearchResults found={{ q: " zzz ", results: [] }} activeId={null} onOpen={() => {}} />);
    expect(html).toContain("No chats mention “zzz”");
    expect(html).not.toContain("data-search-row");
  });
});
