import { ActivityCard } from "@/components/home/ActivityCard";
import { LecturesCard } from "@/components/home/LecturesCard";
import { RecentRow } from "@/components/home/RecentRow";
import { SyncControl } from "@/components/home/SyncControl";
import { UpcomingCard } from "@/components/home/UpcomingCard";
import { useNow } from "@/hooks/ui/useNow";
import { displayLocale } from "@/lib/format/locale";

/**
 * Home is a dashboard over the library: the date and sync state, then two
 * columns when the pane has room — the month's activity over a row of recent
 * items, beside the next few events over the lectures in progress. Each
 * section owns its read.
 */
export default function HomePage() {
  // `useNow`, so a window left open overnight updates its heading; the cards
  // take it too, one clock for the page.
  const now = useNow();

  return (
    <div className="page-scroll @container/home">
      <div className="mx-auto max-w-[1200px] space-y-6 px-4 py-6 @min-[640px]/home:px-8">
        <div className="flex flex-wrap items-center justify-between gap-x-6 gap-y-2">
          <h1 className="text-[22px] font-semibold leading-tight tracking-tight text-foreground">
            {now.toLocaleDateString(displayLocale, {
              weekday: "long",
              day: "numeric",
              month: "long",
            })}
          </h1>
          <SyncControl />
        </div>

        <div className="grid gap-4 @min-[1024px]/home:grid-cols-[minmax(0,1fr)_340px]">
          <div className="min-w-0 space-y-6">
            <ActivityCard now={now} />
            <RecentRow />
          </div>
          <div className="min-w-0 space-y-4">
            <UpcomingCard now={now} />
            <LecturesCard />
          </div>
        </div>
      </div>
    </div>
  );
}
