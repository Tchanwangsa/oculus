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
/** A section heading over rows (Working, Completed, a subject): more room
 *  above than below. */
export const SECTION_HEAD = "pt-4 pb-1";
