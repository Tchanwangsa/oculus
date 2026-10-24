import { PageHeader, PaneTitle, PaneTrail } from "@/components/tabs/PaneHeader";
import { SideNavCollapseToggle } from "@/components/ui/layout/SideNav";
import { useTasksPanel } from "@/layouts/TasksLayout";

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
    <PageHeader>
      {panel && <SideNavCollapseToggle panel={panel} />}
      {title && <PaneTrail><PaneTitle>{title}</PaneTitle></PaneTrail>}
      {children}
    </PageHeader>
  );
}
