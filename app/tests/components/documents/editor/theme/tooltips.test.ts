import { describe, expect, test } from "bun:test";

import { completion } from "@/components/documents/editor/theme/completion";
import { popoverTooltips } from "@/components/documents/editor/theme/popover-tooltips";

describe("tooltip theme", () => {
  test("no rule needs the tooltip to sit inside .cm-editor", () => {
    // Tooltips can be mounted in a container outside the editor.
    const selectors = [...Object.keys(completion), ...Object.keys(popoverTooltips)];
    expect(selectors.filter((s) => /cm-editor[^,]*cm-tooltip/.test(s))).toEqual([]);
    expect(selectors.filter((s) => /cm-math-command/.test(s))).toEqual([]);
  });
});
