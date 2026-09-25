import { getDb } from "@/lib/db";
import { hostOf, isWebUrl } from "@/lib/browser";

/**
 * The in-app browser's history and site icons. **One row per URL, not per
 * visit**: `visits` and `last_visit` are all the address bar and the by-day
 * view need. Written here from the snapshot the frontend mirrors of
 * `app/src-tauri/src/browser.rs`, so ranking runs with no IPC.
 */

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

/** Escapes LIKE wildcards; pair with an `ESCAPE '\\'` clause. */
function likeSafe(text: string): string {
  return text.replace(/[\\%_]/g, (c) => `\\${c}`);
}

// ── What is safe to remember ─────────────────────────────────────────────
// History is shown on screen, so credentials (e.g. Echo360's signed playback
// URLs) are stripped before the first write.

/** Query parameters that carry a credential, whatever the site calls it. */
const SECRET_PARAMS =
  /^(x-amz-.*|access[_-]?token|id[_-]?token|refresh[_-]?token|oauth[_-]?token|token|auth|authorization|api[_-]?key|apikey|key|secret|signature|sig|hmac|policy|credential|expires|session|sessionid|sid|jwt|password|passwd|pwd|code|state|ticket|saml.*|sso.*)$/i;

/** A long unbroken value — an unnamed signature or token blob. */
const OPAQUE_VALUE = /^[A-Za-z0-9._~-]{60,}$/;

/** The URL as it should be remembered, or `null`. Drops the fragment, and
 *  the **whole** query if any part looks like a credential — a signed URL
 *  missing one parameter is neither safe nor useful. */
function historyUrl(raw: string): string | null {
  if (!isWebUrl(raw)) return null;
  let url: URL;
  try {
    url = new URL(raw);
  } catch {
    return null;
  }
  url.hash = "";
  for (const [name, value] of url.searchParams) {
    if (SECRET_PARAMS.test(name) || OPAQUE_VALUE.test(value)) {
      url.search = "";
      break;
    }
  }
  return url.toString();
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

// ── Finding a row again ──────────────────────────────────────────────────

/** The ⌘K palette's rule: every typed word appears somewhere in url or
 *  title, any order, `-`/`_` read as spaces. One LIKE per word. */
function wordFilter(text: string): { sql: string; binds: string[] } {
  const words = text.split(/\s+/).filter(Boolean);
  const binds: string[] = [];
  const clauses = words.map((word, i) => {
    binds.push(`%${likeSafe(word).replace(/[-_]/g, " ").replace(/ /g, "%")}%`);
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

/** `sqliteUtcToMs`, copied to keep the data layer off `format.ts`. */
function sqliteMs(s: string): number | undefined {
  const ms = Date.parse(s.includes("T") ? s : `${s.replace(" ", "T")}Z`);
  return Number.isNaN(ms) ? undefined : ms;
}

/** Frecency, boosted for a host that starts with what is typed. */
function score(entry: HistoryEntry, prefix: string, nowMs: number): number {
  const visited = sqliteMs(entry.last_visit) ?? nowMs;
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

// ── Site icons ───────────────────────────────────────────────────────────

/** host → `data:` URL, read at startup so the strip has icons early. */
export async function loadFavicons(): Promise<Record<string, string>> {
  const db = await getDb();
  const rows = await db.select<{ host: string; icon: string }[]>(
    `SELECT host, icon FROM browser_favicons`,
  );
  return Object.fromEntries(rows.map((r) => [r.host, r.icon]));
}

export async function saveFavicon(host: string, icon: string): Promise<void> {
  const db = await getDb();
  await db.execute(
    `INSERT INTO browser_favicons (host, icon) VALUES ($1, $2)
     ON CONFLICT(host) DO UPDATE SET icon = excluded.icon, updated_at = datetime('now')`,
    [host, icon],
  );
}
