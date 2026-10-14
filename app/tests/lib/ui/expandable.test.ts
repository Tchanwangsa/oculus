import { describe, expect, test } from "bun:test";
import { EXPANDABLE, expandWithin, onExpand, type ExpandRequest } from "@/lib/ui/expandable";

/** A minimal element tree: an EventTarget with attributes and children. */
class Node extends EventTarget {
  children: Node[] = [];
  constructor(private attrs: string[] = [], ...children: Node[]) {
    super();
    this.children = children;
  }
  hasAttribute(name: string) {
    return this.attrs.includes(name);
  }
  querySelectorAll(selector: string): Node[] {
    expect(selector).toBe(`[${EXPANDABLE}]`);
    return this.children.flatMap((c) => [...(c.hasAttribute(EXPANDABLE) ? [c] : []), ...c.querySelectorAll(selector)]);
  }
}

function heard(node: Node) {
  const got: ExpandRequest[] = [];
  onExpand(node as unknown as Element, (r) => got.push(r));
  return got;
}

describe("expandWithin", () => {
  test("reaches every marked element inside the root, and only those", () => {
    const deep = new Node([EXPANDABLE]);
    const near = new Node([EXPANDABLE]);
    const plain = new Node();
    const outside = new Node([EXPANDABLE]);
    const root = new Node([], near, new Node([], deep), plain);
    const [d, n, p, o] = [heard(deep), heard(near), heard(plain), heard(outside)];
    expandWithin(root as unknown as Element, { sync: true, upToId: 7 });
    expect(d).toEqual([{ sync: true, upToId: 7 }]);
    expect(n).toEqual([{ sync: true, upToId: 7 }]);
    expect(p).toEqual([]);
    expect(o).toEqual([]);
  });

  test("a marked root is reached itself, and the event doesn't bubble", () => {
    const child = new Node([EXPANDABLE]);
    const root = new Node([EXPANDABLE], child);
    const [r, c] = [heard(root), heard(child)];
    expandWithin(root as unknown as Element, { sync: false });
    expect(r).toHaveLength(1);
    expect(c).toHaveLength(1);
    expandWithin(null, { sync: false });
  });

  test("the cleanup stops listening", () => {
    const root = new Node([EXPANDABLE]);
    const got: ExpandRequest[] = [];
    const off = onExpand(root as unknown as Element, (r) => got.push(r));
    off();
    expandWithin(root as unknown as Element, { sync: false });
    expect(got).toEqual([]);
  });
});
