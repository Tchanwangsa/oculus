import {
  MessageSquare,
  Video,
  BookOpen,
  RefreshCw,
  ChevronLeft,
  ChevronRight,
} from "lucide-react";
import { cn } from "@/lib/utils";
import NavItem from "./NavItem";
import { SidebarActivity } from "@/components/jobs/SidebarActivity";

interface SidebarProps {
  collapsed: boolean;
  onToggle: () => void;
}

const mainNav = [
  { to: "/chat",     icon: MessageSquare, label: "Chat" },
  { to: "/lectures", icon: Video,         label: "Lectures" },
  { to: "/subjects", icon: BookOpen,      label: "Subjects" },
] as const;

const bottomNav = [
  { to: "/sync", icon: RefreshCw, label: "Sync" },
] as const;

export default function Sidebar({ collapsed, onToggle }: SidebarProps) {
  return (
    <aside
      className={cn(
        "flex flex-col h-full border-r border-sidebar-border bg-sidebar transition-all duration-200 ease-in-out shrink-0 relative",
        collapsed ? "w-[56px]" : "w-[220px]"
      )}
    >
      {/* Logo */}
      <div
        className={cn(
          "flex items-center gap-2.5 px-3.5 h-13 shrink-0",
          collapsed && "justify-center px-0"
        )}
      >
        <img
          src="/oculus-mark.svg"
          alt=""
          className="w-[22px] h-[22px] shrink-0"
        />
        {!collapsed && (
          <span className="font-semibold text-foreground tracking-tight text-[13px]">
            Oculus
          </span>
        )}
      </div>

      {/* Main nav */}
      <nav className="flex-1 overflow-y-auto py-1 px-2 space-y-px">
        {mainNav.map((item) => (
          <NavItem
            key={item.to}
            to={item.to}
            icon={item.icon}
            label={item.label}
            collapsed={collapsed}
          />
        ))}
      </nav>

      {/* Background activity */}
      <SidebarActivity collapsed={collapsed} />

      {/* Bottom nav */}
      <div className="py-2 px-2 space-y-px border-t border-sidebar-border">
        {bottomNav.map((item) => (
          <NavItem
            key={item.to}
            to={item.to}
            icon={item.icon}
            label={item.label}
            collapsed={collapsed}
          />
        ))}
      </div>

      {/* Collapse toggle */}
      <button
        onClick={onToggle}
        className="absolute -right-3 top-[46px] z-10 flex h-6 w-6 items-center justify-center rounded-full border border-sidebar-border bg-sidebar shadow-sm text-muted-foreground hover:text-foreground transition-colors"
        title={collapsed ? "Expand sidebar" : "Collapse sidebar"}
      >
        {collapsed ? <ChevronRight size={12} /> : <ChevronLeft size={12} />}
      </button>
    </aside>
  );
}
