import { useState } from "react";

import { cn } from "@/lib/utils";

export interface ViewTab<T extends string> {
  value: T;
  label: string;
  /** Optional trailing element — a count chip, a dot. */
  badge?: React.ReactNode;
}

/**
 * A non-routed underline tab strip: the same shape as `SubjectLayout`'s nav,
 * for switching what a page renders rather than where it navigates. Inactive
 * tabs are greyed out and only the active one carries the indigo rule, so a
 * page's views read as siblings instead of hiding inside a dropdown.
 *
 * The strip is meant to sit on a container's bottom border — `-mb-px` pulls
 * the active underline down onto that line rather than floating above it.
 *
 * **Reordering is opt-in.** Pass `onReorder` and the tabs become draggable;
 * leave it off and they are plain buttons, which is what a settings page's
 * three fixed views want. Only the dock's strip asks for it — four tabs in a
 * 220px header where which one sits under your thumb is worth choosing.
 */
export function ViewTabs<T extends string>({
  tabs,
  value,
  onChange,
  onReorder,
  className,
}: {
  tabs: ReadonlyArray<ViewTab<T>>;
  value: T;
  onChange: (value: T) => void;
  /** Hand back the whole strip in its new order. A list rather than a pair of
   *  indices: the caller may be showing a subset of what it stores, and only
   *  it knows how to fold this back into the rest. */
  onReorder?: (next: T[]) => void;
  className?: string;
}) {
  /** What is being dragged, and what it is currently over. Both are values
   *  rather than indices, so nothing goes stale if `tabs` changes mid-drag. */
  const [dragged, setDragged] = useState<T | null>(null);
  const [over, setOver] = useState<T | null>(null);

  const clear = () => {
    setDragged(null);
    setOver(null);
  };

  /** Drop `dragged` where `target` sits, closing the gap it came from. */
  const drop = (target: T) => {
    const from = tabs.findIndex((t) => t.value === dragged);
    const to = tabs.findIndex((t) => t.value === target);
    clear();
    if (from < 0 || to < 0 || from === to || !onReorder) return;
    const next = tabs.map((t) => t.value);
    next.splice(to, 0, next.splice(from, 1)[0]);
    onReorder(next);
  };

  return (
    <div role="tablist" className={cn("flex items-center gap-4", className)}>
      {tabs.map((tab) => {
        const active = tab.value === value;
        const marked = over === tab.value && dragged !== null && dragged !== tab.value;
        return (
          <button
            key={tab.value}
            type="button"
            role="tab"
            aria-selected={active}
            onClick={() => onChange(tab.value)}
            draggable={!!onReorder}
            onDragStart={
              onReorder
                ? (e) => {
                    // Load-bearing, and not obvious: WebKit — which is what a
                    // Tauri WKWebView is — aborts a drag whose dragstart sets
                    // no data, so without this call no dragover and no drop
                    // ever fire and a tab cannot be moved at all. The payload
                    // is never read (the value is in state); setting
                    // *something* is the whole point. Do not "clean it up".
                    e.dataTransfer.setData("text/plain", tab.value);
                    e.dataTransfer.effectAllowed = "move";
                    setDragged(tab.value);
                  }
                : undefined
            }
            onDragEnd={onReorder ? clear : undefined}
            onDragOver={
              onReorder
                ? (e) => {
                    if (dragged === null) return;
                    e.preventDefault();
                    e.dataTransfer.dropEffect = "move";
                    setOver(tab.value);
                  }
                : undefined
            }
            onDrop={
              onReorder
                ? (e) => {
                    e.preventDefault();
                    drop(tab.value);
                  }
                : undefined
            }
            className={cn(
              "-mb-px flex cursor-pointer items-center gap-1.5 border-b-2 pb-2.5 pt-1 text-[13px] font-medium transition-colors",
              active
                ? "border-primary text-foreground"
                : "border-transparent text-muted-foreground hover:text-foreground",
              // The dragged tab dims where it came from and the one under the
              // pointer takes the brand rule, so the gap it will land in is
              // legible without a drop line the 36px header has no room for.
              dragged === tab.value && "opacity-40",
              marked && "border-brand",
            )}
          >
            {tab.label}
            {tab.badge}
          </button>
        );
      })}
    </div>
  );
}
