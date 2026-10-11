/** Where + and ⌘T open (`app/src/pages/start/NewTabPage.tsx`). */
export const NEW_TAB_PATH = "/new";

/** Column between two tabs; also part of the distance a swapped tab travels. */
export const SEPARATOR_W = 6;

/** Uniform Chrome-style widths: TAB_W each until the strip is full, then an
 *  equal share down to TAB_MIN_W, then the strip scrolls. Computed rather than
 *  left to `flex-shrink`: a scrolling flex container in WebKit sizes itself to
 *  its content, so the tabs would hug their titles. */
export const TAB_W = 200;
export const TAB_MIN_W = 76;
/** What the new-tab button and its two gaps take out of the tab region. */
export const TRAILING_W = 36;
