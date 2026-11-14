import { describe, expect, test } from "bun:test";
import { threadStatus, threadUnread, type ThreadStatusInputs } from "@/lib/harness";

const quiet: ThreadStatusInputs = { running: false, failed: false, unread: false, approval: false, input: false };

describe("threadStatus", () => {
  test("a quiet thread has none", () => {
    expect(threadStatus(quiet)).toBeNull();
  });

  test("each input alone names its status", () => {
    expect(threadStatus({ ...quiet, approval: true })).toBe("approval");
    expect(threadStatus({ ...quiet, input: true })).toBe("input");
    expect(threadStatus({ ...quiet, running: true })).toBe("working");
    expect(threadStatus({ ...quiet, failed: true })).toBe("failed");
    expect(threadStatus({ ...quiet, unread: true })).toBe("done");
  });

  test("order: Approval, Input, Working, Failed, Done", () => {
    const all = { running: true, failed: true, unread: true, approval: true, input: true };
    expect(threadStatus(all)).toBe("approval");
    expect(threadStatus({ ...all, approval: false })).toBe("input");
    expect(threadStatus({ ...all, approval: false, input: false })).toBe("working");
    expect(threadStatus({ ...all, approval: false, input: false, running: false })).toBe("failed");
    expect(threadStatus({ ...quiet, unread: true, failed: false })).toBe("done");
  });

  test("a failure outranks an unread finish", () => {
    expect(threadStatus({ ...quiet, failed: true, unread: true })).toBe("failed");
  });
});

describe("threadUnread", () => {
  test("a run that ended after the last look is unread", () => {
    expect(threadUnread({ settled_at: "2026-11-15 10:00:05", viewed_at: "2026-11-15T10:00:01.000Z" })).toBe(true);
  });
  test("a look after the run's end reads it, even in the same second", () => {
    expect(threadUnread({ settled_at: "2026-11-15 10:00:05", viewed_at: "2026-11-15T10:00:05.300Z" })).toBe(false);
    expect(threadUnread({ settled_at: "2026-11-15 10:00:05", viewed_at: "2026-11-15 10:00:05" })).toBe(false);
  });
  test("no run ended: nothing to read", () => {
    expect(threadUnread({ settled_at: null, viewed_at: null })).toBe(false);
    expect(threadUnread({})).toBe(false);
  });
  test("never viewed, with a run ended: unread", () => {
    expect(threadUnread({ settled_at: "2026-11-15 10:00:05", viewed_at: null })).toBe(true);
  });
});
