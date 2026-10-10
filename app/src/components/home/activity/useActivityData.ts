import { useCallback, useEffect, useMemo, useState } from "react";
import { useTabActive } from "@/components/tabs/TabContext";
import { useSubjects } from "@/hooks/data/useSubjects";
import { loadCalendar, subjectColors } from "@/lib/planning/calendar";
import {
  chartDays,
  dayKey,
  loadUsageContext,
  loadUsageDays,
  type UsageContextRow,
  type UsageDay,
} from "@/lib/activity/usage";
import { useHomeSection } from "@/components/home/useHomeSection";
import { COLOR_EVENTS, EVENTS, POLL_MS, type View } from "@/components/home/activity/constants";

/** The card's reads: usage per day and per context, the calendar's subject
 *  colours (Subject view only), re-read on events and on a poll. */
export function useActivityData(now: Date, view: View) {
  const [days, setDays] = useState<Map<string, UsageDay> | null>(null);
  const [context, setContext] = useState<UsageContextRow[] | null>(null);
  const [colors, setColors] = useState<Map<number, string>>(new Map());
  const { subjects } = useSubjects();
  const active = useTabActive();

  // Keyed on the date, so the minute tick of `now` doesn't rebuild the range.
  const today = dayKey(now);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const range = useMemo(() => chartDays(now), [today]);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const prior = useMemo(() => chartDays(now, 1), [today]);

  const reload = useCallback(() => {
    loadUsageDays(new Date())
      .then(setDays)
      .catch((e) => {
        console.error(e);
        setDays(new Map());
      });
    loadUsageContext(new Date())
      .then(setContext)
      .catch((e) => {
        console.error(e);
        setContext([]);
      });
  }, []);

  useHomeSection(reload, EVENTS);

  // Only the Subject view needs the calendar, so the read waits for it.
  const reloadColors = useCallback(() => {
    if (view !== "subject") return;
    loadCalendar()
      .then((events) => setColors(subjectColors(events)))
      .catch(console.error);
  }, [view]);

  useHomeSection(reloadColors, COLOR_EVENTS);

  useEffect(() => {
    if (!active) return;
    const id = setInterval(reload, POLL_MS);
    return () => clearInterval(id);
  }, [active, reload]);

  return { days, context, colors, subjects, today, range, prior };
}
