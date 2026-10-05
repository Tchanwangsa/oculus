import { useRef, type ReactNode } from "react";
import { NavLink } from "react-router-dom";
import type { Icon } from "@phosphor-icons/react";

import { useScrollFade } from "@/hooks/useScrollFade";
import { cn } from "@/lib/utils";

/** A nav row's look; exported for rows that are not links (Settings' search results). */
export const SIDE_NAV_ROW =
  "flex w-full items-center gap-2.5 rounded-md px-2 py-1.5 text-[12.5px] transition-colors";
export const SIDE_NAV_ROW_IDLE =
  "text-muted-foreground font-normal hover:bg-sidebar-item-hover hover:text-foreground";
export const SIDE_NAV_ROW_ACTIVE = "bg-sidebar-item-active text-foreground font-medium";

/**
 * A page's own nav column beside its content: `header` stays pinned and
 * `children` scroll beneath it, with the scrollbar hidden like the app
 * sidebar's and the fades carrying the affordance.
 */
export function SideNav({
  header,
  children,
  "aria-label": ariaLabel,
}: {
  header: ReactNode;
  children: ReactNode;
  "aria-label"?: string;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);
  useScrollFade(scrollRef);

  return (
    <nav
      aria-label={ariaLabel}
      className="flex h-full w-52 shrink-0 flex-col border-r border-border-subtle bg-background"
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

/** A route row; `trailing` sits at the row's end (a count badge). */
export function SideNavLink({
  to,
  icon: RowIcon,
  label,
  end,
  trailing,
}: {
  to: string;
  icon: Icon;
  label: string;
  end?: boolean;
  trailing?: ReactNode;
}) {
  return (
    <NavLink
      to={to}
      end={end}
      className={({ isActive }) => cn(SIDE_NAV_ROW, isActive ? SIDE_NAV_ROW_ACTIVE : SIDE_NAV_ROW_IDLE)}
    >
      <RowIcon size={16} className="shrink-0" />
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {trailing}
    </NavLink>
  );
}

export function SideNavGroupLabel({ children }: { children: ReactNode }) {
  return (
    <div className="px-2 py-1 mb-0.5 text-[11px] font-medium tracking-wide text-muted-foreground">
      {children}
    </div>
  );
}
