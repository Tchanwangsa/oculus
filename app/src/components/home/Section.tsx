import type { ReactNode } from "react";
import { ListCard } from "@/components/ui/PageParts";

/**
 * The "next few things" list shape — a plain label, then a bordered column of
 * hairline rows — shared by `app/src/pages/NewTabPage.tsx` and
 * `app/src/components/projects/ProjectOverview.tsx`. Rows match `EventRow`'s
 * padding so they read as one list beside calendar rows.
 */
export function Section({
  title,
  children,
}: {
  /** Optional: omitted for a column that needs no heading. */
  title?: string;
  children: ReactNode;
}) {
  return (
    <section>
      {title && (
        <h2 className="mb-2 px-0.5 text-[13px] font-semibold text-foreground">
          {title}
        </h2>
      )}
      <ListCard>
        {children}
      </ListCard>
    </section>
  );
}

/** One row in a {@link Section}. Not a pill: pills in a column read as a menu. */
export const ROW =
  "flex w-full items-center gap-3 px-3 py-2.5 text-left transition-colors hover:bg-surface";
