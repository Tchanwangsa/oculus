import { useEffect, useMemo, useRef, useState } from "react";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import type { IconSpec, SearchItem, SearchSection } from "@/lib/search";
import { cn } from "@/lib/utils";

/**
 * The result list, drawn the same way wherever search is offered: ⌘K
 * (`app/src/components/palette/CommandPalette.tsx`) and the new-tab page's own
 * field (`app/src/pages/NewTabPage.tsx`).
 *
 * The rows are the two surfaces' one shared piece of appearance; what the rows
 * *are* is `app/src/lib/search.ts`, and what picking one does is that file's
 * `openSearchItem`. Nothing here reads the library or navigates.
 */

function Glyph({ spec }: { spec: IconSpec }) {
  if (spec.kind === "subject") return <SubjectIcon code={spec.code} size={15} />;
  const Icon = spec.icon;
  return <Icon size={15} className="shrink-0 text-muted-foreground" />;
}

/**
 * Keyboard selection over a list that is rebuilt on every keystroke.
 *
 * The index is clamped rather than stored valid: results landing after a
 * keystroke can shorten the list under the highlight, and a new list is a new
 * first row — clamping alone would leave the highlight on whatever happened to
 * land at the old offset.
 */
export function useSearchSelection(
  sections: SearchSection[],
  query: string,
  pick: (item: SearchItem, newTab: boolean) => void,
) {
  const [index, setIndex] = useState(0);
  const flat = useMemo(() => sections.flatMap((s) => s.items), [sections]);
  const selected = Math.min(index, Math.max(0, flat.length - 1));

  useEffect(() => setIndex(0), [query]);

  function onKeyDown(e: React.KeyboardEvent) {
    if (flat.length === 0) return;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setIndex((i) => (i + 1) % flat.length);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setIndex((i) => (i - 1 + flat.length) % flat.length);
    } else if (e.key === "Enter") {
      e.preventDefault();
      pick(flat[selected], e.metaKey || e.ctrlKey);
    }
  }

  return { selected, setIndex, onKeyDown, count: flat.length };
}

export function SearchList({
  sections,
  selected,
  onHover,
  onPick,
  className,
  empty = "No matches.",
}: {
  sections: SearchSection[];
  selected: number;
  onHover: (index: number) => void;
  onPick: (item: SearchItem, newTab: boolean) => void;
  className?: string;
  empty?: string;
}) {
  const listRef = useRef<HTMLDivElement>(null);

  /** Where each section starts in the flattened list, so a row knows its own
   *  keyboard index without the render walking a counter. */
  const offsets = useMemo(() => {
    let n = 0;
    return sections.map((s) => {
      const start = n;
      n += s.items.length;
      return start;
    });
  }, [sections]);

  // Follow the highlight with the scroll, addressed through the DOM rather
  // than a ref per row: the rows are rebuilt on every keystroke and a map of
  // refs would be a second thing to keep in step.
  useEffect(() => {
    listRef.current
      ?.querySelector(`[data-index="${selected}"]`)
      ?.scrollIntoView({ block: "nearest" });
  }, [selected]);

  const total = sections.reduce((n, s) => n + s.items.length, 0);

  return (
    <div ref={listRef} className={cn("overflow-y-auto py-1.5", className)}>
      {total === 0 ? (
        <p className="px-4 py-6 text-center text-[12.5px] text-muted-foreground">
          {empty}
        </p>
      ) : (
        sections.map((section, si) => (
          <div key={section.heading} className="pb-1 last:pb-0">
            {/* No icon: section headers are labels, not rows. */}
            <div className="px-4 pb-0.5 pt-1.5 text-[11px] font-medium tracking-wide text-muted-foreground">
              {section.heading}
            </div>
            {section.items.map((item, ii) => {
              const i = offsets[si] + ii;
              return (
                <button
                  key={item.key}
                  type="button"
                  data-index={i}
                  // `mousemove`, not `mouseenter`: arrowing through a list
                  // that scrolls under a resting pointer would otherwise hand
                  // the selection straight back to the mouse.
                  onMouseMove={() => onHover(i)}
                  onClick={(e) => onPick(item, e.metaKey || e.ctrlKey)}
                  className={cn(
                    "mx-1.5 flex w-[calc(100%-0.75rem)] items-center gap-2.5 rounded-md px-2.5 py-1.5 text-left text-[12.5px]",
                    i === selected
                      ? "bg-accent text-foreground"
                      : "text-foreground/90",
                  )}
                >
                  <Glyph spec={item.icon} />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate">{item.label}</span>
                    {/* The line of prose that matched, for a hit found inside
                        a document. One line, clipped: it is evidence that this
                        is the right file, not a reading of it. */}
                    {item.snippet && (
                      <span className="mt-0.5 block truncate text-[11px] text-muted-foreground">
                        {item.snippet.map((part, pi) => (
                          <span
                            key={pi}
                            // `brand`, the accent token — a marked word is a
                            // selection, not a fill. No background: a row of
                            // highlighter blocks in a list this quiet reads as
                            // damage.
                            className={part.hit ? "text-brand" : undefined}
                          >
                            {part.text}
                          </span>
                        ))}
                      </span>
                    )}
                  </span>
                  {item.meta && (
                    <span className="shrink-0 text-[11px] text-muted-foreground">
                      {item.meta}
                    </span>
                  )}
                </button>
              );
            })}
          </div>
        ))
      )}
    </div>
  );
}
