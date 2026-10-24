import { useRef, type ReactNode } from "react";
import { PageHeader, PaneTitle, PaneTrail } from "@/components/tabs/PaneHeader";
import { SideNavCollapseToggle } from "@/components/ui/layout/SideNav";
import { useRevisionPanel } from "@/layouts/RevisionLayout";
import { useRevisionData } from "@/layouts/RevisionLayout";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
export function RevisionPage({
  title,
  actions,
  children,
}: {
  title: string;
  actions?: ReactNode;
  children: ReactNode;
}) {
  const panel = useRevisionPanel();
  const { loading, error } = useRevisionData();
  const scroll = useRef<HTMLDivElement>(null);
  useScrollFade(scroll);
  return (
    <>
      <PageHeader>
        <SideNavCollapseToggle panel={panel} />
        <PaneTrail>
          <PaneTitle>{title}</PaneTitle>
        </PaneTrail>
        {actions && <div className="flex items-center gap-2">{actions}</div>}
      </PageHeader>
      <div ref={scroll} className="page-scroll min-h-0 flex-1">
        <div className="mx-auto max-w-4xl space-y-5 px-6 py-5">
          {error && (
            <p role="alert" className="text-sm text-destructive">
              {error}
            </p>
          )}
          {loading && (
            <p className="text-sm text-muted-foreground">Loading revision…</p>
          )}
          {children}
        </div>
      </div>
    </>
  );
}
