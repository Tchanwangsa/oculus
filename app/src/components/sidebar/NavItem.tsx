import { NavLink } from "react-router-dom";
import { cn } from "@/lib/utils";
import type { LucideIcon } from "lucide-react";

interface NavItemProps {
  to: string;
  icon: LucideIcon;
  label: string;
  collapsed: boolean;
  badge?: number;
}

export default function NavItem({ to, icon: Icon, label, collapsed, badge }: NavItemProps) {
  return (
    <NavLink
      to={to}
      title={collapsed ? label : undefined}
      className={({ isActive }) =>
        cn(
          "flex items-center gap-3 rounded-lg px-3 py-2.5 text-sm font-medium transition-colors relative group",
          isActive
            ? "bg-sidebar-item-active text-primary"
            : "text-muted-foreground hover:bg-sidebar-item-hover hover:text-foreground"
        )
      }
    >
      <Icon size={18} className="shrink-0" />

      {!collapsed && (
        <span className="truncate">{label}</span>
      )}

      {badge != null && badge > 0 && (
        <span
          className={cn(
            "ml-auto flex h-5 min-w-5 items-center justify-center rounded-full bg-accent text-accent-foreground text-[10px] font-semibold px-1",
            collapsed && "absolute top-1 right-1 h-4 min-w-4 text-[9px]"
          )}
        >
          {badge > 99 ? "99+" : badge}
        </span>
      )}

      {/* Tooltip shown on hover when collapsed */}
      {collapsed && (
        <div className="absolute left-full ml-3 px-2.5 py-1.5 rounded-md bg-foreground text-background text-xs font-medium whitespace-nowrap opacity-0 pointer-events-none group-hover:opacity-100 transition-opacity z-50 shadow-md">
          {label}
        </div>
      )}
    </NavLink>
  );
}
