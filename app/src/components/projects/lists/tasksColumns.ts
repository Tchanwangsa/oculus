import type { PropertyId } from "../display/settings";
import type { SortKey } from "../tasks/taskSort";

/** A column of the Tasks table. Name is always first; the rest follow the
 *  Display settings. */
export type ColumnId = "name" | PropertyId;

interface ColumnSpec {
  label: string;
  /** Pixels; Name takes the remainder. */
  width: number | null;
  /** What a click on the header sorts by; `null` is not sortable. */
  sort: SortKey | null;
}

export const COLUMNS: Record<ColumnId, ColumnSpec> = {
  name: { label: "Name", width: null, sort: "title" },
  tags: { label: "Tags", width: 190, sort: null },
  project: { label: "Project", width: 150, sort: "project" },
  subject: { label: "Subject", width: 110, sort: null },
  due: { label: "Due", width: 140, sort: "due" },
  updated: { label: "Updated", width: 100, sort: "updated" },
  created: { label: "Created", width: 100, sort: "created" },
};

/** Name's least width before the table scrolls sideways. */
const NAME_MIN = 240;
const GAP = 12;
/** `px-5` on each side, plus the 6px scrollbar the header stands in for. */
const CHROME = 46;

/** Name, then the shown properties in order. */
export function columnIds(properties: readonly PropertyId[]): ColumnId[] {
  return ["name", ...properties];
}

/** The grid's `gridTemplateColumns`, built at run time because the columns
 *  are the user's choice. */
export function gridTemplate(columns: readonly ColumnId[]): string {
  return columns
    .map((id) => {
      const width = COLUMNS[id].width;
      return width == null ? `minmax(${NAME_MIN}px,1fr)` : `${width}px`;
    })
    .join(" ");
}

/** The table's least width, so header and rows scroll together past it. */
export function tableMinWidth(columns: readonly ColumnId[]): number {
  const sum = columns.reduce((n, id) => n + (COLUMNS[id].width ?? NAME_MIN), 0);
  return CHROME + sum + GAP * (columns.length - 1);
}
