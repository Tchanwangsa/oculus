import type { Text } from "@codemirror/state";

const DELIMITER = /^\s*\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)*\|?\s*$/;
/** Lines that end a table in GFM, so one below it is not a row. */
const BLOCK_START = /^\s*(```|~~~|>|#{1,6}\s)/;

function cellCount(line: string): number {
  return line.trim().replace(/^\|/, "").replace(/(?<!\\)\|$/, "").split(/(?<!\\)\|/).length;
}

/** Whether `text` ends in a pipe table: a header over a delimiter row of the
 *  same width, then rows up to the last line. */
export function endsInTable(text: string): boolean {
  const lines = text.replace(/\n$/, "").split("\n");
  for (let i = lines.length - 1; i >= 1; i--) {
    const row = lines[i];
    if (!row.trim() || BLOCK_START.test(row)) return false;
    if (DELIMITER.test(row) && lines[i - 1].includes("|") && cellCount(lines[i - 1]) === cellCount(row)) return true;
  }
  return false;
}

/** What goes after `text` inserted up to `to`, so a table it ends in has a
 *  blank line under it: GFM reads the next line as one of its rows. */
export function tailPadding(doc: Text, to: number, text: string): string {
  if (!endsInTable(text)) return "";
  const newlines = /\n*$/.exec(text)![0].length;
  if (newlines >= 2) return "";
  const line = doc.lineAt(to);
  if (line.text.slice(to - line.from).trim()) return newlines ? "\n" : "\n\n";
  if (newlines) return "";
  return line.number < doc.lines && doc.line(line.number + 1).text.trim() ? "\n" : "";
}
