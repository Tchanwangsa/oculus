/**
 * The browser's selection as Finder keeps it: a click picks one, ⌘-click
 * toggles one, ⇧-click takes the run from the anchor (the last plain or ⌘
 * click), ⌘⇧-click adds that run to what is already picked; a marquee
 * (`boxSelect`) picks what its box covers. Keys are entry
 * paths; `order` is the order on screen.
 */

export interface Selection {
  keys: ReadonlySet<string>;
  anchor: string | null;
}

export const EMPTY_SELECTION: Selection = { keys: new Set(), anchor: null };

export interface ClickMods {
  /** ⌘ on macOS, Ctrl elsewhere. */
  toggle: boolean;
  range: boolean;
}

export function clickSelect(sel: Selection, key: string, order: readonly string[], mods: ClickMods): Selection {
  if (mods.range && sel.anchor != null && order.includes(sel.anchor) && order.includes(key)) {
    const run = rangeOf(order, sel.anchor, key);
    const keys = mods.toggle ? new Set([...sel.keys, ...run]) : new Set(run);
    return { keys, anchor: sel.anchor };
  }
  if (mods.toggle) {
    const keys = new Set(sel.keys);
    if (keys.has(key)) keys.delete(key);
    else keys.add(key);
    return { keys, anchor: key };
  }
  return { keys: new Set([key]), anchor: key };
}

function rangeOf(order: readonly string[], a: string, b: string): string[] {
  const i = order.indexOf(a);
  const j = order.indexOf(b);
  return order.slice(Math.min(i, j), Math.max(i, j) + 1);
}

/** A marquee's pick: the entries its box covers (in screen order), alone,
 *  added to what was picked when it started (⇧, `range`), or toggled
 *  against it (⌘, `toggle`), as Finder does. */
export function boxSelect(base: Selection, hit: readonly string[], mods: ClickMods): Selection {
  let keys: Set<string>;
  if (mods.toggle) {
    keys = new Set(base.keys);
    for (const key of hit) {
      if (keys.has(key)) keys.delete(key);
      else keys.add(key);
    }
  } else if (mods.range) {
    keys = new Set([...base.keys, ...hit]);
  } else {
    keys = new Set(hit);
  }
  return { keys, anchor: hit[0] ?? base.anchor };
}

export function selectAll(order: readonly string[]): Selection {
  return { keys: new Set(order), anchor: order[0] ?? null };
}

/** The selection without entries that left the screen (a reload, a filter).
 *  Returns `sel` itself when nothing left, so a re-render keeps identity. */
export function pruneSelection(sel: Selection, order: readonly string[]): Selection {
  const present = new Set(order);
  if ([...sel.keys].every((k) => present.has(k)) && (sel.anchor == null || present.has(sel.anchor))) return sel;
  return {
    keys: new Set([...sel.keys].filter((k) => present.has(k))),
    anchor: sel.anchor != null && present.has(sel.anchor) ? sel.anchor : null,
  };
}

/** The keys in screen order, for commands that act on several. */
export function selectedInOrder(sel: Selection, order: readonly string[]): string[] {
  return order.filter((k) => sel.keys.has(k));
}
