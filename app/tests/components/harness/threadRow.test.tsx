import { afterAll, describe, expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import type { HarnessItem, HarnessThread } from "@/lib/harness";
import type { Subject } from "@/lib/db";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { ThreadRow } from "@/components/harness/threads/ThreadRow";
import { RecentsBody } from "@/components/harness/threads/list/RecentsBody";
import { activityLine } from "@/lib/harness/activity";

// A server render reads a zustand store's initial state, so live rows go there.
const initial = useHarnessStore.getInitialState();
const initialItems = initial.items;
afterAll(() => {
  initial.items = initialItems;
});

const sqlite = (msAgo: number) => new Date(Date.now() - msAgo).toISOString().slice(0, 19).replace("T", " ");

const thread = (patch: Partial<HarnessThread> = {}): HarnessThread => ({
  id: 7,
  provider: "claude",
  provider_session_id: null,
  model: null,
  subject_id: 3,
  lecture_id: null,
  title: "Alpha-beta pruning walkthrough",
  status: "idle",
  usage: null,
  created_at: sqlite(3_600_000),
  updated_at: sqlite(25 * 60_000),
  ...patch,
});

const subject = { id: 3, code: "COMP30024_2026_SM1", name: "Artificial Intelligence" } as Subject;

const render = (patch: Partial<HarnessThread>, props: Partial<Parameters<typeof ThreadRow>[0]> = {}) =>
  renderToStaticMarkup(
    <ThreadRow
      thread={thread(patch)}
      subject={subject}
      preview="So P(D | +) is about 0.09"
      question={undefined}
      active={false}
      running={false}
      confirming={false}
      onOpen={() => {}}
      onNewTab={() => {}}
      onArm={() => {}}
      onDelete={() => {}}
      {...props}
    />,
  );

/** The title's span, by its text. */
const titleClass = (html: string) => html.match(/<span class="([^"]*)">Alpha-beta pruning walkthrough<\/span>/)?.[1] ?? "";

describe("ThreadRow", () => {
  test("an idle, read thread shows its subject, age and last message, title dimmed", () => {
    const html = render({ settled_at: sqlite(30 * 60_000), viewed_at: new Date().toISOString() });
    expect(html).toContain("COMP30024");
    expect(html).not.toContain("COMP30024_2026");
    expect(html).toContain(">25m<");
    expect(html).toContain("So P(D | +) is about 0.09");
    expect(titleClass(html)).toContain("text-foreground/80");
    expect(titleClass(html)).not.toContain("font-medium");
    // Three levels: the preview is smaller and dimmer than even a read title.
    expect(html).toMatch(/text-\[12\.5px\] text-muted-foreground\/85[^>]*><span class="min-w-0 flex-1 truncate">So P/);
    expect(html).toContain("text-[13.5px]");
  });

  test("an unread finished thread is Done in green with a bright, medium title", () => {
    const html = render({ settled_at: sqlite(60_000), viewed_at: null });
    expect(html).toContain("text-success");
    expect(html).toContain("Done");
    expect(html).not.toContain(">25m<");
    expect(titleClass(html)).toContain("font-medium text-foreground");
  });

  test("a waiting question is Question in yellow, and line three is the question", () => {
    const html = render({ pending_requests: 1, pending_questions: 1 }, { question: "Proof or intuition?" });
    expect(html).toContain("text-warning");
    expect(html).toContain("Question");
    expect(html).toContain("Proof or intuition?");
    expect(html).not.toContain("So P(D | +)");
    expect(titleClass(html)).toContain("font-medium");
  });

  test("approval and failure are yellow and red, both bright", () => {
    const approval = render({ pending_requests: 1 });
    expect(approval).toContain("Approval");
    expect(approval).toContain("text-warning");
    const failed = render({ status: "error" });
    expect(failed).toContain("Failed");
    expect(failed).toContain("text-destructive");
    expect(titleClass(failed)).toContain("font-medium");
  });

  test("a working thread shows the dashed ring and its timer, title dimmed, the last message below", () => {
    const html = render({ status: "running", run_started_at: sqlite(9_000) }, { running: true });
    expect(html).toContain("border-dashed");
    expect(html).toMatch(/Working<span class="tabular-nums">(9|10)s<\/span>/);
    expect(titleClass(html)).toContain("text-foreground/80");
    expect(html).toContain("So P(D | +) is about 0.09");
  });

  test("a working thread a view holds says what its run is doing", () => {
    const tool: HarnessItem = {
      id: 1, thread_id: 7, kind: "tool", ref_id: "t1", created_at: "",
      content: "/Users/me/COMP30024/week-7-search.pdf", meta: JSON.stringify({ kind: "read" }),
    };
    initial.items = { 7: [tool] };
    try {
      const html = render({ status: "running" }, { running: true });
      expect(html).toContain("Reading week-7-search.pdf");
      expect(html).not.toContain("So P(D | +)");
    } finally {
      initial.items = initialItems;
    }
  });

  test("a general thread reads General", () => {
    expect(render({ subject_id: null }, { subject: undefined })).toContain("General");
  });

  test("an armed row shows its delete confirm instead of the menu", () => {
    const html = render({}, { confirming: true });
    expect(html).toContain(">Delete<");
    expect(html).not.toContain("Actions for");
  });

  test("hover offers the menu alone", () => {
    const html = render({});
    expect(html).toContain("Actions for Alpha-beta pruning walkthrough");
    expect(html).not.toContain("Complete");
  });
});

describe("activityLine", () => {
  const row = (patch: Partial<HarnessItem>): HarnessItem => ({
    id: 1, thread_id: 1, kind: "tool", ref_id: null, content: null, meta: null, created_at: "", ...patch,
  });
  test("a tool still running names its verb and target", () => {
    expect(activityLine([row({ content: "python bench.py --games 200", meta: '{"kind":"bash"}' })])).toBe("Running python bench.py --games 200");
  });
  test("reasoning is Thinking; a finished tool or a reply says nothing", () => {
    expect(activityLine([row({ kind: "thinking" })])).toBe("Thinking");
    expect(activityLine([row({ meta: '{"kind":"read","ok":true}', content: "a.pdf" })])).toBeNull();
    expect(activityLine([row({ kind: "assistant", content: "hi" })])).toBeNull();
    expect(activityLine(undefined)).toBeNull();
  });
});

describe("Recents", () => {
  const list = [
    thread({ id: 1, updated_at: sqlite(30_000), status: "running" }),
    thread({ id: 2, updated_at: sqlite(20_000), pending_requests: 1 }),
    thread({ id: 3, updated_at: sqlite(60_000), status: "error" }),
    thread({ id: 4, updated_at: sqlite(2_000) }),
  ];
  const body = (more: boolean) =>
    renderToStaticMarkup(
      <RecentsBody
        threads={list}
        more={more}
        onMore={() => {}}
        collapsed={false}
        row={(t) => <div key={t.id} data-row={t.id} />}
      />,
    );

  test("every thread newest first under one heading, working ones in place", () => {
    const html = body(false);
    expect(html).toContain(">Recents<");
    expect([...html.matchAll(/data-row="(\d+)"/g)].map((m) => Number(m[1]))).toEqual([4, 2, 1, 3]);
    expect(html).not.toContain("Show more");
  });

  test("Show more follows the rows while older threads exist", () => {
    expect(body(true)).toMatch(/data-row="3"><\/div><button[^>]*>Show more<\/button>/);
  });
});
