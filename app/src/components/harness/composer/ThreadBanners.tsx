import { useMemo } from "react";
import { Hourglass } from "@phosphor-icons/react";
import { usageLimitHit, type HarnessItem, type RateWindow } from "@/lib/harness";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { useBannerDismissals } from "@/stores/chat/bannerDismissals";
import { BannerLane, type BannerItem } from "./BannerLane";

const NONE: HarnessItem[] = [];
const NOTHING: string[] = [];

/** The usage-limit notice for a limited run, dismissed per run. Pure, so the
 *  order and wording are tested without a store. */
export function threadBannerItems(
  items: HarnessItem[],
  windows: RateWindow[],
  dismissed: string[],
  onDismiss: (id: string) => void,
  now = Date.now(),
): BannerItem[] {
  const out: BannerItem[] = [];
  const limit = usageLimitHit(items, windows, now);
  if (limit) {
    const id = `limit:${limit.key}`;
    if (!dismissed.includes(id)) {
      out.push({
        id,
        tone: "warning",
        icon: Hourglass,
        title: "Usage limit reached",
        description: limit.resets ?? "Reset time unavailable; retry manually",
        onDismiss: () => onDismiss(id),
      });
    }
  }
  return out;
}

/** The open thread's notices, in `BannerLane` above a composer. Reads the
 *  thread's rows itself, so the composer around it never re-renders for them. */
export function ThreadBanners({ threadId, rateLimits }: { threadId: number | null; rateLimits: RateWindow[] }) {
  const items = useHarnessStore((s) => (threadId == null ? NONE : (s.items[threadId] ?? NONE)));
  const dismissed = useBannerDismissals((s) => (threadId == null ? NOTHING : (s.dismissed[threadId] ?? NOTHING)));
  const dismiss = useBannerDismissals((s) => s.dismiss);
  const banners = useMemo(
    () => (threadId == null ? [] : threadBannerItems(items, rateLimits, dismissed, (id) => dismiss(threadId, id))),
    [threadId, items, rateLimits, dismissed, dismiss],
  );
  return <BannerLane items={banners} />;
}
