/**
 * Per-subject "recently visited" trail, kept in localStorage.
 *
 * Deliberately not in SQLite: it's throwaway UI state, it changes on every
 * click, and it must be readable synchronously on first paint so the row on the
 * subject home doesn't flash empty.
 */

const KEY = "oculus-recents";
const PER_SUBJECT_LIMIT = 8;

export type RecentKind = "file" | "lecture";

export interface RecentEntry {
  kind: RecentKind;
  /** relative_path for files, lecture id for lectures. */
  ref: string;
  title: string;
  /** File category ("page", "file", "module", …) — drives the icon. */
  category?: string;
  visitedAt: number;
}

type Store = Record<string, RecentEntry[]>;

function read(): Store {
  try {
    const raw = localStorage.getItem(KEY);
    return raw ? (JSON.parse(raw) as Store) : {};
  } catch {
    return {};
  }
}

function write(store: Store): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(store));
  } catch {
    /* quota or private mode — recents are expendable */
  }
}

export function getRecents(subjectId: number): RecentEntry[] {
  return read()[String(subjectId)] ?? [];
}

/** Records a visit, moving an already-seen item back to the front. */
export function recordRecent(
  subjectId: number,
  entry: Omit<RecentEntry, "visitedAt">,
): RecentEntry[] {
  const store = read();
  const key = String(subjectId);
  const rest = (store[key] ?? []).filter(
    (e) => !(e.kind === entry.kind && e.ref === entry.ref),
  );
  const next = [{ ...entry, visitedAt: Date.now() }, ...rest].slice(
    0,
    PER_SUBJECT_LIMIT,
  );
  store[key] = next;
  write(store);
  return next;
}

/** "just now" / "2h ago" / "Aug 12" — the label under a recent card. */
export function relativeTime(ts: number): string {
  const secs = Math.floor((Date.now() - ts) / 1000);
  if (secs < 60) return "just now";
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d ago`;
  return new Date(ts).toLocaleDateString("en-AU", {
    day: "numeric",
    month: "short",
  });
}
