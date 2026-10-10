import {
  ArrowsClockwise,
  CalendarBlank,
  Chat,
  GearSix,
  Globe,
  Kanban,
  ListChecks,
  MagnifyingGlass,
  VideoCamera,
  type Icon as PhosphorIcon,
} from "@phosphor-icons/react";
import {
  SNIP_CLOSE,
  SNIP_OPEN,
  type LibraryFileHit,
  type LibraryLectureHit,
  type PageTextHit,
  type Subject,
} from "@/lib/db";
import { addressKind, hostOf, normalizeAddress } from "@/lib/browser";
import { categoryIconFor, isPipelineFile } from "@/lib/files/fileTypes";
import { displayCode, displayName } from "@/lib/format/format";
import { fmtLectureDate, lecturePagePath } from "@/lib/lectures";
import { filePagePath, fileTitle } from "@/lib/files/openFile";
import { FILTER_KEYS, filterId, kindOf, type SearchFilter } from "./filters";
import type { IconSpec, SearchItem, SnippetPart } from "./types";

export const PLACES: { label: string; path: string; icon: PhosphorIcon }[] = [
  { label: "Chat", path: "/chat", icon: Chat },
  { label: "Calendar", path: "/calendar", icon: CalendarBlank },
  { label: "Projects", path: "/projects", icon: Kanban },
  { label: "Tasks", path: "/tasks", icon: ListChecks },
  { label: "Sync", path: "/sync", icon: ArrowsClockwise },
  { label: "Settings · Canvas", path: "/settings/canvas", icon: GearSix },
  { label: "Settings · Appearance", path: "/settings/appearance", icon: GearSix },
  { label: "Settings · Browser", path: "/settings/browser", icon: GearSix },
  { label: "Settings · Storage", path: "/settings/storage", icon: GearSix },
  { label: "Settings · Agents", path: "/settings/agents", icon: GearSix },
  { label: "Settings · opencode", path: "/settings/opencode", icon: GearSix },
  { label: "Settings · Jobs", path: "/settings/jobs", icon: GearSix },
  { label: "Settings · Parsing", path: "/settings/parsing", icon: GearSix },
  { label: "Settings · Embeddings", path: "/settings/embeddings", icon: GearSix },
  { label: "Settings · Transcription", path: "/settings/transcription", icon: GearSix },
];

/** Per-kind caps for a typed query, so no one kind pushes the others off. */
export const LIMITS = {
  file: 6,
  page: 4,
  lecture: 3,
  subject: 4,
  project: 3,
  task: 3,
} as const;

export const IDLE_LIMIT = 5;

/** With `type:` only one kind shows, so it gets the room. */
export const TYPED_LIMIT = 15;

export function glyph(icon: PhosphorIcon): IconSpec {
  return { kind: "glyph", icon };
}

/** Strips markdown syntax from a one-line snippet. The match fences are
 *  control characters (`SNIP_OPEN`), so this leaves them alone. */
function tidyMarkdown(raw: string): string {
  return raw
    .replace(/!\[[^\]]*\]\([^)]*\)/g, "") // images say nothing here
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1") // links keep their text
    .replace(/^[\s>|#-]+/, "") // list, quote, table and heading furniture
    .replace(/[*_`~|]+/g, "")
    .replace(/\s+/g, " ")
    .trim();
}

function snippetParts(raw: string): SnippetPart[] {
  const parts: SnippetPart[] = [];
  for (const [i, chunk] of tidyMarkdown(raw).split(SNIP_OPEN).entries()) {
    if (i === 0) {
      if (chunk) parts.push({ text: chunk, hit: false });
      continue;
    }
    const [hit, ...rest] = chunk.split(SNIP_CLOSE);
    if (hit) parts.push({ text: hit, hit: true });
    const tail = rest.join(SNIP_CLOSE);
    if (tail) parts.push({ text: tail, hit: false });
  }
  return parts;
}

export function fileItem(f: LibraryFileHit): SearchItem {
  return {
    key: `file:${f.id}`,
    icon: glyph(categoryIconFor(f)),
    label: fileTitle(f),
    meta: displayCode(f.subject_code),
    target:
      f.category === "file" && !isPipelineFile(f.filename)
        ? { kind: "file", file: f }
        : { kind: "route", path: filePagePath(f.subject_id, f.relative_path) },
  };
}

export function pageItem(h: PageTextHit): SearchItem {
  return {
    key: `page:${h.file_id}:${h.page_no}`,
    icon: glyph(categoryIconFor(h)),
    label: fileTitle(h),
    meta: `${displayCode(h.subject_code)} · p.${h.page_no}`,
    snippet: snippetParts(h.snippet),
    target: { kind: "route", path: filePagePath(h.subject_id, h.relative_path) },
  };
}

export function lectureItem(l: LibraryLectureHit): SearchItem {
  return {
    key: `lecture:${l.id}`,
    icon: glyph(VideoCamera),
    label: l.title,
    // Echo360 titles captures by room and slot; the date tells them apart.
    meta: `${displayCode(l.subject_code)} · ${fmtLectureDate(l.date)}`,
    target: { kind: "route", path: lecturePagePath(l) },
  };
}

export function subjectItem(s: Subject): SearchItem {
  return {
    key: `subject:${s.id}`,
    icon: { kind: "subject", code: s.code },
    label: displayName(s.name, s.code),
    meta: displayCode(s.code),
    target: { kind: "route", path: `/subjects/${s.id}` },
  };
}

export function filterItem(f: SearchFilter): SearchItem {
  const target = { kind: "filter" as const, filter: f };
  if (f.key === "in") return { ...subjectItem(f.subject), key: `filter:${filterId(f)}`, target };
  const k = kindOf(f.kind);
  return {
    key: `filter:${filterId(f)}`,
    icon: glyph(k.icon),
    label: k.label,
    meta: k.description,
    target,
  };
}

export function filterKeyItem(f: (typeof FILTER_KEYS)[number]): SearchItem {
  return {
    key: `filter-key:${f.key}`,
    icon: glyph(f.icon),
    label: f.label,
    snippet: [
      { text: `${f.key}:`, hit: true },
      { text: ` ${f.hint}`, hit: false },
    ],
    target: { kind: "filter-key", key: f.key },
  };
}

/** A URL to open, or a web search — by the address bar's own rule
 *  (`normalizeAddress`), so the two cannot disagree. */
export function webItems(query: string): SearchItem[] {
  const q = query.trim();
  if (!q) return [];
  const address = normalizeAddress(q);
  // Ask `addressKind`, not the result's shape: the search engine is a setting.
  if (addressKind(q) === "url" && hostOf(address)) {
    return [
      {
        key: "link",
        icon: glyph(Globe),
        label: address,
        meta: hostOf(address),
        target: { kind: "url", url: address },
      },
    ];
  }
  return [
    {
      key: "web",
      icon: glyph(MagnifyingGlass),
      label: `Search the web for “${q}”`,
      meta: hostOf(address),
      target: { kind: "url", url: address },
    },
  ];
}

export function placeItem(p: (typeof PLACES)[number]): SearchItem {
  return {
    key: `place:${p.path}`,
    icon: glyph(p.icon),
    label: p.label,
    target: { kind: "route", path: p.path },
  };
}
