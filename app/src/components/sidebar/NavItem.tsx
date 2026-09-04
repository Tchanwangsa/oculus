import { NavLink } from "react-router-dom";
import { cn } from "@/lib/utils";
import type { Icon as PhosphorIcon } from "@phosphor-icons/react";

interface NavItemProps {
  to: string;
  icon: PhosphorIcon;
  label: string;
  /** Trailing slot, e.g. an unread count. */
  badge?: React.ReactNode;
}

export default function NavItem({ to, icon: Icon, label, badge }: NavItemProps) {
  return (
    <NavLink
      to={to}
      className={({ isActive }) =>
        cn(
          "flex items-center gap-2.5 rounded-md px-2 py-1.5 text-[13px] font-medium transition-colors",
          isActive
            ? "bg-sidebar-item-active text-foreground"
            : "text-muted-foreground hover:bg-sidebar-item-hover hover:text-foreground"
        )
      }
    >
      <Icon size={15} className="shrink-0" />
      <span className="flex-1 min-w-0 overflow-hidden whitespace-nowrap text-clip">
        {label}
      </span>
      {badge}
    </NavLink>
  );
}
