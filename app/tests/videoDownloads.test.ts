import { afterAll, expect, mock, spyOn, test } from "bun:test";
import * as db from "../src/lib/db";

let answer: () => Promise<unknown> = async () => "";
mock.module("@tauri-apps/api/core", () => ({ invoke: () => answer() }));
mock.module("@tauri-apps/api/event", () => ({ listen: async () => () => {} }));
const row = { id: 7, relative_path: "courses/X/files/a.mp4" } as db.DbFile;
const lookup = spyOn(db, "getFileByRelativePath").mockResolvedValue(row);
// The store waits on a window event for the row; tests run without a DOM.
const globals = globalThis as { window?: unknown };
const hadWindow = "window" in globals;
globals.window ??= new EventTarget();
afterAll(() => {
  lookup.mockRestore();
  if (!hadWindow) delete globals.window;
});

const { downloadVideo, useVideoDownloads } = await import("../src/stores/videoDownloadStore");
const subject = { id: 1, code: "X" };

test("a failure stays on the row, a cancel clears it, a success keeps the file", async () => {
  answer = async () => { throw "download HTTP 403"; };
  expect(await downloadVideo(subject, 5)).toBeNull();
  expect(useVideoDownloads.getState().downloads[5]).toEqual({ status: "error", error: "download HTTP 403" });

  answer = async () => { throw "cancelled"; };
  expect(await downloadVideo(subject, 5)).toBeNull();
  expect(useVideoDownloads.getState().downloads[5]).toBeUndefined();

  answer = async () => row.relative_path;
  expect(await downloadVideo(subject, 5)).toBe(row);
  expect(useVideoDownloads.getState().downloads[5]).toEqual({ status: "done", file: row });
});

test("a second click on a running download starts nothing", async () => {
  let release!: (v: string) => void;
  let calls = 0;
  answer = () => { calls++; return new Promise((r) => { release = r; }); };
  const first = downloadVideo(subject, 9);
  expect(await downloadVideo(subject, 9)).toBeNull();
  expect(calls).toBe(1);
  release(row.relative_path);
  expect(await first).toBe(row);
});
