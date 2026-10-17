import { escapeCell, type Align } from "./tableModel";

/** A table as rows of cell texts, the shape spreadsheets and web pages trade. */
export type Grid = string[][];

const unquote = (cell: string) =>
  cell.length > 1 && cell.startsWith('"') && cell.endsWith('"') ? cell.slice(1, -1).replace(/""/g, '"') : cell;

/** Tab-separated rows as a grid, or null when `text` is not a grid. `strict`
 *  (pasting into prose) wants several rows with one tab count each, so
 *  indented code and lists stay text; otherwise any tab or line break
 *  makes a grid. */
export function gridFromText(text: string, strict: boolean): Grid | null {
  const lines = text.replace(/\r\n?/g, "\n").replace(/\n+$/, "").split("\n");
  if (!lines[0] && lines.length === 1) return null;
  const rows = lines.map((line) => line.split("\t").map((cell) => unquote(cell.trim())));
  if (strict) {
    if (rows.length < 2 || rows[0].length < 2) return null;
    if (rows.some((row) => row.length !== rows[0].length)) return null;
    return rows;
  }
  return rows.length > 1 || rows[0].length > 1 ? rows : null;
}

/** The only table of an HTML clipboard flavour, or null. `strict` also wants
 *  nothing else copied with it (a page's text around the table). */
export function gridFromHtml(html: string, strict: boolean): Grid | null {
  if (!/<table[\s>]/i.test(html)) return null;
  const doc = new DOMParser().parseFromString(html, "text/html");
  const tables = doc.querySelectorAll("table");
  if (tables.length !== 1) return null;
  const flat = (s: string | null) => (s ?? "").replace(/\s+/g, " ").trim();
  const table = tables[0];
  if (strict && flat(doc.body.textContent).length > flat(table.textContent).length + 8) return null;
  const grid: Grid = [];
  for (const tr of table.querySelectorAll("tr")) {
    const row: string[] = [];
    for (const cell of tr.querySelectorAll("th, td")) {
      row.push(flat(cell.textContent));
      for (let i = 1; i < (Number((cell as HTMLTableCellElement).colSpan) || 1); i++) row.push("");
    }
    if (row.length) grid.push(row);
  }
  return grid.length ? grid : null;
}

/** The grid on the clipboard: an HTML table, else tab-separated text. */
export function clipboardGrid(data: DataTransfer | null, strict: boolean): Grid | null {
  if (!data) return null;
  const html = data.getData("text/html");
  const fromHtml = html ? gridFromHtml(html, strict) : null;
  if (fromHtml && (!strict || fromHtml.length > 1 || fromHtml[0].length > 1)) return fromHtml;
  const text = data.getData("text/plain");
  return text ? gridFromText(text, strict) : null;
}

const width = (grid: Grid) => grid.reduce((n, row) => Math.max(n, row.length), 0);

const line = (cells: string[]) => `| ${cells.join(" | ")} |`;

function delimiter(align: (Align | undefined)[], cols: number): string {
  return line(
    Array.from({ length: cols }, (_, c) => {
      const a = align[c];
      return a === "center" ? ":---:" : a === "right" ? "---:" : a === "left" ? ":---" : "---";
    }),
  );
}

/** A GFM pipe table of `grid`, its first row the header; short rows are
 *  padded and pipes escaped. */
export function gridToMarkdown(grid: Grid, align: (Align | undefined)[] = []): string {
  const cols = width(grid);
  const cells = (row: string[]) =>
    Array.from({ length: cols }, (_, c) => escapeCell((row[c] ?? "").replace(/\s*\n\s*/g, " ").trim()));
  const [header, ...body] = grid;
  return [line(cells(header)), delimiter(align, cols), ...body.map((row) => line(cells(row)))].join("\n");
}

export function gridToText(grid: Grid): string {
  return grid.map((row) => row.join("\t")).join("\n");
}

const escapeHtml = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

export function gridToHtml(grid: Grid): string {
  const rows = grid.map((row) => `<tr>${row.map((cell) => `<td>${escapeHtml(cell)}</td>`).join("")}</tr>`);
  return `<table><tbody>${rows.join("")}</tbody></table>`;
}
