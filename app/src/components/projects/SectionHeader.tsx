import { useLocation, useNavigate } from "react-router-dom";
import { ViewTabs } from "@/components/ui/ViewTabs";

/**
 * The header the Projects and Tasks pages share: one strip naming the
 * section's two views, and the toolbar row under it.
 *
 * They are **one section** — the sidebar has a single row for it, labelled
 * Tasks — because they were always the same rows read two ways: a project's
 * plans on one tab, every task at once on the other. Projects is the landing
 * view, which is why the sidebar row leads to `/projects` while saying Tasks;
 * the section is the work and Projects is where a project (and a task) is
 * created.
 *
 * **A routed strip, not a local one.** Both pages live inside a tab's own
 * memory router (`app/src/routes.tsx`, one per pane), so switching tabs is a
 * real navigation with its own history entry — which is what keeps
 * `ProjectCrumbs`, `taskHref`'s two routes, ⌘-click, a restored tab and
 * `tabInfo.tsx` all working untouched, since every one of them already keys off
 * these two paths. A `localStorage` scope would have had to teach all five
 * about a sixth source of truth. The path is the only one there is.
 *
 * The strip is full width and each page keeps its own body below it: the
 * projects index is a centred `max-w-3xl` scroller, the tasks page is full
 * bleed with a scroller per column. Lifting either shape in here would put the
 * other in the wrong box.
 */
const TABS = [
  { value: "/projects", label: "Projects" },
  { value: "/tasks", label: "Tasks" },
] as const satisfies ReadonlyArray<{ value: string; label: string }>;

type SectionPath = (typeof TABS)[number]["value"];

export function SectionHeader({ children }: { children?: React.ReactNode }) {
  const navigate = useNavigate();
  // Only these two routes render this header, so anything that is not the
  // tasks tab is the projects one — no third state to fall through to.
  const active: SectionPath =
    useLocation().pathname === "/tasks" ? "/tasks" : "/projects";

  return (
    <>
      {/* Tabs alone on the rule the active tab underlines. */}
      <div className="shrink-0 flex items-end border-b border-border-subtle px-5 pt-4">
        <ViewTabs tabs={TABS} value={active} onChange={(to) => navigate(to)} />
      </div>

      {/* Fixed-height toolbar: how the section is going, and what you can do
          about it. Its contents are the page's — the two tabs make and count
          different things. */}
      <div className="shrink-0 flex h-12 items-center gap-2.5 px-5">{children}</div>
    </>
  );
}
