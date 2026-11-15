import { useEffect, useMemo, useState } from "react";
import { Hourglass, Pause, Warning } from "@phosphor-icons/react";
import {
  forceStopLabel,
  lastRunStatus,
  liveRunId,
  stopDueAt,
  stopOverdue,
  usageLimitHit,
  type HarnessItem,
  type HarnessRun,
  type Provider,
  type QueuedMessage,
  type RateWindow,
  type RunStatus,
} from "@/lib/harness";
import { useCapabilities } from "@/hooks/agents/useCapabilities";
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
  /** The run on screen (`liveRunId`), whose stop may have gone unanswered. */
  liveRun?: HarnessRun | null;
  /** The bridge answers every chat from one process (`sharedProcess`). */
  sharedProcess?: boolean;
  /** Kills what answers the run; without it no unresponsive notice. */
  forceStop?: Omit<BannerCommand, "onClick"> & { onClick: (runId: number) => void };
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

/** Why Force stop costs what it does. `others` is null when no count was
 *  told (a stop found overdue after a reload), so a shared server's other
 *  chats are named without a number. */
export function unresponsiveDescription(others: number | null, sharedProcess: boolean): string {
  const lead = "It may still be working. Force stop ends its process; what has arrived is kept.";
  if (!sharedProcess || others === 0) return lead;
  if (others == null) return `${lead} Other chats on the same server stop too.`;
  return `${lead} The same server is answering ${others} other ${others === 1 ? "chat" : "chats"}, which stop too.`;
}

/**
 * The thread's notices, in the lane's order: an agent that has not answered
 * Stop, with Force stop; the usage limit of a limited run (dismissed per
 * run); held messages waiting for Resume. Pure, so the order and wording
 * are tested without a store.
 */
export function threadBannerItems(i: ThreadBannerInputs): BannerItem[] {
  const out: BannerItem[] = [];
  const now = i.now ?? Date.now();
  const run = i.liveRun;
  if (run && i.forceStop && stopOverdue(run, now)) {
    const { forceStop } = i;
    const shared = !!i.sharedProcess;
    const others = run.unresponsive ? run.unresponsive.others : null;
    out.push({
      id: `unresponsive:${run.id}`,
      tone: forceStop.error ? "danger" : "warning",
      icon: Warning,
      title: "Agent not responding to Stop",
      description: forceStop.error
        ? `Couldn't force stop: ${forceStop.error}`
        : unresponsiveDescription(others, shared),
      actions: [
        {
          label: forceStopLabel(others ?? 0, shared),
          primary: true,
          disabled: forceStop.pending,
          onClick: () => forceStop.onClick(run.id),
        },
      ],
    });
  }
  const limit = usageLimitHit(i.items, i.windows, now);
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
  provider,
  rateLimits,
  onResume,
  onForceStop,
}: {
  threadId: number | null;
  provider: Provider;
  rateLimits: RateWindow[];
  onResume?: () => Promise<void>;
  onForceStop?: (runId: number) => Promise<void>;
}) {
  const items = useHarnessStore((s) => (threadId == null ? NONE : (s.items[threadId] ?? NONE)));
  const queued = useHarnessStore((s) => (threadId == null ? NO_QUEUE : (s.queued[threadId] ?? NO_QUEUE)));
  const running = useHarnessStore((s) => threadId != null && (s.live[threadId]?.running ?? false));
  const lastRun = useHarnessStore((s) => (threadId == null ? null : lastRunStatus(s.runs[threadId])));
  const dismissed = useBannerDismissals((s) => (threadId == null ? NOTHING : (s.dismissed[threadId] ?? NOTHING)));
  const dismiss = useBannerDismissals((s) => s.dismiss);
  const liveRun = useHarnessStore((s) => {
    if (threadId == null) return null;
    const id = liveRunId(s.runs[threadId]);
    return id == null ? null : (s.runs[threadId]?.[id] ?? null);
  });
  const sharedProcess = useCapabilities(provider).sharedProcess;
  const { run, pending, error, clear } = useRunCommand();
  const force = useRunCommand();
  const { clear: clearForce } = force;
  useEffect(() => {
    clear();
    clearForce();
  }, [threadId, clear, clearForce]);
  // A failure belongs to the notice it was said in: a later run's stop, or
  // a later hold, starts without it.
  const liveId = liveRun?.id ?? null;
  const anyHeld = queued.some((q) => q.held);
  useEffect(() => clearForce(), [liveId, clearForce]);
  useEffect(() => {
    if (!anyHeld) clear();
  }, [anyHeld, clear]);
  const resumeCommand = useMemo(
    (): BannerCommand | null => (onResume ? { onClick: () => void run(onResume), pending, error } : null),
    [onResume, run, pending, error],
  );
  const forceCommand = useMemo(
    () =>
      onForceStop
        ? { onClick: (runId: number) => void force.run(() => onForceStop(runId)), pending: force.pending, error: force.error }
        : undefined,
    [onForceStop, force.run, force.pending, force.error],
  );
  // A stop read from a snapshot turns overdue on the clock, with no event
  // to say so: look again at that moment.
  const due = stopDueAt(liveRun ?? undefined);
  const [tick, setTick] = useState(0);
  useEffect(() => {
    if (due == null) return;
    const timer = setTimeout(() => setTick((n) => n + 1), Math.max(0, due - Date.now()));
    return () => clearTimeout(timer);
  }, [due]);
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
            liveRun,
            sharedProcess,
            forceStop: forceCommand,
          }),
    // `tick` is the clock: a stop becoming overdue changes nothing else.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [threadId, items, rateLimits, dismissed, dismiss, queued, running, lastRun, resumeCommand, liveRun, sharedProcess, forceCommand, tick],
  );
  return <BannerLane items={banners} />;
}
