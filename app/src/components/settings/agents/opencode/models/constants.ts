/** Column template shared by the header and every row; the model takes the slack. */
export const COLS =
  "grid grid-cols-[16px_minmax(0,1fr)_120px_64px_68px_56px_68px_96px_64px] items-center gap-3 px-5";

export const PAGE_SIZE = 50;

/** The maker filter's value for a model `makerOf` can't place. */
export const UNKNOWN_MAKER = "~unknown";

/** *Unavailable* is the blocked models: never offered, whatever their tick. */
export type Scope = "all" | "offered" | "hidden" | "unavailable";
export type SortKey = "id" | "provider" | "input" | "output" | "context" | "maxOutput" | "released";

/** Where the Providers tab sends the table: a provider to filter to (or none),
 *  and a counter so the same provider twice still resets the filters. */
export interface ModelsFocus {
  provider: string | null;
  n: number;
}
