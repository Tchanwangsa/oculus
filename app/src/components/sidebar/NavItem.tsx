import { cn } from "@/lib/utils";
import { navigateActive } from "@/lib/tabRouters";
import { useActivePath } from "@/stores/tabStore";
import type { Icon as PhosphorIcon } from "@phosphor-icons/react";

interface NavItemProps {
  to: string;
  icon: PhosphorIcon;
  label: string;
  /** Other paths this row also lights on, tested like `to` — for a section
   *  row whose tabs aren't under one prefix (Tasks: `/projects` + `/tasks`). */
  match?: readonly string[];
  /** Trailing slot, e.g. an unread count. */
  badge?: React.ReactNode;
}

/**
 * A sidebar row. Not a `NavLink`: the sidebar is outside every tab's router,
 * so it navigates the front tab via `navigateActive`; ⌘-click opens a new tab
 * off `data-tab-href` (`app/src/lib/newTabClicks.ts`).
 */
export default function NavItem({ to, icon: Icon, label, match, badge }: NavItemProps) {
  // Prefix match on `${to}/`, so Home (`/`) tests "//" and lights only on `/`.
  const here = useActivePath().split("?")[0];
  const lightsOn = (path: string) => here === path || here.startsWith(`${path}/`);
  const isActive = lightsOn(to) || (match?.some(lightsOn) ?? false);

  return (
    <button
      type="button"
      /* Not an href (outside every router); read by the ⌘-click net. */
      data-tab-href={to}
      onClick={() => navigateActive(to)}
      className={cn(
        "flex w-full items-center gap-2.5 rounded-md px-2 py-1.5 text-[12.5px] transition-colors",
        isActive
          ? "bg-sidebar-item-active text-foreground font-medium"
          : "text-muted-foreground font-normal hover:bg-sidebar-item-hover hover:text-foreground",
      )}
    >
      <Icon size={16} className="shrink-0" />
      <span className="flex-1 min-w-0 overflow-hidden whitespace-nowrap text-clip text-left">
        {label}
      </span>
      {badge}
    </button>
  );
}
