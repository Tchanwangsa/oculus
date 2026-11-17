import type { DeadlineItem } from "@/hooks/data/overview";
import { ListCard } from "@/components/ui/layout/PageParts";
import { fmtDue } from "@/lib/format/format";
import { cn } from "@/lib/utils";
import { daysFrom, dueLabel } from "./format";
import { deadlineGlyph } from "./glyphs";
import type { OverviewOpener } from "./open";
import { ListRow, Quiet, RowDate, RowTitle } from "./rows";

/** The subject's next deadlines, whatever week is selected. */
export function ComingUpList({ items, opener, now }: {
  items: DeadlineItem[];
  opener: OverviewOpener;
  now: Date;
}) {
  if (items.length === 0) return <Quiet>Nothing due.</Quiet>;
  return (
    <ListCard>
      {items.map((item) => {
        const { Icon, tone } = deadlineGlyph(item);
        const href = item.href;
        return (
          <ListRow
            key={item.id}
            onOpen={href ? () => opener.open(href) : undefined}
            tabHref={href ? opener.tabHref(href) : undefined}
            title={`Due ${fmtDue(item.date)}`}
          >
            <Icon size={14} className={`shrink-0 ${tone}`} />
            <RowTitle>{item.title}</RowTitle>
            <RowDate className={cn(daysFrom(now, item.date) <= 2 && "font-medium text-warning")}>
              {dueLabel(item.date, now)}
            </RowDate>
          </ListRow>
        );
      })}
    </ListCard>
  );
}
