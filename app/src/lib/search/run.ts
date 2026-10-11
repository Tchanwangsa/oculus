import { CheckSquare, Kanban } from "@phosphor-icons/react";
import { projectHref } from "@/components/projects/nav/projectHref";
import { taskHref } from "@/components/projects/nav/taskHref";
import {
  searchLibraryFiles,
  searchLibraryLectures,
  searchPageText,
} from "@/lib/db";
import { searchProjects, searchTasks } from "@/lib/planning/projects";
import { displayCode } from "@/lib/format/format";
import {
  FILTER_KEYS, currentFirst, filterValues, kindOf, matchesAll, subjectHaystack,
  type Kind, type KindId,
} from "./filters";
import {
  IDLE_LIMIT,
  LIMITS,
  PLACES,
  TYPED_LIMIT,
  fileItem,
  filterItem,
  filterKeyItem,
  glyph,
  lectureItem,
  pageItem,
  placeItem,
  subjectItem,
  webItems,
} from "./items";
import type { SearchOptions, SearchSection } from "./types";

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
