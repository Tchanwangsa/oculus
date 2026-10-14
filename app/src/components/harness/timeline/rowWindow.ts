import type { ViewRow } from "./rows";

/**
 * Which rows `Timeline` mounts: from the row holding item `from` to the end,
 * or every row when `from` is null. An item id, not a row index: rows fold
 * into bundles and append, and an index would drift with both. Matched by
 * membership, not `>=`, because a live row without a stored id has a
 * negative one.
 */

/** Rows mounted on open, and added per load as the reader scrolls up. */
export const PAGE = 30;

/** null mounts every row. */
export type From = number | null;

export function firstId(row: ViewRow): number {
  return row.kind === "bundle" ? row.items[0].id : row.item.id;
}

/** The row holding item `id`, a bundle included whole; -1 when gone. */
function rowOf(rows: ViewRow[], id: number): number {
  return rows.findIndex((r) => (r.kind === "bundle" ? r.items.some((i) => i.id === id) : r.item.id === id));
}

/** The newest `PAGE` rows, or every row when there are no more. */
export function initialFrom(rows: ViewRow[]): From {
  return rows.length <= PAGE ? null : firstId(rows[rows.length - PAGE]);
}

/** The boundary to keep: set once the thread has rows, and set again when a
 *  rewind or edit took its item. undefined while there are no rows. */
export function settleFrom(rows: ViewRow[], from: From | undefined): From | undefined {
  if (!rows.length) return from;
  if (from === undefined) return initialFrom(rows);
  if (from !== null && rowOf(rows, from) < 0) return initialFrom(rows);
  return from;
}

/** Index of the first mounted row. */
export function windowStart(rows: ViewRow[], from: From | undefined): number {
  if (from == null) return 0;
  const i = rowOf(rows, from);
  return i < 0 ? Math.max(0, rows.length - PAGE) : i;
}

/** One page further back. */
export function pageEarlier(rows: ViewRow[], from: From): From {
  const start = windowStart(rows, from) - PAGE;
  return start <= 0 ? null : firstId(rows[start]);
}

/** Far enough back to mount item `id`; never later than `from`. */
export function includeId(rows: ViewRow[], from: From, id: number): From {
  const i = rowOf(rows, id);
  if (i < 0 || i >= windowStart(rows, from)) return from;
  return i === 0 ? null : firstId(rows[i]);
}
