import { cn } from "@/lib/utils";
import { isSheetFile } from "@/lib/files/fileTypes";
import { summaryOf } from "@/lib/pipeline/parseState";
import { useNow } from "@/hooks/ui/useNow";
import {
  fmtEta,
  statusOf,
  uploadEta,
  type PipelineItem,
  type StatusView,
} from "@/stores/sync/pipelineStore";
import { STAGE_LABEL } from "@/components/sync/pipeline/constants";
import { failedStage } from "@/components/sync/pipeline/facts";

/** What is happening, in one line: `statusOf`'s label, with an upload's
 *  time left or a failure's short cause. */
export function captionOf(item: PipelineItem, s: StatusView): string {
  if (s.phase === "failed") {
    const stage = failedStage(item);
    if (stage === "download") return "Download failed — the next sync retries it";
    // The failure vocabulary is MinerU's; a spreadsheet's sentence is in the detail.
    if (stage === "parse" && isSheetFile(item.filename)) return "Conversion failed";
    const why = summaryOf(item.errorKind).replace(/\.$/, "");
    return `${STAGE_LABEL[stage]} failed${why ? ` — ${why}` : ""}`;
  }
  const eta = uploadEta(item);
  return eta != null ? `${s.label} · ${fmtEta(eta)}` : s.label;
}

/** The track's tooltip: the caption, with a finished file's page count. */
export function hintOf(item: PipelineItem, s: StatusView, embedStage: boolean): string {
  if (s.phase !== "done") return captionOf(item, s);
  const pages = embedStage && item.embed === "done" ? item.embedTotalPages : item.totalPages;
  return pages > 0 ? `${s.label} · ${pages} page${pages === 1 ? "" : "s"}` : s.label;
}

/** A held embed's caption counts down by the second; only this row ticks. */
export function RateLimitedCaption({ item, embedStage }: { item: PipelineItem; embedStage: boolean }) {
  const now = useNow(1_000).getTime();
  return <CaptionText text={statusOf(item, embedStage, now).label} />;
}

export function CaptionText({ text, tone }: { text: string; tone?: "bad" }) {
  return (
    <span
      className={cn(
        "min-w-0 truncate text-[11px] tabular-nums @max-2xl:hidden",
        tone === "bad" ? "text-destructive" : "text-muted-foreground",
      )}
    >
      {text}
    </span>
  );
}
