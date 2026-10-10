import { getDb, likeEscape } from "@/lib/db";
import { hostOf } from "@/lib/browser";
import { sqliteUtcToMs } from "@/lib/format/format";
import { historyUrl } from "./historyPolicy";

export interface HistoryEntry {
  url: string;
  host: string;
  title: string;
  visits: number;
  /** SQLite UTC, as `datetime('now')` writes it. */
  last_visit: string;
}

/** A day's worth of history, newest day first, newest entry first within it. */
export interface HistoryDay {
  /** Local midnight, epoch ms — what `fmtDayHeading` takes. */
  day: number;
  entries: HistoryEntry[];
}

/** Records a visit. The title usually arrives later, so an empty one never
 *  overwrites a recorded title. */
export async function recordVisit(raw: string, title: string): Promise<void> {
  const url = historyUrl(raw);
  if (!url) return;
  const host = hostOf(url);
  if (!host) return;
  const db = await getDb();
  await db.execute(
    `INSERT INTO browser_history (url, host, title)
     VALUES ($1, $2, $3)
     ON CONFLICT(url) DO UPDATE SET
       visits     = visits + 1,
       last_visit = datetime('now'),
       title      = CASE WHEN excluded.title <> '' THEN excluded.title ELSE title END`,
    [url, host, title],
  );
}

/** A late title for a recorded page — not a visit, so it does not count. */
export async function recordTitle(raw: string, title: string): Promise<void> {
  const url = historyUrl(raw);
  if (!title || !url) return;
  const db = await getDb();
  await db.execute(`UPDATE browser_history SET title = $2 WHERE url = $1`, [
    url,
    title,
  ]);
}

/** The ⌘K palette's rule: every typed word appears somewhere in url or
 *  title, any order, `-`/`_` read as spaces. One LIKE per word. */
function wordFilter(text: string): { sql: string; binds: string[] } {
  const words = text.split(/\s+/).filter(Boolean);
  const binds: string[] = [];
  const clauses = words.map((word, i) => {
    binds.push(`%${likeEscape(word).replace(/[-_]/g, " ").replace(/ /g, "%")}%`);
    const n = i + 1;
    return (
      `(REPLACE(REPLACE(url, '-', ' '), '_', ' ') LIKE $${n} ESCAPE '\\'` +
      ` OR REPLACE(REPLACE(title, '-', ' '), '_', ' ') LIKE $${n} ESCAPE '\\')`
    );
  });
  return { sql: clauses.join(" AND "), binds };
}

/** A visit's weight halves every week. */
const HALF_LIFE_DAYS = 7;

/** Frecency, boosted for a host that starts with what is typed. */
function score(entry: HistoryEntry, prefix: string, nowMs: number): number {
  const visited = sqliteUtcToMs(entry.last_visit) ?? nowMs;
  const ageDays = Math.max(0, (nowMs - visited) / 86_400_000);
  const frecency = entry.visits * Math.pow(2, -ageDays / HALF_LIFE_DAYS);
  const onPrefix =
    prefix !== "" && entry.host.replace(/^www\./, "").startsWith(prefix);
  return frecency * (onPrefix ? 4 : 1);
}

/** Address-bar suggestions. SQL shortlists the most recent matches and the
 *  ranking runs here, since SQLite has no `exp()` for the decay. */
export async function suggestHistory(
  query: string,
  limit = 6,
): Promise<HistoryEntry[]> {
  const text = query.trim();
  if (!text) return [];
  const { sql, binds } = wordFilter(text);
  if (!sql) return [];
  const db = await getDb();
  const shortlist = await db.select<HistoryEntry[]>(
    `SELECT url, host, title, visits, last_visit
       FROM browser_history
      WHERE ${sql}
      ORDER BY last_visit DESC
      LIMIT 200`,
    binds,
  );
  const prefix = text
    .toLowerCase()
    .replace(/^https?:\/\//, "")
    .replace(/^www\./, "");
  const now = Date.now();
  return shortlist
    .map((entry) => ({ entry, rank: score(entry, prefix, now) }))
    .sort((a, b) => b.rank - a.rank)
    .slice(0, limit)
    .map(({ entry }) => entry);
}

/** The history view's list, newest first, optionally filtered. */
export async function listHistory(
  query = "",
  limit = 500,
): Promise<HistoryEntry[]> {
  const db = await getDb();
  const rows = `SELECT url, host, title, visits, last_visit FROM browser_history`;
  const text = query.trim();
  if (!text) {
    return db.select<HistoryEntry[]>(
      `${rows} ORDER BY last_visit DESC LIMIT $1`,
      [limit],
    );
  }
  const { sql, binds } = wordFilter(text);
  if (!sql) return [];
  // Interpolated, not bound: the binds are numbered by the word clauses.
  return db.select<HistoryEntry[]>(
    `${rows} WHERE ${sql} ORDER BY last_visit DESC LIMIT ${Math.trunc(limit)}`,
    binds,
  );
}

/** Groups by the **local** day of `last_visit` (stored as UTC). */
export function groupByDay(
  entries: HistoryEntry[],
  toMs: (utc: string) => number | undefined,
): HistoryDay[] {
  const days = new Map<number, HistoryEntry[]>();
  for (const entry of entries) {
    const ms = toMs(entry.last_visit);
    if (ms === undefined) continue;
    const d = new Date(ms);
    const day = new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
    const bucket = days.get(day);
    if (bucket) bucket.push(entry);
    else days.set(day, [entry]);
  }
  return [...days.entries()]
    .sort((a, b) => b[0] - a[0])
    .map(([day, list]) => ({ day, entries: list }));
}

export async function forgetUrl(url: string): Promise<void> {
  const db = await getDb();
  await db.execute(`DELETE FROM browser_history WHERE url = $1`, [url]);
}

/** Clears history. Icons stay: they are a per-host cache, not a record. */
export async function clearHistory(): Promise<void> {
  const db = await getDb();
  await db.execute(`DELETE FROM browser_history`);
}
