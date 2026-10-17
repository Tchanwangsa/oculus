import { describe, expect, test } from "bun:test";

import { parseLink, splitLinks } from "@/lib/harness/links";

const kinds = (text: string, settled = false) =>
  splitLinks(text, settled).map((p) => (p.kind === "link" ? `<${p.raw}>` : p.text));

describe("splitLinks", () => {
  test("takes a bare URL without its sentence punctuation", () => {
    expect(kinds("see https://example.com/a?b=1, then")).toEqual(["see ", "<https://example.com/a?b=1>", ", then"]);
    expect(kinds("(https://example.com)")).toEqual(["(", "<https://example.com>", ")"]);
  });

  test("takes a markdown link whole", () => {
    expect(kinds("read [the docs](https://example.com/x) now")).toEqual([
      "read ",
      "<[the docs](https://example.com/x)>",
      " now",
    ]);
  });

  test("leaves code spans and non-web links alone", () => {
    expect(kinds("`https://example.com` and [a](/b) and ftp://x.y")).toEqual([
      "`https://example.com` and [a](/b) and ftp://x.y",
    ]);
  });

  test("settled waits for the space after the link", () => {
    expect(kinds("go https://example.com", true)).toEqual(["go https://example.com"]);
    expect(kinds("go https://example.com ", true)).toEqual(["go ", "<https://example.com>", " "]);
  });

  test("text is unchanged by splitting", () => {
    const text = "a https://x.io/p.q, [b](https://y.io) `c` https://z.io\nend";
    expect(splitLinks(text).map((p) => (p.kind === "link" ? p.raw : p.text)).join("")).toBe(text);
  });
});

describe("parseLink", () => {
  test("reads both spellings", () => {
    expect(parseLink("https://example.com/a")).toEqual({ href: "https://example.com/a", label: null });
    expect(parseLink("[docs](https://example.com)")).toEqual({ href: "https://example.com", label: "docs" });
    expect(parseLink("example.com")).toBeNull();
  });
});
