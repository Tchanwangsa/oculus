import { Link } from "react-router-dom";
import type { Icon } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import {
  SIDE_NAV_FOLDS,
  SIDE_NAV_ROW,
  SIDE_NAV_ROW_ACTIVE,
  SIDE_NAV_ROW_IDLE,
} from "@/components/ui/layout/SideNav";

/**
 * A nav row that is a plain `Link`: unlike `SideNavLink`, whose `NavLink`
 * ignores the query string, it takes `active` from the caller, since the
 * views are `/tasks?view=…`. The anchor's `href` is what ⌘-click opens.
 */
export function TasksNavRow({
  to,
  active,
  icon: RowIcon,
  label,
}: {
  to: string;
  active: boolean;
  icon: Icon;
  label: string;
}) {
  return (
    <Link
      to={to}
      aria-current={active ? "page" : undefined}
      className={cn(SIDE_NAV_ROW, active ? SIDE_NAV_ROW_ACTIVE : SIDE_NAV_ROW_IDLE)}
    >
      <RowIcon size={16} className="shrink-0" />
      <span className={cn("min-w-0 flex-1 truncate", SIDE_NAV_FOLDS)}>{label}</span>
    </Link>
  );
}
