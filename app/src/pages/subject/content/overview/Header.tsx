import { Link } from "react-router-dom";
import { Cards } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import type { Subject } from "@/lib/db";
import { displayName } from "@/lib/format/format";

/** The subject's name, and the one pill: Review, while revision is due here. */
export function OverviewHeader({ subject, revisionDue }: { subject: Subject; revisionDue: number }) {
  const review = `/revision/review?subject=${subject.id}&topic=all`;
  return (
    <div className="flex min-h-8 min-w-0 items-center gap-3">
      <h1 className="min-w-0 flex-1 truncate text-[17px] font-semibold text-foreground">
        {displayName(subject.name, subject.code)}
      </h1>
      {revisionDue > 0 && (
        <Button asChild size="xs" variant="outline">
          <Link to={review} data-tab-href={review}>
            <Cards />
            Review <span className="tabular-nums">{revisionDue}</span>
          </Link>
        </Button>
      )}
    </div>
  );
}
