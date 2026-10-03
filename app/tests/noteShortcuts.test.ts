import { afterEach, expect, test } from "bun:test";
import { linkInFocusedNote, registerNoteLinkCommand } from "../src/lib/noteShortcuts";

const originalDocument = Object.getOwnPropertyDescriptor(globalThis, "document");
function focused(root: Element | null) {
  Object.defineProperty(globalThis, "document", { configurable: true, value: {
    activeElement: root ? { closest: () => root } : null,
  } });
}
afterEach(() => {
  if (originalDocument) Object.defineProperty(globalThis, "document", originalDocument);
  else Reflect.deleteProperty(globalThis, "document");
});

test("menu search delegates only to the focused mounted note", () => {
  const a = {} as Element;
  const b = {} as Element;
  let calls = 0;
  const remove = registerNoteLinkCommand(a, () => { calls++; return true; });
  focused(null);
  expect(linkInFocusedNote()).toBe(false);
  focused(b);
  expect(linkInFocusedNote()).toBe(false);
  focused(a);
  expect(linkInFocusedNote()).toBe(true);
  expect(calls).toBe(1);
  remove();
  expect(linkInFocusedNote()).toBe(false);
  expect(calls).toBe(1);
});

test("cleanup of an older view cannot unregister its replacement", () => {
  const root = {} as Element;
  const old = registerNoteLinkCommand(root, () => false);
  const current = registerNoteLinkCommand(root, () => true);
  old();
  focused(root);
  expect(linkInFocusedNote()).toBe(true);
  current();
  expect(linkInFocusedNote()).toBe(false);
});
