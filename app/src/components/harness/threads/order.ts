export const COLLAPSED_KEY = "oculus-chat-groups-collapsed";
export const ORDER_KEY = "oculus-chat-groups-order";
/** The column's view, Recents or Subject, beside its width and folds. */
export const VIEW_KEY = "oculus-chat-list-view";

/** The column's width bounds; folded, it is gone entirely. `ChatPage` owns the
 *  panel so its header can hold the fold toggle. */
export const THREAD_LIST_PANEL = {
  defaultWidth: 284,
  minWidth: 200,
  maxWidth: 420,
  collapsedWidth: 0,
  storageKey: "oculus-chat-list-width",
};

/** Threads a Subject group shows at first, and how many each "Show more" adds. */
export const PAGE = 5;

/** Recency, overruled by the dragged `order`. Unplaced groups (index -1) sort
 *  above placed ones — a new subject has a live conversation — and stay in
 *  recency order among themselves because `sort` is stable. */
export function arrange<T extends { key: string }>(groups: T[], order: string[]): T[] {
  if (!order.length) return groups;
  return groups.slice().sort((a, b) => order.indexOf(a.key) - order.indexOf(b.key));
}
