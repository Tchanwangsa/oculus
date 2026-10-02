/** Source-preserving GFM table transformations, independent of the editor DOM. */
export type Align = "left" | "center" | "right" | null;

/** One cell of a source line, as offsets into the line: its content `from..to`
 *  (-1 when empty) and the gap between its pipes. */
interface SourceCell {
  from: number;
  to: number;
  gapFrom: number;
  gapTo: number;
}

interface SourceRow {
  /** Line start, relative to the table's start. */
  at: number;
  text: string;
  cells: SourceCell[];
  /** The line ends with a pipe. */
  closed: boolean;
}

export interface TableLayout {
  align: Align[];
  /** The header, then the body rows. */
  rows: SourceRow[];
  delimiter: SourceRow;
  length: number;
}

const PIPE = 124;
const BACKSLASH = 92;

function splitRow(text: string, at: number): SourceRow {
  const cells: SourceCell[] = [];
  let gapFrom = 0;
  let start = -1;
  let end = -1;
  let esc = false;
  let first = true;
  for (let i = 0; i < text.length; i++) {
    const ch = text.charCodeAt(i);
    if (ch === PIPE && !esc) {
      // Whitespace before a leading pipe is not a cell.
      if (!first || start > -1) cells.push({ from: start, to: end, gapFrom, gapTo: i });
      first = false;
      gapFrom = i + 1;
      start = end = -1;
    } else if (esc || (ch !== 32 && ch !== 9)) {
      if (start < 0) start = i;
      end = i + 1;
    }
    esc = !esc && ch === BACKSLASH;
  }
  if (start > -1) cells.push({ from: start, to: end, gapFrom, gapTo: text.length });
  return { at, text, cells, closed: start < 0 && !first };
}

function alignOf(spec: string): Align {
  const left = spec.startsWith(":");
  const right = spec.endsWith(":");
  return left && right ? "center" : right ? "right" : left ? "left" : null;
}

/** A table's source split into rows and cells, or null if it has no
 *  delimiter row. */
export function parseTable(source: string): TableLayout | null {
  const rows: SourceRow[] = [];
  let at = 0;
  for (const text of source.split("\n")) {
    rows.push(splitRow(text, at));
    at += text.length + 1;
  }
  if (rows.length < 2) return null;
  const [header, delimiter, ...body] = rows;
  const align = header.cells.map((_, i) => {
    const cell = delimiter.cells[i];
    return cell && cell.from >= 0 ? alignOf(delimiter.text.slice(cell.from, cell.to)) : null;
  });
  if (!align.length) return null;
  return { align, rows: [header, ...body], delimiter, length: source.length };
}

/** What cell `c` of row `r` shows: its source with `\|` unescaped. A row
 *  short of cells is padded; extra cells are ignored, as GFM does. */
export function cellText(layout: TableLayout, r: number, c: number): string {
  const row = layout.rows[r];
  const cell = row.cells[c];
  if (!cell || cell.from < 0) return "";
  return row.text.slice(cell.from, cell.to).replace(/\\\|/g, "|");
}

/** Escape each pipe not already escaped. A trailing odd backslash run would
 *  escape the closing pipe, so a space follows it. */
function escapeCell(value: string): string {
  let out = value.replace(/(\\*)\|/g, (m, slashes: string) => (slashes.length % 2 ? m : `${slashes}\\|`));
  const tail = /\\+$/.exec(out);
  if (tail && tail[0].length % 2) out += " ";
  return out;
}

export interface Insert {
  from: number;
  to: number;
  insert: string;
}

/** Append cell `c` to a row that has fewer cells, padding the gap. */
export function appendCell(row: SourceRow, lineFrom: number, c: number, value: string): Insert | null {
  const n = row.cells.length;
  const pad = Math.max(0, c - n);
  if (row.closed) {
    const pipe = n ? row.cells[n - 1].gapTo : row.text.lastIndexOf("|");
    const at = lineFrom + pipe + 1;
    return { from: at, to: at, insert: "   |".repeat(pad) + (value ? ` ${value} |` : "   |") };
  }
  // Without a closing pipe an empty last cell would not count; GFM pads it.
  if (!value || !n) return null;
  const at = lineFrom + row.cells[n - 1].to;
  return { from: at, to: at, insert: " |".repeat(pad + 1) + ` ${value}` };
}

/** The change that makes cell `c` of row `r` read `value`, or null. */
export function cellChange(layout: TableLayout, base: number, r: number, c: number, value: string): Insert | null {
  const row = layout.rows[r];
  const lineFrom = base + row.at;
  const text = escapeCell(value);
  const cell = row.cells[c];
  if (!cell) return text ? appendCell(row, lineFrom, c, text) : null;
  if (cell.from < 0) {
    return text ? { from: lineFrom + cell.gapFrom, to: lineFrom + cell.gapTo, insert: ` ${text} ` } : null;
  }
  if (row.text.slice(cell.from, cell.to) === text) return null;
  // Emptying a cell with no pipe on its outer side would drop the cell and
  // shift the row; a pipe keeps it.
  const outer = cell.gapFrom === 0 || (cell.gapTo === row.text.length && !row.closed);
  return { from: lineFrom + cell.from, to: lineFrom + cell.to, insert: text || (outer ? "|" : "") };
}

/** The change that drops cells `c0..c1` from a row, or null when it has none
 *  of them. The row keeps a pipe, which lezer needs to read it as a row. */
export function removeCells(row: SourceRow, lineFrom: number, c0: number, c1: number): Insert | null {
  const { cells, text } = row;
  if (c0 >= cells.length) return null;
  const last = cells[Math.min(c1, cells.length - 1)];
  const bare = c0 === 0 && cells[0].gapFrom === 0;
  if (last.gapTo < text.length) {
    // Each cell goes with the pipe after it; on a row with no leading pipe
    // the last of those stays to lead it.
    return { from: lineFrom + cells[c0].gapFrom, to: lineFrom + last.gapTo + (bare ? 0 : 1), insert: "" };
  }
  return { from: lineFrom + cells[c0].gapFrom, to: lineFrom + text.length, insert: bare ? "|" : "" };
}

/** Index `from` moved to sit before index `to` of the original order. */
function reorder(n: number, from: number, to: number): number[] {
  const order = Array.from({ length: n }, (_, i) => i);
  order.splice(from, 1);
  order.splice(to > from ? to - 1 : to, 0, from);
  return order;
}

/** The table's source with body row `from` moved before row `to`. Lines move
 *  whole, so every row stays byte-identical. */
export function movedRows(layout: TableLayout, from: number, to: number): string {
  const order = reorder(layout.rows.length, from, to);
  const [header, ...body] = order.map((i) => layout.rows[i].text);
  return [header, layout.delimiter.text, ...body].join("\n");
}

/** The table's source with column `from` moved before column `to`. Each row
 *  is rewritten with outer pipes, so a short or pipe-less row keeps every
 *  cell in its column; cells past the header's count stay at the end. */
export function movedColumns(layout: TableLayout, from: number, to: number): string {
  const order = reorder(layout.align.length, from, to);
  const line = (row: SourceRow, blank: string) => {
    const raw = (i: number) => {
      const cell = row.cells[i];
      return cell && cell.from >= 0 ? row.text.slice(cell.from, cell.to) : blank;
    };
    const extra = row.cells.slice(order.length).map((_, i) => raw(order.length + i));
    return `| ${[...order.map(raw), ...extra].join(" | ")} |`;
  };
  const [header, ...body] = layout.rows.map((row) => line(row, ""));
  return [header, line(layout.delimiter, "---"), ...body].join("\n");
}
