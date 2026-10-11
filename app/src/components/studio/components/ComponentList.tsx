import { NavLink } from "react-router-dom";
import { WarningCircle } from "@phosphor-icons/react";

import { SIDE_NAV_ROW, SIDE_NAV_ROW_ACTIVE, SIDE_NAV_ROW_IDLE } from "@/components/ui/layout/SideNav";
import { Skeleton } from "@/components/ui/skeleton";
import type { ComponentInfo } from "@/lib/studio/components";
import { componentHref } from "@/lib/studio/routes";
import { cn } from "@/lib/utils";

/**
 * The components, shells (which lay out named areas) then the rest. A row
 * says when the subject has its own version and when the component does not
 * load.
 */
export function ComponentList({
  components,
  loaded,
  error,
}: {
  components: ComponentInfo[];
  loaded: boolean;
  error: string | null;
}) {
  const shells = components.filter((c) => c.kind === "shell");
  const rest = components.filter((c) => c.kind !== "shell");
  return (
    <nav aria-label="Components" className="min-h-0 overflow-y-auto border-r border-border-subtle px-2 pb-4 pt-2">
      {!loaded ? (
        <div className="space-y-1.5 p-1">
          {Array.from({ length: 8 }, (_, i) => (
            <Skeleton key={i} className="h-7 w-full" />
          ))}
        </div>
      ) : error ? (
        <p data-selectable className="px-2 py-1.5 text-[12px] leading-snug text-destructive">
          {error}
        </p>
      ) : components.length === 0 ? (
        <p className="px-2 py-1.5 text-[12px] text-muted-foreground">No components in this library.</p>
      ) : (
        <>
          <Group label="Shells" items={shells} />
          <Group label="Components" items={rest} />
        </>
      )}
    </nav>
  );
}

function Group({ label, items }: { label: string; items: ComponentInfo[] }) {
  if (items.length === 0) return null;
  return (
    <div className="mb-3">
      <div className="px-2 pb-1 pt-2 text-[11px] font-medium tracking-wide text-muted-foreground">{label}</div>
      <div className="flex flex-col gap-0.5">
        {items.map((c) => (
          <NavLink
            key={c.name}
            to={componentHref(c.name)}
            title={c.problem ?? c.name}
            className={({ isActive }) => cn(SIDE_NAV_ROW, "gap-2", isActive ? SIDE_NAV_ROW_ACTIVE : SIDE_NAV_ROW_IDLE)}
          >
            <span className="min-w-0 flex-1 truncate">{c.name}</span>
            {c.problem != null && (
              <WarningCircle size={13} weight="fill" aria-label="Does not load" className="shrink-0 text-destructive" />
            )}
            {c.level === "subject" && (
              <span className="shrink-0 rounded-full bg-brand-muted px-1.5 py-px text-[10px] font-medium text-brand">
                Own version
              </span>
            )}
          </NavLink>
        ))}
      </div>
    </div>
  );
}
