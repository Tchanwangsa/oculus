import { expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { compactOpen } from "@/components/harness/composer/compactOpen";

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
const HINT = "Ask about COMP30022. @ files, / skills";

// Server rendering reads the stores' initial state and has no focus, so a
// compact box renders at rest; `compactOpen` below is what opens it.
function render(compact: boolean) {
  return renderToStaticMarkup(
    <Composer
      compact={compact}
      draftKey="new:overview:1"
      provider="claude"
      model="opus"
      reasoning="high"
      subjects={[]}
      subjectId={1}
      subjectLocked
      running={false}
      usage={null}
      rateLimits={[{ label: "5h", used_percent: 40, resets_at: null }]}
      onProvider={noop}
      onModel={noop}
      onReasoning={noop}
      onSend={noop}
      onStop={noop}
      placeholder={HINT}
    />,
  );
}

const usageRing = 'viewBox="0 0 18 18"';

test("a compact box at rest is one pill: hint, the agent's mark, mic and send", () => {
  const html = render(true);
  expect(html).toContain('data-compact="rest"');
  expect(html).toContain("rounded-full");
  expect(html).toContain(`aria-placeholder="${HINT}"`);
  // The hint is cut to one line rather than wrapping the bar.
  expect(html).toMatch(/class="[^"]*truncate[^"]*">Ask about COMP30022/);
  // The picker is its mark alone: model and level only in its title.
  expect(html).toContain('aria-label="Model and reasoning"');
  expect(html).not.toContain(">opus<");
  expect(html).not.toContain(usageRing);
  expect(html).toContain('aria-label="Dictate message"');
});

test("focus, words, a menu, dictation, a question or sign-in open a compact box", () => {
  const rest = { focused: false, ready: false, picking: false, dictating: false, answering: false, signingIn: false };
  expect(compactOpen(rest)).toBe(false);
  for (const key of Object.keys(rest) as (keyof typeof rest)[]) {
    expect(compactOpen({ ...rest, [key]: true })).toBe(true);
  }
});

test("a box without compact is unchanged: always the full composer", () => {
  const html = render(false);
  expect(html).not.toContain("data-compact");
  expect(html).not.toContain("rounded-full py-1");
  expect(html).toContain(">opus<");
  expect(html).toContain(usageRing);
});
