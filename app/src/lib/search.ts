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

/**
 * One search, two fields.
 *
 * ⌘K (`app/src/components/palette/CommandPalette.tsx`) and the new-tab page's
 * own field (`app/src/pages/NewTabPage.tsx`) ask the same question, so they
 * ask it through here rather than each growing their own idea of what the
 * library contains. Adding a kind of thing to find is a change in this file
 * and nowhere else.
 *
 * What it searches: **titles** — of subjects, files, lectures, projects and
 * tasks — and the **text inside parsed documents**, through the `pages_fts`
 * index (`searchPageText`). Not the page-image embeddings: those are a cloud
 * round trip per query (`docs/retrieval.md`) and belong to a question you ask
 * Chat, not to a field you are still typing in.
 *
 * And what is not in the library at all: a URL is offered as a page to open,
 * and anything else falls through to a web search, so a field you type into
 * always has somewhere to send you.
 *
 * This file builds *data*. Icons are named, not rendered, and where a row goes
 * is a `target` rather than a closure — the surface supplies the navigation,
 * because the palette navigates the shell and the new-tab field navigates the
 * pane it is drawn in. {@link openSearchItem} is the one place that dispatch
 * lives.
 */

/** Which glyph a row wears. A descriptor, not an element: a subject's icon is
 *  a component with a prop, so the two cannot be one type. */
export type IconSpec =
  | { kind: "glyph"; icon: PhosphorIcon }
  | { kind: "subject"; code: string };

/** Where a row leads. */
export type SearchTarget =
  /** A route, opened in the surface's own way. */
  | { kind: "route"; path: string }
  /** A binary the app cannot render — a .zip, a .mp3 — which leaves for the
   *  system viewer the way any file row in the app does. */
  | { kind: "file"; file: LibraryFileHit }
  /** A web page: the in-app browser, never Safari. */
  | { kind: "url"; url: string };

/** A run of snippet text, and whether it is one of the words that matched. */
export interface SnippetPart {
  text: string;
  hit: boolean;
}

export interface SearchItem {
  key: string;
  icon: IconSpec;
  label: string;
  /** Right-aligned: the subject a document belongs to, a project's subject. */
  meta?: string;
  /** The matched prose, for a hit found inside a document. */
  snippet?: SnippetPart[];
  target: SearchTarget;
}

export interface SearchSection {
  heading: string;
  items: SearchItem[];
}

/** The places search can send you that aren't a document. */
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

/** How many of each kind a typed query gets back. Files lead because they are
 *  what a coursework library is mostly made of; the rest are a handful each,
 *  so no one kind can push the others off the list. */
const LIMITS = {
  file: 6,
  page: 4,
  lecture: 3,
  subject: 4,
  project: 3,
  task: 3,
} as const;

/** With nothing typed the list is a landing, not a dump — five of each. */
const IDLE_LIMIT = 5;

/**
 * Every typed word must appear somewhere, in any order — the same rule the SQL
 * side applies, so what the in-memory lists (subjects, places) do and what the
 * database does stay one behaviour.
 */
function matchesAll(haystack: string, query: string): boolean {
  const hay = haystack.toLowerCase();
  return query
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean)
    .every((w) => hay.includes(w));
}

/** Everything a subject answers to: its display name and its code, with and
 *  without the term suffix. */
function subjectHaystack(s: Subject): string {
  return `${displayName(s.name, s.code)} ${s.code} ${displayCode(s.code)}`;
}

function glyph(icon: PhosphorIcon): IconSpec {
  return { kind: "glyph", icon };
}

/**
 * Markdown, made readable in one line.
 *
 * A snippet is cut out of a parsed page, so it arrives wearing whatever
 * syntax it was sitting in — a heading's `##`, a table row's pipes, emphasis
 * asterisks, an image's `![...](...)`. None of that means anything in a
 * single line of prose under a search result, and all of it costs the few
 * characters there is room for.
 *
 * The fences `snippet()` put around the matched words are control characters
 * (see `SNIP_OPEN`) precisely so that this can run without touching them.
 */
function tidyMarkdown(raw: string): string {
  return raw
    .replace(/!\[[^\]]*\]\([^)]*\)/g, "") // images say nothing here
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1") // links keep their text
    .replace(/^[\s>|#-]+/, "") // list, quote, table and heading furniture
    .replace(/[*_`~|]+/g, "")
    .replace(/\s+/g, " ")
    .trim();
}

/** The snippet as runs of plain and matched text. */
export function snippetParts(raw: string): SnippetPart[] {
  const parts: SnippetPart[] = [];
  // Split on the open fence, then each piece on the close: what lies between
  // them is a hit, what follows is ordinary text.
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
    // A binary we cannot render — a .zip, a .mp3 — has no page to go to.
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
    // Echo360 titles a capture by its room and timetable slot, so a subject's
    // lectures all read alike — the date is the half that tells them apart.
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

/**
 * The one thing outside the library every field like this has to answer for:
 * what you typed is a web address, or it is something to look up.
 *
 * `normalizeAddress` is the address bar's own rule, so the search field and
 * the browser's address bar cannot disagree about what counts as a URL or
 * which engine a query goes to (`app/src/lib/browser.ts`).
 */
function webItems(query: string): SearchItem[] {
  const q = query.trim();
  if (!q) return [];
  const address = normalizeAddress(q);
  // A URL is the answer, not a guess at one: offered as the page it is.
  // Asked of `addressKind` rather than by looking for `?q=` in the result —
  // which engine a query goes to is a setting now, so the shape of a search
  // URL is not something this side may assume.
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
  /** Every subject, for title matching. */
  subjects: Subject[];
  /** This term's, which is what an empty query offers instead. */
  current: Subject[];
  /** Leave the web row out — a surface that has its own address bar, say.
   *  Default false: a field that finds nothing should still go somewhere. */
  noWeb?: boolean;
}

/**
 * The library, and the web, as one ranked list of sections.
 *
 * Two orders, because the useful first row differs. Idle, it is the file you
 * were last in; searching, it is whatever best matches what you typed, and a
 * subject beats a document that merely mentions its code. Prose found *inside*
 * a document comes after the titles: a title match is a thing you were looking
 * for, a text match is a thing you may have been.
 *
 * Empty sections are dropped, never drawn as an empty state.
 */
export async function runSearch(
  query: string,
  { subjects, current, noWeb }: SearchOptions,
): Promise<SearchSection[]> {
  const idle = query.trim() === "";

  if (idle) {
    const files = await searchLibraryFiles(query, IDLE_LIMIT);
    return [
      { heading: "Recent", items: files.map(fileItem) },
      { heading: "Subjects", items: current.slice(0, IDLE_LIMIT).map(subjectItem) },
      { heading: "Go to", items: PLACES.map((p) => placeItem(p)) },
    ].filter((s) => s.items.length > 0);
  }

  // One round trip each, in parallel: they are independent reads of the same
  // SQLite and the slowest of them is what the field waits for, not the sum.
  const [files, pages, lectures, projects, tasks] = await Promise.all([
    searchLibraryFiles(query, LIMITS.file),
    searchPageText(query, LIMITS.page),
    searchLibraryLectures(query, LIMITS.lecture),
    searchProjects(query, LIMITS.project),
    searchTasks(query, LIMITS.task),
  ]);

  // A file that already matched by title says nothing new as a text hit; the
  // snippet is worth showing only for a document the title search missed.
  const byTitle = new Set(files.map((f) => f.id));

  const sections: SearchSection[] = [
    {
      heading: "Subjects",
      items: subjects
        .filter((s) => matchesAll(subjectHaystack(s), query))
        .slice(0, LIMITS.subject)
        .map(subjectItem),
    },
    { heading: "Files", items: files.map(fileItem) },
    {
      heading: "Projects",
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
          // A task title alone names a dozen pieces of work across a
          // semester; the project it is in is what tells them apart — and
          // "Unfiled" is what a task with no project is called everywhere
          // else in the app, rather than a blank where a project should be.
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
      items: PLACES.filter((p) => matchesAll(p.label, query)).map(placeItem),
    },
    { heading: "Web", items: noWeb ? [] : webItems(query) },
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

/** How a surface acts on a picked row. Injected rather than assumed: the
 *  palette navigates the shell from outside every router, and the new-tab
 *  field navigates the pane it is drawn in. */
export interface OpenSearchOptions {
  /** ⌘-click / ⌘↵ — somewhere new, rather than here. */
  newTab: boolean;
  /** Go to a route in this surface's current context. */
  navigate: (path: string) => void;
  /** The same route, in a tab of its own. */
  addTab: (path: string) => void;
  /** Open a web page. */
  openUrl: (url: string) => void;
}

/** Acting on a row, in one place, so two fields cannot disagree about what
 *  picking the same result does. */
export function openSearchItem(item: SearchItem, o: OpenSearchOptions): void {
  switch (item.target.kind) {
    case "file":
      // Out to the system viewer: there is no route to take a .mp3 to, and
      // ⌘ changes nothing about where it opens.
      openFileSmart(item.target.file);
      return;
    case "url":
      o.openUrl(item.target.url);
      return;
    case "route":
      (o.newTab ? o.addTab : o.navigate)(item.target.path);
      return;
  }
}
