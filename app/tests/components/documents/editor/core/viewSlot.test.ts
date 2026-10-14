import { describe, expect, test } from "bun:test";

import { ViewSlot } from "@/components/documents/editor/core/viewSlot";
import { completion } from "@/components/documents/editor/theme/completion";
import { popoverTooltips } from "@/components/documents/editor/theme/popover-tooltips";

class FakeView {
  destroyed = 0;
  destroy() {
    this.destroyed += 1;
  }
}

describe("ViewSlot", () => {
  test("mounting again destroys the view still held", () => {
    const slot = new ViewSlot<FakeView>();
    const first = slot.mount(() => new FakeView());
    const second = slot.mount(() => new FakeView());
    expect(first.destroyed).toBe(1);
    expect(second.destroyed).toBe(0);
  });

  test("a view is destroyed once however many paths reach it", () => {
    const slot = new ViewSlot<FakeView>();
    const first = slot.mount(() => new FakeView());
    slot.mount(() => new FakeView());
    slot.unmount(first);
    expect(first.destroyed).toBe(1);
  });

  test("unmounting a stale view leaves the newer one held", () => {
    const slot = new ViewSlot<FakeView>();
    const first = slot.mount(() => new FakeView());
    const second = slot.mount(() => new FakeView());
    slot.unmount(first);
    slot.mount(() => new FakeView());
    expect(second.destroyed).toBe(1);
  });

  test("a stale view that fails to destroy does not stop the next mounting", () => {
    const slot = new ViewSlot<FakeView>();
    slot.mount(() => {
      const v = new FakeView();
      v.destroy = () => {
        throw new Error("destroy failed");
      };
      return v;
    });
    const log = console.error;
    console.error = () => {};
    try {
      expect(slot.mount(() => new FakeView()).destroyed).toBe(0);
    } finally {
      console.error = log;
    }
  });
});

describe("tooltip theme", () => {
  test("no rule needs the tooltip to sit inside .cm-editor", () => {
    // Tooltips can be mounted in a container outside the editor.
    const selectors = [...Object.keys(completion), ...Object.keys(popoverTooltips)];
    expect(selectors.filter((s) => /cm-editor[^,]*cm-tooltip/.test(s))).toEqual([]);
    expect(selectors.filter((s) => /cm-math-command/.test(s))).toEqual([]);
  });
});
