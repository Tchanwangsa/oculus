import { useRef, type KeyboardEvent } from "react";
import { CheckCircle } from "@phosphor-icons/react";
import { OpenOriginal } from "@/components/files/OpenOriginal";
import { SIDE_NAV_ROW, SIDE_NAV_ROW_ACTIVE, SIDE_NAV_ROW_IDLE } from "@/components/ui/layout/SideNav";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import { lessonProgress, slideIcon, type LessonIndex } from "@/lib/ed/lessons";
import { fmtDue } from "@/lib/format/format";
import { cn } from "@/lib/utils";

/**
 * A lesson's slide list, as in Ed: the lesson's title over its slides, each
 * with its type and a tick once completed. ↑/↓ move between slides.
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
    <aside className="flex w-60 shrink-0 flex-col border-r border-border-subtle">
      <div className="flex items-start gap-1 px-4 pt-4 pb-3">
        <div className="min-w-0 flex-1">
          <h2 className="text-[13px] font-semibold leading-snug text-foreground">{title}</h2>
          <p className="mt-0.5 truncate text-[11px] text-muted-foreground">
            {index.dueAt && `Due ${fmtDue(index.dueAt)} · `}
            <span className="tabular-nums">{done}/{total}</span> completed
          </p>
        </div>
        {edUrl && (
          <OpenOriginal href={edUrl} label="Open in Ed" />
        )}
      </div>
      <div ref={listRef} onKeyDown={onKeyDown} className="min-h-0 flex-1 space-y-px overflow-y-auto px-2 pb-3">
        {index.slides.map((slide, position) => {
          const Icon = slideIcon(slide.type);
          const active = position === selected;
          return (
            <button
              key={slide.id ?? position}
              type="button"
              data-slide
              aria-current={active ? "true" : undefined}
              onClick={() => onSelect(position)}
              className={cn(
                SIDE_NAV_ROW,
                "text-left",
                active ? SIDE_NAV_ROW_ACTIVE : SIDE_NAV_ROW_IDLE,
                // Not opened in Ed yet: a shade stronger, as Ed marks it.
                !active && slide.status === "unseen" && "text-foreground",
              )}
            >
              <Icon size={14} className="shrink-0 opacity-70" aria-hidden />
              <span title={slide.title} className="min-w-0 flex-1 truncate">{slide.title}</span>
              {slide.status === "completed" && (
                <CheckCircle size={13} weight="fill" className="shrink-0 text-success" aria-label="Completed" />
              )}
            </button>
          );
        })}
      </div>
    </aside>
  );
}
