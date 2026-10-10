// The source-map property checks (render.ts --source-map): one formula
// rendered by the fork with `sourceMap` off and on. `data-s`/`data-e` are
// UTF-16 offsets into the formula, which is what a JS string's indices are.
//
//   a  errors: the flag changes no answer (error, panic or success) and no
//      error's kind (its message without the position and context). An
//      error that only moves is counted, not failed: with the flag on, an
//      error on a macro body's token points at the invocation.
//   b  ranges: every pair is two integers, 0 ≤ s ≤ e ≤ length, inside the
//      nearest mapped ancestor's range.
//   c  glyphs: every visible character in the HTML has a mapped
//      ancestor-or-self.
//   d  coverage: every non-space source character lies in a mapped leaf (an
//      element with no mapped descendants), except structure syntax: see
//      `syntax`.
//   e  identity: the flag-on HTML with the ranges and placeholders removed
//      is the flag-off HTML, glyph runs split (each glyph its own span) aside.
//      `italic` counts formulas equal but for split glyphs that keep their
//      own italic correction (`margin-right`), which a merged run has only
//      on its last glyph; formulas with a placeholder are counted apart,
//      since the placeholder has height.

export interface El {
  tag: string;
  attrs: Map<string, string>;
  children: Node[];
}
export type Node = El | string;

const ENTITIES: Record<string, string> = { amp: "&", lt: "<", gt: ">", quot: '"', "#x27": "'", "#39": "'" };
function decode(s: string): string {
  return s.replace(/&(#x[0-9a-f]+|#\d+|[a-z]+);/gi, (m, e: string) => {
    if (e in ENTITIES) return ENTITIES[e];
    if (e[0] === "#") return String.fromCodePoint(e[1] === "x" ? Number.parseInt(e.slice(2), 16) : Number(e.slice(1)));
    return m;
  });
}

/** KaTeX's markup: quoted attributes, `/>` for empty elements, no comments. */
export function parseMarkup(html: string): El {
  const root: El = { tag: "#root", attrs: new Map(), children: [] };
  const stack: El[] = [root];
  const token = /<(\/?)([a-zA-Z][\w:-]*)((?:\s+[^\s=/>]+(?:\s*=\s*(?:"[^"]*"|'[^']*'))?)*)\s*(\/?)>|[^<]+/g;
  for (const m of html.matchAll(token)) {
    const top = stack[stack.length - 1];
    if (m[2] === undefined) {
      top.children.push(decode(m[0]));
    } else if (m[1]) {
      if (stack.length > 1) stack.pop();
    } else {
      const attrs = new Map<string, string>();
      for (const a of m[3].matchAll(/([^\s=/>]+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'))?/g)) attrs.set(a[1], decode(a[2] ?? a[3] ?? ""));
      const el: El = { tag: m[2], attrs, children: [] };
      top.children.push(el);
      if (!m[4]) stack.push(el);
    }
  }
  return root;
}

export function serialize(n: Node): string {
  if (typeof n === "string") return n;
  if (n.tag === "#root") return n.children.map(serialize).join("");
  const attrs = [...n.attrs].map(([k, v]) => ` ${k}="${v}"`).join("");
  return `<${n.tag}${attrs}>${n.children.map(serialize).join("")}</${n.tag}>`;
}

const classes = (el: El) => (el.attrs.get("class") ?? "").split(/\s+/).filter(Boolean);
const hasClass = (el: El, c: string) => classes(el).includes(c);

function find(el: El, pred: (e: El) => boolean): El | undefined {
  if (pred(el)) return el;
  for (const c of el.children) {
    if (typeof c === "string") continue;
    const hit = find(c, pred);
    if (hit) return hit;
  }
  return undefined;
}

/** The HTML half (MathML is left alone by the flag; check e covers it). */
function htmlPart(root: El): El {
  return find(root, (e) => hasClass(e, "katex-html")) ?? root;
}

export const PLACEHOLDER = "oc-placeholder";

/** A source token: a command (`\name`, `\,`), or one character. */
function tokens(tex: string): { start: number; end: number; text: string }[] {
  const out: { start: number; end: number; text: string }[] = [];
  const re = /\\(?:[a-zA-Z@]+\*?|[\s\S])|[\s\S]/g;
  for (const m of tex.matchAll(re)) out.push({ start: m.index!, end: m.index! + m[0].length, text: m[0] });
  return out;
}

/** A grouping name for a range: its first token, and `…` when it goes on. */
function kindOf(tex: string, s: number, e: number): string {
  if (s === e) return "(empty slot)";
  const first = tokens(tex.slice(s, e))[0]?.text ?? "";
  const name = /^[a-zA-Z]$/.test(first) ? "letter" : /^\d$/.test(first) ? "digit" : first;
  return e - s > first.length ? `${name}…` : name;
}

export interface Failure {
  check: "a" | "b" | "c" | "d" | "e";
  /** The group key: what kind of node or character fails, and how. */
  key: string;
}
export interface Outcome {
  failures: Failure[];
  /** Check e's class: "equal", "italic", "placeholder", or "differs". */
  identity: "equal" | "italic" | "placeholder" | "differs" | "n/a";
  /** Characters excluded from coverage, by reason (for the report). */
  excluded: Map<string, number>;
  /** Check a: the same error, at another position. */
  errorMoved: boolean;
}

interface Mapped {
  el: El;
  s: number;
  e: number;
  parent?: Mapped;
  hasMappedChild: boolean;
}

/** Commands that draw nothing of their own (a switch, or a node drawn as a
 *  fragment), so no element carries their range; a braced argument that
 *  follows (`\color{red}`) is theirs too. */
const NO_GLYPH =
  /^\\(displaystyle|textstyle|scriptstyle|scriptscriptstyle|tiny|scriptsize|footnotesize|small|normalsize|large|Large|LARGE|huge|Huge|color|textcolor|nonumber|notag|phantom|hphantom|vphantom|mathchoice|hbox)$/;

/** Fonts and colour macros: drawn as a fragment when their argument is. */
const FRAGMENT =
  /^\\(math[a-z]+|boldsymbol|bm|textcolor|(?:red|blue|green|gold|gray|maroon|mint|orange|pink|purple|teal|kaBlue|kaGreen)[A-I]?)$/;

/** Definitions, which draw nothing. */
const DEFINE = /^\\(def|gdef|edef|xdef|let|newcommand|renewcommand|providecommand)$/;

/** Characters that are syntax, not content, and why. `mapped` holds every
 *  mapped range; `inStructure(i)` says whether a mapped element with mapped
 *  children covers `i`. A group in a structure with no mapped range inside
 *  is one of the structure's parameters. */
function syntax(
  tex: string,
  mapped: { s: number; e: number }[],
  inStructure: (i: number) => boolean,
): (i: number) => string | undefined {
  const reason: (string | undefined)[] = new Array(tex.length);
  const mark = (r: string, from: number, to: number) => {
    for (let i = from; i < to; i++) reason[i] ??= r;
  };
  const toks = tokens(tex);
  const next = (k: number) => {
    let n = k + 1;
    while (n < toks.length && /^\s$/.test(toks[n].text)) n++;
    return n;
  };
  // Argument groups: every {…}, and […] right after a command or argument.
  const groups: { open: number; close: number; after?: string }[] = [];
  const stack: { k: number; bracket: boolean }[] = [];
  toks.forEach((t, k) => {
    if (t.text === "{") stack.push({ k, bracket: false });
    else if (t.text === "[") {
      let p = k - 1;
      while (p >= 0 && /^\s$/.test(toks[p].text)) p--;
      if (p >= 0 && (toks[p].text.startsWith("\\") || toks[p].text === "}")) stack.push({ k, bracket: true });
    } else if ((t.text === "}" || t.text === "]") && stack.length && stack[stack.length - 1].bracket === (t.text === "]")) {
      const open = stack.pop()!.k;
      let p = open - 1;
      while (p >= 0 && /^\s$/.test(toks[p].text)) p--;
      groups.push({ open: toks[open].start, close: t.end, after: toks[p]?.text });
    }
  });
  const switchArgs = new Set<number>();
  const groupAt = new Map(groups.map((g) => [g.open, g]));
  const DIMENSION = /^\s*[-+]?(?:\d+\.?\d*|\.\d+)\s*[a-z]{2}/;
  // A structure's parameters written as bare tokens, not groups:
  // `\genfrac`'s delimiters, bar size and style (`\genfrac ( ] {0.8pt}{}`),
  // `\above`'s dimension (`\above1.0pt`).
  const bareParameters = (t: { start: number; end: number; text: string }, k: number) => {
    const bare = (from: number, to: number) => mark("bare-token parameter of a structure", from, to);
    if (t.text === "\\above") {
      const m = DIMENSION.exec(tex.slice(t.end));
      if (m) bare(t.end, t.end + m[0].length);
    } else if (t.text === "\\genfrac") {
      let at = k;
      for (let arg = 0; arg < 4; arg++) {
        at = next(at);
        if (at >= toks.length) return;
        const g = groupAt.get(toks[at].start);
        if (g) {
          while (at < toks.length && toks[at].end < g.close) at++;
        } else if (arg === 2 && DIMENSION.test(tex.slice(toks[at].start))) {
          const end = toks[at].start + DIMENSION.exec(tex.slice(toks[at].start))![0].length;
          bare(toks[at].start, end);
          while (at + 1 < toks.length && toks[at + 1].start < end) at++;
        } else bare(toks[at].start, toks[at].end);
      }
    }
  };
  toks.forEach((t, k) => {
    const n = next(k);
    if (DEFINE.test(t.text)) {
      // A definition draws nothing: its name, parameters and body.
      let end = t.end;
      let m = n;
      while (m < toks.length) {
        const g = groupAt.get(toks[m].start);
        if (g) end = g.close;
        else if (toks[m].text.startsWith("\\") || toks[m].text === "#" || /^\d$/.test(toks[m].text)) end = toks[m].end;
        else break;
        const body = g && tex[g.open] === "{" && (toks[m].text === "{" && !/^\{\s*\\[a-zA-Z@]+\s*\}$/.test(tex.slice(g.open, g.close)));
        while (m < toks.length && toks[m].start < end) m++;
        while (m < toks.length && /^\s$/.test(toks[m].text)) m++;
        if (body) break;
      }
      mark("a macro definition", t.start, end);
    } else if (t.text === "\\\\") mark("row break \\\\", t.start, t.end);
    else if (/^\\(begin|end)$/.test(t.text)) {
      // The environment name, braces included.
      const m = /^\s*\{[^}]*\}/.exec(tex.slice(t.end));
      mark("\\begin/\\end{name}", t.start, t.end + (m ? m[0].length : 0));
    } else if (/^\\(left|right|middle|[bB]igg?[lmr]?)$/.test(t.text) && inStructure(t.start)) {
      // The delimiter is drawn by the structure, not by a node of its own.
      mark("\\left/\\right/\\big and its delimiter", t.start, t.end);
      if (n < toks.length) mark("\\left/\\right/\\big and its delimiter", toks[n].start, toks[n].end);
    } else if (NO_GLYPH.test(t.text)) {
      mark("a command that draws nothing of its own", t.start, t.end);
      if (/color$/.test(t.text) && toks[n]?.text === "{") switchArgs.add(toks[n].start);
    } else if (t.text.length > 1 && t.text.startsWith("\\") && inStructure(t.start)) {
      mark("command name of a structure", t.start, t.end);
      bareParameters(t, k);
    } else if ("{}".includes(t.text)) mark("braces", t.start, t.end);
    else if ("^_".includes(t.text)) mark("^ and _", t.start, t.end);
    else if (t.text === "&") mark("& (cell separator)", t.start, t.end);
    else if (t.text === "$") mark("$", t.start, t.end);
  });
  // CD arrows: `@>a>b>`, `@AxAA`, `@=`, `@|`, `@.`; the labels are mapped.
  for (const cd of tex.matchAll(/\\begin\s*\{CD\}[\s\S]*?\\end\s*\{CD\}/g)) {
    for (const a of cd[0].matchAll(/@([<>AV])([^@]*?)\1([^@]*?)\1|@[=|.]/g)) {
      const at = cd.index! + a.index!;
      mark("CD arrow syntax", at, at + 2);
      if (a[1]) {
        const first = at + 2 + a[2].length;
        mark("CD arrow syntax", first, first + 1);
        mark("CD arrow syntax", first + 1 + a[3].length, first + 2 + a[3].length);
      }
    }
  }
  for (const g of groups) {
    if (switchArgs.has(g.open)) mark("a command that draws nothing of its own", g.open, g.close);
    else if (g.after === "\\\\" && tex[g.open] === "[") mark("row break \\\\", g.open, g.close);
    else if (
      FRAGMENT.test(g.after ?? "") &&
      tex[g.open] === "{" &&
      !inStructure(g.open) &&
      mapped.some((m) => g.open <= m.s && m.e <= g.close)
    ) {
      // `\mathbf{\hbox{…}}`, `\blue{x}`: a font over a body drawn as a
      // fragment, or a colour, has no element of its own.
      const cmd = toks.find((t) => t.text === g.after && t.end <= g.open && !tex.slice(t.end, g.open).trim());
      if (cmd) mark("command of a node drawn as a fragment", cmd.start, cmd.end);
    }
    else if (inStructure(g.open) && !mapped.some((m) => g.open <= m.s && m.e <= g.close)) {
      // A structure's parameter: a size, a colour, column spec, a delimiter
      // it draws itself, `\smash`'s [t].
      mark("parameter argument of a structure", g.open, g.close);
    } else if (tex[g.open] === "[" && inStructure(g.open)) {
      mark("optional-argument brackets of a structure", g.open, g.open + 1);
      mark("optional-argument brackets of a structure", g.close - 1, g.close);
    }
  }
  return (i) => reason[i];
}

export function checkFormula(tex: string, off: Answer, on: Answer): Outcome {
  const failures: Failure[] = [];
  const excluded = new Map<string, number>();
  const fail = (check: Failure["check"], key: string) => failures.push({ check, key });
  const kind = (a: Answer) => ("html" in a ? "renders" : "error" in a ? "error" : "panic");

  // a
  let errorMoved = false;
  if (kind(off) !== kind(on)) fail("a", `${kind(off)} → ${kind(on)}`);
  else if ("error" in off && "error" in on && off.error !== on.error) {
    if (errorKind(off.error) === errorKind(on.error)) errorMoved = true;
    else fail("a", `error kind: ${errorKind(off.error)} → ${errorKind(on.error)}`);
  }
  if (!("html" in on) || !("html" in off)) return { failures, identity: "n/a", excluded, errorMoved };

  const root = parseMarkup(on.html);
  const html = htmlPart(root);
  const len = tex.length;
  const mapped: Mapped[] = [];

  // b and c, in one walk.
  const walk = (n: Node, nearest: Mapped | undefined) => {
    if (typeof n === "string") {
      for (const ch of n) {
        if (/\s|​/.test(ch)) continue;
        if (!nearest) fail("c", `unmapped glyph ${/^[a-zA-Z]$/.test(ch) ? "letter" : /^\d$/.test(ch) ? "digit" : ch}`);
      }
      return;
    }
    let here = nearest;
    const ds = n.attrs.get("data-s");
    const de = n.attrs.get("data-e");
    if (ds !== undefined || de !== undefined) {
      if (!/^\d+$/.test(ds ?? "") || !/^\d+$/.test(de ?? "")) {
        fail("b", "malformed data-s/data-e");
      } else {
        const s = Number(ds);
        const e = Number(de);
        if (!(s <= e && e <= len)) fail("b", `out of bounds: ${kindOf(tex, Math.min(s, len), Math.min(e, len))}`);
        else if (nearest && !(nearest.s <= s && e <= nearest.e)) {
          fail("b", `${kindOf(tex, s, e)} not inside ${kindOf(tex, nearest.s, nearest.e)}`);
        }
        const m: Mapped = { el: n, s, e, parent: nearest, hasMappedChild: false };
        if (nearest) nearest.hasMappedChild = true;
        mapped.push(m);
        here = m;
      }
    }
    for (const c of n.children) walk(c, here);
  };
  walk(html, undefined);

  // d
  const leaf = new Uint8Array(len);
  const structure = new Uint8Array(len);
  for (const m of mapped) {
    if (m.e > len) continue;
    const arr = m.hasMappedChild ? structure : leaf;
    for (let i = m.s; i < m.e; i++) arr[i] = 1;
  }
  const why = syntax(tex, mapped, (i) => structure[i] === 1);
  const toks = tokens(tex);
  const tokenAt = new Int32Array(len);
  toks.forEach((t, k) => tokenAt.fill(k, t.start, t.end));
  const reported = new Set<number>();
  for (let i = 0; i < len; i++) {
    const ch = tex[i];
    if (/\s/.test(ch) || leaf[i]) continue;
    // A low surrogate is its high surrogate's character.
    if (ch >= "\udc00" && ch <= "\udfff") continue;
    const reason = why(i);
    if (reason) {
      excluded.set(reason, (excluded.get(reason) ?? 0) + 1);
      continue;
    }
    // One failure per token.
    if (reported.has(tokenAt[i])) continue;
    reported.add(tokenAt[i]);
    const tok = toks[tokenAt[i]];
    const label = /^[a-zA-Z]$/.test(tok.text) ? "letter" : /^\d$/.test(tok.text) ? "digit" : tok.text;
    fail("d", `${structure[i] ? "inside a structure" : "unmapped"}: ${label}`);
  }

  // e
  const offRoot = parseMarkup(off.html);
  const hasPlaceholder = !!find(root, (e) => hasClass(e, PLACEHOLDER));
  const normal = normalize(root);
  const result = compare(offRoot.children, normal.children);
  let identity: Outcome["identity"];
  if (result === "equal") identity = "equal";
  else if (result === "italic") identity = "italic";
  else if (hasPlaceholder) identity = "placeholder";
  else {
    identity = "differs";
    fail("e", result);
  }
  return { failures, identity, excluded, errorMoved };
}

/** A KaTeX error message without its position and context window. */
function errorKind(message: string): string {
  return message.replace(/ at (?:position \d+|end of input): [\s\S]*$/, "");
}

export type Answer = { html: string } | { error: string } | { panic: string };

/** Flag-on markup without the ranges and placeholders; a mapped glyph span
 *  left with no attributes is a bare glyph (a symbol with no class or
 *  style). */
function normalize(el: El): El {
  const children: Node[] = [];
  for (const c of el.children) {
    if (typeof c === "string") {
      push(children, c);
      continue;
    }
    if (hasClass(c, PLACEHOLDER)) continue;
    const n = normalize(c);
    n.attrs.delete("data-s");
    n.attrs.delete("data-e");
    if (c.attrs.has("data-s") && n.tag === "span" && n.attrs.size === 0 && glyphSpan(n)) {
      push(children, n.children[0] as string);
    } else children.push(n);
  }
  return { tag: el.tag, attrs: new Map(el.attrs), children };
}

function push(list: Node[], text: string) {
  const last = list[list.length - 1];
  if (typeof last === "string") list[list.length - 1] = last + text;
  else list.push(text);
}

const glyphSpan = (n: Node): n is El =>
  typeof n !== "string" && n.tag === "span" && n.children.length === 1 && typeof n.children[0] === "string";
const MARGIN = /margin-right:[^;]*;/;
const withoutMargin = (el: El) => (el.attrs.get("style") ?? "").replace(MARGIN, "");

/** Flag-off children `a` against normalised flag-on children `b`: "equal",
 *  "italic" (equal but for the split glyphs' own italic margins), or where
 *  they first differ. */
function compare(a: Node[], b: Node[]): string {
  let italic = false;
  let j = 0;
  for (let i = 0; i < a.length; i++) {
    const x = a[i];
    const y = b[j];
    if (y === undefined) return `missing ${describe(x)}`;
    if (typeof x === "string" || typeof y === "string") {
      if (x !== y) return `${describe(x)} ≠ ${describe(y)}`;
      j++;
      continue;
    }
    if (glyphSpan(x) && glyphSpan(y) && x.attrs.get("class") === y.attrs.get("class")) {
      // A merged run against its split glyphs.
      // The run's margin is its last glyph's; a split glyph keeps its own.
      const text = x.children[0] as string;
      let got = "";
      let k = j;
      while (k < b.length && got.length < text.length) {
        const z = b[k];
        if (!glyphSpan(z) || z.attrs.get("class") !== x.attrs.get("class")) break;
        if (withoutMargin(z) !== withoutMargin(x)) return `style of ${describe(z)}`;
        got += z.children[0] as string;
        const last = got.length >= text.length;
        if (last ? z.attrs.get("style") !== x.attrs.get("style") : MARGIN.test(z.attrs.get("style") ?? "")) italic = true;
        k++;
      }
      if (got !== text) return `${describe(x)} ≠ split ${JSON.stringify(got)}`;
      const otherAttrs = (e: El) => [...e.attrs].filter(([k2]) => k2 !== "style").join("|");
      if (otherAttrs(x) !== otherAttrs(b[k - 1] as El)) return `attributes of ${describe(x)}`;
      j = k;
      continue;
    }
    if (x.tag !== y.tag) return `<${x.tag}> ≠ <${y.tag}>`;
    const ax = [...x.attrs].join("|");
    const ay = [...y.attrs].join("|");
    if (ax !== ay) return `attributes of ${describe(x)}`;
    const inner = compare(x.children, y.children);
    if (inner === "italic") italic = true;
    else if (inner !== "equal") return inner;
    j++;
  }
  if (j < b.length) return `extra ${describe(b[j])}`;
  return italic ? "italic" : "equal";
}

function describe(n: Node): string {
  if (typeof n === "string") return "text";
  return `<${n.tag} class="${n.attrs.get("class") ?? ""}">`;
}
