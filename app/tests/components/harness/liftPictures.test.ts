import { describe, expect, test } from "bun:test";

import { liftPictures } from "@/components/harness/timeline/liftPictures";
import { withAttachments } from "@/lib/harness/attachments";

const A = "./attachments/20261115-a.png";
const B = "./attachments/20261115-b.png";
const PDF = "./attachments/20261115-c.pdf";

describe("liftPictures", () => {
  test("an older message's trailing picture line is lifted as before, its prose unchanged", () => {
    const { pictures, prose } = liftPictures(withAttachments("test test\n", [A, B]));
    expect(pictures).toEqual(["agents/attachments/20261115-a.png", "agents/attachments/20261115-b.png"]);
    expect(prose).toEqual([{ kind: "text", text: "test test" }]);
    expect(liftPictures(withAttachments("", [A])).prose).toEqual([]);
  });

  test("chips named in the sentence stay in place, and their pictures join the row", () => {
    const { pictures, prose } = liftPictures(`compare \`${A}\` with \`${B}\` and \`${PDF}\``);
    expect(pictures).toEqual(["agents/attachments/20261115-a.png", "agents/attachments/20261115-b.png"]);
    expect(prose.map((p) => p.kind)).toEqual(["text", "image", "text", "image", "text", "attachment"]);
    expect(prose[5]).toEqual({ kind: "attachment", path: "agents/attachments/20261115-c.pdf", raw: PDF });
  });

  test("a chipped picture and an unreferenced shelf picture: one row, each once", () => {
    const { pictures, prose } = liftPictures(withAttachments(`see \`${A}\``, [B]));
    expect(pictures).toEqual(["agents/attachments/20261115-a.png", "agents/attachments/20261115-b.png"]);
    expect(prose.map((p) => p.kind)).toEqual(["text", "image"]);
  });

  test("a message with no attachments is its parts", () => {
    expect(liftPictures("  just words  ")).toEqual({ pictures: [], prose: [{ kind: "text", text: "  just words  " }] });
  });
});
