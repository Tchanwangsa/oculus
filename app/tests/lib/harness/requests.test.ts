import { describe, expect, test } from "bun:test";
import {
  accessLines,
  awaitsApproval,
  changeKindLabel,
  diffLines,
  requestActions,
  requestDetail,
  requestState,
  reviewView,
  waitingRequests,
  type HarnessRequest,
} from "@/lib/harness";

const APPROVE = { id: "accept", label: "Approve once", decision: "accept" as const };
const SESSION = {
  id: "accept_for_session",
  label: "Always allow this exact command",
  decision: "accept_for_session" as const,
  scope: "Codex runs this exact command again in this chat without asking while its session lasts.",
};
const DECLINE = { id: "decline", label: "Decline", decision: "decline" as const };

const request = (patch: Partial<HarnessRequest> = {}): HarnessRequest => ({
  id: 3, thread_id: 9, run_id: 1, item_id: 12, provider: "codex", kind: "approval", action: "command",
  payload: {
    command: "/bin/zsh -lc 'touch /private/tmp/out/accepted'", cwd: "/private/tmp/probe",
    reason: "Allow this touch?", outsideSandbox: true, tool: "exec-1",
  },
  options: [APPROVE, SESSION, DECLINE],
  status: "pending", responder: "live", reason: null, response: null, delivery: "none",
  created_at: "2026-11-16 00:00:00", resolved_at: null, ...patch,
});

describe("what a request row shows per action", () => {
  test("a command: the whole command, its folder, its reason and that it runs outside the sandbox", () => {
    const d = requestDetail(request());
    expect(d.title).toBe("Codex wants to run a command");
    expect(d.command).toBe("/bin/zsh -lc 'touch /private/tmp/out/accepted'");
    expect(d.cwd).toBe("/private/tmp/probe");
    expect(d.reason).toBe("Allow this touch?");
    expect(d.outsideSandbox).toBe(true);
    expect(d.agent).toBeNull();
  });

  test("the sandbox line only where Rust says so", () => {
    const d = requestDetail(request({ payload: { command: "ls", cwd: "/x" } }));
    expect(d.outsideSandbox).toBe(false);
  });

  test("a file change: each path with its kind and diff", () => {
    const d = requestDetail(request({
      action: "file_change",
      options: [{ ...APPROVE, label: "Approve" }, DECLINE],
      payload: {
        changes: [
          { path: "/tmp/out/dummy.txt", kind: { type: "update", move_path: null }, diff: "@@ -1 +1 @@\n-old\n+new\n" },
          { path: "/tmp/out/b.md", kind: { type: "add" }, diff: "+hello\n" },
        ],
        reason: null,
      },
    }));
    expect(d.title).toBe("Codex wants to change 2 files");
    expect(d.changes.map((c) => [c.path, changeKindLabel(c)])).toEqual([["/tmp/out/dummy.txt", "Edit"], ["/tmp/out/b.md", "New file"]]);
    expect(d.subject).toBe("dummy.txt, b.md");
    expect(d.outsideSandbox).toBe(false);
    expect(diffLines(d.changes[0].diff).map((l) => l.tone)).toEqual(["hunk", "remove", "add"]);
  });

  test("one file is 'a file'; a move names where it goes", () => {
    const d = requestDetail(request({
      action: "file_change",
      payload: { changes: [{ path: "/a/x.txt", kind: { type: "update", move_path: "/a/y.txt" }, diff: "" }] },
    }));
    expect(d.title).toBe("Codex wants to change a file");
    expect(changeKindLabel(d.changes[0])).toBe("Move to /a/y.txt");
  });

  test("permissions: the access asked for, each path once", () => {
    const permissions = {
      network: null,
      fileSystem: { read: null, write: ["/tmp/outside"], entries: [{ path: { type: "path", path: "/tmp/outside" }, access: "write" }] },
    };
    const d = requestDetail(request({ action: "permissions", payload: { permissions, cwd: "/tmp/probe", reason: "probe" } }));
    expect(d.title).toBe("Codex wants more access");
    expect(d.access).toEqual(["Write to /tmp/outside"]);
    expect(accessLines({ network: { enabled: true }, fileSystem: { read: ["/r"], write: [] } })).toEqual(["Read /r", "Reach the network"]);
  });

  test("a child agent's request names the agent", () => {
    const d = requestDetail(request({
      payload: { command: "touch child", agent: { path: "/root/child_approval_probe", thread: "t2" }, outsideSandbox: true },
    }));
    expect(d.agent).toBe("child_approval_probe");
    expect(d.title).toBe("Codex's subagent child_approval_probe wants to run a command");
  });
});

describe("how a request stands", () => {
  test("pending waits for approval", () => {
    expect(requestState(request())).toEqual({ kind: "waiting", label: "Waiting for approval", notes: [], warn: false });
  });
  test("a failed delivery is pending again and says to answer again", () => {
    const s = requestState(request({ delivery: "failed" }));
    expect(s.kind).toBe("waiting");
    expect(s.notes).toEqual(["Your answer did not reach Codex. Answer again."]);
    expect(s.warn).toBe(true);
  });
  test("approved, always allowed with its scope, declined", () => {
    expect(requestState(request({ status: "resolved", response: "accept", delivery: "acknowledged" })))
      .toEqual({ kind: "approved", label: "Approved", notes: [], warn: false });
    const always = requestState(request({ status: "resolved", response: "accept_for_session", delivery: "acknowledged" }));
    expect(always.label).toBe("Always allowed");
    expect(always.notes).toEqual([SESSION.scope]);
    expect(requestState(request({ status: "resolved", response: "decline", delivery: "acknowledged" })).label).toBe("Declined");
  });
  test("an answer on its way says so", () => {
    expect(requestState(request({ status: "resolved", response: "accept", delivery: "sending" })).notes)
      .toEqual(["Sending your answer to Codex…"]);
  });
  test("an uncertain delivery is said and warned", () => {
    const s = requestState(request({ status: "resolved", response: "accept", delivery: "uncertain" }));
    expect(s.warn).toBe(true);
    expect(s.notes[0]).toStartWith("Delivery uncertain");
  });
  test("expired and cancelled carry Rust's reason", () => {
    const expired = requestState(request({ status: "expired", responder: "not_resumable", reason: "The app was closed." }));
    expect([expired.kind, expired.label, expired.notes]).toEqual(["expired", "Expired", ["The app was closed."]]);
    const cancelled = requestState(request({ status: "cancelled", responder: "not_resumable", reason: "The turn was stopped." }));
    expect([cancelled.kind, cancelled.label, cancelled.notes]).toEqual(["cancelled", "Cancelled", ["The turn was stopped."]]);
  });
});

describe("which answers are offered", () => {
  test("Decline and Approve inline, the session grant under More", () => {
    const a = requestActions(request());
    expect(a.decline?.id).toBe("decline");
    expect(a.approve?.id).toBe("accept");
    expect(a.more.map((o) => o.label)).toEqual(["Always allow this exact command"]);
  });
  test("no More where no grant is offered", () => {
    expect(requestActions(request({ options: [APPROVE, DECLINE] })).more).toEqual([]);
  });
  test("nothing for a request that ended or whose route is gone", () => {
    for (const r of [
      request({ status: "resolved", response: "accept" }),
      request({ status: "expired", responder: "not_resumable" }),
      request({ responder: "not_resumable" }),
    ]) {
      expect(requestActions(r)).toEqual({ decline: null, approve: null, more: [] });
    }
  });
});

describe("waiting requests and the sidebar's Approval", () => {
  const byId = (...rs: HarnessRequest[]) => Object.fromEntries(rs.map((r) => [r.id, r]));
  test("the oldest answerable first", () => {
    const rs = byId(request({ id: 7 }), request({ id: 4 }), request({ id: 2, status: "cancelled", responder: "not_resumable" }));
    expect(waitingRequests(rs).map((r) => r.id)).toEqual([4, 7]);
  });
  test("a run waits when one of its own requests does", () => {
    const rs = byId(request({ id: 1, run_id: 5 }));
    expect(awaitsApproval(rs, 5)).toBe(true);
    expect(awaitsApproval(rs, 6)).toBe(false);
    expect(awaitsApproval(rs)).toBe(true);
  });
});

describe("a reviewer's decision on a command row", () => {
  test("approved, with risk and rationale", () => {
    expect(reviewView({ status: "approved", risk: "low", rationale: "Harmless.", source: "agent" }))
      .toEqual({ label: "Reviewer approved", tone: "muted", risk: "Low risk", rationale: "Harmless." });
  });
  test("denied reads as a refusal", () => {
    const v = reviewView({ status: "denied", risk: "medium", rationale: "Too broad." });
    expect([v?.label, v?.tone, v?.risk]).toEqual(["Reviewer denied", "danger", "Medium risk"]);
  });
  test("no review, nothing", () => {
    expect(reviewView(undefined)).toBeNull();
  });
});
