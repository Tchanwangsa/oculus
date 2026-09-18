import type { ReactNode } from "react";

/**
 * Home's row grammar — and, since a project's Overview wanted the same list of
 * what is coming up, the shape every "here are the next few" section in the
 * app now draws (`app/src/components/projects/ProjectOverview.tsx`). It lives
 * under `home/` because that is where it was needed first, not because Home
 * owns it.
 *
 * It is the calendar's Agenda day group
 * (`app/src/components/calendar/AgendaView.tsx`) with the subject overview's
 * heading (`app/src/pages/subject/OverviewPage.tsx`): a plain sentence-case
 * label with no icon, then a bordered column of hairline-divided rows. Today
 * literally renders `EventRow`, the same component the Agenda does, so
 * anything Continue, Projects or a project's own Upcoming tasks invented for
 * itself would read as a different list on the same page.
 */
export function Section({
  title,
  children,
}: {
  /** Optional: a column of rows that needs no naming — the new-tab page's two
   *  doors say what they are — is the same column without a heading over it. */
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
      <div className="overflow-hidden rounded-lg border border-border divide-y divide-border-subtle">
        {children}
      </div>
    </section>
  );
}

/** One row inside a {@link Section}. A row is a row, not a pill — the pill
 *  radius belongs to buttons and chips, and a stack of pills inside a bordered
 *  column reads as a menu rather than a list. */
export const ROW =
  "flex w-full items-center gap-3 px-3 py-2.5 text-left transition-colors hover:bg-surface";
