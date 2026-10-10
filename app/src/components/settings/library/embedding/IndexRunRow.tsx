import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { useIndexStore, type IndexProgress } from "@/stores/sync/indexStore";
import { fmtResume, usePipelineStore } from "@/stores/sync/pipelineStore";
import { useNow } from "@/hooks/ui/useNow";

/**
 * Files done plus the fraction of the current document, over the queue as it
 * stands now (it can grow mid-run). The page term keeps a slow run visibly
 * moving.
 */
function runPercent(progress: IndexProgress): number {
  if (progress.total <= 0) return 0;
  const inside =
    progress.totalPages > 0 ? Math.min(progress.pagesDone / progress.totalPages, 1) : 0;
  return Math.min(((progress.done + inside) / progress.total) * 100, 100);
}

/** The same in words; pages only once the document has reported some. */
function runLabel(progress: IndexProgress): string {
  const files = `${progress.done} of ${progress.total}`;
  if (progress.totalPages > 0) {
    return `${files} · page ${progress.pagesDone} of ${progress.totalPages}`;
  }
  return files;
}

/**
 * Start, watch and stop an index run. Names the current file so a slow run
 * shows it is alive (see docs/retrieval.md: an embed blocks for minutes).
 */
export function IndexRunRow({
  outstanding,
  ready,
}: {
  outstanding: number | null;
  ready: boolean;
}) {
  const nothingToDo = outstanding === 0;
  const running = useIndexStore((state) => state.running);
  const progress = useIndexStore((state) => state.progress);
  const stopping = useIndexStore((state) => state.stopping);
  const result = useIndexStore((state) => state.result);
  const error = useIndexStore((state) => state.error);
  const start = useIndexStore((state) => state.start);
  const stop = useIndexStore((state) => state.stop);
  // The run's file held by a rate limit, from its pipeline row: without it a
  // paced run reads as stuck on one page.
  const held = usePipelineStore((state) =>
    running && progress?.filename
      ? Object.values(state.items).find(
          (it) =>
            it.filename === progress.filename &&
            it.embed === "active" &&
            it.embedWaitingUntil != null,
        )
      : undefined,
  );
  const now = useNow(held ? 1_000 : 60_000).getTime();

  return (
    <div className="pt-2">
      <div className="flex items-center justify-between gap-4">
        <div className="min-w-0">
          <p className="text-xs text-foreground">Build the index</p>
          <p className="text-[11px] text-muted-foreground">
            {running
              ? progress
                ? `${runLabel(progress)}${
                    progress.filename ? ` · ${progress.filename}` : ""
                  }${
                    held?.embedWaitingUntil != null
                      ? ` · ${held.embedWaitingReason ?? "rate-limited"}, ${fmtResume(held.embedWaitingUntil, now)}`
                      : ""
                  }`
                : "Working out what is outstanding…"
              : outstanding == null
                ? "Embeds every parsed PDF that is not in the current space."
                : nothingToDo
                  ? "Every parsed PDF is in the current space."
                  : `${outstanding} file${outstanding === 1 ? "" : "s"} to embed.`}
          </p>
        </div>
        {running ? (
          <Button variant="outline" size="xs" disabled={stopping} onClick={stop}>
            {/* Stopping lands on a file boundary. */}
            {stopping ? "Stopping…" : "Stop"}
          </Button>
        ) : (
          <Button
            size="xs"
            disabled={!ready || nothingToDo}
            onClick={() => void start()}
          >
            Index
          </Button>
        )}
      </div>

      {running && progress ? (
        <Progress value={runPercent(progress)} className="mt-2 h-1" />
      ) : null}

      {!ready && !running ? (
        <p className="mt-2 text-[11px] text-muted-foreground">
          Save a Voyage API key first — there is nothing to embed against without one.
        </p>
      ) : null}

      {result ? (
        <p className="mt-2 text-[11px] leading-relaxed text-muted-foreground">
          {result.stopped ? "Stopped after " : "Indexed "}
          {result.files} file{result.files === 1 ? "" : "s"} ·{" "}
          {result.pages.toLocaleString()} pages
          {result.errors.length
            ? ` · ${result.errors.length} failed`
            : ""}
        </p>
      ) : null}

      {/* Named, not counted: reasons differ per file. */}
      {result?.errors.length ? (
        <ul className="mt-1 space-y-0.5">
          {result.errors.slice(0, 5).map((message) => (
            <li key={message} className="text-[11px] leading-relaxed text-destructive">
              {message}
            </li>
          ))}
          {result.errors.length > 5 ? (
            <li className="text-[11px] text-muted-foreground">
              …and {result.errors.length - 5} more
            </li>
          ) : null}
        </ul>
      ) : null}

      {error ? (
        <p className="mt-2 text-[11px] leading-relaxed text-destructive">{error}</p>
      ) : null}
    </div>
  );
}
