import { useRef, type KeyboardEvent } from "react";
import { OpenOriginal } from "@/components/files/OpenOriginal";
import { SideNavCollapseToggle } from "@/components/ui/layout/SideNav";
import { useResizablePanel } from "@/hooks/gestures/useResizablePanel";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import { lessonProgress, type LessonIndex } from "@/lib/ed/lessons";
import { fmtDue } from "@/lib/format/format";
import { SlideRow } from "./SlideRow";

/** A fixed column that folds to a rail of glyphs, the fold remembered. The
 *  rail is a 14px glyph inside the rows' 8px padding and the list's 8px
 *  gutter, so the glyphs and the toggle share one centre line. */
const SIDEBAR = {
  defaultWidth: 240,
  minWidth: 240,
  maxWidth: 240,
  collapsedWidth: 46,
  storageKey: "oculus-ed-lesson-sidebar",
} as const;

/**
 * A lesson's slide list, as in Ed: the lesson's title over its slides, each
 * with its type and a tick once completed. The toggle folds it to a rail of
 * slide glyphs (`SlideRow`). ↑/↓ move between slides.
 */
export function LessonSidebar({
  title,
  index,
  selected,
  onSelect,
  edUrl,
}: {
  title: string;
  index: LessonIndex;
  /** Position of the slide shown. */
  selected: number;
  onSelect: (position: number) => void;
  /** The shown slide's page in Ed, else the lesson's. */
  edUrl: string | null;
}) {
  const panel = useResizablePanel(SIDEBAR);
  const { collapsed } = panel;
  const listRef = useRef<HTMLDivElement>(null);
  useScrollFade(listRef, "y", index.slides.length);
  const { done, total } = lessonProgress(index);

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const step = e.key === "ArrowDown" ? 1 : e.key === "ArrowUp" ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    const next = Math.min(total - 1, Math.max(0, selected + step));
    if (next === selected) return;
    onSelect(next);
    listRef.current?.querySelectorAll<HTMLButtonElement>("[data-slide]")[next]?.focus();
  };

  return (
    <aside
      aria-label="Slides"
      data-collapsed={collapsed}
      style={{ width: panel.width }}
      className="group/nav flex shrink-0 flex-col overflow-hidden border-r border-border-subtle transition-[width] duration-200 ease-out"
    >
      {/* Laid out at the open width, so a fold clips the title, never rewraps it. */}
      <div style={{ width: panel.restWidth }} className="flex items-start gap-1.5 px-[7px] pt-3 pb-2">
        <SideNavCollapseToggle collapsed={collapsed} onToggle={panel.toggle} />
        {!collapsed && (
          <>
            <div className="min-w-0 flex-1 pt-1.5">
              <h2 className="text-[13px] font-semibold leading-snug text-foreground">{title}</h2>
              <p className="mt-0.5 truncate text-[11px] text-muted-foreground">
                {index.dueAt && `Due ${fmtDue(index.dueAt)} · `}
                <span className="tabular-nums">{done}/{total}</span> completed
              </p>
            </div>
            {edUrl && <OpenOriginal href={edUrl} label="Open in Ed" />}
          </>
        )}
      </div>
      <div
        ref={listRef}
        onKeyDown={onKeyDown}
        // The scrollbar hidden as in `SideNav`, so the rail keeps its centre line.
        className="min-h-0 flex-1 space-y-px overflow-y-auto overflow-x-hidden px-2 pb-3 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
      >
        {index.slides.map((slide, position) => (
          <SlideRow
            key={slide.id ?? position}
            slide={slide}
            active={position === selected}
            collapsed={collapsed}
            onSelect={() => onSelect(position)}
          />
        ))}
      </div>
    </aside>
  );
}
