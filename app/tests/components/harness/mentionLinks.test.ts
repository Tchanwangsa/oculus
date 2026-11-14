import { describe, expect, test } from "bun:test";

import { caretChunk, linkChunks, sameChunks, type Chunk } from "@/components/harness/mentions/chunks";

const text = (t: string): Chunk => ({ kind: "text", text: t });
const link = (raw: string): Chunk => ({ kind: "link", raw });

describe("linkChunks", () => {
  test("a keystroke mid-word draws no new link", () => {
    const chunks = [text("see https://example.com")];
    expect(linkChunks(chunks, "typed")).toEqual(chunks);
    expect(linkChunks([text("see https://example.com ")], "settled")).toEqual([text("see "), link("https://example.com"), text(" ")]);
  });

  test("typing into a link splits off what is not part of it", () => {
    expect(linkChunks([link("https://example.com,")], "typed")).toEqual([link("https://example.com"), text(",")]);
    expect(linkChunks([link("https://example.com eee")], "typed")).toEqual([link("https://example.com"), text(" eee")]);
  });

  test("a link edited until it is no longer one is prose", () => {
    expect(linkChunks([text("a "), link("https:/")], "typed")).toEqual([text("a "), text("https:/")]);
  });

  test("an unchanged link draws the same nodes", () => {
    const chunks = [text("a "), link("https://example.com/x"), text(" b")];
    expect(sameChunks(linkChunks(chunks, "typed"), chunks)).toBe(true);
    expect(sameChunks(chunks, [text("a "), link("https://example.com/y"), text(" b")])).toBe(false);
  });
});

describe("caretChunk", () => {
  const chunks = [text("go "), link("https://x.io"), text(" now")];

  test("an offset inside a bare link lands in its text", () => {
    expect(caretChunk(chunks, 3)).toEqual({ chunk: 0, offset: 3 });
    expect(caretChunk(chunks, 8)).toEqual({ chunk: 1, offset: 5 });
    expect(caretChunk(chunks, 15)).toEqual({ chunk: 2, offset: 0 });
  });

  test("an offset inside a markdown link lands in its words", () => {
    const md = [text("go "), link("[docs](https://x.io)"), text(" now")];
    expect(caretChunk(md, 4)).toEqual({ chunk: 1, offset: 0 });
    expect(caretChunk(md, 6)).toEqual({ chunk: 1, offset: 2 });
    expect(caretChunk(md, 12)).toEqual({ chunk: 1, offset: 4 });
  });

  test("a leading link takes the caret at its start", () => {
    expect(caretChunk([link("https://x.io")], 0)).toEqual({ chunk: 0, offset: 0 });
  });
});
