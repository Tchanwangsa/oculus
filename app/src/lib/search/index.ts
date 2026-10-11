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

export { runSearch } from "./run";
export { openSearchItem, type OpenSearchOptions } from "./open";
export type { IconSpec, SearchItem, SearchOptions, SearchSection } from "./types";
export {
  filterChip, filterId, filterPlaceholder, filterToken, matchesFilterDraft, resolveFilter, withFilter,
  type FilterDraft, type FilterKey, type FilterToken, type KindId, type SearchFilter,
} from "./filters";
