import { describe, expect, test } from "bun:test";
import { changeRequestText } from "@/lib/studio/changeRequest";

describe("changeRequestText", () => {
  test("a component goes through a scratch video and components promote", () => {
    const text = changeRequestText({ kind: "component", name: "Bars", level: "global", subject: null, request: " slower bars " });
    const [first, blank, how] = text.split("\n");
    expect(first).toBe("Change the Bars component (global level): slower bars");
    expect(blank).toBe("");
    expect(how).toContain("oculus studio components promote <DIR> Bars --to global");
  });

  test("a subject theme names its file and promotes back to the subject", () => {
    const text = changeRequestText({ kind: "theme", name: "chalk", level: "subject", subject: "MAST20004", request: "warmer accent" });
    expect(text.split("\n")[0]).toBe("Change the chalk theme (subject level, MAST20004): warmer accent");
    expect(text).toContain("`../studio/subjects/MAST20004/themes/chalk.json`");
    expect(text).toContain("(subject MAST20004)");
    expect(text).toContain("oculus studio gallery <DIR> --theme themes/chalk.json");
    expect(text).toContain("oculus studio themes promote <DIR> themes/chalk.json --to subject");
  });

  test("a global theme ignores the browsed subject", () => {
    const text = changeRequestText({ kind: "theme", name: "warm", level: "global", subject: null, request: "x" });
    expect(text).toContain("`../studio/library/themes/warm.json` in the global library");
    expect(text).toContain("--to global");
    expect(text).not.toContain("subject");
  });
});
