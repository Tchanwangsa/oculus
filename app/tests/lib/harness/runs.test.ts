import { expect, test } from "bun:test";
import { liveRunId, runEnded, runFromRow, runningSince, STOP_PATIENCE_MS, stopOverdue, type RunRow } from "@/lib/harness/runs";

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
