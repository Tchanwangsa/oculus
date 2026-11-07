import { afterEach, describe, expect, test } from "bun:test";
import { followTimeline, noteScrollIntent } from "@/lib/ui/followTimeline";
import { HELD_ANCHOR } from "@/lib/ui/scrollAnchor";

const originalObserver = globalThis.ResizeObserver;
afterEach(() => { globalThis.ResizeObserver = originalObserver; });

function timeline({ running = true, empty = false } = {}) {
  let resize = () => {};
  let disconnected = false;
  globalThis.ResizeObserver = class {
    constructor(callback: () => void) { resize = callback; }
    observe() {}
    disconnect() { disconnected = true; }
  } as unknown as typeof ResizeObserver;
  const g = { height: 1200, viewport: 400, top: 0, shift: 0, running, empty };
  const writes: number[] = [];
  const away: boolean[] = [];
  const attributes = new Set<string>();
  const el = Object.assign(new EventTarget(), {
    closest: () => null,
    offsetHeight: 400,
    getBoundingClientRect: () => ({ top: 0, height: 400 }),
  });
  Object.defineProperties(el, {
    scrollTop: {
      get: () => g.top,
      set: (value: number) => {
        g.top = Math.max(0, Math.min(value, g.height - g.viewport));
        writes.push(g.top);
      },
    },
    scrollHeight: { get: () => g.height },
    clientHeight: { get: () => g.viewport },
  });
  const row = {
    isConnected: true,
    children: [],
    firstElementChild: null,
    closest: (selector: string) => selector.startsWith(".") ? row : null,
    hasAttribute: (name: string) => attributes.has(name),
    setAttribute: (name: string) => attributes.add(name),
    removeAttribute: (name: string) => attributes.delete(name),
    getBoundingClientRect: () => ({ top: g.shift - g.top, bottom: g.height - g.top, height: g.height }),
  };
  const content = {
    get offsetHeight() { return g.height; },
    getElementsByClassName: () => ({ get length() { return g.empty ? 0 : 1; }, 0: row }),
    contains: (node: unknown) => node === row,
  };
  const control = followTimeline(el as unknown as HTMLElement, content as unknown as HTMLElement, () => g.running, (value) => away.push(value));
  const event = (type: string, fields = {}) => el.dispatchEvent(Object.assign(new Event(type), fields));
  // The reader touching anything (a scrollbar press, say): only movement soon
  // after input counts as theirs; any other movement is layout.
  const user = () => {
    globalThis.dispatchEvent(new Event("pointerdown"));
    globalThis.dispatchEvent(new Event("pointerup"));
  };
  resize();
  writes.length = 0;
  return { g, writes, away, control, resize: () => resize(), event, user, attributes, disconnected: () => disconnected };
}

describe("timeline bottom following", () => {
  test("opens idle history at the bottom, follows its late layout, then leaves idle growth alone", () => {
    const t = timeline({ running: false });
    expect(t.g.top).toBe(800);
    t.g.height += 40;
    t.resize();
    expect(t.g.top).toBe(840);
    t.event("pointerdown");
    t.g.height += 40;
    t.resize();
    expect(t.g.top).toBe(840);
    expect(t.away).toEqual([true]);
  });

  test("typing in a field is not scroll input: a clamp as the composer resizes keeps following", () => {
    const t = timeline({ running: false });
    t.event("pointerdown");
    const key = new Event("keydown");
    Object.defineProperty(key, "target", { value: { closest: () => ({}) } });
    globalThis.dispatchEvent(key);
    // The composer shrank and regrew within the keystroke: the range is back,
    // but scrollTop was clamped in between.
    t.g.top -= 16;
    t.event("scroll");
    t.g.viewport -= 16;
    t.resize();
    expect(t.g.top).toBe(t.g.height - t.g.viewport);
  });

  test("an idle composer deletion that shrinks and restores within one edit stays at bottom", () => {
    const t = timeline({ running: false });
    t.event("pointerdown");
    // WebKit briefly removes the last editable line during Cmd+Backspace.
    // Its scroll clamp survives even though the final viewport is unchanged.
    t.g.top -= 20;
    t.event("scroll");
    t.resize();
    expect(t.g.top).toBe(800);
    expect(t.away).toEqual([]);
    t.control.destroy();
  });

  test("a composer layout clamp does not re-pin someone reading earlier replies", () => {
    const t = timeline({ running: false });
    t.event("wheel", { deltaY: -100 });
    t.g.top = 700;
    t.event("scroll");
    t.g.top -= 20;
    t.event("scroll");
    t.resize();
    expect(t.g.top).toBe(680);
    t.control.destroy();
  });

  test("editing immediately after clicking into the composer is not a recent scroll gesture", () => {
    const t = timeline({ running: false });
    t.event("pointerdown");
    t.user();
    const key = new Event("keydown");
    Object.defineProperty(key, "target", { value: { closest: () => ({}) } });
    globalThis.dispatchEvent(key);
    t.g.top -= 20;
    t.event("scroll");
    expect(t.g.top).toBe(800);
    expect(t.away).toEqual([]);
    t.control.destroy();
  });

  test("upward movement with no input is layout and keeps following", () => {
    const t = timeline();
    t.g.top -= 41;
    t.g.height += 70;
    t.event("scroll");
    t.resize();
    expect(t.g.top).toBe(870);
  });

  test("waits for asynchronous history before the initial jump", () => {
    const t = timeline({ running: false, empty: true });
    expect(t.g.top).toBe(0);
    t.g.empty = false;
    t.resize();
    expect(t.g.top).toBe(800);
  });

  test("follows growing output and a shrinking viewport at bottom", () => {
    const t = timeline();
    t.g.height += 20;
    t.resize();
    expect(t.g.top).toBe(820);
    t.g.viewport -= 30;
    t.resize();
    expect(t.g.top).toBe(850);
  });

  test("tiny upward wheel releases before scrolling or resize", () => {
    const t = timeline();
    t.event("wheel", { deltaY: -0.1 });
    t.g.height += 20;
    t.resize();
    expect(t.g.top).toBe(800);
    expect(t.writes).toEqual([]);
  });

  test("fractional upward movement releases inside bottom tolerance", () => {
    const t = timeline();
    t.user();
    t.g.top -= 0.25;
    t.event("scroll");
    t.g.height += 20;
    t.resize();
    expect(t.g.top).toBe(799.75);
    expect(t.writes).toEqual([]);
  });

  test("resize before the scroll event cannot undo user movement", () => {
    const t = timeline();
    t.user();
    t.g.top -= 12;
    t.g.height += 20;
    t.resize();
    t.event("scroll");
    t.resize();
    expect(t.g.top).toBe(788);
    expect(t.writes).toEqual([]);
  });

  test("repeated small upward inputs never snap an idle reply back down", () => {
    const t = timeline({ running: false });
    for (let n = 0; n < 12; n++) {
      t.event("wheel", { deltaY: -2 });
      t.g.top -= 2;
      t.resize();
      t.event("scroll");
    }
    expect(t.g.top).toBe(776);
    expect(t.writes).toEqual([]);
  });

  test("scrolling back down re-arms streaming within 32px of the bottom; further up does not", () => {
    const t = timeline();
    t.user();
    t.g.top = 700;
    t.event("scroll");
    t.g.top = 740;
    t.event("scroll");
    t.g.height += 20;
    t.resize();
    expect(t.g.top).toBe(740);
    // 50px short of the moving bottom: still reading.
    t.g.top = 770;
    t.event("scroll");
    expect(t.g.top).toBe(770);
    // 10px short: re-armed, and the rest is closed at once.
    t.g.top = 810;
    t.event("scroll");
    expect(t.g.top).toBe(820);
    t.g.height += 20;
    t.resize();
    expect(t.g.top).toBe(840);
  });

  test("arrow jumps, resumes streaming, and hides itself; up still releases", () => {
    const t = timeline();
    t.user();
    t.g.top -= 100;
    t.event("scroll");
    expect(t.away).toEqual([true]);
    t.control.scrollToBottom();
    expect(t.g.top).toBe(800);
    expect(t.away).toEqual([true, false]);
    t.g.height += 25;
    t.resize();
    expect(t.g.top).toBe(825);
    t.event("wheel", { deltaY: -1 });
    t.g.top -= 1;
    t.resize();
    expect(t.g.top).toBe(824);
  });

  test("a finished reply collapsing to a placeholder and redrawing stays at the bottom", () => {
    const t = timeline();
    t.g.height += 10;
    t.resize();
    t.g.running = false;
    // The live tail leaves; the committed row first lays out at its estimate,
    // so the range shrinks and the browser clamps scrollTop.
    t.g.height -= 300;
    t.g.top = t.g.height - t.g.viewport;
    t.event("scroll");
    t.resize();
    t.g.height += 300;
    t.resize();
    expect(t.g.top).toBe(810);
    expect(t.away.at(-1) ?? false).toBe(false);
  });

  test("after a reply, the reader's own input stops idle following", () => {
    const t = timeline();
    t.g.height += 10;
    t.resize();
    t.g.running = false;
    t.g.height += 10;
    t.resize();
    expect(t.g.top).toBe(820);
    t.event("pointerdown");
    t.g.height += 40;
    t.resize();
    expect(t.g.top).toBe(820);
  });

  test("a new turn while reading above does not re-pin", () => {
    const t = timeline();
    t.user();
    t.g.top -= 50;
    t.event("scroll");
    t.g.running = false;
    t.g.height += 10;
    t.resize();
    t.g.running = true;
    t.g.height += 10;
    t.resize();
    expect(t.g.top).toBe(750);
  });

  test("layout above preserves the reading block and ignores its own scroll event", () => {
    const t = timeline();
    t.user();
    t.g.top -= 100;
    t.event("scroll");
    t.g.shift += 60;
    t.g.height += 60;
    t.resize();
    expect(t.g.top).toBe(760);
    t.event("scroll");
    t.g.shift += 10;
    t.g.height += 10;
    t.resize();
    expect(t.g.top).toBe(770);
  });

  test("keyboard and touch upward intent release before scrolling", () => {
    for (const key of ["ArrowUp", "PageUp", "Home", " "]) {
      const t = timeline();
      t.event("keydown", { key, shiftKey: true });
      t.g.height += 20;
      t.resize();
      expect(t.g.top).toBe(800);
    }
    const t = timeline();
    t.event("touchstart", { touches: [{ clientY: 100 }] });
    t.event("touchmove", { touches: [{ clientY: 101 }] });
    t.g.height += 20;
    t.resize();
    expect(t.g.top).toBe(800);
  });

  test("bottom rubber-band overscroll and rebound never flash the arrow or correct native motion", () => {
    const t = timeline({ running: false });
    t.user();
    for (const top of [802, 809, 830, 812, 801, 800]) {
      t.g.top = top;
      t.event("scroll");
      t.resize();
    }
    expect(t.away).toEqual([]);
    expect(t.writes).toEqual([]);
  });

  test("the arrow ignores tiny bottom fluctuations without delaying follow release", () => {
    const t = timeline({ running: false });
    t.event("pointerdown");
    t.user();
    for (const top of [799.5, 798, 800, 794, 800]) {
      t.g.top = top;
      t.event("scroll");
    }
    expect(t.away).toEqual([]);
    t.g.top = 780;
    t.event("scroll");
    expect(t.away).toEqual([true]);
    t.g.top = 790;
    t.event("scroll");
    expect(t.away).toEqual([true]);
    t.g.top = 800;
    t.event("scroll");
    expect(t.away).toEqual([true, false]);
  });

  test("cleanup removes listeners, observer and held row marker", () => {
    const t = timeline();
    expect(t.attributes.has(HELD_ANCHOR)).toBe(true);
    t.control.destroy();
    expect(t.disconnected()).toBe(true);
    expect(t.attributes.has(HELD_ANCHOR)).toBe(false);
    t.event("wheel", { deltaY: -2 });
    expect(t.attributes.has(HELD_ANCHOR)).toBe(false);
  });

  // Last: the declared intent stays fresh for 250ms, into any test after it.
  test("a declared jump (find, typed into its field) is the reader's and releases", () => {
    const t = timeline();
    noteScrollIntent();
    t.g.top -= 300;
    t.event("scroll");
    t.g.height += 20;
    t.resize();
    expect(t.g.top).toBe(500);
  });
});
