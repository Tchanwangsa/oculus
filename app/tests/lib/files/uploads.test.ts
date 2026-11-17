import { afterAll, beforeEach, expect, mock, spyOn, test } from "bun:test";
import * as db from "@/lib/db";

/** Every invoke and row write, in order. */
let log: string[] = [];
let imported: unknown = [];
let done = false;
mock.module("@tauri-apps/api/core", () => ({
  invoke: async (cmd: string, args: Record<string, unknown>) => {
    // Module mocks outlive the file; later files get the failure Tauri's absence gives.
    if (done) throw new Error("Tauri is not available");
    log.push(`${cmd} ${String(args.relativePath ?? args.subjectCode)}`);
    return cmd === "import_uploads" ? imported : undefined;
  },
}));
const upsert = spyOn(db, "upsertFile").mockImplementation(async (_subject, filename, _path, _type, _size, category) => {
  log.push(`row ${filename} ${category}`);
});
// `announce` dispatches on window; tests run without a DOM.
const globals = globalThis as { window?: unknown };
const hadWindow = "window" in globals;
globals.window ??= new EventTarget();
afterAll(() => {
  done = true;
  upsert.mockRestore();
  if (!hadWindow) delete globals.window;
});

const { addUploads, recordUploads, UPLOADS_CHANGED_EVENT } = await import("@/lib/files/uploads");
const subject = { id: 7, code: "COMP10001_2026_SM2" };

function outcome(filename: string, error: string | null = null) {
  const category = filename.endsWith(".md") ? "document" : "upload";
  return {
    source: filename,
    file: { filename, relative_path: `courses/${subject.code}/${filename}`, file_type: "x", size_bytes: 1, category },
    error,
  };
}

let announced = 0;
(globals.window as EventTarget).addEventListener(UPLOADS_CHANGED_EVENT, () => announced++);
beforeEach(() => {
  log = [];
  announced = 0;
});

test("addUploads copies, writes each row before its parse kick, and announces", async () => {
  const outcomes = [
    outcome("week1.pdf"),
    outcome("data.zip"),
    outcome("slides.pptx", "added, but not converted for search"),
    { source: "broken.pdf", file: null, error: "could not read it" },
  ];
  imported = outcomes;
  expect(await addUploads(subject, ["/a/week1.pdf"])).toBe(outcomes);
  expect(log).toEqual([
    `import_uploads ${subject.code}`,
    "row week1.pdf upload",
    `parse_file courses/${subject.code}/week1.pdf`,
    "row data.zip upload",
    "row slides.pptx upload",
  ]);
  expect(announced).toBe(1);
});

test("recordUploads does the same for outcomes Rust already landed", async () => {
  await recordUploads(subject, [outcome("sheet.xlsx"), outcome("photo.png")]);
  expect(log).toEqual([
    "row sheet.xlsx upload",
    `parse_file courses/${subject.code}/sheet.xlsx`,
    "row photo.png upload",
  ]);
  expect(announced).toBe(1);
});

test("each row takes the category Rust read off its path", async () => {
  await recordUploads(subject, [outcome("plan.md")]);
  expect(log).toEqual(["row plan.md document"]);
});
