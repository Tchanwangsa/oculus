import { createContext, useContext, useRef, useState, type ComponentProps, type ReactNode, type Ref } from "react";
import { NavLink } from "react-router-dom";
import { MagnifyingGlass, SidebarSimple, X, type Icon } from "@phosphor-icons/react";

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useScrollFade } from "@/hooks/useScrollFade";
import { cn } from "@/lib/utils";

/** A nav row's look; exported for rows that are not links (search results). */
export const SIDE_NAV_ROW =
  "flex w-full items-center gap-2.5 rounded-md px-2 py-1.5 text-[12.5px] transition-colors";
export const SIDE_NAV_ROW_IDLE =
  "text-muted-foreground font-normal hover:bg-sidebar-item-hover hover:text-foreground";
export const SIDE_NAV_ROW_ACTIVE = "bg-sidebar-item-active text-foreground font-medium";

/** Whether the column is folded to its icons; rows read it for tooltips. */
const CollapsedContext = createContext(false);

/** The folded column's width: a nav row's icon, centred. */
export const SIDE_NAV_COLLAPSED_WIDTH = 48;

/** Fades a label out while the column is collapsed. The layout is the same in
 *  both states and the column clips it, so icons never move as it folds. */
export const SIDE_NAV_FOLDS =
  "transition-opacity duration-150 group-data-[collapsed=true]/nav:opacity-0";

/**
 * A page's own nav column beside its content: `header` and `footer` stay
 * pinned and `children` scroll between them, with the scrollbar hidden like
 * the app sidebar's and the fades carrying the affordance. `width` makes it
 * the caller's (a resizable column); `collapsed` folds it to
 * `SIDE_NAV_COLLAPSED_WIDTH`, every icon named by its tooltip.
 */
export function SideNav({
  header,
  footer,
  width,
  collapsed = false,
  animate = false,
  children,
  "aria-label": ariaLabel,
}: {
  header: ReactNode;
  footer?: ReactNode;
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
      style={width == null ? undefined : { width }}
      className={cn(
        "group/nav flex h-full shrink-0 flex-col overflow-hidden border-r border-border-subtle bg-background",
        width == null && "w-52",
        animate && "transition-[width] duration-200 ease-out",
      )}
    >
      <CollapsedContext.Provider value={collapsed}>
        <div className="shrink-0">{header}</div>
        <div
          ref={scrollRef}
          className="min-h-0 flex-1 overflow-y-auto overflow-x-hidden px-2 pt-1 pb-3 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
        >
          {/* One wrapper: the fade's ResizeObserver watches the first child. */}
          <div>{children}</div>
        </div>
        {footer && <div className="shrink-0 px-2 pb-3">{footer}</div>}
      </CollapsedContext.Provider>
    </nav>
  );
}

/** A control's name beside it, shown only while the column is collapsed and
 *  its labels are hidden — or always, for an icon that never has one. */
export function SideNavTip({
  label,
  always = false,
  children,
}: {
  label: string;
  always?: boolean;
  children: ReactNode;
}) {
  const collapsed = useContext(CollapsedContext);
  const [open, setOpen] = useState(false);
  return (
    <Tooltip open={open && (always || collapsed)} onOpenChange={setOpen}>
      <TooltipTrigger asChild>{children}</TooltipTrigger>
      <TooltipContent side="right">{label}</TooltipContent>
    </Tooltip>
  );
}

/** Folds or unfolds the column; it sits in the footer in both states, so
 *  the way back is where the way out was. */
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
    <SideNavTip label={shortcut ? `${label} (${shortcut})` : label} always>
      <button
        type="button"
        aria-label={label}
        onClick={onToggle}
        className="flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-sidebar-item-hover hover:text-foreground"
      >
        <SidebarSimple size={16} />
      </button>
    </SideNavTip>
  );
}

/** A route row; `trailing` sits at the row's end (a count badge) and `dot`
 *  marks the icon's corner in its place while the column is collapsed. */
export function SideNavLink(props: {
  to: string;
  icon: Icon;
  label: string;
  end?: boolean;
  trailing?: ReactNode;
  dot?: boolean;
}) {
  return (
    <SideNavTip label={props.label}>
      <RowLink {...props} />
    </SideNavTip>
  );
}

/**
 * Its own component because the tooltip's `Slot` joins a child's `className`
 * as a string, which mangles NavLink's function form; here the trigger's
 * props pass through untouched.
 */
function RowLink({
  to,
  end,
  icon: RowIcon,
  label,
  trailing,
  dot,
  ...trigger
}: {
  to: string;
  end?: boolean;
  icon: Icon;
  label: string;
  trailing?: ReactNode;
  dot?: boolean;
} & Omit<ComponentProps<typeof NavLink>, "to" | "end" | "className" | "children">) {
  return (
    <NavLink
      {...trigger}
      to={to}
      end={end}
      className={({ isActive }) => cn(
        SIDE_NAV_ROW,
        "relative overflow-hidden",
        isActive ? SIDE_NAV_ROW_ACTIVE : SIDE_NAV_ROW_IDLE,
      )}
    >
      <RowIcon size={16} className="shrink-0" />
      <span className={cn("min-w-0 flex-1 truncate", SIDE_NAV_FOLDS)}>{label}</span>
      {trailing && <span className={cn("flex shrink-0", SIDE_NAV_FOLDS)}>{trailing}</span>}
      {dot && (
        <span
          aria-hidden
          className="absolute top-1 left-5 size-1.5 rounded-full bg-brand opacity-0 transition-opacity duration-150 group-data-[collapsed=true]/nav:opacity-100"
        />
      )}
    </NavLink>
  );
}

/** The pill search field under a column's header. Escape clears it and Enter
 *  calls `onEnter` (the caller opens its first result). Collapsed, an icon
 *  button takes the pill's slot and calls `onExpand`. */
export function SideNavSearch({
  value,
  onChange,
  onEnter,
  onExpand,
  placeholder,
  inputRef,
}: {
  value: string;
  onChange: (value: string) => void;
  onEnter: () => void;
  onExpand?: () => void;
  placeholder: string;
  inputRef?: Ref<HTMLInputElement>;
}) {
  const collapsed = useContext(CollapsedContext);
  return (
    <div className="relative px-2 pb-2">
      <div
        className={cn(
          "flex h-8 items-center gap-2 rounded-full border border-input bg-card px-3 transition-[color,box-shadow,opacity] focus-within:border-ring focus-within:ring-[3px] focus-within:ring-ring/25",
          collapsed && "pointer-events-none opacity-0",
        )}
      >
        <MagnifyingGlass size={13} className="shrink-0 text-muted-foreground" />
        <input
          ref={inputRef}
          value={value}
          tabIndex={collapsed ? -1 : undefined}
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
      {onExpand && (
        <SideNavTip label={placeholder}>
          <button
            type="button"
            aria-label={placeholder}
            tabIndex={collapsed ? undefined : -1}
            onClick={onExpand}
            className={cn(
              "absolute top-0 left-2 flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-[color,background-color,opacity] hover:bg-sidebar-item-hover hover:text-foreground",
              !collapsed && "pointer-events-none opacity-0",
            )}
          >
            <MagnifyingGlass size={16} />
          </button>
        </SideNavTip>
      )}
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
