import { CheckCircle } from "@phosphor-icons/react";
import { SIDE_NAV_FOLDS, SIDE_NAV_ROW_ACTIVE, SIDE_NAV_ROW_IDLE } from "@/components/ui/layout/SideNav";
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
 * it. Folded, the button is a 30px square round the glyph, as the app rail's
 * are, with a small tick or an unseen dot on it and the title as a tooltip.
 * The label is the button's sibling, laid out at the open width, so a fold
 * fades and clips it instead of squeezing it into the square.
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
    <div className="relative">
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            type="button"
            data-slide
            aria-current={active ? "true" : undefined}
            aria-label={done ? `${slide.title}, completed` : slide.title}
            title={collapsed ? undefined : slide.title}
            onClick={onSelect}
            className={cn(
              "peer flex h-[30px] items-center rounded-md px-2 transition-colors",
              collapsed ? "w-[30px]" : "w-full",
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
          </button>
        </TooltipTrigger>
        {collapsed && (
          <TooltipContent side="right" sideOffset={14}>
            {slide.title}
          </TooltipContent>
        )}
      </Tooltip>
      {/* Past the glyph: the row's 8px padding, the 14px glyph and a 10px gap. */}
      <span
        aria-hidden
        className={cn(
          "pointer-events-none absolute inset-y-0 right-2 left-8 flex items-center gap-2.5 text-[12.5px]",
          active ? "font-medium text-foreground" : unseen ? "text-foreground" : "text-muted-foreground peer-hover:text-foreground",
          SIDE_NAV_FOLDS,
        )}
      >
        <span className="min-w-0 flex-1 truncate">{slide.title}</span>
        {done && <CheckCircle size={13} weight="fill" className="shrink-0 text-success" />}
      </span>
    </div>
  );
}
