import { describe, expect, test } from "bun:test";

import { chunksText, dropAttachment, insertAt, normalize, type Chunk } from "@/components/harness/mentions/chunks";
import { splitLibraryPaths } from "@/lib/files/libraryPaths";
import {
  appendAttachmentRefs,
  attachmentPath,
  attachmentRefs,
  distinctPaths,
  refersTo,
  withShelf,
} from "@/lib/harness/attachments";
import { addName, attachmentKindLabel } from "@/stores/chat/attachmentNames";
import { useDraftStore } from "@/stores/chat/draftStore";

// `dom.ts` reaches modules that may listen on `window` and `document` at
// load; they get inert stand-ins for the import, then the globals go back.
const globals = globalThis as { window?: unknown; document?: unknown };
const before = { window: globals.window, document: globals.document };
const inert = { addEventListener: () => {}, removeEventListener: () => {} };
globals.window ??= inert;
globals.document ??= { ...inert, documentElement: { style: {} } };
const { toChunks } = await import("@/components/harness/mentions/dom");
Object.assign(globals, before);

const A = "./attachments/20261115-101500-0000000a.png";
const B = "./attachments/20261115-101500-0000000b.png";
const PDF = "./attachments/20261115-101501-0000000c.pdf";
const text = (t: string): Chunk => ({ kind: "text", text: t });
const att = (path: string): Chunk => ({ kind: "attachment", path });

describe("attachment chips in the box", () => {
  test("a draft's attachment paths load as chips and read back as the same text", () => {
    const draft = `compare \`${A}\` with \`${B}\` and \`courses/MAST10006/Course/Files/notes.pdf\`, see \`${PDF}\``;
    const chunks = toChunks(draft);
    expect(chunks.filter((c) => c.kind === "attachment")).toEqual([att(A), att(B), att(PDF)]);
    expect(chunks.some((c) => c.kind === "chip")).toBe(true);
    expect(chunksText(chunks)).toBe(draft);
  });

  test("a chip in either spelling keeps the message's own", () => {
    const draft = "`agents/attachments/x.png` and `../agents/attachments/y.pdf`";
    expect(chunksText(toChunks(draft))).toBe(draft);
    expect(toChunks(draft).filter((c) => c.kind === "attachment")).toEqual([
      att("agents/attachments/x.png"),
      att("../agents/attachments/y.pdf"),
    ]);
  });

  test("a chip that starts the box gets the caret's landing strip, which reads as nothing", () => {
    const chunks = normalize([att(A), text(" hi")]);
    expect(chunks[0]).toEqual(text("​"));
    expect(chunksText(chunks)).toBe(`\`${A}\` hi`);
  });

  test("inserting at the caret puts a space on each side and the caret after", () => {
    const { chunks, caret } = insertAt([text("compare with")], 7, [att(A)]);
    expect(chunksText(chunks)).toBe(`compare \`${A}\` with`);
    expect(caret).toBe(`compare \`${A}\``.length);
    const end = insertAt([text("look:")], 5, [att(A), att(B)]);
    expect(chunksText(end.chunks)).toBe(`look: \`${A}\` \`${B}\` `);
    expect(end.caret).toBe(chunksText(end.chunks).length);
    const empty = insertAt([], 0, [att(A)]);
    expect(chunksText(empty.chunks)).toBe(`\`${A}\` `);
  });

  test("an insert next to another chip lands after it", () => {
    const { chunks } = insertAt([text("a "), att(A)], 3, [att(B)]);
    expect(chunksText(chunks)).toBe(`a \`${A}\` \`${B}\` `);
  });

  test("two same-named pictures are two chips with two paths", () => {
    let names: Record<string, string> = {};
    names = addName(names, attachmentPath(A)!, "Screenshot.png");
    names = addName(names, attachmentPath(B)!, "Screenshot.png");
    const { chunks } = insertAt([], 0, [att(A), att(B)]);
    const paths = chunks.flatMap((c) => (c.kind === "attachment" ? [c.path] : []));
    expect(paths).toEqual([A, B]);
    expect(distinctPaths([A, B])).toEqual([A, B]);
    expect(names[attachmentPath(A)!]).toBe("Screenshot.png");
    expect(names[attachmentPath(B)!]).toBe("Screenshot.png");
  });

  test("removing a picture takes every chip of it, closing the gaps", () => {
    const chunks = toChunks(`see \`${A}\` and \`${B}\` then \`${A}\``);
    const { chunks: next, caret } = dropAttachment(chunks, A, chunksText(chunks).length);
    expect(chunksText(next)).toBe(`see and \`${B}\` then`);
    expect(caret).toBe(chunksText(next).length);
    const lead = dropAttachment(normalize([att(A), text(" hi")]), A, 0);
    expect(chunksText(lead.chunks)).toBe("hi");
    expect(lead.caret).toBe(0);
  });

  test("an unknown path removes nothing", () => {
    const chunks = toChunks(`see \`${A}\``);
    expect(dropAttachment(chunks, B, 3).chunks).toBe(chunks);
  });
});

describe("the shelf and the sentence", () => {
  test("deleting a picture's chip keeps it on the shelf, and it is still sent", () => {
    const key = "test:shelf";
    const store = useDraftStore.getState();
    store.setShelf(key, [A]);
    store.setDraft(key, `look \`${A}\``);
    const edited = chunksText(dropAttachment(toChunks(`look \`${A}\``), A, 0).chunks);
    store.setDraft(key, edited);
    expect(useDraftStore.getState().shelves[key]).toEqual([A]);
    expect(withShelf(edited, useDraftStore.getState().shelves[key])).toBe(`look\n\n\`${A}\``);
    store.setShelf(key, []);
    store.setDraft(key, "");
  });

  test("a PDF is only its chip: deleting the last one leaves nothing to send", () => {
    const draft = `read \`${PDF}\` please`;
    expect(attachmentRefs(draft).map((r) => [r.raw, r.picture])).toEqual([[PDF, false]]);
    const edited = chunksText(dropAttachment(toChunks(draft), PDF, 0).chunks);
    expect(edited).toBe("read please");
    expect(attachmentRefs(edited)).toEqual([]);
    expect(withShelf(edited, [])).toBe("read please");
  });

  test("sending leaves every chip's path in place and adds only an unreferenced shelf picture", () => {
    const words = `compare \`${A}\` with this`;
    expect(withShelf(words, [A, B])).toBe(`compare \`${A}\` with this\n\n\`${B}\``);
    expect(withShelf(words, [A])).toBe(words);
    expect(withShelf("", [A, A])).toBe(`\`${A}\``);
  });

  test("an attachment in any spelling counts as named", () => {
    expect(refersTo(`x \`agents/attachments/${A.slice(14)}\``, A)).toBe(true);
    expect(refersTo(`x \`${B}\``, A)).toBe(false);
  });

  test("a write that lands after the box moved on is appended to its draft", () => {
    expect(appendAttachmentRefs("compare ", [A])).toBe(`compare \`${A}\``);
    expect(appendAttachmentRefs("", [A, PDF])).toBe(`\`${A}\` \`${PDF}\``);
    expect(appendAttachmentRefs("line\n", [A])).toBe(`line\n\`${A}\``);
  });
});

describe("attachment paths", () => {
  test("a picture and a file are told apart, in place in the prose", () => {
    const parts = splitLibraryPaths(`a \`${A}\` b \`${PDF}\``);
    expect(parts.map((p) => p.kind)).toEqual(["text", "image", "text", "attachment"]);
  });

  test("no dot-name is an attachment", () => {
    expect(attachmentPath("./attachments/..")).toBeNull();
    expect(attachmentPath("./attachments/.env")).toBeNull();
    expect(attachmentPath("attachments/../x.png")).toBeNull();
    expect(attachmentPath("./attachments/a.b.txt")).toBe("agents/attachments/a.b.txt");
  });

  test("names are kept newest last, the oldest dropped past the cap", () => {
    let names: Record<string, string> = {};
    for (let i = 0; i < 4; i++) names = addName(names, `agents/attachments/${i}.png`, `n${i}`, 3);
    expect(Object.keys(names)).toEqual(["agents/attachments/1.png", "agents/attachments/2.png", "agents/attachments/3.png"]);
    expect(attachmentKindLabel("agents/attachments/x.pdf")).toBe("PDF");
    expect(attachmentKindLabel("agents/attachments/x.png")).toBe("Picture");
    expect(attachmentKindLabel("agents/attachments/x.md")).toBe("Text file");
  });
});

describe("handed-back words", () => {
  test("keep their chips in place and give every picture back to the shelf", async () => {
    const { splitHandedBack, withPastedText } = await import("@/lib/harness/attachments");
    const sent = withShelf(withPastedText(`compare \`${A}\` and \`${PDF}\``, ["long paste"]), [A, B]);
    expect(splitHandedBack(sent)).toEqual({
      text: `compare \`${A}\` and \`${PDF}\``,
      pasted: ["long paste"],
      pictures: [B, A],
    });
  });

  test("an older message's trailing picture line comes back as shelf pictures only", async () => {
    const { splitHandedBack, withAttachments } = await import("@/lib/harness/attachments");
    expect(splitHandedBack(withAttachments("look", [A]))).toEqual({ text: "look", pasted: [], pictures: [A] });
  });
});

test("a long attachment name is cut in the middle, keeping its extension", async () => {
  const { middleTruncate } = await import("@/components/markdown/AttachmentChip");
  expect(middleTruncate("short.png")).toBe("short.png");
  const cut = middleTruncate("Screenshot 2026-11-07 at 16.26.37 in the afternoon.png");
  expect(cut.length).toBe(40);
  expect(cut.startsWith("Screenshot")).toBe(true);
  expect(cut.endsWith("afternoon.png")).toBe(true);
  expect(cut).toContain("…");
});

test("a stash entry's words come back with their attachment chips", () => {
  const stashed = `compare \`${A}\` with \`${PDF}\``;
  const chunks = toChunks(stashed);
  expect(chunks.filter((c) => c.kind === "attachment")).toEqual([att(A), att(PDF)]);
  expect(chunksText(chunks)).toBe(stashed);
});
