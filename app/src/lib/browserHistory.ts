import { getDb } from "@/lib/db";
import { hostOf, isWebUrl } from "@/lib/browser";

/**
 * Where the in-app browser has been, and what the sites it visited look like.
 *
 * **One row per URL, not one per visit** (migration 36). The address bar's
 * question is "which page do you mean", which `visits` and `last_visit`
 * answer between them; a log of every load would grow forever to answer a
 * question nothing asks, and the history view groups by day from `last_visit`
 * either way — so a page you opened again today moves to today rather than
 * appearing under both. If something later wants per-visit data (time spent
 * per site, say), it belongs in a `browser_visit` table beside this one rather
 * than in a reshaping of it.
 *
 * Rust is the one that sees page loads (`app/src-tauri/src/browser.rs`), but
 * the writing happens here, from the snapshot the frontend already mirrors:
 * the tables belong to the frontend the way every other table does, and the
 * ranking below is then a query the address bar runs while you type, with no
 * IPC in the way.
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

/** `%` and `_` mean something in LIKE; a pasted URL is full of neither, but a
 *  typed `_` is common enough in a slug to be worth not treating as "any
 *  character". Paired with an `ESCAPE` clause at every call site. */
function likeSafe(text: string): string {
  return text.replace(/[\\%_]/g, (c) => `\\${c}`);
}

// ── What is safe to remember ─────────────────────────────────────────────
//
// A history table is a table we intend to *show* — in a dropdown, in Settings,
// and to whoever is looking at the screen. So the rule is not "record the URL"
// but "record the URL with nothing in it that should not be read aloud".
//
// Echo360 is the case that forces it: its playback URLs are signed, and a
// lecture watched in a browser tab would otherwise put a live credential in
// the suggestion list. A table that has already swallowed tokens cannot be
// un-swallowed without a migration, so the filtering happens before the first
// write and never after.

/** Query parameters that carry a credential, whatever the site calls it. */
const SECRET_PARAMS =
  /^(x-amz-.*|access[_-]?token|id[_-]?token|refresh[_-]?token|oauth[_-]?token|token|auth|authorization|api[_-]?key|apikey|key|secret|signature|sig|hmac|policy|credential|expires|session|sessionid|sid|jwt|password|passwd|pwd|code|state|ticket|saml.*|sso.*)$/i;

/** A value with no structure and a lot of it — an unnamed signature. Sixty
 *  characters is well past anything a person types into a query and well short
 *  of nothing: a real search of sixty unbroken characters is not a thing, and a
 *  base64 or hex blob almost always is. */
const OPAQUE_VALUE = /^[A-Za-z0-9._~-]{60,}$/;

/**
 * The URL as it should be remembered, or `null` for one that should not be.
 *
 * Drops the fragment always — it never helps a match and doubles the rows —
 * and drops the **whole** query as soon as any part of it looks like a
 * credential. Whole, not the offending parameter: a signed URL missing one of
 * its parameters is neither safe to keep nor useful to return to, and half a
 * credential is still a credential.
 */
export function historyUrl(raw: string): string | null {
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

/**
 * Records that `url` was opened, or that it was opened again.
 *
 * The title usually lands after the URL does — WebKit reports it when the
 * document has parsed its `<head>` — so an empty one never overwrites a title
 * already recorded for that URL.
 */
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

/** The title arriving for a page already recorded. Not a visit: a document
 *  that renames itself as it loads must not count twice. */
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

/** The ⌘K palette's matching rule, which this reuses rather than inventing a
 *  second one: every word you typed has to appear somewhere in the row, in any
 *  order, with `-` and `_` read as spaces. "canvas quiz" then finds
 *  `canvas…/quizzes/…` without your having to remember which way round it was.
 *
 *  Each word becomes its own LIKE, so the shape of the statement follows the
 *  query — hence the built SQL and the counted binds. */
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

/** How fast a visit stops counting. A week to halve it: often enough that
 *  yesterday's rabbit hole has faded out of the list by the weekend, slow
 *  enough that the page you open every Monday survives to the next one. */
const HALF_LIFE_DAYS = 7;

/** `sqliteUtcToMs` lives in `format.ts`, which is a UI module; this is the one
 *  place here that needs it, and three lines beats making the data layer
 *  depend on the formatting layer. */
function sqliteMs(s: string): number | undefined {
  const ms = Date.parse(s.includes("T") ? s : `${s.replace(" ", "T")}Z`);
  return Number.isNaN(ms) ? undefined : ms;
}

/** Frecency — how often, decayed by how long ago — with one thumb on the
 *  scale for a host you are typing the start of. Someone who types "can" means
 *  `canvas…`, however many pages of it they have never been back to. */
function score(entry: HistoryEntry, prefix: string, nowMs: number): number {
  const visited = sqliteMs(entry.last_visit) ?? nowMs;
  const ageDays = Math.max(0, (nowMs - visited) / 86_400_000);
  const frecency = entry.visits * Math.pow(2, -ageDays / HALF_LIFE_DAYS);
  const onPrefix =
    prefix !== "" && entry.host.replace(/^www\./, "").startsWith(prefix);
  return frecency * (onPrefix ? 4 : 1);
}

/**
 * What the address bar offers for what has been typed so far.
 *
 * Two steps on purpose: SQL narrows to the rows that match at all, ordered by
 * recency so the shortlist is the *relevant* few hundred rather than an
 * arbitrary few hundred, and the ranking then runs here over that shortlist.
 * Frecency wants an exponential, SQLite has no `exp()`, and a decay curve
 * approximated in SQL is harder to read than the thing it approximates.
 */
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

/** Everything visited, newest first — the history view's list. `query`
 *  filters it; the cap is there so a long history cannot make the settings
 *  page pause on open. */
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
  // The limit is interpolated rather than bound: the binds are numbered from
  // the word clauses, and one more placeholder after a variable number of them
  // is a counting bug waiting to happen. It is a number this module chose.
  return db.select<HistoryEntry[]>(
    `${rows} WHERE ${sql} ORDER BY last_visit DESC LIMIT ${Math.trunc(limit)}`,
    binds,
  );
}

/** Groups a list by the **local** day it was last visited.
 *
 *  Local, not the stored UTC: `datetime('now')` writes UTC, and grouping on
 *  that string would file a Melbourne afternoon under tomorrow. */
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

/** Everything gone. The icons stay: they are a cache keyed by host, not a
 *  record of where you have been, and throwing them away only means fetching
 *  them again the next time you open one of those sites. */
export async function clearHistory(): Promise<void> {
  const db = await getDb();
  await db.execute(`DELETE FROM browser_history`);
}

// ── Site icons ───────────────────────────────────────────────────────────

/** Every icon found so far, host → `data:` URL. Read once on startup so the
 *  tab strip has icons before any page has loaded. */
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
