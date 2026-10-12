// Code: source code on a rounded panel, coloured by a small per-language
// tokenizer, every character on a fixed column grid. Every line is keyed
// `<id>/l<key>` (key = the line's index, or `keys[i]`), so a Steps between
// two versions of the code slides the lines that persist instead of
// flashing them; its number is `<id>/l<key>/g<number>` and each coloured
// run `<id>/l<key>/<text>#<nth>`, so a changed run cross-fades and a
// renumbered line swaps its digits as it slides. Other keys: `<id>/panel`,
// `<id>/d<key>` (diff bands), `<id>/band`, `<id>/bar` (focus),
// `<id>/caret`, `<id>/n<i>` (notes), `<id>/trace`, `<id>/v<name>` (trace rows).
// Time-driven: focus (band + dimming), caret, reveal, notes, trace values
// and a diff all take plain times a scene passes from `word()`. Line indices
// are 0-based into `lines`. This is the one starter set in monospace.
// Characters outside the bundled fonts' coverage (CJK, emoji) render as boxes.
// Copy `tokenize`, `glide` and `Code` into a scene component to compose it.
export const props = {
  lines: list(str()),
  lang: oneOf(["python", "javascript", "c", "sql", "plain"]).default("plain"),
  // The line identities for a Steps (default: the index). Insert a line and
  // give the others their old keys: they slide down instead of flashing.
  keys: list(str()).default([]),
  numbers: bool().default(true),
  first: num().default(1),
  // Pins the code size (px on the 1920 frame); long code is still scaled down to fit.
  // Unset, short code grows up to 1.5 x the theme's size to fill the frame.
  fontSize: num().optional(),
  // Focus k: lines focusLine[k] to focusEnd[k] (default the same line) are lit
  // and the rest dimmed from focusAt[k]; -1 clears.
  focusLine: list(num()).default([]),
  focusEnd: list(num()).default([]),
  focusAt: list(num()).default([]),
  // The execution marker sits on caretLine[k] from caretAt[k] (ascending).
  caretLine: list(num()).default([]),
  caretAt: list(num()).default([]),
  // Lines enter one after another from revealAt, revealEach apart.
  revealAt: num().optional(),
  revealEach: num().default(0.2),
  // A note beside line noteLine[k] from noteAt[k].
  noteLine: list(num()).default([]),
  noteText: list(str()).default([]),
  noteAt: list(num()).default([]),
  // A trace table: variable vars[k] takes the value varVals[k] at varAt[k].
  vars: list(str()).default([]),
  varAt: list(num()).default([]),
  varVals: list(str()).default([]),
  // Diff from diffAt: added lines green, removed lines red.
  added: list(num()).default([]),
  removed: list(num()).default([]),
  diffAt: num().default(3),
  // Demo only: after afterAt the code is replaced by `after` (keys afterKeys).
  after: list(str()).default([]),
  afterKeys: list(str()).default([]),
  afterAt: num().default(3.6),
};

export const examples = {
  search: {
    lines: [
      "def binary_search(xs, target):",
      "    lo, hi = 0, len(xs) - 1",
      "    while lo <= hi:",
      "        mid = (lo + hi) // 2",
      "        if xs[mid] == target:",
      "            return mid",
      "        if xs[mid] < target:",
      "            lo = mid + 1",
      "        else:",
      "            hi = mid - 1",
      "    return -1",
    ],
    lang: "python",
    caretLine: [1, 2, 3, 4, 6, 7, 2, 3, 4, 5],
    caretAt: [0.5, 0.95, 1.4, 1.85, 2.3, 2.75, 3.2, 3.6, 4.0, 4.4],
    vars: ["lo", "hi", "mid", "lo", "mid"],
    varAt: [0.5, 0.5, 1.4, 2.75, 3.6],
    varVals: ["0", "6", "3", "4", "5"],
    noteLine: [7, 5],
    noteText: ["drop the left half", "found at index 5"],
    noteAt: [2.75, 4.4],
  },
  query: {
    lines: [
      "SELECT d.name, COUNT(*) AS staff, AVG(e.salary) AS pay",
      "FROM employees e",
      "JOIN departments d ON d.id = e.dept_id",
      "WHERE e.hired >= '2020-01-01'",
      "GROUP BY d.name",
      "HAVING COUNT(*) > 5",
      "ORDER BY pay DESC;",
    ],
    lang: "sql",
    focusLine: [1, 3, 4, 5, 0, 6],
    focusEnd: [2, 3, 4, 5, 0, 6],
    focusAt: [0.6, 1.5, 2.4, 3.2, 4.0, 4.8],
    noteLine: [2, 3, 4, 5, 0, 6],
    noteText: ["pair each row with its department", "keep recent hires", "one group per department", "keep groups of 6 or more", "only now compute the columns", "sort the result"],
    noteAt: [0.6, 1.5, 2.4, 3.2, 4.0, 4.8],
  },
  fix: {
    lines: [
      "function lastBelow(xs, max) {",
      "  const keep = xs.filter((x) => x !== null && x <= max);",
      "  for (let i = keep.length; i >= 0; i--) {",
      "  for (let i = keep.length - 1; i >= 0; i--) {",
      "    if (keep[i] !== undefined) return keep[i];",
      "  }",
      "  return null;",
      "}",
    ],
    lang: "javascript",
    revealAt: 0.3,
    revealEach: 0.12,
    added: [3],
    removed: [2],
    diffAt: 1.8,
    after: [
      "function lastBelow(xs, max) {",
      "  const keep = xs.filter((x) => x !== null && x <= max);",
      "  for (let i = keep.length - 1; i >= 0; i--) {",
      "    if (keep[i] !== undefined) return keep[i];",
      "  }",
      "  return null;",
      "}",
    ],
    afterKeys: ["0", "1", "3", "4", "5", "6", "7"],
    afterAt: 3.6,
  },
};

// JetBrains Mono's advance is 0.6 em, so a column is a fixed number of px.
const ADVANCE = 0.6;

// Per language: keyword list, line comment, [open, close, kind] blocks that
// can run over several lines, quote characters (charQuote: a single quote
// starts a char literal only as 'x' or '\\x'; else a lifetime). Honest, not a parser.
const LANGS = {
  python: {
    kw: "and as assert async await break class continue def del elif else except finally for from global if import in is lambda None nonlocal not or pass raise return try while with yield True False",
    line: "#",
    blocks: [['"""', '"""', "str"], ["'''", "'''", "str"]],
    quotes: "\"'",
  },
  javascript: {
    kw: "async await break case catch class const continue default delete do else export extends finally for from function if import in instanceof let new of return static super switch this throw try typeof var void while yield null undefined true false interface type enum implements public private readonly",
    line: "//",
    blocks: [["/*", "*/", "com"]],
    quotes: "\"'`",
  },
  c: {
    kw: "auto bool break case catch char class const continue default defer do double else enum extern false final float fn for func go goto if impl import in inline int let long loop match mod mut new null NULL package pub public private range return self short signed sizeof static struct super switch this throw true try type typedef union unsigned use var void volatile where while",
    line: "//",
    blocks: [["/*", "*/", "com"]],
    quotes: "\"'",
    charQuote: true,
    directive: true,
  },
  sql: {
    kw: "select from where group by order having limit offset join left right inner outer full cross on as and or not in is null like between distinct count sum avg min max case when then else end insert into values update set delete create table drop alter union all asc desc with over partition exists",
    line: "--",
    blocks: [["/*", "*/", "com"]],
    quotes: "'\"",
    nocase: true,
    plainQuotes: true,
  },
  plain: { kw: "", quotes: "", blocks: [] },
};

/**
 * Colours each line of `lines` for `lang`: one array per line of runs
 * `{ col, text, kind }`, kind one of kw, str, num, com, id, op. Runs of one
 * kind a single space apart merge into one run; leading whitespace stays
 * out of the runs and is carried by `col`. Tabs count as four columns.
 */
const tokenize = (lines, lang) => {
  const L = LANGS[lang] ?? LANGS.plain;
  const words = new Set(L.kw.split(" ").filter(Boolean));
  let open = null;
  return lines.map((raw) => {
    const s = raw.replace(/\t/g, "    ").trimEnd();
    const out = [];
    const push = (from, to, kind) => {
      const last = out[out.length - 1];
      if (last && last.kind === kind && s.slice(last.col + last.text.length, from).length <= 1) {
        last.text = s.slice(last.col, to);
      } else {
        out.push({ col: from, text: s.slice(from, to), kind });
      }
    };
    let i = 0;
    while (i < s.length) {
      if (open) {
        let j = s.indexOf(open.close, i);
        while (open.esc && j > 0 && s[j - 1] === "\\") j = s.indexOf(open.close, j + 1);
        const to = j < 0 ? s.length : j + open.close.length;
        push(i, to, open.kind);
        i = to;
        if (j >= 0) open = null;
        continue;
      }
      const c = s[i];
      if (c === " ") {
        i++;
        continue;
      }
      if (L.line && s.startsWith(L.line, i)) {
        push(i, s.length, "com");
        break;
      }
      const block = L.blocks.find(([o]) => s.startsWith(o, i));
      if (block) {
        open = { close: block[1], kind: block[2] };
        push(i, i + block[0].length, block[2]);
        i += block[0].length;
        continue;
      }
      const directive = L.directive && c === "#" ? /^#\s*\w+/.exec(s.slice(i)) : null;
      if (directive) {
        push(i, i + directive[0].length, "kw");
        i += directive[0].length;
        continue;
      }
      if (L.quotes.includes(c)) {
        if (L.charQuote && c === "'") {
          const ch = /^'(\\.[^']*|[^\\'])'/.exec(s.slice(i));
          if (ch) {
            push(i, i + ch[0].length, "str");
            i += ch[0].length;
            continue;
          }
        } else {
          let j = i + 1;
          while (j < s.length && s[j] !== c) j += s[j] === "\\" && !L.plainQuotes ? 2 : 1;
          const to = Math.min(j + 1, s.length);
          push(i, to, "str");
          i = to;
          // A template literal left open carries on into the next line.
          if (c === "`" && j >= s.length) open = { close: "`", kind: "str", esc: true };
          continue;
        }
      }
      const num = /^(0[xX][\da-fA-F_]+|\d[\d_]*(\.\d+)?([eE][+-]?\d+)?)/.exec(s.slice(i));
      if (num) {
        push(i, i + num[0].length, "num");
        i += num[0].length;
        continue;
      }
      const id = /^[A-Za-z_$][\w$]*/.exec(s.slice(i));
      if (id) {
        const w = L.nocase ? id[0].toLowerCase() : id[0];
        push(i, i + id[0].length, words.has(w) ? "kw" : "id");
        i += id[0].length;
        continue;
      }
      push(i, i + 1, lang === "plain" ? "id" : "op");
      i++;
    }
    return out;
  });
};

/**
 * A value that is `vals[0]` before `at[0]` and from each `at[k]` eases
 * to `vals[k + 1]` over `dur` (cut short by the next cue, so cues chain).
 */
const glide = (t, at, vals, dur) => {
  let k = -1;
  for (let j = 0; j < at.length; j++) if (t >= at[j]) k = j;
  if (k < 0) return vals[0];
  const span = Math.max(Math.min(dur, (at[k + 1] ?? Infinity) - at[k]), 0.001);
  return lerp(vals[k], vals[k + 1], easeInOutCubic(clamp((t - at[k]) / span, 0, 1)));
};

/**
 * `lines` of source on a panel sized to the longest line. Options (all
 * optional): `lang`, `keys`, `numbers`, `first`, `fontSize`, `id`;
 * `focus: [{ from, to, at }]` (from -1 clears); `caret: [{ line, at }]`;
 * `reveal: { at, each }`; `notes: [{ line, text, at }]`;
 * `trace: [{ name, value, at }]`; `added`/`removed` line indices with
 * `diffAt`; `marks` keeps the +/- gutter without bands, so a Steps from a
 * diff to the clean code does not resize the panel. The font shrinks (or, with no `fontSize`, grows up to 1.5 x
 * the theme's) so the panel (with its notes and trace) fits
 * `maxWidth` x `maxHeight` (default: the frame less `space.xl` all round)
 * instead of clipping, so a column is always `0.6 * fontSize` px.
 */
const Code = (
  { lines, lang = "plain", keys = [], numbers = true, first = 1, fontSize, id = "code", focus = [], caret = [], reveal, notes = [], trace = [], added = [], removed = [], diffAt = 0, marks = false, maxWidth, maxHeight },
  { t, theme, size: frame },
) => {
  const { color, type, space, motion } = theme;
  const base = fontSize ?? type.code.fontSize;
  const runs = tokenize(lines, lang);
  const cols = Math.max(24, ...lines.map((l) => l.replace(/\t/g, "    ").trimEnd().length));
  const digits = String(first + lines.length - 1).length;
  const names = [...new Set(trace.map((e) => e.name))];
  const diff = marks || added.length + removed.length > 0;
  const longest = (xs) => Math.max(0, ...xs.map((x) => x.length));

  // Every length is a multiple of the code size, so one measure at the
  // wanted size says by how much the whole panel must shrink to fit.
  const measure = (fs) => {
    const r = fs / type.code.fontSize;
    const sp = (k) => space[k] * r;
    const cw = fs * ADVANCE;
    const lh = fs * type.code.lineHeight;
    const pad = sp("l");
    const small = fs * 0.85 * (type.body.fontSize / type.code.fontSize);
    const codeX = pad + (numbers ? digits * cw + sp("m") : 0) + (diff ? 2 * cw : 0);
    const noteX = codeX + cols * cw + sp("l");
    const noteW = notes.length ? fs * 0.12 + sp("s") + longest(notes.map((n) => n.text)) * small * 0.9 * 0.56 : 0;
    const panelW = (notes.length ? noteX + noteW : codeX + cols * cw) + pad;
    const panelH = 2 * pad + lines.length * lh;
    const tpad = pad * 0.8;
    const nameW = longest(names) * small * 0.58;
    const valW = longest(trace.map((e) => String(e.value))) * small * 0.62;
    const traceW = names.length ? 2 * tpad + nameW + sp("m") + valW : 0;
    const traceH = names.length ? 2 * tpad + names.length * lh : 0;
    const gap = names.length ? sp("l") : 0;
    return { fs, r, sp, cw, lh, pad, small, codeX, noteX, noteW, panelW, panelH, tpad, nameW, valW, traceW, traceH, gap, w: panelW + gap + traceW, h: Math.max(panelH, traceH) };
  };
  const fit = measure(base);
  const k = Math.min(fontSize === undefined ? 1.5 : 1, (maxWidth ?? frame.width - 2 * space.xl) / fit.w, (maxHeight ?? frame.height - 2 * space.xl) / fit.h);
  const m = k === 1 ? fit : measure(base * k);
  const { fs, sp, cw, lh, pad, small, codeX, noteX, panelW, panelH } = m;

  const ink = {
    kw: color.accent,
    str: color.positive,
    num: mix(color.warning, color.text, 0.35),
    com: color.muted,
    op: mix(color.text, color.muted, 0.5),
    id: color.text,
  };
  const code = { ...type.code, fontSize: fs };
  // Bands stop at the panel's border instead of painting over it.
  const bw = Math.max(1, fs / 14);
  const body = { ...type.body, fontSize: small };
  const at = (j) => pad + j * lh;
  const key = (i) => keys[i] ?? i;
  const dur = motion.duration.base;
  const ease = (from, over = dur) => easeOutCubic(clamp((t - from) / over, 0, 1));

  // Focus: a window of lines [a, b] that slides between cues and a weight w
  // that fades it in and out; each line is lit by how much of it the window covers.
  const lit = focus.filter((f) => f.from >= 0);
  let fa = lit.length ? lit[0].from : 0;
  let fb = lit.length ? (lit[0].to ?? lit[0].from) : 0;
  const A = [fa];
  const B = [fb];
  const W = [0];
  for (const f of focus) {
    if (f.from >= 0) {
      fa = f.from;
      fb = f.to ?? f.from;
    }
    A.push(fa);
    B.push(fb);
    W.push(f.from >= 0 ? 1 : 0);
  }
  const fat = focus.map((f) => f.at);
  const wa = glide(t, fat, A, dur);
  const wb = glide(t, fat, B, dur);
  const ww = lit.length ? glide(t, fat, W, dur) : 0;
  const dim = 0.4;
  const shown = (i) => 1 - ww * (1 - clamp(Math.min(i + 1, wb + 1) - Math.max(i, wa), 0, 1)) * (1 - dim);

  // The caret rides between line slots; the number it is on turns accent.
  const cat = caret.map((c) => c.at);
  const cpos = caret.length ? glide(t, cat, [caret[0].line, ...caret.map((c) => c.line)], dur) : 0;
  const cshow = caret.length ? ease(caret[0].at) : 0;

  const reveal1 = (i) => (reveal ? ease(reveal.at + i * reveal.each) : 1);
  const dk = ease(diffAt);

  // The number sits inside its line so it slides with the text; keyed by
  // its digits, a renumbered line cross-fades them instead of swapping halves.
  const lineBox = (i) => {
    const r = reveal1(i);
    const nth = {};
    return (
      <Box key={`${id}/l${key(i)}`} style={{ position: "absolute", left: 0, top: at(i), width: panelW, height: lh, opacity: shown(i) * r, translateY: (1 - r) * lh * 0.4 }}>
        {numbers ? (
          <Text
            key={`${id}/l${key(i)}/g${first + i}`}
            style={{ ...code, color: mix(color.faint, color.accent, cshow * clamp(1 - Math.abs(cpos - i), 0, 1)), position: "absolute", left: pad, top: 0, width: digits * cw, textAlign: "right" }}
          >
            {String(first + i)}
          </Text>
        ) : null}
        {runs[i].map((run) => {
          const n = (nth[run.text] = (nth[run.text] ?? -1) + 1);
          return (
            <Text key={`${id}/l${key(i)}/${run.text}#${n}`} style={{ ...code, color: ink[run.kind], position: "absolute", left: codeX + run.col * cw, top: 0 }}>
              {run.text}
            </Text>
          );
        })}
      </Box>
    );
  };

  const diffBand = (i, tone, mark) => (
    <Box key={`${id}/d${key(i)}`} style={{ position: "absolute", left: bw, top: at(i), width: panelW - 2 * bw, height: lh, fill: mix(color.surface, tone, 0.16 * dk) }}>
      <Text style={{ ...code, color: tone, opacity: dk, position: "absolute", left: codeX - 2 * cw, top: 0 }}>{mark}</Text>
    </Box>
  );

  // Notes sit beside their line; a later note on the same line replaces it.
  const noteBox = (n, j) => {
    const next = notes.filter((o, p) => o.line === n.line && p !== j && o.at > n.at).map((o) => o.at);
    const o = ease(n.at) * (1 - (next.length ? ease(Math.min(...next)) : 0));
    return (
      <Box key={`${id}/n${j}`} style={{ position: "absolute", left: noteX, top: at(n.line), height: lh, flexDirection: "row", alignItems: "center", gap: sp("s"), opacity: o, translateX: (o - 1) * sp("s") }}>
        <Box style={{ width: fs * 0.12, height: lh * 0.55, radius: sp("xxs"), fill: color.accent }} />
        <Text style={{ ...body, color: color.muted }}>{n.text}</Text>
      </Box>
    );
  };

  // Each trace row shows the latest value at t; a change slides the old
  // value out upward and the new one in from below, flashing accent.
  const traceRow = (name) => {
    const ev = trace.filter((e) => e.name === name).sort((p, q) => p.at - q.at);
    let j = -1;
    for (let p = 0; p < ev.length; p++) if (t >= ev[p].at) j = p;
    const cur = j >= 0 ? ev[j] : null;
    const prev = j > 0 ? ev[j - 1] : null;
    const c = cur ? ease(cur.at) : 1;
    const value = (e, o, dy, ink) => (
      <Text style={{ ...body, fontVariantNumeric: "tabular-nums", color: ink, position: "absolute", left: 0, top: (lh - body.fontSize * body.lineHeight) / 2, opacity: o, translateY: dy }}>{String(e.value)}</Text>
    );
    return (
      <Row key={`${id}/v${name}`} style={{ height: lh, alignItems: "center", gap: sp("m"), opacity: ease(ev[0].at) }}>
        <Text style={{ ...body, color: color.muted, width: m.nameW }}>{name}</Text>
        <Box style={{ width: m.valW, height: lh }}>
          {prev && c < 1 ? value(prev, 1 - c, -c * lh * 0.3, color.text) : null}
          {cur ? value(cur, c, (1 - c) * lh * 0.3, mix(color.accent, color.text, c)) : null}
        </Box>
      </Row>
    );
  };

  const panel = (
    <Box
      key={`${id}/panel`}
      style={{ position: "relative", width: panelW, height: panelH, radius: sp("m"), fill: color.surface, border: { width: bw, color: color.border }, shadow: { x: 0, y: sp("xs"), blur: sp("l"), spread: 0, color: color.shadow } }}
    >
      {ww > 0 ? <Box key={`${id}/band`} style={{ position: "absolute", left: bw, top: at(wa), width: panelW - 2 * bw, height: (wb - wa + 1) * lh, fill: mix(color.surface, color.accent, 0.12), opacity: ww }} /> : null}
      {removed.map((i) => diffBand(i, color.negative, "-"))}
      {added.map((i) => diffBand(i, color.positive, "+"))}
      {ww > 0 ? <Box key={`${id}/bar`} style={{ position: "absolute", left: bw, top: at(wa), width: fs * 0.14, height: (wb - wa + 1) * lh, fill: color.accent, opacity: ww }} /> : null}
      {lines.map((_, i) => lineBox(i))}
      {notes.map(noteBox)}
      {caret.length ? (
        <Path
          key={`${id}/caret`}
          d="M0 0 L10 6 L0 12 Z"
          viewBox={[0, 0, 10, 12]}
          fill={color.accent}
          style={{ position: "absolute", left: pad * 0.3, top: at(cpos) + (lh - lh * 0.42) / 2, width: lh * 0.35, height: lh * 0.42, opacity: cshow }}
        />
      ) : null}
    </Box>
  );
  const card = names.length ? (
    <Stack
      key={`${id}/trace`}
      style={{ width: m.traceW, height: m.traceH, padding: m.tpad, radius: sp("m"), fill: color.surface, border: { width: bw, color: color.border }, shadow: { x: 0, y: sp("xs"), blur: sp("l"), spread: 0, color: color.shadow } }}
    >
      {names.map(traceRow)}
    </Stack>
  ) : null;
  return (
    <Row gap={m.gap} style={{ width: m.w, height: m.h, alignItems: "flex-start" }}>
      {panel}
      {card}
    </Row>
  );
};

export default (p, { theme }) => {
  const { color, motion } = theme;
  const common = {
    lang: p.lang,
    numbers: p.numbers,
    first: p.first,
    fontSize: p.fontSize,
    focus: p.focusLine.map((from, i) => ({ from, to: p.focusEnd[i], at: p.focusAt[i] })),
    caret: p.caretLine.map((line, i) => ({ line, at: p.caretAt[i] })),
    reveal: p.revealAt === undefined ? undefined : { at: p.revealAt, each: p.revealEach },
    notes: p.noteLine.map((line, i) => ({ line, text: p.noteText[i], at: p.noteAt[i] })),
    trace: p.vars.map((name, i) => ({ name, value: p.varVals[i], at: p.varAt[i] })),
    id: "code",
  };
  // The diff's bands stay in the clean state as invisible ones (marks keeps
  // the gutter), so a band that survives glides with its line and fades out.
  const kept = p.added.map((i) => (p.keys[i] ?? i).toString()).map((k) => p.after.findIndex((_, j) => (p.afterKeys[j] ?? j).toString() === k));
  const state = (lines, keys, diff) => (
    <Code
      {...common}
      lines={lines}
      keys={keys}
      added={diff ? p.added : kept.filter((j) => j >= 0)}
      removed={diff ? p.removed : []}
      marks={!diff && p.added.length + p.removed.length > 0}
      diffAt={diff ? p.diffAt : Infinity}
    />
  );
  const stage = (child) => (
    <Stack justify="center" align="center" style={{ flexGrow: 1, fill: color.bg }}>
      {child}
    </Stack>
  );
  if (!p.after.length) return stage(state(p.lines, p.keys, true));
  return stage(
    <Steps at={[p.afterAt]} dur={motion.duration.slow}>
      {state(p.lines, p.keys, true)}
      {state(p.after, p.afterKeys, false)}
    </Steps>,
  );
};
