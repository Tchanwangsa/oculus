/**
 * Turns a scraper slug back into a readable title:
 * "2026-07-14-welcome-to-comp30022-it-project" → "Welcome to COMP30022 IT Project".
 * Only for slugged .md names — real download filenames already read fine.
 */
export function humanizeSlug(raw: string): string {
  let s = raw.replace(/\.md$/i, "");
  s = s.replace(/^\d{4}-\d{2}-\d{2}-/, "").replace(/^\d+-/, "");
  const small = new Set(["a", "an", "and", "at", "by", "for", "in", "of", "on", "or", "the", "to", "with"]);
  return s
    .split(/[-_]+/)
    .filter(Boolean)
    .map((w, i) => {
      if (/^[a-z]{3,4}\d{4,5}$/.test(w)) return w.toUpperCase(); // course codes
      if (/^(ai|it|faq|lms|vm|api|ui|ux)$/.test(w)) return w.toUpperCase();
      if (i > 0 && small.has(w)) return w;
      return w.charAt(0).toUpperCase() + w.slice(1);
    })
    .join(" ");
}

/** "MULT20015_2026_SM2" → "MULT20015" — the term suffix lives in the badge. */
export function displayCode(code: string): string {
  return code.split("_")[0] || code;
}

/** Strips the "(MULT20015_2026_SM2)" Canvas appends to its course names. */
export function displayName(name: string, code: string): string {
  const esc = code.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return name.replace(new RegExp(`\\s*\\(${esc}\\)\\s*$`), "");
}

/** The `YYYY-MM-DD` prefix the scraper stamps on announcements, if present. */
export function dateFromSlug(raw: string): Date | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})-/.exec(raw);
  if (!m) return null;
  const d = new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3]));
  return Number.isNaN(d.getTime()) ? null : d;
}

export function fmtSize(bytes: number | null): string {
  if (bytes == null) return "";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** Like fmtSize but scales through GB — storage totals outgrow MB fast. */
export function fmtBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const kb = bytes / 1024;
  if (kb < 1024) return `${kb.toFixed(1)} KB`;
  const mb = kb / 1024;
  if (mb < 1024) return `${mb.toFixed(1)} MB`;
  return `${(mb / 1024).toFixed(2)} GB`;
}

export function fmtDate(iso: string | null): string {
  if (!iso) return "Never";
  const d = new Date(iso);
  const now = new Date();
  const diff = now.getTime() - d.getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return "Just now";
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h ago`;
  return d.toLocaleDateString();
}

/** Relative time from an epoch-ms timestamp — same buckets as `fmtDate`. */
export function fmtAgo(ms: number | undefined): string {
  if (!ms) return "—";
  return fmtDate(new Date(ms).toISOString());
}

/** Per-subject sync recency for the sync rail rows. */
export function fmtSynced(iso: string | null): string {
  const ms = sqliteUtcToMs(iso);
  if (ms == null) return "Never synced";
  const mins = Math.floor((Date.now() - ms) / 60000);
  if (mins < 1) return "Synced just now";
  if (mins < 60) return `Synced ${mins} min ago`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `Synced ${hrs} hr ago`;
  const days = Math.floor(hrs / 24);
  if (days < 7) return `Synced ${days} day${days === 1 ? "" : "s"} ago`;
  const weeks = Math.floor(days / 7);
  return `Synced ${weeks} week${weeks === 1 ? "" : "s"} ago`;
}

/** Time of day only: "2:18 am". */
export function fmtTime(ms: number | undefined): string {
  if (!ms) return "";
  return new Date(ms).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
}

/** Day heading for date-grouped lists: "Sat 16 Aug — Today",
 *  "Fri 15 Aug — Yesterday", else just "Thu 14 Aug" (with the year once it
 *  differs from the current one). */
export function fmtDayHeading(ms: number): string {
  const startOfDay = (x: Date) =>
    new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const d = new Date(ms);
  const today = new Date();
  const daysAgo = Math.round((startOfDay(today) - startOfDay(d)) / 86_400_000);
  const opts: Intl.DateTimeFormatOptions = { weekday: "short", day: "numeric", month: "short" };
  if (d.getFullYear() !== today.getFullYear()) opts.year = "numeric";
  const date = d.toLocaleDateString([], opts);
  if (daysAgo === 0) return `${date} — Today`;
  if (daysAgo === 1) return `${date} — Yesterday`;
  return date;
}

/** Wall-clock time for timeline entries: "2:18 am" today, "15 Aug, 2:18 am"
 *  otherwise. Pass `withDate` to always include the date. */
export function fmtClock(ms: number | undefined, withDate = false): string {
  if (!ms) return "";
  const d = new Date(ms);
  const time = fmtTime(ms);
  const today = new Date();
  const sameDay =
    d.getFullYear() === today.getFullYear() &&
    d.getMonth() === today.getMonth() &&
    d.getDate() === today.getDate();
  if (sameDay && !withDate) return time;
  return `${d.toLocaleDateString([], { day: "numeric", month: "short" })}, ${time}`;
}

/** Elapsed time between two epoch-ms stamps: "34s", "2m 10s", "1h 4m". */
export function fmtDuration(startMs: number | undefined, endMs: number | undefined): string {
  if (startMs == null || endMs == null) return "—";
  const secs = Math.max(0, Math.round((endMs - startMs) / 1000));
  if (secs < 60) return `${secs}s`;
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m ${secs % 60}s`;
  return `${Math.floor(mins / 60)}h ${mins % 60}m`;
}

/** SQLite's `datetime('now')` is UTC but carries no zone marker, so a naive
 *  `new Date(...)` reads it as local time. Returns epoch ms, or undefined. */
export function sqliteUtcToMs(s: string | null): number | undefined {
  if (!s) return undefined;
  const iso = s.includes("T") ? s : s.replace(" ", "T") + "Z";
  const ms = Date.parse(iso);
  return Number.isNaN(ms) ? undefined : ms;
}
