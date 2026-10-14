import { SideNavCollapseToggle } from "@/components/ui/layout/SideNav";
import { useTasksPanel } from "@/layouts/TasksLayout";
import { shortcut } from "@/lib/platform";

/**
 * The Tasks/Projects pages' 48px header row: the nav column's fold toggle at
 * its start (absent where there is no column, e.g. in the side panel), the
 * page's title, then its own toolbar as `children`.
 */
export function SectionHeader({
  title,
  children,
}: {
  title?: string;
  children?: React.ReactNode;
}) {
  const panel = useTasksPanel();

  return (
    <div className="shrink-0 flex h-12 items-center gap-2.5 px-5">
      {panel && (
        <SideNavCollapseToggle
          collapsed={panel.collapsed}
          onToggle={panel.toggle}
          shortcut={shortcut("mod+alt+B")}
        />
      )}
      {title && (
        <h1 className="min-w-0 truncate font-display text-[15px] font-semibold leading-none tracking-tight text-foreground">
          {title}
        </h1>
      )}
      {children}
    </div>
  );
}
