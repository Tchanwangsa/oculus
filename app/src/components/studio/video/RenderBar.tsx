import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { PillTabs } from "@/components/ui/table/PillTabs";
import type { VideoPlayback } from "@/hooks/studio/useVideoPlayback";
import { fmtAgo } from "@/lib/format/format";
import type { RenderQuality } from "@/lib/studio/render";
import type { VideoDetail } from "@/lib/studio/videos";
import { useRenderStore } from "@/stores/studio/renderStore";

const QUALITY_LABEL: Record<RenderQuality, string> = { draft: "Draft", final: "Final" };

/**
 * Under the player: which render it shows (when both exist), when that was
 * made, and Render final (Render draft too while there is nothing to play). A render in flight swaps the button for its bar and
 * Cancel; a failure stays until the next one starts. Rendering is local
 * (ffmpeg), so unlike Settings it may start from here.
 */
export function RenderBar({ detail, playback }: { detail: VideoDetail; playback: VideoPlayback }) {
  const { dir, slug } = detail.row;
  const active = useRenderStore((s) => s.active[dir]);
  const error = useRenderStore((s) => s.errors[dir]);
  const start = useRenderStore((s) => s.start);
  const cancel = useRenderStore((s) => s.cancel);
  const { shown } = playback;
  const qualities = detail.renders.map((r) => r.quality);

  if (active) {
    const pct = active.fraction == null ? null : Math.round(active.fraction * 100);
    return (
      <div className="flex items-center gap-3">
        <div className="min-w-0 flex-1">
          <div className="mb-1 flex items-center justify-between text-[11.5px] text-muted-foreground">
            <span>
              {active.phase === "muxing" ? "Adding the narration" : `Rendering ${active.quality}`}
              {active.phase === "planning" && "…"}
            </span>
            {pct != null && <span className="tabular-nums">{pct}%</span>}
          </div>
          <Progress value={pct ?? 0} className="h-1" />
        </div>
        <Button variant="outline" size="sm" onClick={() => cancel(dir)}>
          Cancel
        </Button>
      </div>
    );
  }

  return (
    <div className="space-y-1.5">
      <div className="flex min-h-8 items-center gap-3">
        <div className="flex min-w-0 flex-1 items-center gap-2 text-[11.5px] text-muted-foreground">
          {qualities.length > 1 && shown && (
            <PillTabs
              tabs={qualities.map((q) => ({ value: q, label: QUALITY_LABEL[q] }))}
              value={shown.quality}
              onChange={playback.pick}
            />
          )}
          {shown && (
            <span className="truncate">
              {qualities.length > 1 ? "" : `${QUALITY_LABEL[shown.quality]} · `}
              {fmtAgo(shown.modified * 1000)}
            </span>
          )}
        </div>
        {!shown && (
          <Button variant="outline" size="sm" onClick={() => void start(dir, slug, "draft")}>
            Render draft
          </Button>
        )}
        <Button size="sm" onClick={() => void start(dir, slug, "final")}>
          Render final
        </Button>
      </div>
      {error && <p className="text-[12px] leading-snug text-destructive">{error}</p>}
    </div>
  );
}
