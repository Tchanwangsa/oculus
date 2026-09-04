import {
  Chat,
  CalendarBlank,
  ArrowsClockwise,
  CaretDoubleLeft,
  GearSix,
} from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import NavItem from "./NavItem";
import SubjectsNavGroup from "./SubjectsNavGroup";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";

interface SidebarProps {
  collapsed: boolean;
  onToggle: () => void;
}

const WIDTH = 220;

/**
 * Collapsed means gone: the sidebar animates to zero width (no icon rail), and
 * the way back in is the sidebar button in the title bar — Notion's pattern.
 */
export default function Sidebar({ collapsed, onToggle }: SidebarProps) {
  const width = collapsed ? 0 : WIDTH;
  return (
    <aside
      /* width/min/max are pinned together so flexbox can never clamp the box to
         its min-content size mid-transition. */
      style={{ width, minWidth: width, maxWidth: width }}
      className={cn(
        "group/sidebar flex flex-col h-full shrink-0 grow-0 overflow-hidden bg-sidebar",
        "transition-[width,min-width,max-width] duration-200 ease-out",
        !collapsed && "border-r border-sidebar-border",
      )}
    >
      {/* Inner keeps its full width during the slide so content doesn't reflow,
          it just gets clipped. */}
      <div className="flex flex-col h-full" style={{ width: WIDTH, minWidth: WIDTH }}>
        {/* Header: logo + collapse toggle */}
        <div className="relative flex items-center h-13 px-[13px] shrink-0">
          <img src="/oculus-mark.svg" alt="" className="w-[22px] h-[22px] shrink-0" />
          <span className="ml-2.5 flex-1 min-w-0 overflow-hidden whitespace-nowrap font-semibold text-foreground tracking-tight text-[13px]">
            Oculus
          </span>

          <Tooltip>
            <TooltipTrigger asChild>
              <button
                type="button"
                onClick={onToggle}
                aria-label="Close sidebar"
                className={cn(
                  "absolute top-1/2 -translate-y-1/2 right-2 flex h-7 w-7 items-center justify-center rounded-full",
                  "text-muted-foreground hover:text-foreground hover:bg-sidebar-item-hover active:bg-sidebar-item-active",
                  "opacity-0 focus-visible:opacity-100 group-hover/sidebar:opacity-100 transition-[opacity,color,background-color] duration-150",
                )}
              >
                <CaretDoubleLeft size={13} />
              </button>
            </TooltipTrigger>
            <TooltipContent side="bottom" align="end" className="flex flex-col items-start gap-0.5">
              Close sidebar
              <span className="text-[11px] text-background/60">⌘\</span>
            </TooltipContent>
          </Tooltip>
        </div>

        {/* Main nav */}
        <nav className="flex-1 min-h-0 py-1 px-2 space-y-px overflow-y-auto overflow-x-hidden">
          <NavItem to="/chat" icon={Chat} label="Chat" />
          <NavItem to="/calendar" icon={CalendarBlank} label="Calendar" />
          <div className="pt-6">
            <SubjectsNavGroup />
          </div>
        </nav>

        {/* Bottom nav */}
        <div className="py-2 px-2 space-y-px border-t border-sidebar-border">
          <NavItem to="/settings" icon={GearSix} label="Settings" />
          <NavItem to="/sync" icon={ArrowsClockwise} label="Sync" />
        </div>
      </div>
    </aside>
  );
}
