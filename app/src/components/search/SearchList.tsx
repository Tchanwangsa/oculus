import { useEffect, useMemo, useRef, useState } from "react";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import type { IconSpec, SearchItem, SearchSection } from "@/lib/search";
import { cn } from "@/lib/utils";

/**
 * The result list for ⌘K (`app/src/components/palette/CommandPalette.tsx`) and
 * the new-tab page (`app/src/pages/NewTabPage.tsx`). Items and picking live in
 * `app/src/lib/search.ts`; nothing here reads the library or navigates.
 */

function Glyph({ spec }: { spec: IconSpec }) {
  if (spec.kind === "subject") return <SubjectIcon code={spec.code} size={15} />;
  const Icon = spec.icon;
  return <Icon size={15} className="shrink-0 text-muted-foreground" />;
}

/**
 * Keyboard selection over a list rebuilt every keystroke: clamped, and reset
 * to the first row when the list changes.
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

  /** Section start offsets, so a row knows its keyboard index. */
  const offsets = useMemo(() => {
    let n = 0;
    return sections.map((s) => {
      const start = n;
      n += s.items.length;
      return start;
    });
  }, [sections]);

  // Scroll the highlight into view via the DOM; rows rebuild every keystroke.
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
                  // `mousemove`, not `mouseenter`: arrowing scrolls rows under a
                  // resting pointer, which would steal the selection back.
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
                    {/* The matching line inside a document, clipped to one line. */}
                    {item.snippet && (
                      <span className="mt-0.5 block truncate text-[11px] text-muted-foreground">
                        {item.snippet.map((part, pi) => (
                          <span
                            key={pi}
                            // `brand`, text only — highlighter blocks read as damage.
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
