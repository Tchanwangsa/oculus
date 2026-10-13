import { CheckCircle } from "@phosphor-icons/react";
import { SIDE_NAV_FOLDS, SIDE_NAV_ROW, SIDE_NAV_ROW_ACTIVE, SIDE_NAV_ROW_IDLE } from "@/components/ui/layout/SideNav";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { slideIcon, type LessonSlide } from "@/lib/ed/lessons";
import { cn } from "@/lib/utils";

/** Shown over the glyph only in the folded rail, where the row's own tick and
 *  shade are gone with its label. */
const RAIL_ONLY =
  "pointer-events-none absolute opacity-0 transition-opacity will-change-[opacity] duration-150 group-data-[collapsed=true]/nav:opacity-100";

/**
 * One slide in `LessonSidebar`: its type's glyph, its title and a tick once
 * completed; a slide not opened in Ed yet reads a shade stronger, as Ed marks
 * it. Folded to the rail it is the glyph alone, with a small tick or an
 * unseen dot on it and the title as a tooltip.
 */
export function SlideRow({
  slide,
  active,
  collapsed,
  onSelect,
}: {
  slide: LessonSlide;
  active: boolean;
  /** The sidebar is folded to its rail. */
  collapsed: boolean;
  onSelect: () => void;
}) {
  const Icon = slideIcon(slide.type);
  const done = slide.status === "completed";
  const unseen = slide.status === "unseen";
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          data-slide
          aria-current={active ? "true" : undefined}
          onClick={onSelect}
          className={cn(
            SIDE_NAV_ROW,
            "text-left",
            active ? SIDE_NAV_ROW_ACTIVE : SIDE_NAV_ROW_IDLE,
            !active && unseen && "text-foreground",
          )}
        >
          <span className="relative flex shrink-0">
            <Icon size={14} className="opacity-70" aria-hidden />
            {done && (
              <CheckCircle size={9} weight="fill" className={cn(RAIL_ONLY, "-right-[5px] -bottom-[4px] text-success")} aria-hidden />
            )}
            {unseen && <span className={cn(RAIL_ONLY, "-top-[2px] -right-[3px] size-[5px] rounded-full bg-brand")} aria-hidden />}
          </span>
          <span title={collapsed ? undefined : slide.title} className={cn("min-w-0 flex-1 truncate", SIDE_NAV_FOLDS)}>
            {slide.title}
          </span>
          {done && (
            <CheckCircle
              size={13}
              weight="fill"
              className={cn("shrink-0 text-success", SIDE_NAV_FOLDS)}
              aria-label="Completed"
            />
          )}
        </button>
      </TooltipTrigger>
      {collapsed && (
        <TooltipContent side="right" sideOffset={6}>
          {slide.title}
        </TooltipContent>
      )}
    </Tooltip>
  );
}
