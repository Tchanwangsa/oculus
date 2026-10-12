/** Front matter on scraped markdown (Ed threads and lessons). The sync writes
 * every value as a JSON literal, which is also valid YAML: `key: <json>` on one
 * line, or `key:` followed by `  - <json>` items. See docs/sync.md. */
export type FrontValue = string | number | boolean | null | FrontValue[] | { [key: string]: FrontValue };

export interface Frontmatter {
  data: Record<string, FrontValue>;
  body: string;
}

const parseValue = (raw: string): FrontValue | undefined => {
  try {
    return JSON.parse(raw) as FrontValue;
  } catch {
    return undefined;
  }
};

/** Split `text` into its front matter and body. Text without a front matter
 * block comes back whole with empty data — including a page that merely opens
 * with a `---` rule: every line of the block must be a key with a JSON value,
 * or a list item under one. */
export function splitFrontmatter(text: string): Frontmatter {
  const none = { data: {}, body: text };
  const match = /^---\r?\n([\s\S]*?)\r?\n---[ \t]*(?:\r?\n|$)/.exec(text);
  if (!match) return none;
  const data: Record<string, FrontValue> = {};
  let list: FrontValue[] | null = null;
  for (const line of match[1].split(/\r?\n/)) {
    if (!line.trim()) continue;
    const item = /^\s+-\s+(.*)$/.exec(line);
    const pair = item ? null : /^([A-Za-z0-9_]+):\s*(.*)$/.exec(line);
    if (item && list) {
      const value = parseValue(item[1].trim());
      if (value === undefined) return none;
      list.push(value);
    } else if (pair?.[2] === "") {
      list = [];
      data[pair[1]] = list;
    } else if (pair) {
      const value = parseValue(pair[2]);
      if (value === undefined) return none;
      list = null;
      data[pair[1]] = value;
    } else {
      return none;
    }
  }
  return Object.keys(data).length ? { data, body: text.slice(match[0].length) } : none;
}

/** The body of `text` with any front matter removed, for display. */
export const stripFrontmatter = (text: string): string => splitFrontmatter(text).body;

export const frontString = (value: FrontValue | undefined): string | null =>
  typeof value === "string" && value !== "" ? value : null;

export const frontNumber = (value: FrontValue | undefined): number | null =>
  typeof value === "number" && Number.isFinite(value) ? value : null;
