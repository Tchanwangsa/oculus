import type { DbFile } from "@/lib/db";
import { humanizeSlug } from "@/lib/format/format";
import { frontNumber, frontString, splitFrontmatter } from "@/lib/markdown/frontmatter";
import { isEdUrl } from "./links";

export const THREAD_TYPES = [
  { value: "question", label: "Questions" },
  { value: "post", label: "Posts" },
  { value: "announcement", label: "Announcements" },
  { value: "unknown", label: "Unknown type" },
] as const;

export const THREAD_STATUSES = [
  { value: "resolved", label: "Answered" },
  { value: "unresolved", label: "Unanswered" },
  { value: "unknown", label: "Unknown answer status" },
] as const;

export interface EdThread {
  file: DbFile;
  number: number | null;
  title: string;
  posted: Date | null;
  kind: "question" | "post" | "announcement" | null;
  resolved: boolean | null;
  /** The category path, `Assignments / A2`. */
  category: string | null;
  /** Ed's colour for the top-level category; null when unknown. */
  categoryColor: string | null;
  /** The thread on Ed. */
  url: string | null;
  searchText: string;
  /** The post and replies as one line of plain prose, for match snippets. */
  prose: string;
}

const KINDS = ["question", "post", "announcement"] as const;

const normalize = (text: string) => text.normalize("NFKC").toLowerCase();

/** Markdown marks dropped and whitespace collapsed: what a snippet shows. */
function plainProse(markdown: string): string {
  return markdown
    .replace(/!\[[^\]]*\]\([^)]*\)/g, " ")
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/<[^>]+>/g, " ")
    .replace(/^\s{0,3}(#{1,6}\s+|>\s?|[-*+]\s+|---+\s*$)/gm, "")
    .replace(/[*_`~]+/g, "")
    .replace(/\s+/g, " ")
    .trim();
}

/** Read a scraped thread: its front matter where the sync wrote one, else the
 * `**#46 · question · …**` meta line older scrapes carry. Keeps a search index
 * of the entire thread, replies included. Unreadable scrapes still get a
 * filename title. */
export function parseEdThread(markdown: string, file: DbFile): EdThread {
  const { data, body } = splitFrontmatter(markdown);
  const header = body.split(/\n---\s*\n/, 1)[0];
  const title = /^# (.+)$/m.exec(header)?.[1]?.trim() || humanizeSlug(file.filename);
  const tokens = (/^\*\*(#\d+ ·[^*]*)\*\*\s*$/m.exec(header)?.[1] ?? "")
    .split(" · ").map((token) => token.trim());

  const numberText = tokens.find((token) => /^#\d+$/.test(token))?.slice(1)
    ?? /^(\d+)-/.exec(file.filename)?.[1];
  const number = frontNumber(data.number) ?? (numberText ? Number(numberText) : null);
  const kind = KINDS.find((value) => value === data.type)
    ?? KINDS.find((value) => tokens.includes(value)) ?? null;
  const answered = typeof data.answered === "boolean" ? data.answered
    : tokens.includes("resolved") ? true : tokens.includes("unresolved") ? false : null;
  const frontCategory = [data.category, data.subcategory, data.subsubcategory]
    .map(frontString).filter(Boolean).join(" / ");
  const category = frontCategory || (tokens.find((token) => token && !token.startsWith("#")
    && !([...KINDS, "resolved", "unresolved"] as string[]).includes(token)) ?? null);

  const by = /^\*\*By:\*\* .* · (\d{4}-\d{2}-\d{2}) (\d{2}:\d{2})\s*$/m.exec(header);
  // Scraped timestamps are written in the course's local timezone.
  const posted = by ? new Date(`${by[1]}T${by[2]}:00`) : null;
  return {
    file, number, title, kind, category,
    resolved: kind === "question" ? answered : null,
    categoryColor: frontString(data.category_color),
    url: [frontString(data.url), file.source_url].find(isEdUrl) ?? null,
    posted: posted && !Number.isNaN(posted.getTime()) ? posted : null,
    searchText: normalize(`${number == null ? "" : `#${number}`} ${title} ${body}`),
    prose: plainProse(body.slice(header.length)),
  };
}

export function threadTitle(thread: EdThread): string {
  return thread.number == null ? thread.title : `#${thread.number}: ${thread.title}`;
}

export function threadStatus(thread: EdThread): string | null {
  if (thread.kind !== "question") return null;
  return thread.resolved === true ? "resolved" : thread.resolved === false ? "unresolved" : "unknown";
}

export interface ThreadFilters {
  query: string;
  types: ReadonlySet<string>;
  statuses: ReadonlySet<string>;
  categories: ReadonlySet<string>;
}

/** The search box's keywords, as the filter matches them. */
export const queryWords = (query: string): string[] =>
  normalize(query).trim().split(/\s+/).filter(Boolean);

export interface TextPart {
  text: string;
  hit: boolean;
}

/** `text` cut at every case-insensitive occurrence of `words`. */
export function markHits(text: string, words: readonly string[]): TextPart[] {
  if (!words.length) return [{ text, hit: false }];
  const escaped = [...words].sort((a, b) => b.length - a.length)
    .map((w) => w.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
  const pattern = new RegExp(`(${escaped.join("|")})`, "gi");
  return text.split(pattern).filter(Boolean)
    .map((part) => ({ text: part, hit: words.includes(part.toLowerCase()) }));
}

/** Where a keyword first occurs in the thread's post or replies: that stretch
 * of prose, from a little before the hit, with every keyword marked. Null when
 * no keyword is in the prose (it matched the title or metadata only). */
export function matchSnippet(thread: EdThread, words: readonly string[], lead = 40): TextPart[] | null {
  const lower = thread.prose.toLowerCase();
  const at = Math.min(...words.map((w) => lower.indexOf(w)).filter((i) => i >= 0));
  if (!Number.isFinite(at)) return null;
  // Start on a word boundary so the snippet doesn't open mid-word.
  let start = Math.max(0, at - lead);
  if (start > 0) start = thread.prose.indexOf(" ", start) + 1 || start;
  if (start > at) start = at;
  const text = `${start > 0 ? "…" : ""}${thread.prose.slice(start, at + 240)}`;
  return markHits(text, words);
}

/** Each keyword must occur somewhere in the title, body, metadata or replies.
 * Selections within a filter are OR; separate filters combine with AND. */
export function filterEdThreads(threads: readonly EdThread[], filters: ThreadFilters): EdThread[] {
  const keywords = queryWords(filters.query);
  return threads.filter((thread) =>
    keywords.every((word) => thread.searchText.includes(word))
    && (!filters.types.size || filters.types.has(thread.kind ?? "unknown"))
    && (!filters.statuses.size || filters.statuses.has(threadStatus(thread) ?? ""))
    && (!filters.categories.size || filters.categories.has(thread.category ?? "Uncategorised")),
  );
}
