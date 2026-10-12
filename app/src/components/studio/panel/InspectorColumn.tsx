import { useRef, type ReactNode } from "react";
import { SidebarSimple } from "@phosphor-icons/react";

import { ResizeHandle } from "@/components/ui/layout/ResizeHandle";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useResizablePanel } from "@/hooks/gestures/useResizablePanel";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import { cn } from "@/lib/utils";

/** An inspector's width bounds. Folded, it is gone entirely. */
const INSPECTOR_PANEL = {
  defaultWidth: 296,
  minWidth: 264,
  maxWidth: 420,
  side: "right",
  collapsedWidth: 0,
} as const;

/**
 * A page's inspector column: its width and fold, remembered per page under
 * `storageKey`. The page holds it so the header's toggle and the column
 * beside the stage share one state.
 */
export function useInspectorPanel(storageKey: string) {
  return useResizablePanel({ ...INSPECTOR_PANEL, storageKey });
}

export type InspectorPanel = ReturnType<typeof useInspectorPanel>;

/** How wide the stage's column may grow: more of the window once the
 *  inspector is folded away. */
export const stageMaxWidth = (panel: InspectorPanel) => (panel.collapsed ? "max-w-[1080px]" : "max-w-[920px]");

/** Folds or unfolds the inspector, from the page's header row. */
export function InspectorToggle({ panel }: { panel: InspectorPanel }) {
  const label = panel.collapsed ? "Show inspector" : "Hide inspector";
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          aria-label={label}
          aria-pressed={!panel.collapsed}
          onClick={panel.toggle}
          className="flex size-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-sidebar-item-hover hover:text-foreground"
        >
          <SidebarSimple size={16} mirrored weight={panel.collapsed ? "regular" : "fill"} />
        </button>
      </TooltipTrigger>
      <TooltipContent side="bottom">{label}</TooltipContent>
    </Tooltip>
  );
}

/**
 * The column beside the stage: a header row (a title and its actions) over a
 * scroller for the sections. It is drag-resizable from its seam and folds to
 * nothing; the content keeps its unfolded width while it eases shut, so it is
 * clipped rather than reflowed.
 */
export function InspectorColumn({
  panel,
  label,
  title,
  actions,
  children,
}: {
  panel: InspectorPanel;
  /** The column's accessible name. */
  label: string;
  title: string;
  actions?: ReactNode;
  children: ReactNode;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);
  useScrollFade(scrollRef);
  return (
    <>
      {/* On the seam: negative margins cost no layout width. */}
      {!panel.collapsed && (
        <ResizeHandle onMouseDown={panel.onMouseDown} dragging={panel.dragging} label={`Resize ${label.toLowerCase()}`} className="-mx-0.5" />
      )}
      <aside
        aria-label={label}
        inert={panel.width === 0}
        style={{ width: panel.width }}
        className={cn(
          "min-h-0 shrink-0 overflow-hidden border-l border-border-subtle",
          panel.width === 0 && "border-l-0",
          !panel.dragging && "transition-[width] duration-200 ease-out",
        )}
      >
        <div style={{ width: panel.restWidth }} className="flex h-full flex-col">
          <div className="flex h-10 shrink-0 items-center gap-1 border-b border-border-subtle pl-4 pr-3">
            <h2 className="min-w-0 flex-1 truncate text-[12px] font-medium text-foreground">{title}</h2>
            {actions}
          </div>
          <div ref={scrollRef} className="min-h-0 flex-1 overflow-y-scroll pb-6 pl-4 pr-2.5">
            <div>{children}</div>
          </div>
        </div>
      </aside>
    </>
  );
}
