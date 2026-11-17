import type { ActivityItem } from "@/hooks/data/overview";
import { Badge } from "@/components/ui/badge";
import { ListCard } from "@/components/ui/layout/PageParts";
import { activityGlyph } from "./glyphs";
import { weekday } from "./format";
import type { OverviewOpener } from "./open";
import { ListRow, Quiet, RowDate, RowTitle } from "./rows";

const UNTIMETABLED = "Not in your timetable: another stream, or a recording with no class";

/** The selected week's new and changed material, newest first. */
export function ActivityList({ items, opener, empty }: {
  items: ActivityItem[];
  opener: OverviewOpener;
  empty: string;
}) {
  if (items.length === 0) return <Quiet>{empty}</Quiet>;
  return (
    <ListCard>
      {items.map((item) => {
        const file = opener.fileFor(item.href);
        const { Icon, tone } = activityGlyph(item, file);
        const untimetabled = item.recordingStatus === "untimetabled";
        const badge = item.kind === "announcement" ? "Announcement" : item.badge;
        return (
          <ListRow
            key={item.id}
            onOpen={() => opener.open(item.href)}
            tabHref={opener.tabHref(item.href)}
            title={untimetabled ? UNTIMETABLED : undefined}
            muted={untimetabled}
          >
            <Icon size={14} className={`shrink-0 ${tone}`} />
            <RowTitle>{item.title}</RowTitle>
            {item.isNew && (
              <span title="New since your last visit" className="size-1.5 shrink-0 rounded-full bg-brand" />
            )}
            <Badge
              variant={badge.startsWith("New") || badge === "Announcement" ? "default" : "warning"}
              className="h-5 px-1.5 text-[10.5px]"
            >
              {badge}
            </Badge>
            <RowDate className="w-8 text-right">{weekday(item.date)}</RowDate>
          </ListRow>
        );
      })}
    </ListCard>
  );
}
