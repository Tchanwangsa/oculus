import { describe, expect, test } from "bun:test";
import { folderOf, sizeWords, stateWords } from "@/lib/browser/downloads/describe";
import { download } from "./fixtures";

const upload = {
  source: "a.pdf",
  file: { filename: "a.pdf", relative_path: "courses/X/uploads/a.pdf", file_type: "pdf", size_bytes: 10 },
  error: null,
};

describe("stateWords", () => {
  test("receiving with no choice, and with one", () => {
    expect(stateWords(download(1))).toEqual({ text: "Receiving", place: null, detail: null, tone: "brand" });
    expect(stateWords(download(1, { choice: { kind: "oculus", subject_code: "X_2026" } }))).toEqual({
      text: "Will save to",
      place: { kind: "subject", code: "X_2026" },
      detail: null,
      tone: "brand",
    });
    expect(stateWords(download(1, { choice: { kind: "path", path: "/Users/s/Desktop/a.pdf" } })).place).toEqual({
      kind: "folder",
      name: "Desktop",
    });
  });

  test("ready waits for a choice, or says why a delivery did not land", () => {
    expect(stateWords(download(1, { state: "ready" })).text).toBe("Waiting for a choice");
    expect(stateWords(download(1, { state: "ready", error: "a file is already there" }))).toEqual({
      text: "Not saved",
      place: null,
      detail: "a file is already there",
      tone: "error",
    });
  });

  test("delivering names where it is going", () => {
    const d = download(1, { state: "delivering", choice: { kind: "oculus", subject_code: "X_2026" } });
    expect(stateWords(d)).toMatchObject({ text: "Saving to", place: { kind: "subject", code: "X_2026" } });
  });

  test("saved names the subject or the folder; a failed conversion is a detail, not a failure", () => {
    const inOculus = download(1, {
      state: "saved",
      saved_to: { kind: "oculus", subject_code: "X_2026", outcome: upload },
    });
    expect(stateWords(inOculus)).toEqual({
      text: "Saved to",
      place: { kind: "subject", code: "X_2026" },
      detail: null,
      tone: "success",
    });
    const unconverted = download(1, {
      state: "saved",
      saved_to: { kind: "oculus", subject_code: "X_2026", outcome: { ...upload, error: "added, but not converted" } },
    });
    expect(stateWords(unconverted)).toMatchObject({ tone: "success", detail: "added, but not converted" });
    const onDisk = download(1, { state: "saved", saved_to: { kind: "path", path: "C:\\Users\\s\\Downloads\\a.pdf" } });
    expect(stateWords(onDisk).place).toEqual({ kind: "folder", name: "Downloads" });
  });

  test("discarded and failed", () => {
    expect(stateWords(download(1, { state: "discarded" }))).toEqual({ text: "Discarded", place: null, detail: null, tone: "muted" });
    expect(stateWords(download(1, { state: "failed", error: "the download did not finish" }))).toEqual({
      text: "Failed",
      place: null,
      detail: "the download did not finish",
      tone: "error",
    });
  });
});

describe("sizeWords", () => {
  test("bytes so far while receiving, the final size after; never a total", () => {
    expect(sizeWords(download(1, { received: 4.2 * 1024 * 1024 }))).toBe("4.2 MB received");
    expect(sizeWords(download(1, { state: "ready", received: 2048 }))).toBe("2.0 KB");
  });
});

test("folderOf takes the parent of either separator", () => {
  expect(folderOf("/Users/s/Downloads/a.pdf")).toBe("Downloads");
  expect(folderOf("D:\\Work\\a.pdf")).toBe("Work");
  expect(folderOf("a.pdf")).toBe("a.pdf");
});
