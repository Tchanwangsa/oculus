import {
  ArrowsClockwise,
  BookOpen,
  CalendarBlank,
  Chat,
  CheckSquare,
  GearSix,
  Globe,
  Kanban,
  ListChecks,
  MagnifyingGlass,
  VideoCamera,
  type Icon as PhosphorIcon,
} from "@phosphor-icons/react";
import { projectHref } from "@/components/projects/projectHref";
import { taskHref } from "@/components/projects/taskHref";
import {
  searchLibraryFiles,
  searchLibraryLectures,
  searchPageText,
  SNIP_CLOSE,
  SNIP_OPEN,
  type LibraryFileHit,
  type LibraryLectureHit,
  type PageTextHit,
  type Subject,
} from "@/lib/db";
import { searchProjects, searchTasks } from "@/lib/projects";
import { addressKind, hostOf, normalizeAddress } from "@/lib/browser";
import { categoryIconFor, isPdfBacked } from "@/lib/fileTypes";
import { displayCode, displayName } from "@/lib/format";
import { fmtLectureDate, lecturePagePath } from "@/lib/lectures";
import { filePagePath, fileTitle, openFileSmart } from "@/lib/openFile";
import {
  FILTER_KEYS, currentFirst, filterId, filterValues, kindOf, matchesAll, subjectHaystack,
  type FilterDraft, type FilterKey, type Kind, type KindId, type SearchFilter,
} from "./searchFilters";
export {
  filterChip, filterId, filterPlaceholder, filterToken, matchesFilterDraft, resolveFilter, withFilter,
  type FilterDraft, type FilterKey, type FilterToken, type KindId, type SearchFilter,
} from "./searchFilters";

/**
 * The one search behind ⌘K (`CommandPalette.tsx`) and the new-tab page's field
 * (`NewTabPage.tsx`); a new kind of thing to find is a change here only.
 *
 * Searches titles (subjects, files, lectures, projects, tasks) and parsed text
 * via `pages_fts` — not page-image embeddings, which cost a cloud round trip
 * per query (`docs/retrieval.md`). A URL is offered as a page; anything else
 * falls through to a web search.
 *
 * Builds data only: icons are named and a row's destination is a `target`, so
 * each surface supplies its own navigation via {@link openSearchItem}.
 *
 * ⌘K also takes Discord-style filters: `in:<subject>` and `type:<kind>`, typed
 * as tokens and kept as chips ({@link SearchFilter}); the palette owns the chips.
 */

/** A descriptor, not an element: a subject's icon is a component with a prop. */
export type IconSpec =
  | { kind: "glyph"; icon: PhosphorIcon }
  | { kind: "subject"; code: string };

type SearchTarget =
  | { kind: "route"; path: string }
  /** A binary the app cannot render (.zip, .mp3): opens in the system viewer. */
  | { kind: "file"; file: LibraryFileHit }
  /** Opens in the in-app browser. */
  | { kind: "url"; url: string }
  /** ⌘K only: starts an `in:` / `type:` token in the field. */
  | { kind: "filter-key"; key: FilterKey }
  /** ⌘K only: becomes a chip that narrows the search. */
  | { kind: "filter"; filter: SearchFilter };

interface SnippetPart {
  text: string;
  hit: boolean;
}

export interface SearchItem {
  key: string;
  icon: IconSpec;
  label: string;
  /** Right-aligned: the subject a document belongs to, a project's subject. */
  meta?: string;
  /** A second line: matched prose for a hit inside a document, or a filter's
   *  syntax. `hit` parts render in the brand colour. */
  snippet?: SnippetPart[];
  target: SearchTarget;
}

export interface SearchSection {
  heading: string;
  items: SearchItem[];
}

const PLACES: { label: string; path: string; icon: PhosphorIcon }[] = [
  { label: "Chat", path: "/chat", icon: Chat },
  { label: "Calendar", path: "/calendar", icon: CalendarBlank },
  { label: "Projects", path: "/projects", icon: Kanban },
  { label: "Tasks", path: "/tasks", icon: ListChecks },
  { label: "Subjects", path: "/subjects", icon: BookOpen },
  { label: "Sync", path: "/sync", icon: ArrowsClockwise },
  { label: "Settings · Canvas", path: "/settings/canvas", icon: GearSix },
  { label: "Settings · AI", path: "/settings/ai", icon: GearSix },
  { label: "Settings · Storage", path: "/settings/storage", icon: GearSix },
  { label: "Settings · Library", path: "/settings/library", icon: GearSix },
];

/** Per-kind caps for a typed query, so no one kind pushes the others off. */
const LIMITS = {
  file: 6,
  page: 4,
  lecture: 3,
  subject: 4,
  project: 3,
  task: 3,
} as const;

const IDLE_LIMIT = 5;

/** With `type:` only one kind shows, so it gets the room. */
const TYPED_LIMIT = 15;

function glyph(icon: PhosphorIcon): IconSpec {
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

function fileItem(f: LibraryFileHit): SearchItem {
  return {
    key: `file:${f.id}`,
    icon: glyph(categoryIconFor(f)),
    label: fileTitle(f),
    meta: displayCode(f.subject_code),
    target:
      f.category === "file" && !isPdfBacked(f.filename)
        ? { kind: "file", file: f }
        : { kind: "route", path: filePagePath(f.subject_id, f.relative_path) },
  };
}

function pageItem(h: PageTextHit): SearchItem {
  return {
    key: `page:${h.file_id}:${h.page_no}`,
    icon: glyph(categoryIconFor(h)),
    label: fileTitle(h),
    meta: `${displayCode(h.subject_code)} · p.${h.page_no}`,
    snippet: snippetParts(h.snippet),
    target: { kind: "route", path: filePagePath(h.subject_id, h.relative_path) },
  };
}

function lectureItem(l: LibraryLectureHit): SearchItem {
  return {
    key: `lecture:${l.id}`,
    icon: glyph(VideoCamera),
    label: l.title,
    // Echo360 titles captures by room and slot; the date tells them apart.
    meta: `${displayCode(l.subject_code)} · ${fmtLectureDate(l.date)}`,
    target: { kind: "route", path: lecturePagePath(l) },
  };
}

function subjectItem(s: Subject): SearchItem {
  return {
    key: `subject:${s.id}`,
    icon: { kind: "subject", code: s.code },
    label: displayName(s.name, s.code),
    meta: displayCode(s.code),
    target: { kind: "route", path: `/subjects/${s.id}` },
  };
}

function filterItem(f: SearchFilter): SearchItem {
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

function filterKeyItem(f: (typeof FILTER_KEYS)[number]): SearchItem {
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
function webItems(query: string): SearchItem[] {
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

export interface SearchOptions {
  subjects: Subject[];
  /** This term's subjects, offered for an empty query. */
  current: Subject[];
  /** Leave out the web row (e.g. a surface with its own address bar). */
  noWeb?: boolean;
  /** The palette's chips. Any filter drops Web and Go to. */
  filters?: SearchFilter[];
  /** Offer filter rows when idle (⌘K). */
  offerFilters?: boolean;
  /** A chip being typed: the list is only its values. */
  draft?: FilterDraft | null;
}

/**
 * One ranked list of non-empty sections. Idle leads with recent files; a query
 * leads with subjects, and in-document text hits come after title matches.
 * A chip being typed (`in:comp`) lists only its values.
 */
export async function runSearch(
  query: string,
  { subjects, current, noWeb, filters = [], offerFilters, draft }: SearchOptions,
): Promise<SearchSection[]> {
  if (draft) {
    return [
      {
        heading: draft.key === "in" ? "Subjects" : "Types",
        items: filterValues(draft, subjects).map(filterItem),
      },
    ].filter((s) => s.items.length > 0);
  }

  const filtered = filters.length > 0;
  const idle = query.trim() === "" && !filtered;

  if (idle) {
    const files = await searchLibraryFiles(query, IDLE_LIMIT);
    return [
      { heading: "Filters", items: offerFilters ? FILTER_KEYS.map(filterKeyItem) : [] },
      { heading: "Recent", items: files.map(fileItem) },
      { heading: "Subjects", items: current.slice(0, IDLE_LIMIT).map(subjectItem) },
      { heading: "Go to", items: PLACES.map((p) => placeItem(p)) },
    ].filter((s) => s.items.length > 0);
  }

  // With filters and no text, the SQL's empty match lists by recency.
  let subjectId: number | undefined;
  let kind: Kind | undefined;
  for (const f of filters) {
    if (f.key === "in") subjectId = f.subject.id;
    else kind = kindOf(f.kind);
  }
  const wants = (id: KindId) => !kind || kind.id === id;
  const wantsFiles = !kind || kind.id === "file" || kind.category !== undefined;
  const limit = (n: number) => (kind ? TYPED_LIMIT : n);
  const fileScope = { subjectId, category: kind?.category };

  const [files, pages, lectures, projects, tasks] = await Promise.all([
    wantsFiles ? searchLibraryFiles(query, limit(LIMITS.file), fileScope) : [],
    wantsFiles ? searchPageText(query, limit(LIMITS.page), fileScope) : [],
    wants("lecture") ? searchLibraryLectures(query, limit(LIMITS.lecture), { subjectId }) : [],
    wants("project") ? searchProjects(query, limit(LIMITS.project), { subjectId }) : [],
    wants("task") ? searchTasks(query, limit(LIMITS.task), { subjectId }) : [],
  ]);

  // A text hit is shown only for a file the title search missed.
  const byTitle = new Set(files.map((f) => f.id));

  const sections: SearchSection[] = [
    {
      heading: "Subjects",
      // `in:` alone hides subjects; with `type:subject` it is that one subject.
      items:
        !wants("subject") || (subjectId !== undefined && !kind)
          ? []
          : (kind ? currentFirst(subjects) : subjects)
              .filter((s) => subjectId === undefined || s.id === subjectId)
              .filter((s) => matchesAll(subjectHaystack(s), query))
              .slice(0, limit(LIMITS.subject))
              .map(subjectItem),
    },
    { heading: "Files", items: files.map(fileItem) },
    {
      heading: kind?.id === "task" ? "Tasks" : "Projects",
      items: [
        ...projects.map((p) => ({
          key: `project:${p.id}`,
          icon: glyph(Kanban),
          label: p.name,
          meta: [p.subject_code && displayCode(p.subject_code), p.status === "archived" && "Archived"]
            .filter(Boolean)
            .join(" · "),
          target: { kind: "route" as const, path: projectHref(p) },
        })),
        ...tasks.map((t) => ({
          key: `task:${t.id}`,
          icon: glyph(CheckSquare),
          label: t.title,
          meta: t.project_name ?? "Unfiled",
          target: {
            kind: "route" as const,
            path: taskHref(t.project_id, t),
          },
        })),
      ],
    },
    { heading: "Lectures", items: lectures.map(lectureItem) },
    {
      heading: "In documents",
      items: pages.filter((p) => !byTitle.has(p.file_id)).map(pageItem),
    },
    {
      heading: "Go to",
      items: filtered ? [] : PLACES.filter((p) => matchesAll(p.label, query)).map(placeItem),
    },
    { heading: "Web", items: noWeb || filtered ? [] : webItems(query) },
  ];
  return sections.filter((s) => s.items.length > 0);
}

function placeItem(p: (typeof PLACES)[number]): SearchItem {
  return {
    key: `place:${p.path}`,
    icon: glyph(p.icon),
    label: p.label,
    target: { kind: "route", path: p.path },
  };
}

/** Injected per surface: the palette navigates the shell from outside every
 *  router, the new-tab field the pane it is drawn in. */
export interface OpenSearchOptions {
  /** ⌘-click / ⌘↵ — somewhere new, rather than here. */
  newTab: boolean;
  navigate: (path: string) => void;
  addTab: (path: string) => void;
  openUrl: (url: string) => void;
}

export function openSearchItem(item: SearchItem, o: OpenSearchOptions): void {
  switch (item.target.kind) {
    case "file":
      // System viewer; ⌘ changes nothing.
      openFileSmart(item.target.file);
      return;
    case "url":
      o.openUrl(item.target.url);
      return;
    case "route":
      (o.newTab ? o.addTab : o.navigate)(item.target.path);
      return;
    case "filter-key":
    case "filter":
      // The palette edits its own field for these before it gets here.
      return;
  }
}
