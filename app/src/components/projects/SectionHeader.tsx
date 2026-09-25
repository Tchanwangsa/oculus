import { useLocation, useNavigate } from "react-router-dom";
import { ViewTabs } from "@/components/ui/ViewTabs";

/**
 * The Projects/Tasks section header: a view strip plus the page's toolbar.
 * The strip navigates routes rather than holding local state, because crumbs,
 * `taskHref`, ⌘-click, tab restore and `tabInfo.tsx` all key off the path.
 */
const TABS = [
  { value: "/projects", label: "Projects" },
  { value: "/tasks", label: "Tasks" },
] as const satisfies ReadonlyArray<{ value: string; label: string }>;

type SectionPath = (typeof TABS)[number]["value"];

export function SectionHeader({ children }: { children?: React.ReactNode }) {
  const navigate = useNavigate();
  const active: SectionPath =
    useLocation().pathname === "/tasks" ? "/tasks" : "/projects";

  return (
    <>
      <div className="shrink-0 flex items-end border-b border-border-subtle px-5 pt-4">
        <ViewTabs tabs={TABS} value={active} onChange={(to) => navigate(to)} />
      </div>

      <div className="shrink-0 flex h-12 items-center gap-2.5 px-5">{children}</div>
    </>
  );
}
