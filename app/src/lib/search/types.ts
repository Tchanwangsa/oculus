import type { Icon as PhosphorIcon } from "@phosphor-icons/react";
import type { LibraryFileHit, Subject } from "@/lib/db";
import type { FilterDraft, FilterKey, SearchFilter } from "./filters";

/** A descriptor, not an element: a subject's icon is a component with a prop. */
export type IconSpec =
  | { kind: "glyph"; icon: PhosphorIcon }
  | { kind: "subject"; code: string };

export type SearchTarget =
  | { kind: "route"; path: string }
  /** A binary the app cannot render (.zip, .mp3): opens in the system viewer. */
  | { kind: "file"; file: LibraryFileHit }
  /** Opens in the in-app browser. */
  | { kind: "url"; url: string }
  /** ⌘K only: starts an `in:` / `type:` token in the field. */
  | { kind: "filter-key"; key: FilterKey }
  /** ⌘K only: becomes a chip that narrows the search. */
  | { kind: "filter"; filter: SearchFilter };

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
  /** A second line: matched prose for a hit inside a document, or a filter's
   *  syntax. `hit` parts render in the brand colour. */
  snippet?: SnippetPart[];
  target: SearchTarget;
}

export interface SearchSection {
  heading: string;
  items: SearchItem[];
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
