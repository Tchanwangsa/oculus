import { BookOpen, CheckSquare, Kanban, Shapes, VideoCamera, type Icon as PhosphorIcon } from "@phosphor-icons/react";
import { categoryIconFor } from "./fileTypes";
import { displayCode, displayName } from "./format";
import type { Subject } from "./db";
import type { IconSpec } from "./search";

/** What `type:` narrows to. A kind with a `category` is files of that
 *  `files.category` (`category_from_path` in `app/src-tauri/src/paths.rs`). */
export type KindId =
  | "subject"
  | "file"
  | "assignment"
  | "announcement"
  | "quiz"
  | "discussion"
  | "page"
  | "note"
  | "lecture"
  | "project"
  | "task";

export interface Kind {
  id: KindId;
  label: string;
  /** For the field's placeholder: "Search lectures…". */
  plural: string;
  description: string;
  icon: PhosphorIcon;
  category?: string;
  /** Other values `type:` takes, e.g. the category's own name. */
  aliases?: string[];
}

/** The icon the file's subject tab uses (`categoryIconFor`). */
function categoryIcon(category: string): PhosphorIcon {
  return categoryIconFor({ category, filename: "" });
}

const KINDS: Kind[] = [
  { id: "subject", label: "Subject", plural: "subjects", description: "A subject's home", icon: BookOpen },
  { id: "file", label: "File", plural: "files", description: "Anything in the library", icon: categoryIcon("file") },
  { id: "assignment", label: "Assignment", plural: "assignments", description: "Canvas assignments", icon: categoryIcon("assignment"), category: "assignment" },
  { id: "announcement", label: "Announcement", plural: "announcements", description: "Canvas announcements", icon: categoryIcon("announcement"), category: "announcement" },
  { id: "quiz", label: "Quiz", plural: "quizzes", description: "Canvas quizzes", icon: categoryIcon("quiz"), category: "quiz" },
  { id: "discussion", label: "Discussion", plural: "discussions", description: "Ed threads", icon: categoryIcon("ed"), category: "ed", aliases: ["ed"] },
  { id: "page", label: "Page", plural: "pages", description: "Canvas pages", icon: categoryIcon("page"), category: "page" },
  { id: "note", label: "Note", plural: "notes", description: "Your own documents", icon: categoryIcon("document"), category: "document", aliases: ["document"] },
  { id: "lecture", label: "Lecture", plural: "lectures", description: "Lecture recordings", icon: VideoCamera },
  { id: "project", label: "Project", plural: "projects", description: "Your projects", icon: Kanban },
  { id: "task", label: "Task", plural: "tasks", description: "Tasks in your projects", icon: CheckSquare },
];

export function kindOf(id: KindId): Kind {
  return KINDS.find((k) => k.id === id)!;
}

/** A chip in the palette's field; at most one per key. */
export type SearchFilter =
  | { key: "in"; subject: Subject }
  | { key: "type"; kind: KindId };

export type FilterKey = SearchFilter["key"];

/** The empty palette's rows that start a token, Discord's filter menu. */
export const FILTER_KEYS: { key: FilterKey; label: string; hint: string; icon: PhosphorIcon }[] = [
  { key: "in", label: "In a subject", hint: "subject code", icon: BookOpen },
  { key: "type", label: "Of a type", hint: "file, lecture, task…", icon: Shapes },
];

/** A chip still being typed: its key is fixed, its value is the search for one. */
export interface FilterDraft {
  key: FilterKey;
  value: string;
}

/** A `key:value` typed (or pasted) at the end of the field; `start` is the key's index. */
export interface FilterToken extends FilterDraft {
  start: number;
}

const TOKEN = /(?:^|\s)(in|type):(\S*)$/i;

export function filterToken(text: string): FilterToken | null {
  const m = TOKEN.exec(text);
  if (!m) return null;
  return {
    key: m[1].toLowerCase() as FilterKey,
    value: m[2],
    start: text.length - m[1].length - 1 - m[2].length,
  };
}

/** Every typed word, in any order — the same rule the SQL side applies. */
export function matchesAll(haystack: string, query: string): boolean {
  const hay = haystack.toLowerCase();
  return query
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean)
    .every((w) => hay.includes(w));
}

/** Display name, plus the code with and without the term suffix. */
export function subjectHaystack(s: Subject): string {
  return `${displayName(s.name, s.code)} ${s.code} ${displayCode(s.code)}`;
}

export function currentFirst(subjects: Subject[]): Subject[] {
  return [...subjects].sort((a, b) => Number(b.is_current) - Number(a.is_current));
}

function kindWords(k: Kind): string[] {
  return [k.id, ...(k.aliases ?? [])];
}

/** The values a token's text could mean: subjects by every typed word, kinds
 *  by prefix (so `type:le` is lecture, not file). */
export function filterValues(token: FilterDraft, subjects: Subject[]): SearchFilter[] {
  if (token.key === "in") {
    return currentFirst(subjects)
      .filter((s) => matchesAll(subjectHaystack(s), token.value))
      .map((subject) => ({ key: "in", subject }));
  }
  const v = token.value.toLowerCase();
  return KINDS.filter((k) => kindWords(k).some((w) => w.startsWith(v))).map((k) => ({
    key: "type",
    kind: k.id,
  }));
}

/** The one value a finished token names: an exact code or kind, else the only
 *  match. Null when it is ambiguous or names nothing. */
export function resolveFilter(token: FilterDraft, subjects: Subject[]): SearchFilter | null {
  const v = token.value.toLowerCase();
  if (!v) return null;
  if (token.key === "in") {
    const exact = currentFirst(subjects).find(
      (s) => s.code.toLowerCase() === v || displayCode(s.code).toLowerCase() === v,
    );
    if (exact) return { key: "in", subject: exact };
  } else {
    const exact = KINDS.find((k) => kindWords(k).includes(v));
    if (exact) return { key: "type", kind: exact.id };
  }
  const values = filterValues(token, subjects);
  return values.length === 1 ? values[0] : null;
}

/** Adds a chip, replacing one with the same key in place. */
export function withFilter(filters: SearchFilter[], f: SearchFilter): SearchFilter[] {
  const i = filters.findIndex((x) => x.key === f.key);
  return i < 0 ? [...filters, f] : filters.map((x, j) => (j === i ? f : x));
}

/** Stable identity for a chip (React keys, selection reset). */
export function filterId(f: SearchFilter): string {
  return f.key === "in" ? `in:${f.subject.id}` : `type:${f.kind}`;
}

/** A chip's value and icon: `COMP30022`, `Lecture`. */
export function filterChip(f: SearchFilter): { value: string; icon: IconSpec } {
  if (f.key === "in") {
    return { value: displayCode(f.subject.code), icon: { kind: "subject", code: f.subject.code } };
  }
  const k = kindOf(f.kind);
  return { value: k.label, icon: { kind: "glyph", icon: k.icon } };
}

/** The field's placeholder under chips: "Search lectures in COMP30022…". */
export function filterPlaceholder(filters: SearchFilter[]): string {
  let text = "Search";
  for (const f of filters) if (f.key === "type") text += ` ${kindOf(f.kind).plural}`;
  for (const f of filters) if (f.key === "in") text += ` in ${displayCode(f.subject.code)}`;
  return `${text}…`;
}


/** Debounced results must still match the currently edited key and value. */
export function matchesFilterDraft(draft: FilterDraft | null, filter: SearchFilter | null): boolean {
  if (!draft) return true;
  if (filter?.key !== draft.key) return false;
  return filter.key === "in"
    ? matchesAll(subjectHaystack(filter.subject), draft.value)
    : kindWords(kindOf(filter.kind)).some((word) => word.startsWith(draft.value.toLowerCase()));
}
