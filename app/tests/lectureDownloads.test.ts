import { expect, mock, test } from "bun:test";
import type { EventCallback } from "@tauri-apps/api/event";
import type { DlProgress } from "../src/lib/lectures";

let callback: EventCallback<DlProgress>;
let unlistened = false;
mock.module("@tauri-apps/api/event", () => ({
  listen: async (_name: string, fn: EventCallback<DlProgress>) => {
    callback = fn;
    return () => { unlistened = true; };
  },
}));

const { dlKey, useLectureDownloads, watchLectureDownloads } = await import("../src/stores/lectureDownloadStore");
const progress = (source: 1 | 2, phase: DlProgress["phase"]) => ({
  mediaId: "recording", source, phase, percent: phase === "complete" ? 100 : 10,
}) as DlProgress;
const emit = (payload: DlProgress) => callback({ event: "lecture-download-progress", id: 1, payload });

test("completion cleanup cannot erase a retry or keep running after the watcher is disposed", async () => {
  useLectureDownloads.setState({ progress: {}, active: {} });
  const off = watchLectureDownloads();
  await Promise.resolve();
  try {
    emit(progress(1, "complete"));
    const retry = progress(1, "downloading");
    emit(retry);
    const settled = progress(2, "complete");
    emit(settled);
    off();
    await Promise.resolve();
    expect(unlistened).toBe(true);
    emit(progress(1, "error"));
    expect(useLectureDownloads.getState().progress[dlKey("recording", 1)]).toBe(retry);
    await new Promise((resolve) => setTimeout(resolve, 2100));
    expect(useLectureDownloads.getState().progress[dlKey("recording", 1)]).toBe(retry);
    expect(useLectureDownloads.getState().progress[dlKey("recording", 2)]).toBe(settled);
  } finally {
    off();
    useLectureDownloads.setState({ progress: {}, active: {} });
  }
});
