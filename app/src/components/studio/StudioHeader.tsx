import type { ReactNode } from "react";
import { useNavigate } from "react-router-dom";
import { ArrowLeft } from "@phosphor-icons/react";

import { PageHeader, PaneTitle, PaneTrail } from "@/components/tabs/PaneHeader";
import { SideNavCollapseToggle } from "@/components/ui/layout/SideNav";
import { useStudioPanel } from "@/layouts/StudioLayout";

/** A Studio page's 48px header row: the nav column's toggle, an optional
 *  way back, the title, and the page's actions at the end. */
export function StudioHeader({
  title,
  back,
  actions,
}: {
  title: string;
  /** Where the arrow before the title goes. */
  back?: string;
  actions?: ReactNode;
}) {
  const panel = useStudioPanel();
  const navigate = useNavigate();
  return (
    <PageHeader>
      <SideNavCollapseToggle panel={panel} />
      {back && (
        <button
          type="button"
          aria-label="Back"
          data-tab-href={back}
          onClick={() => navigate(back)}
          className="-mr-1 flex size-7 shrink-0 items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
        >
          <ArrowLeft size={15} />
        </button>
      )}
      <PaneTrail>
        <PaneTitle>{title}</PaneTitle>
      </PaneTrail>
      {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
    </PageHeader>
  );
}
