import { WarningCircle } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import type { Lecture } from "@/lib/db";
import { lectureList } from "./format";

/**
 * "You're 2 lectures behind", over the strip and never inside a week. The
 * hook keeps it quiet before teaching, in breaks and after teaching; this
 * renders nothing when the count is zero.
 */
export function BehindWarning({ behind, onCatchUp, catchUpHref }: {
  behind: { count: number; lectures: Lecture[] };
  onCatchUp: (lecture: Lecture) => void;
  catchUpHref?: string;
}) {
  const first = behind.lectures[0];
  if (behind.count <= 0 || !first) return null;
  const noun = behind.count === 1 ? "lecture" : "lectures";
  const titles = lectureList(behind.lectures.map((lecture) => lecture.title));
  return (
    <div role="status" className="flex min-w-0 items-center gap-2.5 rounded-lg bg-warning/10 py-1.5 pr-1.5 pl-3 text-[12px] text-foreground">
      <WarningCircle size={15} weight="fill" className="shrink-0 text-warning" />
      <span className="min-w-0 flex-1 truncate" title={titles}>
        You're <span className="tabular-nums">{behind.count}</span> {noun} behind: {titles}
      </span>
      <Button
        size="xs"
        variant="outline"
        data-tab-href={catchUpHref}
        onClick={() => onCatchUp(first)}
      >
        Catch up
      </Button>
    </div>
  );
}
