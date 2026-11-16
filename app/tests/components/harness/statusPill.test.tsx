import { describe, expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import type { HarnessThread } from "@/lib/harness";
import { StatusPill, threadRowStatus } from "@/components/harness/threads/StatusPill";

const thread = (patch: Partial<HarnessThread>): HarnessThread => ({
  id: 1,
  provider: "claude",
  provider_session_id: null,
  model: null,
  subject_id: null,
  lecture_id: null,
  title: "t",
  status: "idle",
  usage: null,
  created_at: "2026-11-15 10:00:00",
  updated_at: "2026-11-15 10:00:00",
  ...patch,
});

describe("threadRowStatus", () => {
  test("a running thread works, whatever else it is", () => {
    expect(threadRowStatus(thread({ status: "error", settled_at: "2026-11-15 10:00:05" }), true)).toBe("working");
  });
  test("a failed run shows until the next one", () => {
    expect(threadRowStatus(thread({ status: "error", settled_at: "2026-11-15 10:00:05", viewed_at: "2026-11-15T10:01:00.000Z" }), false)).toBe("failed");
  });
  test("a run that ended while the student was elsewhere is done", () => {
    expect(threadRowStatus(thread({ settled_at: "2026-11-15 10:00:05", viewed_at: "2026-11-15T10:00:00.000Z" }), false)).toBe("done");
  });
  test("a read thread is quiet", () => {
    expect(threadRowStatus(thread({ settled_at: "2026-11-15 10:00:05", viewed_at: "2026-11-15T10:01:00.000Z" }), false)).toBeNull();
  });
});

describe("StatusPill", () => {
  test("working is a brand dashed ring, turning only when motion is welcome", () => {
    const html = renderToStaticMarkup(<StatusPill status="working" />);
    expect(html).toContain("text-brand");
    expect(html).toContain("border-dashed");
    expect(html).toContain("motion-safe:animate-");
    expect(html).toContain("Working");
  });
  test("working with a start shows its elapsed time; without one, only the word", () => {
    const since = Date.now() - 64_000;
    expect(renderToStaticMarkup(<StatusPill status="working" since={since} />)).toMatch(/Working<span class="tabular-nums">1m [45]s<\/span>/);
    expect(renderToStaticMarkup(<StatusPill status="working" />)).not.toContain("tabular-nums");
  });
  test("each status takes its tone", () => {
    expect(renderToStaticMarkup(<StatusPill status="failed" />)).toContain("text-destructive");
    expect(renderToStaticMarkup(<StatusPill status="done" />)).toContain("text-success");
    expect(renderToStaticMarkup(<StatusPill status="approval" />)).toContain("text-warning");
    expect(renderToStaticMarkup(<StatusPill status="input" />)).toContain("text-warning");
    expect(renderToStaticMarkup(<StatusPill status="input" />)).toContain("Question");
  });
});

describe("Approval in the sidebar", () => {
  test("a waiting request outranks working and failed", () => {
    expect(threadRowStatus(thread({ status: "error", pending_requests: 1 }), true)).toBe("approval");
  });
  test("no waiting request, no Approval", () => {
    expect(threadRowStatus(thread({ pending_requests: 0 }), true)).toBe("working");
    expect(threadRowStatus(thread({}), true)).toBe("working");
  });
});

test("a question shows Question; an approval alongside it retains Approval priority", () => {
  expect(threadRowStatus(thread({ pending_requests: 1, pending_questions: 1 }), true)).toBe("input");
  expect(threadRowStatus(thread({ pending_requests: 2, pending_questions: 1 }), true)).toBe("approval");
  expect(threadRowStatus(thread({ pending_requests: 1, pending_questions: 1 }), false)).toBe("input");
});
