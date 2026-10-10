/** One run of text with the SGR styling that was active when it was printed. */
export interface AnsiSpan {
  text: string;
  className?: string;
  style?: { color: string };
}

// Static strings so Tailwind sees them; the dark pair is the lighter shade.
const FG: Record<number, string> = {
  30: "text-neutral-500",
  31: "text-red-600 dark:text-red-400",
  32: "text-green-600 dark:text-green-400",
  33: "text-yellow-600 dark:text-yellow-400",
  34: "text-blue-600 dark:text-blue-400",
  35: "text-purple-600 dark:text-purple-400",
  36: "text-cyan-600 dark:text-cyan-400",
  37: "text-neutral-500 dark:text-neutral-300",
};

// The 16 base colours for 38;5;n; the rest of the 256 palette is computed.
const BASE16 = [
  "#737373", "#dc2626", "#16a34a", "#ca8a04", "#2563eb", "#9333ea", "#0891b2", "#a3a3a3",
  "#525252", "#ef4444", "#22c55e", "#eab308", "#3b82f6", "#a855f7", "#06b6d4", "#e5e5e5",
];

function color256(n: number): string {
  if (n < 16) return BASE16[n];
  if (n >= 232) {
    const v = 8 + (n - 232) * 10;
    return `rgb(${v},${v},${v})`;
  }
  const i = n - 16;
  const level = (c: number) => (c === 0 ? 0 : 55 + c * 40);
  return `rgb(${level(Math.floor(i / 36))},${level(Math.floor(i / 6) % 6)},${level(i % 6)})`;
}

interface Sgr {
  fg?: number;
  rgb?: string;
  bold: boolean;
  dim: boolean;
  underline: boolean;
}

function applySgr(params: string, s: Sgr): Sgr {
  const codes = params === "" ? [0] : params.split(/[;:]/).map((c) => Number(c) || 0);
  let next = { ...s };
  for (let i = 0; i < codes.length; i++) {
    const c = codes[i];
    if (c === 0) next = { bold: false, dim: false, underline: false };
    else if (c === 1) next.bold = true;
    else if (c === 2) next.dim = true;
    else if (c === 4) next.underline = true;
    else if (c === 22) (next.bold = false), (next.dim = false);
    else if (c === 24) next.underline = false;
    else if (c === 39) (next.fg = undefined), (next.rgb = undefined);
    else if ((c >= 30 && c <= 37) || (c >= 90 && c <= 97)) {
      next.fg = c >= 90 ? c - 60 : c;
      next.rgb = undefined;
    } else if (c === 38 && codes[i + 1] === 5) {
      next.rgb = color256(codes[i + 2] ?? 0);
      next.fg = undefined;
      i += 2;
    } else if (c === 38 && codes[i + 1] === 2) {
      next.rgb = `rgb(${codes[i + 2] ?? 0},${codes[i + 3] ?? 0},${codes[i + 4] ?? 0})`;
      next.fg = undefined;
      i += 4;
    }
    // Backgrounds and the rest are ignored: the block keeps its own.
  }
  return next;
}

const classOf = (s: Sgr): string | undefined => {
  const parts = [s.fg ? FG[s.fg] : "", s.bold ? "font-semibold" : "", s.dim ? "opacity-60" : "", s.underline ? "underline" : ""];
  return parts.filter(Boolean).join(" ") || undefined;
};

/**
 * One line of terminal output as styled spans. Colours, bold, dim and
 * underline are kept; every other escape (cursor moves, hide/show cursor,
 * titles) is dropped. A carriage return is a redraw, so only what follows the
 * last one is shown.
 */
export function parseAnsi(line: string): AnsiSpan[] {
  const drawn = line.split("\r").filter((p) => p !== "").pop() ?? "";
  const spans: AnsiSpan[] = [];
  let sgr: Sgr = { bold: false, dim: false, underline: false };
  let text = "";
  const flush = () => {
    if (text) {
      spans.push({ text, className: classOf(sgr), style: sgr.rgb ? { color: sgr.rgb } : undefined });
      text = "";
    }
  };
  for (let i = 0; i < drawn.length; i++) {
    const ch = drawn[i];
    if (ch !== "\u001b") {
      text += ch;
      continue;
    }
    const kind = drawn[i + 1];
    if (kind === "[") {
      let j = i + 2;
      while (j < drawn.length && !/[@-~]/.test(drawn[j])) j++;
      if (drawn[j] === "m") {
        flush();
        sgr = applySgr(drawn.slice(i + 2, j), sgr);
      }
      i = j;
    } else if (kind === "]") {
      let j = i + 2;
      while (j < drawn.length && drawn[j] !== "\u0007" && drawn[j] !== "\u001b") j++;
      i = drawn[j] === "\u001b" ? j + 1 : j;
    } else {
      i += 1;
    }
  }
  flush();
  return spans;
}

/** Lines worth drawing: a line that held only escapes (a spinner's cursor
 *  hide) is no output, but a truly empty line is kept as a gap. */
export function visibleLines(lines: string[]): AnsiSpan[][] {
  const out: AnsiSpan[][] = [];
  for (const line of lines) {
    const spans = parseAnsi(line);
    if (spans.length === 0 && line !== "") continue;
    out.push(spans);
  }
  return out;
}
