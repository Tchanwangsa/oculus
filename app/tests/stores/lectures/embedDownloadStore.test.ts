import { afterAll, expect, mock, spyOn, test } from "bun:test";
import * as tauriEvent from "@tauri-apps/api/event";
import * as db from "@/lib/db";

const calls: string[] = [];
let answer: (cmd: string) => Promise<unknown> = async () => null;
mock.module("@tauri-apps/api/core", () => ({
  invoke: (cmd: string) => {
    calls.push(cmd);
    return answer(cmd);
  },
}));
mock.module("@tauri-apps/api/event", () => ({ ...tauriEvent, listen: async () => () => {} }));
const file = { id: 3, relative_path: "courses/X/files/talk.mp4" } as db.DbFile;
const lookup = spyOn(db, "getFileByRelativePath").mockResolvedValue(file);
// The store waits on a window event for the row; tests run without a DOM.
const globals = globalThis as { window?: unknown };
const hadWindow = "window" in globals;
globals.window ??= new EventTarget();
afterAll(() => {
  lookup.mockRestore();
  // Module mocks outlive the file; leave later files an invoke that fails as it does without Tauri.
  answer = async () => { throw new Error("Tauri is not available"); };
  if (!hadWindow) delete globals.window;
});

const { DOWNLOAD_OFF, downloadEmbed, embedInfo, useEmbeds } = await import(
  "@/stores/lectures/embedDownloadStore"
);
type Row = Awaited<ReturnType<typeof embedInfo>>;

const embed = (linkId: string) => ({ subjectId: 1, subjectCode: "X", linkId, src: "https://canvas/retrieve" });
const row = (linkId: string, over: Partial<Row> = {}): Row => ({
  linkId,
  mediaId: "m",
  name: "talk.mp4",
  durationS: 61,
  restrictDownload: false,
  hasCaptions: false,
  posterRel: null,
  fileRel: null,
  extraSources: 0,
  ...over,
});

test("a link is asked once; a rejection is dropped so the next ask tries again", async () => {
  calls.length = 0;
  answer = async () => { throw "Canvas rejected the session"; };
  await expect(embedInfo(embed("a"))).rejects.toBe("Canvas rejected the session");
  expect(useEmbeds.getState().infos.a).toEqual({ status: "error", error: "Canvas rejected the session" });

  answer = async () => row("a");
  const [first, second] = await Promise.all([embedInfo(embed("a")), embedInfo(embed("a"))]);
  expect(first).toBe(second);
  await embedInfo(embed("a"));
  expect(calls).toEqual(["echo360_embed_info", "echo360_embed_info"]);
  expect(useEmbeds.getState().infos.a).toEqual({ status: "ready", row: row("a") });
});

test("a download flips the row to its file, a cancel returns to idle, a refusal marks it", async () => {
  answer = async () => row("b");
  await embedInfo(embed("b"));

  answer = async () => { throw "cancelled"; };
  await downloadEmbed(embed("b"));
  expect(useEmbeds.getState().downloads.b).toBeUndefined();

  answer = async () => { throw "download HTTP 403"; };
  await downloadEmbed(embed("b"));
  expect(useEmbeds.getState().downloads.b).toEqual({ status: "error", error: "download HTTP 403" });

  answer = async () => row("b", { fileRel: file.relative_path });
  await downloadEmbed(embed("b"));
  expect(useEmbeds.getState().downloads.b).toBeUndefined();
  expect(useEmbeds.getState().infos.b).toEqual({ status: "ready", row: row("b", { fileRel: file.relative_path }) });

  answer = async () => row("c");
  await embedInfo(embed("c"));
  answer = async () => { throw DOWNLOAD_OFF; };
  await downloadEmbed(embed("c"));
  expect(useEmbeds.getState().downloads.c).toBeUndefined();
  expect(useEmbeds.getState().infos.c).toEqual({ status: "ready", row: row("c", { restrictDownload: true }) });
});
