import { Play, SidebarSimple, SkipForward } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { isSheetFile } from "@/lib/files/fileTypes";
import { fmtClock } from "@/lib/format/format";
import { filePagePath } from "@/lib/files/openFile";
import { fmtMb, type PipelineItem, type StageState } from "@/stores/sync/pipelineStore";
import { openItem, type RowActions } from "@/components/sync/pipeline/actions";
import { HATCH } from "@/components/sync/pipeline/constants";
import { uploadWaiting } from "@/components/sync/pipeline/facts";

const DOT: Record<StageState, string> = {
  pending: "bg-muted-foreground/25",
  queued: "bg-brand/40",
  active: "bg-brand",
  done: "bg-success",
  error: "bg-destructive",
  skipped: "bg-muted-foreground/50",
};

interface Step {
  key: string;
  label: string;
  state: StageState;
  /** Only what the collapsed row doesn't already say. */
  value?: string;
}

/** The upload as its own step, for a cloud parse that reported one. */
function uploadStep(item: PipelineItem): Step | null {
  if (item.bytesTotal == null) return null;
  const size = `${fmtMb(item.bytesTotal)} MB`;
  if (item.parse === "active" && item.parsePhase === "upload_wait") {
    return { key: "upload", label: "Upload", state: "queued", value: `${size}, waiting its turn` };
  }
  if (item.parse === "active" && item.parsePhase === "uploading") {
    const since = item.uploadFirstAt ? `started ${fmtClock(item.uploadFirstAt)}` : undefined;
    return { key: "upload", label: "Uploading", state: "active", value: since };
  }
  if (item.uploadedAt != null || item.parse === "done") {
    const at = fmtClock(item.uploadedAt);
    return { key: "upload", label: "Uploaded", state: "done", value: at ? `${size} · ${at}` : size };
  }
  return { key: "upload", label: "Upload", state: "pending" };
}

function detailSteps(item: PipelineItem, embedStage: boolean): Step[] {
  const steps: Step[] = [];
  const d = item.download;
  steps.push({
    key: "download",
    label: d === "done" ? "Downloaded" : d === "active" ? "Downloading" : d === "error" ? "Download failed" : "Download",
    state: d,
    value: d === "done" ? fmtClock(item.downloadedAt) || undefined : undefined,
  });

  const upload = uploadStep(item);
  if (upload) steps.push(upload);

  const p = item.parse;
  // While its upload runs, the parse itself has not started.
  const uploadingNow = p === "active" && item.parsePhase !== undefined && item.parsePhase !== "processing";
  const parseState: StageState = uploadingNow ? "pending" : p;
  const sheet = isSheetFile(item.filename);
  const parseLabel =
    parseState === "done"
      ? sheet ? "Converted" : "Parsed"
      : parseState === "active"
        ? sheet ? "Converting" : "Parsing"
        : parseState === "error"
          ? sheet ? "Conversion failed" : "Parse failed"
          : parseState === "skipped"
            ? "Skipped"
            : sheet ? "Convert" : "Parse";
  const parseValue =
    parseState === "done"
      ? fmtClock(item.parsedAt) || undefined
      : parseState === "active"
        ? item.uploadedAt
          ? `started ${fmtClock(item.uploadedAt)}`
          : undefined
        : parseState === "skipped"
          ? fmtClock(item.skippedAt) || undefined
          : undefined;
  steps.push({ key: "parse", label: parseLabel, state: parseState, value: parseValue });

  if (embedStage) {
    const e = item.embed;
    steps.push({
      key: "embed",
      label: e === "done" ? "Embedded" : e === "active" ? "Embedding" : e === "error" ? "Embed failed" : "Embed",
      state: e,
      value: e === "done" ? fmtClock(item.embeddedAt) || undefined : undefined,
    });
  }
  return steps;
}

/** One plain sentence about why the row is where it is, when that isn't
 *  obvious from the row: an error's whole message, or what a wait means. */
function noteOf(item: PipelineItem): { text: string; bad: boolean } | null {
  if (item.download === "error") {
    return { text: `${item.error ?? "Download failed"}. The next sync downloads it again.`, bad: true };
  }
  if (item.parse === "error" || item.embed === "error") {
    const text = item.errorLatching
      ? `${item.error ?? "Failed"} — this holds every file until it is fixed.`
      : (item.error ?? "Failed");
    return { text, bad: true };
  }
  if (uploadWaiting(item)) {
    return {
      text: "Another file in the same upload batch is still uploading; this one goes up after it.",
      bad: false,
    };
  }
  if (item.parse === "skipped") {
    return { text: "It stays without Markdown, search or embeddings until you parse it.", bad: false };
  }
  return null;
}

export function Detail({
  item,
  embedStage,
  actions,
  onRun,
  onSkip,
}: {
  item: PipelineItem;
  embedStage: boolean;
  actions: RowActions;
  onRun?: () => void;
  onSkip?: () => void;
}) {
  const steps = detailSteps(item, embedStage);
  const note = noteOf(item);
  return (
    // Indented to the filename: px-5, the caret, the file icon and their gaps.
    <div className="pb-3.5 pl-[58px] pr-5">
      <div className="grid w-fit grid-cols-[6px_auto_auto] items-center gap-x-2.5 gap-y-1">
        {steps.map((st) => (
          <div key={st.key} className="contents">
            <span
              className={cn("size-1.5 rounded-full", DOT[st.state])}
              style={st.state === "skipped" ? HATCH : undefined}
            />
            <span
              className={cn(
                "text-xs",
                st.state === "pending" ? "text-muted-foreground/60" : "text-foreground",
              )}
            >
              {st.label}
            </span>
            <span className="text-[11px] tabular-nums text-muted-foreground">{st.value}</span>
          </div>
        ))}
      </div>

      {note && (
        <p
          data-selectable={note.bad || undefined}
          className={cn(
            "mt-2 max-w-prose text-xs leading-relaxed",
            note.bad ? "text-destructive" : "text-muted-foreground",
          )}
        >
          {note.text}
        </p>
      )}

      <div className="mt-2.5 flex items-center gap-1.5">
        <span className="mr-auto min-w-0 truncate text-[11px] text-muted-foreground/70">
          {item.relativePath}
        </span>
        {actions.run && onRun && (
          <Button variant="secondary" size="xs" data-tab-skip onClick={onRun}>
            <Play weight="fill" /> {actions.run.label}
          </Button>
        )}
        {actions.skip && onSkip && (
          <Button variant="secondary" size="xs" data-tab-skip onClick={onSkip}>
            <SkipForward /> Skip
          </Button>
        )}
        <Button
          variant="secondary"
          size="xs"
          data-tab-href={filePagePath(item.subjectId, item.relativePath)}
          onClick={() => openItem(item)}
        >
          <SidebarSimple className="scale-x-[-1]" /> Open beside
        </Button>
      </div>
    </div>
  );
}
