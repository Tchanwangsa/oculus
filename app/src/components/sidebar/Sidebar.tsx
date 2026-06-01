import {
  MessageSquare,
  Video,
  Network,
  BookOpen,
  Bell,
  RefreshCw,
  ChevronLeft,
  ChevronRight,
  Settings,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { Separator } from "@/components/ui/separator";
import NavItem from "./NavItem";

interface SidebarProps {
  collapsed: boolean;
  onToggle: () => void;
}

const mainNav = [
  { to: "/chat",          icon: MessageSquare, label: "Chat" },
  { to: "/lectures",      icon: Video,         label: "Lectures" },
  { to: "/graph",         icon: Network,       label: "Graph" },
  { to: "/subjects",      icon: BookOpen,      label: "Subjects" },
  { to: "/notifications", icon: Bell,          label: "Notifications", badge: 3 },
] as const;

const bottomNav = [
  { to: "/sync",     icon: RefreshCw, label: "Sync" },
] as const;

export default function Sidebar({ collapsed, onToggle }: SidebarProps) {
  return (
    <aside
      className={cn(
        "flex flex-col h-full border-r border-sidebar-border bg-sidebar transition-all duration-200 ease-in-out shrink-0 relative",
        collapsed ? "w-[60px]" : "w-[220px]"
      )}
    >
      {/* Logo */}
      <div
        className={cn(
          "flex items-center gap-3 px-3 h-14 border-b border-sidebar-border shrink-0",
          collapsed && "justify-center px-0"
        )}
      >
        <div className="flex items-center justify-center w-8 h-8 shrink-0">
          <img
            src="/oculus-icon.svg"
            alt="Oculus"
            className="w-7 h-7 object-contain"
            onError={(e) => {
              (e.target as HTMLImageElement).style.display = "none";
            }}
          />
          <span
            className="w-7 h-7 rounded-full bg-primary flex items-center justify-center text-primary-foreground font-bold text-sm leading-none"
            style={{ display: "none" }}
          >
            O
          </span>
        </div>
        {!collapsed && (
          <span className="font-semibold text-foreground tracking-tight text-base">
            Oculus
          </span>
        )}
      </div>

      {/* Main nav */}
      <nav className="flex-1 overflow-y-auto py-3 px-2 space-y-0.5">
        {mainNav.map((item) => (
          <NavItem
            key={item.to}
            to={item.to}
            icon={item.icon}
            label={item.label}
            collapsed={collapsed}
            badge={"badge" in item ? item.badge : undefined}
          />
        ))}
      </nav>

      <Separator />

      {/* Bottom nav */}
      <div className="py-3 px-2 space-y-0.5">
        {bottomNav.map((item) => (
          <NavItem
            key={item.to}
            to={item.to}
            icon={item.icon}
            label={item.label}
            collapsed={collapsed}
          />
        ))}
        <NavItem
          to="/settings"
          icon={Settings}
          label="Settings"
          collapsed={collapsed}
        />
      </div>

      {/* Collapse toggle */}
      <button
        onClick={onToggle}
        className={cn(
          "absolute -right-3 top-[52px] z-10 flex h-6 w-6 items-center justify-center rounded-full border border-sidebar-border bg-sidebar shadow-sm text-muted-foreground hover:text-foreground transition-colors",
        )}
        title={collapsed ? "Expand sidebar" : "Collapse sidebar"}
      >
        {collapsed ? <ChevronRight size={12} /> : <ChevronLeft size={12} />}
      </button>
    </aside>
  );
}
