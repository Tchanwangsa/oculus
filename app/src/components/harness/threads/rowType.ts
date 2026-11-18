/**
 * The thread column's three text levels, after T3's sidebar: a small muted
 * meta line, a title a clear step brighter than the preview even when read,
 * and a smaller, dimmer preview. Shared by thread rows, search results and
 * the recent list, so they read alike.
 */
export const ROW_META = "text-[11.5px] text-muted-foreground";
export const ROW_TITLE = "text-[13.5px]";
/** Done, Approval, Question or Failed since last on screen. */
export const TITLE_ATTENTION = "font-medium text-foreground";
/** Read: foreground dimmed, still brighter than the preview in every theme. */
export const TITLE_READ = "text-foreground/80";
export const TITLE_READ_ACTIVE = "text-foreground/90";
export const ROW_PREVIEW = "text-[12.5px] text-muted-foreground/85";
/* Both below apply while the hover action (⋯) shows: the row is hovered, the
 * action holds focus, or its menu is open. Not the row's own focus, which
 * stays on the open chat's row after a click. Written out in full so
 * Tailwind finds every class. */

/** The line the action sits on: its end fades out under it, as a tab's
 *  title does under its ×, so the action needs no surface of its own. */
export const UNDER_ACTIONS =
  "group-hover/thread:[mask-image:linear-gradient(to_right,#000_calc(100%_-_64px),transparent_calc(100%_-_32px))] group-has-[[data-row-actions]:focus-within]/thread:[mask-image:linear-gradient(to_right,#000_calc(100%_-_64px),transparent_calc(100%_-_32px))] group-has-[[data-state=open]]/thread:[mask-image:linear-gradient(to_right,#000_calc(100%_-_64px),transparent_calc(100%_-_32px))]";
/** The status or age under the action, hidden while it shows. */
export const HIDDEN_UNDER_ACTIONS =
  "group-hover/thread:invisible group-has-[[data-row-actions]:focus-within]/thread:invisible group-has-[[data-state=open]]/thread:invisible";
