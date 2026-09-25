import { ArrowsOutSimple, X } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";

interface PanelHeaderProps {
  title: string;
  /** Promote to a full page: in this tab, or a new one on ⌘-click (`newTab`). */
  onExpand?: (newTab: boolean) => void;
  onClose: () => void;
  /** Right-aligned controls (e.g. the PDF ↔ Markdown toggle). */
  actions?: React.ReactNode;
}

/** The side panel's title row, rendered by each item since only it knows its
 *  expand target and actions. */
export function PanelHeader({ title, onExpand, onClose, actions }: PanelHeaderProps) {
  return (
    <div className="h-10 shrink-0 flex items-center gap-0.5 px-2 border-b border-border-subtle">
      {onExpand && (
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon-xs"
              onClick={(e) => onExpand(e.metaKey || e.ctrlKey)}
              aria-label="Open as full page"
              className="text-muted-foreground hover:text-foreground"
            >
              <ArrowsOutSimple size={14} />
            </Button>
          </TooltipTrigger>
          <TooltipContent className="flex flex-col items-start gap-0.5">
            Open as full page
            <span className="text-[11px] text-background/60">⌘-click for a new tab</span>
          </TooltipContent>
        </Tooltip>
      )}

      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="ghost"
            size="icon-xs"
            onClick={onClose}
            aria-label="Close"
            className="text-muted-foreground hover:text-foreground"
          >
            <X size={14} />
          </Button>
        </TooltipTrigger>
        <TooltipContent>Close (Esc)</TooltipContent>
      </Tooltip>

      <span className="flex-1 min-w-0 truncate text-[12px] font-medium text-foreground px-1.5">
        {title}
      </span>

      {actions}
    </div>
  );
}
