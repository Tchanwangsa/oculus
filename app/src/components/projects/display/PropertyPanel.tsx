import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { ArrowLeft, MagnifyingGlass, X } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { DRAG_SURFACE, useStripReorder } from "@/hooks/gestures/usePointerDrag";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import { NAME_PROPERTY, propertyIcon, propertyLabel } from "./propertyMeta";
import { PropertyRow, PropertySectionLabel } from "./PropertyRow";
import {
  hiddenProperties,
  hideAllProperties,
  hideProperty,
  moveProperty,
  showAllProperties,
  showProperty,
  type PropertyId,
  type TasksDisplay,
} from "./settings";

/**
 * The second panel of the Display menu: which properties show, and in what
 * order. Shown rows drag (`useStripReorder`, the sidebar rail's gesture) or
 * move with the arrow keys on their handle; Name is first and locked. While a
 * search narrows the list the order is not draggable, since the rows on screen
 * are not the whole order.
 */
export function PropertyPanel({
  display,
  onChange,
  query,
  onQuery,
  active,
  onBack,
  onClose,
}: {
  display: TasksDisplay;
  onChange: (next: TasksDisplay) => void;
  query: string;
  onQuery: (query: string) => void;
  /** Whether this panel is the one showing, to take focus when it slides in. */
  active: boolean;
  onBack: () => void;
  onClose: () => void;
}) {
  const search = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  useScrollFade(listRef, "y");

  useEffect(() => {
    // Not scrolling: the panel is still sliding in beside the one leaving.
    if (active) search.current?.focus({ preventScroll: true });
  }, [active]);

  const q = query.trim().toLowerCase();
  const matches = (label: string) => !q || label.toLowerCase().includes(q);
  const shown = display.properties;
  const hidden = hiddenProperties(display);
  const visibleShown = shown.filter((id) => matches(propertyLabel(id)));
  const visibleHidden = hidden.filter((id) => matches(propertyLabel(id)));
  const nameMatches = matches(NAME_PROPERTY.label);
  const reorderable = q === "" && shown.length > 1;

  const [announcement, setAnnouncement] = useState("");
  const handles = useRef(new Map<PropertyId, HTMLButtonElement>());
  // React may move the focused row's node to reorder, which blurs it.
  const refocus = useRef<PropertyId | null>(null);
  useEffect(() => {
    if (!refocus.current) return;
    handles.current.get(refocus.current)?.focus();
    refocus.current = null;
  }, [shown]);

  const strip = useStripReorder({
    keys: shown,
    axis: "y",
    // Equal-height rows: a clamped centre never passes the end ones.
    swapOn: "edge",
    gap: 0,
    onDrop: ({ order }) => onChange({ ...display, properties: order }),
  });

  const nudge = (id: PropertyId, by: -1 | 1) => {
    const next = moveProperty(display, id, by);
    if (next === display) return;
    refocus.current = id;
    onChange(next);
    setAnnouncement(
      `${propertyLabel(id)} moved to position ${next.properties.indexOf(id) + 1} of ${shown.length}`,
    );
  };

  const onHandleKey = (e: KeyboardEvent<HTMLButtonElement>, id: PropertyId) => {
    if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;
    e.preventDefault();
    nudge(id, e.key === "ArrowUp" ? -1 : 1);
  };

  const nothing = !nameMatches && visibleShown.length === 0 && visibleHidden.length === 0;

  return (
    <div className="p-3">
      <div className="mb-2 flex h-7 items-center gap-1">
        <button
          type="button"
          aria-label="Back to display options"
          onClick={onBack}
          className="flex size-7 shrink-0 cursor-pointer items-center justify-center rounded-md text-muted-foreground outline-none hover:bg-accent hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50"
        >
          <ArrowLeft size={14} />
        </button>
        <h2 className="min-w-0 flex-1 truncate font-display text-[13px] font-semibold text-foreground">
          Property visibility
        </h2>
        <button
          type="button"
          aria-label="Close"
          onClick={onClose}
          className="flex size-7 shrink-0 cursor-pointer items-center justify-center rounded-md text-muted-foreground outline-none hover:bg-accent hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50"
        >
          <X size={14} />
        </button>
      </div>

      <div className="mb-1 flex h-8 items-center gap-2 rounded-full border border-input bg-card px-3 transition-[color,box-shadow] focus-within:border-ring focus-within:ring-[3px] focus-within:ring-ring/25">
        <MagnifyingGlass size={13} className="shrink-0 text-muted-foreground" />
        <input
          ref={search}
          value={query}
          onChange={(e) => onQuery(e.target.value)}
          onKeyDown={(e) => {
            // The menu's Escape handler leaves a typed query to this.
            if (e.key === "Escape" && query) onQuery("");
          }}
          placeholder="Search for a property…"
          aria-label="Search for a property"
          spellCheck={false}
          autoComplete="off"
          className="min-w-0 flex-1 bg-transparent text-[13px] text-foreground outline-none placeholder:text-muted-foreground"
        />
      </div>

      <div ref={listRef} className="max-h-72 overflow-y-auto">
        {(nameMatches || visibleShown.length > 0) && (
          <section aria-label="Shown properties">
            <PropertySectionLabel
              action={shown.length > 0 ? { label: "Hide all", onClick: () => onChange(hideAllProperties(display)) } : undefined}
            >
              Shown properties
            </PropertySectionLabel>
            <ul className={cn(DRAG_SURFACE)}>
              {nameMatches && (
                <PropertyRow icon={NAME_PROPERTY.icon} label={NAME_PROPERTY.label} visible locked />
              )}
              {visibleShown.map((id) => {
                const index = shown.indexOf(id);
                return (
                  <PropertyRow
                    key={id}
                    icon={propertyIcon(id)}
                    label={propertyLabel(id)}
                    position={`${index + 1} of ${shown.length}`}
                    visible
                    onToggle={() => onChange(hideProperty(display, id))}
                    drag={
                      reorderable
                        ? {
                            rowRef: strip.itemRef(id),
                            grabbed: strip.drag?.key === id,
                            dragging: strip.drag != null,
                            style: strip.styleFor(id, index),
                            handleRef: (node) => {
                              if (node) handles.current.set(id, node);
                              else handles.current.delete(id);
                            },
                            onPointerDown: (e) => strip.onPointerDown(e, id),
                            onClickCapture: (e) => {
                              if (strip.didDrag()) e.stopPropagation();
                            },
                            onKeyDown: (e) => onHandleKey(e, id),
                          }
                        : undefined
                    }
                  />
                );
              })}
            </ul>
          </section>
        )}

        {visibleHidden.length > 0 && (
          <section aria-label="Hidden properties" className="mt-1">
            <PropertySectionLabel
              action={{ label: "Show all", onClick: () => onChange(showAllProperties(display)) }}
            >
              Hidden properties
            </PropertySectionLabel>
            <ul>
              {visibleHidden.map((id) => (
                <PropertyRow
                  key={id}
                  icon={propertyIcon(id)}
                  label={propertyLabel(id)}
                  visible={false}
                  onToggle={() => onChange(showProperty(display, id))}
                />
              ))}
            </ul>
          </section>
        )}

        {nothing && (
          <p className="px-1 py-4 text-center text-xs text-muted-foreground">No property matches.</p>
        )}
      </div>
      <p className="sr-only" aria-live="polite">
        {announcement}
      </p>
    </div>
  );
}
