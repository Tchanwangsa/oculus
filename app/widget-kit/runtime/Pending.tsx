import type { ComponentType } from "react";

const parts = new Map<string, ComponentType>();

/** What a preview draws for a component it names but hasn't written yet
 *  (`widget_transpile`'s `pending`): a quiet block that shimmers, the same
 *  one per name, so a re-render doesn't remount it. */
export function pendingPart(name: string): ComponentType {
  let part = parts.get(name);
  if (!part) {
    part = function Pending() {
      return <div className="o-pending" aria-hidden="true" data-pending={name} />;
    };
    parts.set(name, part);
  }
  return part;
}
