import { NavLink } from "react-router-dom";
import { cn } from "@/lib/utils";
import type { ComponentType, SVGProps } from "react";

/** Shape of a @heroicons/react component. */
export type HeroIcon = ComponentType<SVGProps<SVGSVGElement>>;

interface NavItemProps {
  to: string;
  icon: HeroIcon;
  label: string;
  collapsed: boolean;
}

export default function NavItem({ to, icon: Icon, label, collapsed }: NavItemProps) {
  return (
    <NavLink
      to={to}
      className={({ isActive }) =>
        cn(
          "flex items-center gap-2.5 rounded-md px-2 py-1.5 text-[13px] font-medium transition-colors relative group",
          collapsed && "justify-center px-0 py-2",
          isActive
            ? "bg-sidebar-item-active text-foreground"
            : "text-muted-foreground hover:bg-sidebar-item-hover hover:text-foreground"
        )
      }
    >
      <Icon className="size-[15px] shrink-0" />

      {!collapsed && <span className="truncate">{label}</span>}

      {/* Tooltip shown on hover when collapsed */}
      {collapsed && (
        <div className="absolute left-full ml-3 px-2 py-1 rounded-md bg-foreground text-background text-xs font-medium whitespace-nowrap opacity-0 pointer-events-none group-hover:opacity-100 transition-opacity z-50 shadow-md">
          {label}
        </div>
      )}
    </NavLink>
  );
}
