import { describe, expect, test } from "bun:test";
import { statusOf, usePipelineStore, type PipelineItem } from "@/stores/sync/pipelineStore";

import { actionsOf } from "@/components/sync/pipeline/rowActions";

const row = (patch: Partial<PipelineItem>): PipelineItem => ({
  relativePath: "courses/X/files/deck.pdf",
  subjectId: 1,
  code: "X",
  filename: "deck.pdf",
  download: "done",
  parse: "queued",
  embed: "pending",
  pagesDone: 0,
  totalPages: 0,
  embedPagesDone: 0,
  embedTotalPages: 0,
  paused: false,
  startedAt: 0,
  updatedAt: 0,
  ...patch,
});

const prioritiseOf = (item: PipelineItem, embedStage = true, can = true) =>
  actionsOf(item, statusOf(item, embedStage), embedStage, true, true, can).prioritise;

describe("the prioritise action", () => {
  test("is offered to a parse that is queued", () => {
    expect(prioritiseOf(row({}))).toBe(true);
  });

  test("is offered to an upload still waiting behind another", () => {
    expect(prioritiseOf(row({ parse: "active", parsePhase: "upload_wait" }))).toBe(true);
  });

  test("is offered to an embed that is queued", () => {
    expect(prioritiseOf(row({ parse: "done", embed: "queued" }))).toBe(true);
  });

  test("is not offered to work already running or finished", () => {
    expect(prioritiseOf(row({ parse: "active", parsePhase: "uploading" }))).toBe(false);
    expect(prioritiseOf(row({ parse: "active", parsePhase: "processing" }))).toBe(false);
    expect(prioritiseOf(row({ parse: "done", embed: "active" }))).toBe(false);
    expect(prioritiseOf(row({ parse: "done", embed: "done" }))).toBe(false);
  });

  test("is not offered to a file that is not waiting, or already pushed", () => {
    expect(prioritiseOf(row({ parse: "pending" }))).toBe(false);
    expect(prioritiseOf(row({ parse: "skipped" }))).toBe(false);
    expect(prioritiseOf(row({ parse: "error" }))).toBe(false);
    expect(prioritiseOf(row({ prioritisedAt: 5 }))).toBe(false);
  });

  test("is not offered to an embed when embedding is off, or without a handler", () => {
    expect(prioritiseOf(row({ parse: "done", embed: "queued" }), false)).toBe(false);
    expect(prioritiseOf(row({}), true, false)).toBe(false);
  });
});

describe("a prioritised row", () => {
  test("says so while it waits, and stops once it starts", () => {
    const { touch } = usePipelineStore.getState();
    const path = "courses/X/files/pushed.pdf";
    touch(path, 1, { download: "done", parse: "queued" });
    touch(path, 1, { prioritisedAt: 10 });
    const waiting = usePipelineStore.getState().items[path]!;
    expect(statusOf(waiting, true).label).toBe("Queued to parse — prioritised");

    touch(path, 1, { parse: "active", parsePhase: "uploading" });
    const started = usePipelineStore.getState().items[path]!;
    expect(started.prioritisedAt).toBeUndefined();

    touch(path, 1, { parse: "queued", parsePhase: undefined });
    expect(statusOf(usePipelineStore.getState().items[path]!, true).label).toBe(
      "Queued to parse",
    );
  });

  test("keeps the mark through a queued or upload-waiting event", () => {
    const { touch } = usePipelineStore.getState();
    const path = "courses/X/files/kept.pdf";
    touch(path, 1, { download: "done", parse: "queued", prioritisedAt: 10 });
    touch(path, 1, { parse: "active", parsePhase: "upload_wait" });
    expect(usePipelineStore.getState().items[path]!.prioritisedAt).toBe(10);
  });
});
