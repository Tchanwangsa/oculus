import type { ReactNode } from "react";
import { CaretDown, CaretRight } from "@phosphor-icons/react";
import { SIDE_NAV_ROW, SIDE_NAV_ROW_ACTIVE, SIDE_NAV_ROW_IDLE } from "@/components/ui/layout/SideNav";
import type { DropHover } from "@/hooks/drive/useDriveDrop";
import { cn } from "@/lib/utils";

/** Pixels each level of the tree steps in. */
const INDENT = 12;

/**
 * One row of the folder tree, in the nav column's row look: a disclosure
 * caret (blank when the folder holds no folders), the glyph, the name and a
 * muted count. The caret is a `span` with a button role, since the row is
 * itself clickable (`docs/ui.md`); it swaps icons and never rotates. A row
 * that is a folder carries the drop attributes `useDriveDrop` reads.
 */
export function TreeRow({
  depth,
  icon,
  label,
  title,
  count,
  expanded,
  onToggle,
  active,
  disabled = false,
  href,
  onActivate,
  drop,
  hover,
}: {
  depth: number;
  icon: ReactNode;
  label: string;
  title?: string;
  count?: number;
  /** Undefined: nothing to expand. */
  expanded?: boolean;
  onToggle?: () => void;
  active: boolean;
  disabled?: boolean;
  href?: string;
  onActivate: () => void;
  drop?: { rel: string; writable: boolean };
  hover: DropHover | null;
}) {
  const over = drop && hover?.rel === drop.rel ? (hover.ok ? "ok" : "refused") : null;
  const Caret = expanded ? CaretDown : CaretRight;
  return (
    <div
      role="treeitem"
      aria-expanded={expanded}
      aria-selected={active}
      aria-disabled={disabled || undefined}
      tabIndex={disabled ? -1 : 0}
      title={title ?? label}
      data-tab-href={href}
      data-drop-rel={drop?.rel}
      data-drop-writable={drop ? String(drop.writable) : undefined}
      onClick={() => !disabled && onActivate()}
      onKeyDown={(e) => {
        if (disabled) return;
        if (e.key === "Enter") onActivate();
        else if (e.key === "ArrowRight" && expanded === false) onToggle?.();
        else if (e.key === "ArrowLeft" && expanded) onToggle?.();
      }}
      style={{ paddingLeft: 8 + depth * INDENT }}
      className={cn(
        SIDE_NAV_ROW,
        "cursor-pointer gap-1.5 outline-none select-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/50",
        over === "ok" ? "bg-brand/10 text-foreground" : over === "refused" ? "opacity-50" : active ? SIDE_NAV_ROW_ACTIVE : SIDE_NAV_ROW_IDLE,
        disabled && "cursor-default opacity-50 hover:bg-transparent",
      )}
    >
      {expanded == null ? (
        <span className="size-3.5 shrink-0" />
      ) : (
        <span
          role="button"
          tabIndex={-1}
          aria-label={expanded ? `Fold ${label}` : `Open ${label}`}
          data-tab-skip
          onClick={(e) => {
            e.stopPropagation();
            onToggle?.();
          }}
          className="flex size-3.5 shrink-0 items-center justify-center rounded-sm text-muted-foreground/70 hover:text-foreground"
        >
          <Caret size={10} />
        </span>
      )}
      <span className="flex size-4 shrink-0 items-center justify-center">{icon}</span>
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {over ? (
        <span className={cn("shrink-0 text-[11px]", over === "ok" ? "text-brand" : "text-muted-foreground")}>
          {over === "ok" ? "Drop" : "Can't add"}
        </span>
      ) : count != null && count > 0 ? (
        <span className="shrink-0 text-[11px] text-muted-foreground/70 tabular-nums">{count}</span>
      ) : null}
    </div>
  );
}
