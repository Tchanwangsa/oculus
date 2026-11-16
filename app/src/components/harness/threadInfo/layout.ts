/**
 * Where Thread info goes in the chat pane. The card floats at the pane's top
 * right while the timeline column moves left to clear it, narrowing to no
 * less than `MIN_COLUMN`, with the question rail's gutter kept on its left;
 * otherwise it is the header button's popover. Widths are layout pixels
 * (`clientWidth`), which page zoom does not scale.
 */

/** The scroller's side padding (`px-6`) while the card is away. */
export const PAD = 24;
/** The left padding while the card floats: the gutter `ThreadMap`'s rail
 *  needs beside the column. */
export const RAIL = 44;
/** The narrowest the column gets to make room. */
export const MIN_COLUMN = 640;
export const CARD_WIDTH = 280;
/** From the pane's edges, and between the card and the column. */
export const GAP = 12;
/** Right padding the scroller and composer take while the card floats: the
 *  card, its inset from the pane's edge and a gap before the column. */
export const RESERVE = CARD_WIDTH + GAP * 2;

/**
 * Whether the card floats in a pane `width` × `height` px. `cardHeight` is
 * its last measured height (0 before it has drawn): it never scrolls, so a
 * card taller than the pane is a popover instead.
 */
export function threadInfoFloats(width: number, height: number, cardHeight: number): boolean {
  return width - RAIL - RESERVE >= MIN_COLUMN && cardHeight + GAP * 2 <= height;
}
