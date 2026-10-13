import { describe, expect, test } from "bun:test";

import { diffHunks, diffStats, diffText, type DiffChange } from "@/lib/notes/versionDiff";

/** Each change as `[removed, inserted]` text, for readable expectations. */
const edits = (a: string, b: string) =>
  diffText(a, b).map((c) => [a.slice(c.fromA, c.toA), b.slice(c.fromB, c.toB)]);

/** Rebuilds `b` from `a` and the changes, checking that every gap between
 *  changes really is equal text. */
function apply(a: string, b: string, changes: DiffChange[]): string {
  let out = "";
  let pa = 0;
  let pb = 0;
  for (const c of changes) {
    expect(a.slice(pa, c.fromA)).toBe(b.slice(pb, c.fromB));
    out += a.slice(pa, c.fromA) + b.slice(c.fromB, c.toB);
    pa = c.toA;
    pb = c.toB;
  }
  expect(a.slice(pa)).toBe(b.slice(pb));
  return out + a.slice(pa);
}

const splitsPair = (s: string, i: number) =>
  i > 0 && i < s.length && /[\uD800-\uDBFF]/.test(s[i - 1]) && /[\uDC00-\uDFFF]/.test(s[i]);

/** Sorted, non-overlapping, non-adjacent, non-empty, whole code points. */
function checkShape(a: string, b: string, changes: DiffChange[]) {
  for (let i = 0; i < changes.length; i++) {
    const c = changes[i];
    expect(c.toA >= c.fromA && c.toB >= c.fromB).toBe(true);
    expect(c.toA > c.fromA || c.toB > c.fromB).toBe(true);
    for (const o of [c.fromA, c.toA]) expect(splitsPair(a, o)).toBe(false);
    for (const o of [c.fromB, c.toB]) expect(splitsPair(b, o)).toBe(false);
    const next = changes[i + 1];
    if (next) {
      expect(next.fromA).toBeGreaterThan(c.toA);
      expect(next.fromB).toBeGreaterThan(c.toB);
    }
  }
}

/** mulberry32: a small seeded generator, so failures reproduce. */
function rng(seed: number) {
  let s = seed >>> 0;
  return () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const SAMPLE = `---
title: Quantum states
tags: [physics, notes]
---

# Week 3 — Superposition

A qubit is written as $|\\psi\\rangle = \\alpha|0\\rangle + \\beta|1\\rangle$ where
$|\\alpha|^2 + |\\beta|^2 = 1$. See **Nielsen & Chuang**, §1.2.

## Measurement

1. Measuring in the computational basis gives $0$ with probability $|\\alpha|^2$.
2. The state *collapses* — afterwards it is $|0\\rangle$ or $|1\\rangle$.
   - Repeat measurements agree.
   - [ ] Check the Bloch sphere picture.

$$
\\begin{aligned}
H|0\\rangle &= \\tfrac{1}{\\sqrt{2}}(|0\\rangle + |1\\rangle) \\\\
H|1\\rangle &= \\tfrac{1}{\\sqrt{2}}(|0\\rangle - |1\\rangle)
\\end{aligned}
$$

| Gate | Matrix | Effect |
| ---- | ------ | ------ |
| X    | $\\sigma_x$ | bit flip |
| Z    | $\\sigma_z$ | phase flip |

> สวัสดีครับ วันนี้เรียนเรื่องควอนตัม 🧪 and 👍🏽 emoji.

\`\`\`python
def hadamard(state):
    return (state[0] + state[1]) / 2 ** 0.5
\`\`\`

![Bloch sphere](attachments/bloch.png)
`;

describe("diffText", () => {
  test("identical texts have no changes", () => {
    expect(diffText(SAMPLE, SAMPLE)).toEqual([]);
    expect(diffText("", "")).toEqual([]);
  });

  test("empty to text and text to empty", () => {
    expect(diffText("", "hello world")).toEqual([{ fromA: 0, toA: 0, fromB: 0, toB: 11 }]);
    expect(diffText("hello world", "")).toEqual([{ fromA: 0, toA: 11, fromB: 0, toB: 0 }]);
  });

  test("a word replaced mid-sentence is the whole word", () => {
    expect(edits("The cat sat on the mat.", "The cat lay on the mat.")).toEqual([["sat", "lay"]]);
    expect(edits("I like cats a lot", "I like cots a lot")).toEqual([["cats", "cots"]]);
  });

  test("neighbouring replaced words read as one replacement", () => {
    expect(edits("The quick brown fox jumps", "The slow red fox jumps")).toEqual([["quick brown", "slow red"]]);
  });

  test("insertion at the start and at the end", () => {
    expect(diffText("world", "hello world")).toEqual([{ fromA: 0, toA: 0, fromB: 0, toB: 6 }]);
    expect(diffText("hello", "hello world")).toEqual([{ fromA: 5, toA: 5, fromB: 5, toB: 11 }]);
  });

  test("a whole paragraph deleted is one deletion of its lines", () => {
    const a = "Para one.\n\nPara two is here.\nSecond line.\n\nPara three.";
    const b = "Para one.\n\nPara three.";
    expect(edits(a, b)).toEqual([["Para two is here.\nSecond line.\n\n", ""]]);
  });

  test("a list item inserted lands on line boundaries", () => {
    expect(edits("- x\n- y\n", "- x\n- new\n- y\n")).toEqual([["", "- new\n"]]);
  });

  test("LaTeX edits mark the changed command or subscript", () => {
    expect(edits("$|\\psi_1\\rangle$", "$|\\psi_2\\rangle$")).toEqual([["psi_1", "psi_2"]]);
    expect(edits("$\\alpha + \\beta$", "$\\alpha - \\beta$")).toEqual([["+", "-"]]);
    expect(edits("$x^2$", "$x^{2}$")).toEqual([["2", "{2}"]]);
  });

  test("CRLF is one newline and edits keep it whole", () => {
    expect(edits("a\r\nb", "a\nb")).toEqual([["\r\n", "\n"]]);
    expect(edits("one\r\ntwo\r\n", "one\r\nthree\r\n")).toEqual([["two", "three"]]);
    const changes = diffText("x\r\ny", "x\r\nz\r\ny");
    for (const c of changes) {
      expect("x\r\ny"[c.fromA - 1] === "\r" && "x\r\ny"[c.fromA] === "\n").toBe(false);
    }
  });

  test("changes on separate lines stay separate", () => {
    expect(edits("alpha\nbeta\n", "gamma\ndelta\n")).toEqual([
      ["alpha", "gamma"],
      ["beta", "delta"],
    ]);
  });

  test("Thai splits into words", () => {
    expect(edits("สวัสดีครับวันนี้", "สวัสดีค่ะวันนี้")).toEqual([["ครับ", "ค่ะ"]]);
  });

  test("emoji are never split", () => {
    expect(edits("x 😀 y", "x 😃 y")).toEqual([["😀", "😃"]]);
    expect(edits("hi 👍 there", "hi 👍🏽 there")).toEqual([["👍", "👍🏽"]]);
    const a = "🧪🧪🧪";
    const b = "🧪🧫🧪";
    const changes = diffText(a, b);
    checkShape(a, b, changes);
    expect(apply(a, b, changes)).toBe(b);
  });

  test("random edits to a markdown note always rebuild the new text", () => {
    const rand = rng(20261013);
    const pieces = ["the", " ", "\n", "$", "\\psi", "_1", "**", "qubit", "สวัสดี", "🧪", "👍🏽", "\r\n", "- ", "|", "{", "}", "x"];
    const pick = () => pieces[Math.floor(rand() * pieces.length)];
    for (let iter = 0; iter < 200; iter++) {
      const base = Array.from(rand() < 0.5 ? SAMPLE : SAMPLE.slice(0, 400));
      const next = base.slice();
      const n = 1 + Math.floor(rand() * 8);
      for (let e = 0; e < n; e++) {
        const at = Math.floor(rand() * (next.length + 1));
        const cut = rand() < 0.6 ? Math.floor(rand() * 12) : 0;
        const add: string[] = [];
        const k = rand() < 0.7 ? Math.floor(rand() * 6) : 0;
        for (let j = 0; j < k; j++) add.push(...Array.from(pick()));
        next.splice(at, cut, ...add);
      }
      const a = base.join("");
      const b = next.join("");
      const changes = diffText(a, b);
      checkShape(a, b, changes);
      expect(apply(a, b, changes)).toBe(b);
    }
  });

  test("a small edit in a 50k-character note is fast", () => {
    let a = "";
    for (let i = 0; a.length < 50_000; i++) a += SAMPLE.replace("Week 3", `Week ${i}`);
    const b = a.slice(0, 25_000) + " inserted words " + a.slice(25_000);
    const t0 = performance.now();
    const changes = diffText(a, b);
    const ms = performance.now() - t0;
    expect(apply(a, b, changes)).toBe(b);
    expect(changes.length).toBe(1);
    expect(ms).toBeLessThan(100);
  });

  test("two unrelated 50k-character notes finish quickly", () => {
    const words = (seed: number) => {
      const rand = rng(seed);
      const vocab = Array.from({ length: 3000 }, (_, i) => `w${i.toString(36)}${seed % 7}`);
      let s = "";
      while (s.length < 50_000) {
        s += vocab[Math.floor(rand() * vocab.length)];
        const r = rand();
        s += r < 0.05 ? "\n\n" : r < 0.15 ? ".\n" : r < 0.2 ? ", " : " ";
      }
      return s;
    };
    const a = words(1);
    const b = words(2);
    const t0 = performance.now();
    const changes = diffText(a, b);
    const ms = performance.now() - t0;
    expect(apply(a, b, changes)).toBe(b);
    expect(ms).toBeLessThan(500);
  });
});

describe("diffHunks", () => {
  const b = "l0\nl1\nl2\nl3\nl4\nl5\nl6\n";
  const at = (line: number, len = 2): DiffChange => {
    const from = line * 3;
    return { fromA: from, toA: from + len, fromB: from, toB: from + len };
  };

  test("changes on the same line form one hunk", () => {
    const c1 = { fromA: 6, toA: 7, fromB: 6, toB: 7 };
    const c2 = { fromA: 7, toA: 7, fromB: 8, toB: 8 };
    expect(diffHunks([c1, c2], b)).toEqual([{ fromB: 6, toB: 8, changes: [c1, c2] }]);
  });

  test("changes on adjacent lines join; one unchanged line between splits", () => {
    const hunks = diffHunks([at(1), at(2), at(4)], b);
    expect(hunks.map((h) => h.changes.length)).toEqual([2, 1]);
    expect(hunks[0]).toMatchObject({ fromB: 3, toB: 8 });
  });

  test("a change spanning lines reaches the next line's change", () => {
    const multi: DiffChange = { fromA: 0, toA: 0, fromB: 0, toB: 6 }; // "l0\nl1\n"
    const hunks = diffHunks([multi, at(2)], b);
    expect(hunks.length).toBe(1);
    // A change ending in its line's newline does not reach the line after next.
    expect(diffHunks([multi, at(3)], b).length).toBe(2);
  });

  test("a pure deletion still forms a hunk at its position", () => {
    const del: DiffChange = { fromA: 3, toA: 10, fromB: 9, toB: 9 };
    expect(diffHunks([del], b)).toEqual([{ fromB: 9, toB: 9, changes: [del] }]);
    expect(diffHunks([at(2), del], b).length).toBe(1);
    expect(diffHunks([at(0), del], b).length).toBe(2);
  });

  test("hunks from a real diff", () => {
    const a = "one\ntwo\nthree\nfour\nfive\nsix\n";
    const nb = "one\n2\nthree\nfour\nfive\n6\n";
    const hunks = diffHunks(diffText(a, nb), nb);
    expect(hunks.length).toBe(2);
    expect(diffHunks([], nb)).toEqual([]);
  });
});

describe("diffStats", () => {
  test("counts words inserted and deleted", () => {
    const a = "The quick brown fox sleeps.";
    const b = "The slow red fox sleeps. Then it jumps high.";
    expect(edits(a, b)).toEqual([
      ["quick brown", "slow red"],
      ["", " Then it jumps high."],
    ]);
    expect(diffStats(a, b, diffText(a, b))).toEqual({ inserted: 6, deleted: 2 });
  });

  test("whitespace and punctuation edits count no words but still exist", () => {
    const a = "Hello world.";
    const b = "Hello,  world!";
    const changes = diffText(a, b);
    expect(changes.length).toBeGreaterThan(0);
    expect(diffStats(a, b, changes)).toEqual({ inserted: 0, deleted: 0 });
  });
});
