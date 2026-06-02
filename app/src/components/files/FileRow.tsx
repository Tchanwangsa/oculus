import { FileText, RefreshCw } from "lucide-react";
import { cn } from "@/lib/utils";

interface FileRowProps {
  icon: typeof FileText;
  label: string;
  size: string;
  active: boolean;
  onClick: () => void;
  dimmed?: boolean;
  rightIcon?: typeof FileText;
  onRescrape?: () => void;
  isRescaping?: boolean;
}

export function FileRow({
  icon: Icon,
  label,
  size,
  active,
  onClick,
  dimmed = false,
  rightIcon: RightIcon,
  onRescrape,
  isRescaping = false,
}: FileRowProps) {
  return (
    <div
      onClick={onClick}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => e.key === "Enter" && onClick()}
      className={cn(
        "w-full text-left px-3 py-2 mx-1 rounded-lg flex items-center gap-2.5 transition-colors cursor-pointer group",
        active
          ? "bg-surface-raised text-foreground"
          : "text-muted-foreground hover:bg-surface hover:text-foreground",
        dimmed && !active && "opacity-70",
      )}
      style={{ width: "calc(100% - 8px)" }}
    >
      <Icon size={13} className="shrink-0" />
      <span className="text-xs flex-1 truncate">{label}</span>
      <span className="text-[10px] text-muted-foreground/70">{size}</span>
      {onRescrape ? (
        <button
          title={isRescaping ? "Re-downloading..." : "Re-download file"}
          disabled={isRescaping}
          onClick={(e) => {
            e.stopPropagation();
            onRescrape();
          }}
          className={cn(
            "shrink-0 p-0.5 rounded transition-opacity",
            isRescaping
              ? "opacity-60"
              : "opacity-0 group-hover:opacity-70 hover:!opacity-100 hover:text-foreground",
          )}
        >
          <RefreshCw size={10} className={cn(isRescaping && "animate-spin")} />
        </button>
      ) : RightIcon ? (
        <RightIcon size={10} className="shrink-0 opacity-50" />
      ) : null}
    </div>
  );
}
