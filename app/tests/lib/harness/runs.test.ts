import { expect, test } from "bun:test";
import { lastRunStatus, liveRunId, runEnded, runFromRow, runningSince, STOP_PATIENCE_MS, stopDueAt, stopOverdue, type RunRow } from "@/lib/harness/runs";
import type { HarnessRun } from "@/lib/harness/types";

const row = (id: number, execution_status: string, more: Partial<RunRow> = {}): RunRow => ({
  id, execution_status, delivery_state: "unknown", answer_item_id: null, started_at: "2026-11-15 00:00:00", ended_at: null, ...more,
});

test("a run that is going out is live like one being answered, and neither has ended", () => {
  const runs = Object.fromEntries(
    [row(1, "completed", { ended_at: "2026-11-15 00:01:00" }), row(2, "starting"), row(3, "interrupted", { ended_at: "2026-11-15 00:02:00" })].map((r) => [r.id, runFromRow(r)]),
  );
  expect(runs[2].status).toBe("starting");
  expect(runEnded(runs[2])).toBe(false);
  expect(liveRunId(runs)).toBe(2);
  expect(runningSince(runs)).toBe(runs[2].started_ms);
  expect(liveRunId({ 1: runs[1], 3: runs[3] })).toBe(null);
  expect(liveRunId(undefined)).toBe(null);
});

test("a stop asked of a run shows until the run ends", () => {
  expect(runFromRow(row(4, "running", { stop_requested_at: "2026-11-15 00:00:30" })).stopping).toBe(true);
  expect(runFromRow(row(4, "running")).stopping).toBeUndefined();
  const ended = runFromRow(row(4, "interrupted", { stop_requested_at: "2026-11-15 00:00:30", ended_at: "2026-11-15 00:00:31" }));
  expect(ended.stopping).toBeUndefined();
});

test("a status the timeline cannot draw reads as failed, never as running", () => {
  // `harness_load` lists no waiting or cancelled run; one arriving anyway is not live.
  for (const status of ["queued", "cancelled", "something-new"]) {
    const run = runFromRow(row(5, status));
    expect(run.status).toBe("failed");
    expect(liveRunId({ 5: run })).toBe(null);
  }
});

test("a stop is overdue once Rust says so, or fifteen seconds after it was asked", () => {
  const asked = runFromRow(row(6, "running", { stop_requested_at: "2026-11-15 00:00:30" }));
  const at = asked.stop_requested_ms!;
  expect(stopOverdue(asked, at + STOP_PATIENCE_MS - 1)).toBe(false);
  expect(stopOverdue(asked, at + STOP_PATIENCE_MS)).toBe(true);
  expect(stopOverdue({ ...asked, stop_requested_ms: null, unresponsive: { others: 1 } }, at)).toBe(true);
  expect(stopOverdue(runFromRow(row(6, "running")), at + 60_000)).toBe(false);
  expect(stopOverdue(undefined, at)).toBe(false);
});

const run = (patch: Partial<HarnessRun>): HarnessRun => ({
  id: 5, status: "running", delivery: "unknown", answer_item_id: null, started_ms: 0, ended_ms: null, ...patch,
});

test("stopDueAt: a stop read from a snapshot is due STOP_PATIENCE_MS after it was asked", () => {
  expect(stopDueAt(run({ stopping: true, stop_requested_ms: 1000 }))).toBe(1000 + STOP_PATIENCE_MS);
  // Rust already said so, or there is no stop or no time: nothing to wait for.
  expect(stopDueAt(run({ stopping: true, stop_requested_ms: 1000, unresponsive: { others: 0 } }))).toBeNull();
  expect(stopDueAt(run({}))).toBeNull();
  expect(stopDueAt(run({ stopping: true, stop_requested_ms: null }))).toBeNull();
  expect(stopDueAt(undefined)).toBeNull();
});

test("lastRunStatus: the newest run's status, or null before any", () => {
  expect(lastRunStatus(undefined)).toBeNull();
  expect(lastRunStatus({ 3: run({ id: 3, status: "failed" }), 9: run({ id: 9, status: "interrupted" }) })).toBe("interrupted");
});
