import { cn } from "@/lib/utils";
import { navigateActive } from "@/lib/tabRouters";
import { useActivePath } from "@/stores/tabStore";
import type { Icon as PhosphorIcon } from "@phosphor-icons/react";

interface NavItemProps {
  to: string;
  icon: PhosphorIcon;
  label: string;
  /**
   * Other paths this row also lights on, tested exactly as `to` is.
   *
   * For a row that leads a *section* rather than a page: Tasks lands on
   * `/projects` and the section's other tab is `/tasks`, which is not under it,
   * so the prefix test alone would leave the row dark on half its own section.
   * `data-tab-href` stays `to` — ⌘-click opens the section's landing view.
   */
  match?: readonly string[];
  /** Trailing slot, e.g. an unread count. */
  badge?: React.ReactNode;
}

/**
 * A row in the sidebar. Not a `NavLink`: the sidebar is outside every tab's
 * router, so it has neither a location to match against nor one to navigate
 * — it asks the strip which tab is in front and where that tab is, and sends
 * the tab somewhere new through `navigateActive`. ⌘-click still opens a tab of
 * its own, off `data-tab-href` (`app/src/lib/newTabClicks.ts`).
 */
export default function NavItem({ to, icon: Icon, label, match, badge }: NavItemProps) {
  // `NavLink`'s default, not its `end`: /settings lights for /settings/canvas.
  // The prefix half is `${to}/` rather than `to`, so Home (`to="/"`) tests for
  // "//" and matches nothing — it lights on the exact path and no other.
  const here = useActivePath().split("?")[0];
  const lightsOn = (path: string) => here === path || here.startsWith(`${path}/`);
  const isActive = lightsOn(to) || (match?.some(lightsOn) ?? false);

  return (
    <button
      type="button"
      /* Not an href: the sidebar is outside every router, so the plain click
         has to go through `navigateActive`. The attribute is how the ⌘-click
         net finds where the row leads (`lib/newTabClicks.ts`). */
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
