import { describe, expect, test } from "bun:test";

import { dataUrlText, runFileView } from "@/lib/ed/runFiles";

const file = (mime: string, dataUrl: string | null, path: string | null = null) => ({
  name: "x",
  mime,
  dataUrl,
  path,
});

describe("run files", () => {
  test("pictures show inline, text in a dialog, the rest in the file manager", () => {
    expect(runFileView(file("image/png", "data:image/png;base64,AA=="))).toBe("image");
    expect(runFileView(file("text/plain", "data:text/plain;base64,aGk="))).toBe("text");
    expect(runFileView(file("application/json", "data:application/json;base64,e30="))).toBe("text");
    expect(runFileView(file("application/pdf", null, "/tmp/x.pdf"))).toBe("reveal");
  });

  test("a data URL's text is read as UTF-8", () => {
    expect(dataUrlText("data:text/plain;base64,aMOpbGxvCg==")).toBe("héllo\n");
    expect(dataUrlText("data:text/plain,a%20b")).toBe("a b");
    expect(dataUrlText("https://x")).toBeNull();
  });
});
