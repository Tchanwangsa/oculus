import { useRef, type ReactNode, type Ref } from "react";
import { NavLink } from "react-router-dom";
import { MagnifyingGlass, SidebarSimple, X, type Icon } from "@phosphor-icons/react";

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import { cn } from "@/lib/utils";

/** A nav row's look; exported for rows that are not links (search results). */
export const SIDE_NAV_ROW =
  "flex w-full items-center gap-2.5 rounded-md px-2 py-1.5 text-[12.5px] transition-colors";
export const SIDE_NAV_ROW_IDLE =
  "text-muted-foreground font-normal hover:bg-sidebar-item-hover hover:text-foreground";
export const SIDE_NAV_ROW_ACTIVE = "bg-sidebar-item-active text-foreground font-medium";

/** Fades a label out while the column folds away, so it is gone before the
 *  narrowing width can be seen truncating it. */
export const SIDE_NAV_FOLDS =
  "transition-opacity will-change-[opacity] duration-150 group-data-[collapsed=true]/nav:opacity-0";

/**
 * A page's own nav column beside its content: `header` stays pinned and
 * `children` scroll under it, with the scrollbar hidden like the app
 * sidebar's and the fades carrying the affordance. `width` makes it the
 * caller's (a resizable column); `collapsed` folds it away to nothing, the
 * way back being `SideNavCollapseToggle` in the page's header.
 */
export function SideNav({
  header,
  width,
  collapsed = false,
  animate = false,
  children,
  "aria-label": ariaLabel,
}: {
  header: ReactNode;
  /** Drawn width in px; 208 when unset. */
  width?: number;
  collapsed?: boolean;
  /** Ease width changes (a fold); off while dragging. */
  animate?: boolean;
  children: ReactNode;
  "aria-label"?: string;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);
  useScrollFade(scrollRef);

  return (
    <nav
      aria-label={ariaLabel}
      data-collapsed={collapsed}
      // A zero-width column is gone: no seam border, nothing to tab into.
      inert={width === 0}
      style={width == null ? undefined : { width }}
      className={cn(
        "group/nav flex h-full shrink-0 flex-col overflow-hidden border-r border-border-subtle bg-background",
        width == null && "w-52",
        width === 0 && "border-r-0",
        animate && "transition-[width] duration-200 ease-out",
      )}
    >
      <div className="shrink-0">{header}</div>
      <div
        ref={scrollRef}
        className="min-h-0 flex-1 overflow-y-auto overflow-x-hidden px-2 pt-1 pb-3 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
      >
        {/* One wrapper: the fade's ResizeObserver watches the first child. */}
        <div>{children}</div>
      </div>
    </nav>
  );
}

/** Folds or unfolds the column, from the page's header row beside its title,
 *  so it stays in one place in both states. */
export function SideNavCollapseToggle({
  collapsed,
  onToggle,
  shortcut,
}: {
  collapsed: boolean;
  onToggle: () => void;
  /** Shown after the label in the tooltip, e.g. "⌘⌥B". */
  shortcut?: string;
}) {
  const label = collapsed ? "Expand sidebar" : "Collapse sidebar";
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          aria-label={label}
          onClick={onToggle}
          className="flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-sidebar-item-hover hover:text-foreground"
        >
          <SidebarSimple size={16} />
        </button>
      </TooltipTrigger>
      <TooltipContent side="bottom">{shortcut ? `${label} (${shortcut})` : label}</TooltipContent>
    </Tooltip>
  );
}

/** A route row; `trailing` sits at the row's end (a count badge). */
export function SideNavLink({
  to,
  end,
  icon: RowIcon,
  label,
  trailing,
}: {
  to: string;
  end?: boolean;
  icon: Icon;
  label: string;
  trailing?: ReactNode;
}) {
  return (
    <NavLink
      to={to}
      end={end}
      className={({ isActive }) => cn(SIDE_NAV_ROW, isActive ? SIDE_NAV_ROW_ACTIVE : SIDE_NAV_ROW_IDLE)}
    >
      <RowIcon size={16} className="shrink-0" />
      <span className={cn("min-w-0 flex-1 truncate", SIDE_NAV_FOLDS)}>{label}</span>
      {trailing && <span className={cn("flex shrink-0", SIDE_NAV_FOLDS)}>{trailing}</span>}
    </NavLink>
  );
}

/** The pill search field under a column's header. Escape clears it and Enter
 *  calls `onEnter` (the caller opens its first result). */
export function SideNavSearch({
  value,
  onChange,
  onEnter,
  placeholder,
  inputRef,
}: {
  value: string;
  onChange: (value: string) => void;
  onEnter: () => void;
  placeholder: string;
  inputRef?: Ref<HTMLInputElement>;
}) {
  return (
    <div className="px-2 pb-2">
      <div
        className={cn(
          "flex h-8 items-center gap-2 rounded-full border border-input bg-card px-3 transition-[color,box-shadow] focus-within:border-ring focus-within:ring-[3px] focus-within:ring-ring/25",
          SIDE_NAV_FOLDS,
        )}
      >
        <MagnifyingGlass size={13} className="shrink-0 text-muted-foreground" />
        <input
          ref={inputRef}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape" && value) {
              e.preventDefault();
              onChange("");
            } else if (e.key === "Enter") {
              e.preventDefault();
              onEnter();
            }
          }}
          placeholder={placeholder}
          aria-label={placeholder}
          spellCheck={false}
          autoComplete="off"
          className="min-w-0 flex-1 bg-transparent text-[13px] text-foreground outline-none placeholder:text-muted-foreground"
        />
        {value && (
          <button
            type="button"
            aria-label="Clear search"
            onClick={() => onChange("")}
            className="-mr-1 flex h-5 w-5 shrink-0 items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-sidebar-item-hover hover:text-foreground"
          >
            <X size={11} weight="bold" />
          </button>
        )}
      </div>
    </div>
  );
}

export function SideNavGroupLabel({ children }: { children: ReactNode }) {
  return (
    <div className="px-2 py-1 mb-0.5 text-[11px] font-medium tracking-wide text-muted-foreground">
      {children}
    </div>
  );
}
