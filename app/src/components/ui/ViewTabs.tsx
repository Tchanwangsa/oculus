import { DRAG_SURFACE, useStripReorder } from "@/hooks/usePointerDrag";
import { cn } from "@/lib/utils";

export interface ViewTab<T extends string> {
  value: T;
  label: string;
  /** Optional trailing element — a count chip, a dot. */
  badge?: React.ReactNode;
}

/**
 * A non-routed underline tab strip (the shape of `SubjectLayout`'s nav) for
 * switching what a page renders. Sits on a container's bottom border: `-mb-px`
 * puts the active underline on that line. Pass `onReorder` to make the tabs
 * draggable; without it they are plain buttons.
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
  /** The whole strip in its new order — a list, not indices, since the caller
   *  may show a subset of what it stores. */
  onReorder?: (next: T[]) => void;
  className?: string;
}) {
  const strip = useStripReorder({
    keys: tabs.map((t) => t.value),
    swapOn: "edge",
    onDrop: ({ order }) => onReorder?.(order),
  });

  return (
    <div
      role="tablist"
      className={cn(
        "flex items-center gap-4",
        // See docs/frontend.md: a text selection pre-empts the drag in WebKit.
        onReorder && DRAG_SURFACE,
        className,
      )}
    >
      {tabs.map((tab, i) => {
        const active = tab.value === value;
        const grabbed = strip.drag?.key === tab.value;
        return (
          <button
            key={tab.value}
            type="button"
            role="tab"
            aria-selected={active}
            ref={strip.itemRef(tab.value)}
            // Activate on click, not press (unlike `TopTabBar`), so keyboard
            // activation works; skip the click a drag's release raises.
            onClick={() => {
              if (strip.didDrag()) return;
              onChange(tab.value);
            }}
            onPointerDown={onReorder ? (e) => strip.onPointerDown(e, tab.value) : undefined}
            style={strip.styleFor(tab.value, i)}
            className={cn(
              "-mb-px flex cursor-pointer items-center gap-1.5 border-b-2 pb-2.5 pt-1 text-[13px] font-medium transition-colors",
              active
                ? "border-primary text-foreground"
                : "border-transparent text-muted-foreground hover:text-foreground",
              grabbed
                ? "relative z-10 text-foreground"
                : strip.drag && "transition-transform duration-200 ease-out",
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
