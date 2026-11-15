import { useEffect, useMemo } from "react";
import { Hourglass, Pause } from "@phosphor-icons/react";
import {
  lastRunStatus,
  usageLimitHit,
  type HarnessItem,
  type QueuedMessage,
  type RateWindow,
  type RunStatus,
} from "@/lib/harness";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { useBannerDismissals } from "@/stores/chat/bannerDismissals";
import { BannerLane, type BannerItem } from "./BannerLane";
import { useRunCommand } from "./useRunCommand";

const NONE: HarnessItem[] = [];
const NOTHING: string[] = [];
const NO_QUEUE: QueuedMessage[] = [];

/** A command a notice offers, with what the lane shows while it is out and
 *  after it failed. */
export interface BannerCommand {
  onClick: () => void;
  pending: boolean;
  error: string | null;
}

export interface ThreadBannerInputs {
  items: HarnessItem[];
  windows: RateWindow[];
  dismissed: string[];
  onDismiss: (id: string) => void;
  now?: number;
  /** The thread's waiting messages (`harnessStore.queued`). */
  queued?: QueuedMessage[];
  /** A turn is open on the thread. */
  running?: boolean;
  /** How the thread's newest run ended (`lastRunStatus`). */
  lastRun?: RunStatus | null;
  /** Lifts the hold; without it no held notice is shown. */
  resume?: BannerCommand | null;
}

const RESUME_HINT = "Resume sends them in order; a new message also resumes them and goes after them.";

/** What held the queue, as far as the webview can tell. A failed run always
 *  holds it, so a failed last run is named; a stop and a restart hold it
 *  alike and leave nothing to tell them apart, so they share their words. */
export function heldDescription(lastRun: RunStatus | null | undefined): string {
  return lastRun === "failed"
    ? `The last reply failed, so they wait for you. ${RESUME_HINT}`
    : `Held after a stop or a restart. ${RESUME_HINT}`;
}

/**
 * The thread's notices, in the lane's order: the usage limit of a limited
 * run (dismissed per run), then held messages waiting for Resume. Pure, so
 * the order and wording are tested without a store.
 */
export function threadBannerItems(i: ThreadBannerInputs): BannerItem[] {
  const out: BannerItem[] = [];
  const limit = usageLimitHit(i.items, i.windows, i.now ?? Date.now());
  if (limit) {
    const id = `limit:${limit.key}`;
    if (!i.dismissed.includes(id)) {
      out.push({
        id,
        tone: "warning",
        icon: Hourglass,
        title: "Usage limit reached",
        description: limit.resets ?? "Reset time unavailable; retry manually",
        onDismiss: () => i.onDismiss(id),
      });
    }
  }
  const held = (i.queued ?? NO_QUEUE).filter((q) => q.held).length;
  // While a turn is open the bubbles say Held; the notice waits for the chat
  // to be idle, when only Resume or a new message moves it.
  if (held > 0 && !i.running && i.resume) {
    const { resume } = i;
    out.push({
      id: "held",
      tone: resume.error ? "danger" : "neutral",
      icon: Pause,
      title: `${held} ${held === 1 ? "message" : "messages"} waiting`,
      description: resume.error ? `Couldn't resume: ${resume.error}` : heldDescription(i.lastRun),
      actions: [{ label: "Resume", primary: true, disabled: resume.pending, onClick: resume.onClick }],
    });
  }
  return out;
}

/** The open thread's notices, in `BannerLane` above a composer. Reads the
 *  thread's rows itself, so the composer around it never re-renders for them. */
export function ThreadBanners({
  threadId,
  rateLimits,
  onResume,
}: {
  threadId: number | null;
  rateLimits: RateWindow[];
  onResume?: () => Promise<void>;
}) {
  const items = useHarnessStore((s) => (threadId == null ? NONE : (s.items[threadId] ?? NONE)));
  const queued = useHarnessStore((s) => (threadId == null ? NO_QUEUE : (s.queued[threadId] ?? NO_QUEUE)));
  const running = useHarnessStore((s) => threadId != null && (s.live[threadId]?.running ?? false));
  const lastRun = useHarnessStore((s) => (threadId == null ? null : lastRunStatus(s.runs[threadId])));
  const dismissed = useBannerDismissals((s) => (threadId == null ? NOTHING : (s.dismissed[threadId] ?? NOTHING)));
  const dismiss = useBannerDismissals((s) => s.dismiss);
  const { run, pending, error, clear } = useRunCommand();
  useEffect(() => clear(), [threadId, clear]);
  const resumeCommand = useMemo(
    (): BannerCommand | null => (onResume ? { onClick: () => void run(onResume), pending, error } : null),
    [onResume, run, pending, error],
  );
  const banners = useMemo(
    () =>
      threadId == null
        ? []
        : threadBannerItems({
            items,
            windows: rateLimits,
            dismissed,
            onDismiss: (id) => dismiss(threadId, id),
            queued,
            running,
            lastRun,
            resume: resumeCommand,
          }),
    [threadId, items, rateLimits, dismissed, dismiss, queued, running, lastRun, resumeCommand],
  );
  return <BannerLane items={banners} />;
}
