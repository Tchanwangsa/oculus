import type { ReactNode } from "react";
import { useNavigate } from "react-router-dom";
import { Tray, type Icon } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { DOCK_ROOT, DOCK_SERVICES, dockPath, type DockServiceId } from "@/lib/dock";
import { useDockStore } from "@/stores/shell/dockStore";
import { NewCountBadge } from "@/components/NewCountBadge";

/**
 * Messaging's header row: the Inbox tab, then one tab per enabled service
 * with its unread count, and `actions` at the right end. Switching replaces
 * the route, so Back leaves Messaging instead of walking its tabs.
 */
export function DockHeader({
  current,
  enabled,
  actions,
}: {
  /** The service shown, or null on the Inbox. */
  current: DockServiceId | null;
  enabled: readonly DockServiceId[];
  actions?: ReactNode;
}) {
  const navigate = useNavigate();
  const pages = useDockStore((s) => s.pages);
  const go = (to: string) => navigate(to, { replace: true });

  return (
    <div className="flex h-10 shrink-0 items-center gap-1 border-b border-border pr-2 pl-2">
      <div role="tablist" className="flex min-w-0 flex-1 items-center gap-0.5">
        <HeaderTab
          icon={Tray}
          label="Inbox"
          active={current === null}
          onSelect={() => go(DOCK_ROOT)}
        />
        {DOCK_SERVICES.filter((s) => enabled.includes(s.id)).map((s) => (
          <HeaderTab
            key={s.id}
            icon={s.icon}
            label={s.label}
            count={pages.find((p) => p.id === s.id)?.unread ?? 0}
            active={s.id === current}
            onSelect={() => go(dockPath(s.id))}
          />
        ))}
      </div>
      {actions}
    </div>
  );
}

function HeaderTab({
  icon: Icon,
  label,
  count = 0,
  active,
  onSelect,
}: {
  icon: Icon;
  label: string;
  count?: number;
  active: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      role="tab"
      aria-selected={active}
      onClick={() => !active && onSelect()}
      className={cn(
        "flex h-7 shrink-0 cursor-pointer items-center gap-1.5 rounded-full px-2.5 text-[12.5px] transition-colors",
        active
          ? "bg-secondary text-foreground"
          : "text-muted-foreground hover:bg-sidebar-item-hover hover:text-foreground",
      )}
    >
      <Icon size={14} className="shrink-0" />
      {label}
      <NewCountBadge count={count} />
    </button>
  );
}
