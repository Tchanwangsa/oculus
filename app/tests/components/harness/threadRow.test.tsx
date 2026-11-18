import { afterAll, describe, expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import type { HarnessItem, HarnessThread } from "@/lib/harness";
import type { Subject } from "@/lib/db";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { ThreadRow } from "@/components/harness/threads/ThreadRow";
import { RECENTS_FIRST, RecentsBody } from "@/components/harness/threads/list/RecentsBody";
import { SubjectBody } from "@/components/harness/threads/list/SubjectBody";
import { ListHeader, readThreadView } from "@/components/harness/threads/list/Header";
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
      showSubject
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

  test("the Subject view drops the subject line and keeps the status on the title's line", () => {
    const html = render({ settled_at: sqlite(60_000) }, { showSubject: false });
    expect(html).not.toContain("COMP30024");
    expect(html).toMatch(/Alpha-beta pruning walkthrough<\/span>.*Done/);
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
  const body = (more: boolean, threads = list) =>
    renderToStaticMarkup(
      <RecentsBody threads={threads} more={more} onMore={() => {}} row={(t) => <div key={t.id} data-row={t.id} />} />,
    );
  const rows = (html: string) => [...html.matchAll(/data-row="(\d+)"/g)].map((m) => Number(m[1]));

  test("every thread newest first with no heading, working ones in place", () => {
    const html = body(false);
    expect(html).not.toContain(">Recents<");
    expect(rows(html)).toEqual([4, 2, 1, 3]);
    expect(html).not.toContain("aria-hidden");
  });

  test("draws the first rows, with a scroll sentinel while more exist", () => {
    const many = Array.from({ length: RECENTS_FIRST + 3 }, (_, i) => thread({ id: i + 1, updated_at: sqlite((i + 1) * 1000) }));
    const html = body(false, many);
    expect(rows(html)).toHaveLength(RECENTS_FIRST);
    expect(html).toContain('aria-hidden="true"');
    expect(body(true)).toContain('aria-hidden="true"');
  });
});

describe("Subject view", () => {
  const other = { id: 4, code: "COMP30026_2026_SM1", name: "Models of Computation" } as Subject;
  const list = [
    thread({ id: 1, subject_id: 3, updated_at: sqlite(30_000) }),
    thread({ id: 2, subject_id: 4, updated_at: sqlite(20_000) }),
    thread({ id: 3, subject_id: null, updated_at: sqlite(60_000) }),
    thread({ id: 4, subject_id: 3, updated_at: sqlite(2_000) }),
  ];
  const body = (more: boolean, threads = list) =>
    renderToStaticMarkup(
      <SubjectBody
        threads={threads}
        subjects={[subject, other]}
        activeId={null}
        runningIds={new Set()}
        folded={new Set()}
        onGroupOpen={() => {}}
        collapsed={false}
        onNew={() => {}}
        more={more}
        onMore={() => {}}
        row={(t) => <div key={t.id} data-row={t.id} />}
      />,
    );

  test("groups by subject, newest group first, rows newest first", () => {
    const html = body(false);
    expect([...html.matchAll(/title="New thread in ([^"]+)"/g)].map((m) => m[1])).toEqual([
      "COMP30024",
      "COMP30026",
      "General",
    ]);
    expect([...html.matchAll(/data-row="(\d+)"/g)].map((m) => Number(m[1]))).toEqual([4, 1, 2, 3]);
    expect(html).not.toContain("more");
  });

  test("a group pages by five, then offers older threads while the database has them", () => {
    const many = Array.from({ length: 7 }, (_, i) => thread({ id: i + 1, updated_at: sqlite(i * 1000) }));
    expect(body(false, many)).toMatch(/Show <span class="tabular-nums">2<\/span> more/);
    expect(body(false, list)).not.toContain("Show more");
    expect(body(true, list).match(/>Show more</g)?.length).toBe(3);
  });
});

describe("view switch", () => {
  const header = (view: "recents" | "subject") =>
    renderToStaticMarkup(
      <ListHeader
        view={view}
        onView={() => {}}
        query=""
        onQuery={() => {}}
        onSearchEnter={() => {}}
        fresh={false}
        onNew={() => {}}
        collapsed={false}
      />,
    );

  test("Recents and Subject tabs, the stored one selected", () => {
    expect(header("recents")).toMatch(/aria-selected="true"[^>]*>.*?Recents/);
    expect(header("subject")).toMatch(/aria-selected="true"[^>]*>.*?Subject/);
  });

  test("anything but subject opens on Recents, status included", () => {
    expect(readThreadView("subject")).toBe("subject");
    expect(readThreadView("status")).toBe("recents");
    expect(readThreadView(null)).toBe("recents");
  });
});
