import { expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { canSend, UNSCOPED_PLACEHOLDER } from "@/components/harness/composer/sendGate";
import { enterKey } from "@/lib/harness/sendMode";

// The composer's import graph reaches modules that listen on `window` and
// `document` at load; they get inert stand-ins for the import, then the
// globals go back so no other test sees a half DOM.
const globals = globalThis as { window?: unknown; document?: unknown };
const before = { window: globals.window, document: globals.document };
const inert = { addEventListener: () => {}, removeEventListener: () => {} };
globals.window ??= inert;
globals.document ??= { ...inert, documentElement: { style: {} } };
const { Composer } = await import("@/components/harness/composer/Composer");
Object.assign(globals, before);

const noop = () => {};
// Server rendering reads the stores' initial state, so the box renders empty
// here: these check what the box says, and `canSend`/`enterAction` below are
// the gate it applies to typed words (`enterKey` is tested with the send
// modes in tests/lib/harness/sendMode.test.ts).
function render(unscoped: boolean, placeholder?: string) {
  return renderToStaticMarkup(
    <Composer
      draftKey="new:chat"
      provider="claude"
      model="opus"
      reasoning="high"
      subjects={[]}
      subjectId={null}
      subjectLocked={false}
      unscoped={unscoped}
      running={false}
      usage={null}
      rateLimits={[]}
      onProvider={noop}
      onModel={noop}
      onReasoning={noop}
      onSend={noop}
      onStop={noop}
      placeholder={placeholder}
    />,
  );
}

test("an unscoped box says why it will not send, over any caller's hint", () => {
  expect(render(true)).toContain(`aria-placeholder="${UNSCOPED_PLACEHOLDER}"`);
  expect(render(true, "Ask for a change")).toContain(`aria-placeholder="${UNSCOPED_PLACEHOLDER}"`);
  expect(render(false)).not.toContain(UNSCOPED_PLACEHOLDER);
  expect(render(false, "Ask for a change")).toContain('aria-placeholder="Ask for a change"');
});

test("an empty box names its two menus unless the caller gives its own hint", () => {
  expect(render(false)).toContain('aria-placeholder="Ask anything. @ files, / skills"');
  expect(render(false, "Ask for a change")).not.toContain("Ask anything.");
});

test("a box with no onSubject shows no subject chip: Chat's headline picks it", () => {
  expect(render(true)).not.toContain('aria-label="Subject:');
});

test("Enter is swallowed while unscoped; Shift+Enter still breaks the line", () => {
  const key = (k: string, shiftKey: boolean) => ({ key: k, shiftKey, metaKey: false, ctrlKey: false });
  expect(enterKey(key("Enter", false), true, true)).toBe("hold");
  expect(enterKey(key("Enter", false), false, true)).toBe("send");
  expect(enterKey(key("Enter", true), true, true)).toBeNull();
  expect(enterKey(key("a", false), true, true)).toBeNull();
});

test("the button needs something to send and a chosen scope", () => {
  expect(canSend({ ready: true, unscoped: true })).toBe(false);
  expect(canSend({ ready: false, unscoped: false })).toBe(false);
  expect(canSend({ ready: true, unscoped: false })).toBe(true);
});
