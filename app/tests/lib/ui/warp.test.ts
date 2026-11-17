import { afterEach, expect, test } from "bun:test";
import { WARP_NAME, warpLanding, warpTo } from "@/lib/ui/warp";

const globals = globalThis as { document?: unknown; matchMedia?: unknown };
const before = { document: globals.document, matchMedia: globals.matchMedia };
afterEach(() => Object.assign(globals, before));

function source() {
  return { style: { viewTransitionName: "" } } as unknown as HTMLElement;
}

/** A browser's `startViewTransition` whose update callback the test runs. */
function stubTransitions() {
  const calls: { update: () => Promise<void>; finish: () => void }[] = [];
  globals.document = {
    startViewTransition(update: () => Promise<void>) {
      let finish = () => {};
      const finished = new Promise<void>((r) => (finish = r));
      calls.push({ update, finish });
      return { ready: Promise.resolve(), finished, updateCallbackDone: Promise.resolve() };
    },
  };
  globals.matchMedia = () => ({ matches: false });
  return calls;
}

test("without View Transitions the warp is a plain navigation", () => {
  globals.document = {};
  let went = 0;
  const el = source();
  warpTo(el, 1, 7, () => went++);
  expect(went).toBe(1);
  expect(el.style.viewTransitionName).toBe("");
  expect(warpLanding()).toBeNull();
});

test("reduced motion navigates without a transition", () => {
  const calls = stubTransitions();
  globals.matchMedia = () => ({ matches: true });
  let went = 0;
  warpTo(source(), 1, 7, () => went++);
  expect(went).toBe(1);
  expect(calls).toHaveLength(0);
});

test("the name moves from the source to the landing composer, and waits for it", async () => {
  const calls = stubTransitions();
  const el = source();
  let went = 0;
  warpTo(el, 3, 42, () => went++);
  // The old snapshot is of the source.
  expect(el.style.viewTransitionName).toBe(WARP_NAME);
  expect(went).toBe(0);

  let done = false;
  const update = calls[0].update().then(() => (done = true));
  expect(el.style.viewTransitionName).toBe("");
  expect(went).toBe(1);
  const landing = warpLanding();
  expect(landing).toMatchObject({ pane: 3, thread: 42 });

  // The update holds until the thread page has drawn its composer.
  await Promise.resolve();
  expect(done).toBe(false);
  landing?.land();
  await update;
  expect(done).toBe(true);

  calls[0].finish();
  await new Promise((r) => setTimeout(r, 0));
  expect(warpLanding()).toBeNull();
});
