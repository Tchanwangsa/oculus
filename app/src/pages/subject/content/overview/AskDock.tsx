import type { Subject } from "@/lib/db";
import { OverviewAskBar } from "./AskBar";

/**
 * Holds the ask bar to the bottom of the page's own scroller, in the same
 * column as the rows above. Its top edge is a short fade, so a row passing
 * under it softens rather than being cut.
 */
export function AskDock({ subject }: { subject: Subject }) {
  return (
    <div className="sticky bottom-0 z-10 mt-auto pt-6">
      <div aria-hidden className="pointer-events-none absolute inset-x-0 top-0 h-6 bg-linear-to-t from-card to-card/0" />
      <div className="bg-card pb-4">
        <OverviewAskBar subject={subject} />
      </div>
    </div>
  );
}
