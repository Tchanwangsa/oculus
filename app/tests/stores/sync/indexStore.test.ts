import { afterAll, expect, mock, spyOn, test } from "bun:test";
import * as tauriCore from "@tauri-apps/api/core";
import * as db from "@/lib/db";

const embedded: string[] = [];
const release: Array<() => void> = [];
let live = true;
mock.module("@tauri-apps/api/core", () => ({
  ...tauriCore,
  invoke: (cmd: string, args: { relativePath?: string }) => {
    // Module mocks outlive the file: later files see the failure they would without Tauri.
    if (!live) return Promise.reject(new Error("Tauri is not available"));
    if (cmd !== "embed_file") return Promise.resolve(null);
    embedded.push(args.relativePath ?? "");
    return new Promise((resolve) =>
      release.push(() => resolve({ pages_embedded: 1, pages_total: 1 })),
    );
  },
}));
const marks = [
  spyOn(db, "markEmbedQueued").mockResolvedValue(undefined),
  spyOn(db, "clearEmbedQueued").mockResolvedValue(undefined),
];
afterAll(() => {
  live = false;
  for (const spy of marks) spy.mockRestore();
});

const { useIndexStore } = await import("@/stores/sync/indexStore");
const { usePipelineStore } = await import("@/stores/sync/pipelineStore");

const job = (name: string) => ({
  fileId: name.charCodeAt(0),
  subjectId: 1,
  relativePath: `courses/X/files/${name}.pdf`,
  filename: `${name}.pdf`,
});
const path = (name: string) => job(name).relativePath;
const settle = () => new Promise((resolve) => setTimeout(resolve, 20));

test("the worker embeds a prioritised file next, in the order files were prioritised", async () => {
  useIndexStore.getState().enqueue(["head", "a", "b", "c", "d"].map(job));
  await settle();
  // `head` is running now; the rest wait.
  expect(embedded).toEqual([path("head")]);

  const { prioritise } = useIndexStore.getState();
  expect(prioritise(path("head"))).toBe(false); // running: not interrupted
  expect(prioritise(path("nowhere"))).toBe(false);
  expect(prioritise(path("d"))).toBe(true);
  expect(prioritise(path("b"))).toBe(true);

  for (let i = 0; i < 5; i++) {
    release[i]?.();
    await settle();
  }
  expect(embedded).toEqual(["head", "d", "b", "a", "c"].map(path));
  expect(useIndexStore.getState().running).toBe(false);
});

test("a stop hands every waiting file back as pending", async () => {
  embedded.length = 0;
  release.length = 0;
  useIndexStore.getState().enqueue(["head", "a"].map(job));
  await settle();
  expect(usePipelineStore.getState().items[path("a")]?.embed).toBe("queued");
  useIndexStore.getState().stop();
  expect(useIndexStore.getState().prioritise(path("a"))).toBe(false);
  expect(usePipelineStore.getState().items[path("a")]?.embed).toBe("pending");
  release[0]?.();
  await settle();
});
