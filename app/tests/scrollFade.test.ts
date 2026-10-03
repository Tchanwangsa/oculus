import { describe, expect, test } from "bun:test";
import { syncScrollFade } from "../src/lib/scrollFade";

function scroller() {
  const values = new Map<string, string>();
  const operations: string[] = [];
  const geometry = { scrollTop: 0, scrollLeft: 0, scrollHeight: 400, scrollWidth: 500,
    clientHeight: 100, clientWidth: 200, offsetWidth: 215 };
  const el = {
    dataset: new Proxy({} as Record<string, string>, {
      set(target, key, value) { operations.push("write"); return Reflect.set(target, key, value); },
    }),
    style: {
      getPropertyValue: (key: string) => values.get(key) ?? "",
      setProperty: (key: string, value: string) => { operations.push("write"); values.set(key, value); },
    },
  };
  for (const key of Object.keys(geometry) as (keyof typeof geometry)[]) {
    Object.defineProperty(el, key, { get: () => { operations.push("read"); return geometry[key]; } });
  }
  return { el: el as unknown as HTMLElement, geometry, operations, values };
}

describe("scroll fade geometry", () => {
  test("reads before writing and only changes styles when an edge state changes", () => {
    const { el, geometry, operations, values } = scroller();
    syncScrollFade(el, "y");
    expect(operations.slice(operations.indexOf("write"))).not.toContain("read");
    expect(values.get("--fade-y-start")).toBe("0");
    expect(values.get("--fade-y-end")).toBe("1");
    expect(values.get("--fade-bar")).toBe("15px");
    operations.length = 0;
    syncScrollFade(el, "y");
    expect(operations).not.toContain("write");

    geometry.scrollTop = 40;
    syncScrollFade(el, "y");
    expect(operations.filter((op) => op === "write")).toHaveLength(1);
    expect(values.get("--fade-y-start")).toBe("1");
    geometry.scrollTop = 300;
    syncScrollFade(el, "y");
    expect(values.get("--fade-y-end")).toBe("0");
  });

  test("horizontal and combined axes preserve the subpixel overflow slack", () => {
    const { el, geometry, values } = scroller();
    geometry.scrollWidth = geometry.clientWidth + 1;
    geometry.scrollHeight = geometry.clientHeight + 1;
    syncScrollFade(el, "xy");
    expect([...values.values()]).toEqual(["0", "0", "0", "0"]);
    geometry.scrollWidth = 500;
    geometry.scrollLeft = 300;
    syncScrollFade(el, "x");
    expect(el.dataset.scrollFade).toBe("x");
    expect(values.get("--fade-x-start")).toBe("1");
    expect(values.get("--fade-x-end")).toBe("0");
  });
});
