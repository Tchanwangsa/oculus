import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

// The shipped JS matcher against the same fixture `rules.rs` tests, so the
// guard in the page and the native guard agree on every captured URL.
const scripts = join(import.meta.dir, "../../../src-tauri/src/shell/dock/scripts");
const read = (name: string) => readFileSync(join(scripts, name), "utf8");

type Place = { kind: string; id?: string };
type Hop = {
  classify: (href: string, base: string) => Place;
  allows: (state: string | null, place: Place) => boolean;
  next: (state: string | null, place: Place) => string | null;
};
type Case = { url: string; from?: string; kind: string; id?: string; allow: boolean };

// `scripts.rs` prepends the table as `const RULES = <json>;`; the test does
// the same with the file's text.
const hop = new Function(`const RULES = ${read("rules/instagram.json")};\n${read("hop.js")}\nreturn hop;`)() as Hop;
const cases = JSON.parse(read("rules/instagram.cases.json")) as Case[];

test("the fixture covers the captured shapes", () => {
  expect(cases.length).toBeGreaterThan(20);
});

for (const c of cases) {
  test(`${c.url} from ${c.from ?? "messages"}`, () => {
    const place = hop.classify(c.url, "https://www.instagram.com/direct/inbox/");
    expect(place.kind).toBe(c.kind);
    expect(place.id).toBe(c.id);
    expect(hop.allows(c.from ?? null, place)).toBe(c.allow);
  });
}

test("entering messages leaves an item; login keeps it", () => {
  const at = (path: string) => hop.classify(path, "https://www.instagram.com/");
  expect(hop.next(null, at("/reels/DcJ2NdLxhAs/"))).toBe("post:DcJ2NdLxhAs");
  expect(hop.next("post:DcJ2NdLxhAs", at("/direct/t"))).toBeNull();
  expect(hop.next("post:DcJ2NdLxhAs", at("/accounts/login/"))).toBe("post:DcJ2NdLxhAs");
});

test("relative hrefs resolve against the page", () => {
  expect(hop.classify("/explore/", "https://www.instagram.com/direct/t/1/").kind).toBe("blocked");
  expect(hop.classify("../../../p/abc/", "https://www.instagram.com/direct/t/1/").id).toBe("post:abc");
});

// A fake session history and window, enough to run the shipped guard: the
// router follows every popstate, trusted or not, as Instagram's does.
function page(start: string) {
  const origin = "https://www.instagram.com";
  const entries = [{ url: origin + start, state: null as unknown }];
  let index = 0;
  const listeners: Record<string, ((e: any) => void)[]> = {};
  const pending: (() => void)[] = [];
  const location = {
    get href() {
      return entries[index].url;
    },
    get pathname() {
      return new URL(entries[index].url).pathname;
    },
  };
  const window: any = {
    setTimeout: (f: () => void) => pending.push(f),
    webkit: undefined,
    addEventListener: (type: string, f: (e: any) => void) => (listeners[type] ??= []).push(f),
    dispatchEvent: (e: any) => (listeners[e.type] ?? []).forEach((f) => f(e)),
    open: (...args: unknown[]) => (window.opened = args),
  };
  const fire = (state: unknown) => window.dispatchEvent({ type: "popstate", state, isTrusted: true });
  const history: any = {
    get state() {
      return entries[index].state;
    },
    pushState(state: unknown, _: string, url: string) {
      entries.splice(index + 1);
      entries.push({ url: new URL(url, location.href).href, state });
      index++;
    },
    replaceState(state: unknown, _: string, url: string) {
      entries[index] = { url: new URL(url, location.href).href, state };
    },
    back() {
      pending.push(() => {
        index--;
        fire(entries[index].state);
      });
    },
  };
  class PopStateEvent {
    type: string;
    state: unknown;
    isTrusted = false;
    constructor(type: string, init: { state: unknown }) {
      this.type = type;
      this.state = init.state;
    }
  }
  const views: string[] = [];
  window.addEventListener("popstate", () => views.push(location.href));
  const body = `const RULES = ${read("rules/instagram.json")};\n${read("hop.js")}\n${read("post.js")}\n${read("guard.js")}`;
  new Function("window", "history", "location", "PopStateEvent", `'use strict';\n${body}`)(window, history, location, PopStateEvent);
  const settle = () => {
    while (pending.length) pending.shift()!();
  };
  const click = (href: string, target = "") => {
    const a = {
      tagName: "A",
      target,
      href: new URL(href, location.href).href,
      hasAttribute: () => true,
      getAttribute: () => href,
    };
    const e = { type: "click", button: 0, prevented: false, composedPath: () => [a], preventDefault() { this.prevented = true; }, stopImmediatePropagation() {} };
    window.dispatchEvent(e);
    return e.prevented;
  };
  return { history, location, window, views, settle, click, path: () => location.pathname };
}

test("a blocked push from an item goes back to the item", () => {
  const p = page("/direct/t/1/");
  p.history.pushState(null, "", "/p/A/");
  p.history.pushState(null, "", "/fabriziorom/");
  p.settle();
  expect(p.path()).toBe("/p/A/");
  p.history.pushState(null, "", "/p/A/c/1/");
  expect(p.path()).toBe("/p/A/c/1/");
});

test("the expired-story redirect lands back on the thread", () => {
  const p = page("/direct/t/1/");
  p.history.pushState(null, "", "/fabriziorom/?show_story_unavailable=1");
  p.settle();
  expect(p.path()).toBe("/direct/t/1/");
});

test("a story steps through its owner; a replace to another item is undone", () => {
  const p = page("/direct/t/1/");
  p.history.pushState(null, "", "/stories/fabriziorom/1?reel_id=2");
  p.history.replaceState(null, "", "/stories/fabriziorom/2/");
  expect(p.path()).toBe("/stories/fabriziorom/2/");
  p.history.replaceState(null, "", "/p/DeUiyY6CNyh/");
  expect(p.path()).toBe("/stories/fabriziorom/2/");
  expect(p.views.at(-1)).toBe("https://www.instagram.com/stories/fabriziorom/2/");
  p.history.replaceState(null, "", "/direct/t");
  p.history.pushState(null, "", "/p/DeUiyY6CNyh/");
  expect(p.path()).toBe("/p/DeUiyY6CNyh/");
});

test("clicks: profiles and the feed do nothing, off-site links go to the browser", () => {
  const p = page("/direct/t/1/");
  expect(p.click("/pondhubdotcom/")).toBe(true);
  expect(p.click("/")).toBe(true);
  expect(p.click("#")).toBe(false);
  expect(p.click("/stories/fabriziorom/1/")).toBe(false);
  expect(p.click("https://example.com/")).toBe(true);
  expect(p.window.opened[0]).toBe("https://example.com/");
  expect(p.click("https://example.com/x", "_blank")).toBe(false);
});

test("leaving on park goes to the inbox and the guard lets it", () => {
  const p = page("/direct/t/1/");
  p.history.pushState(null, "", "/p/A/");
  const leave = `const PARK_PATH = "/direct/inbox/";\nconst RULES = ${read("rules/instagram.json")};\n${read("hop.js")}\n${read("leave.js")}`;
  new Function("window", "history", "location", "PopStateEvent", `'use strict';\n${leave}`)(p.window, p.history, p.location, class {
    type: string;
    state: unknown;
    constructor(type: string, init: { state: unknown }) {
      this.type = type;
      this.state = init.state;
    }
  });
  expect(p.path()).toBe("/direct/inbox/");
  expect(p.views.at(-1)).toBe("https://www.instagram.com/direct/inbox/");
});
