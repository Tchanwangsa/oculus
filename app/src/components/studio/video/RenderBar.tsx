import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { PillTabs } from "@/components/ui/table/PillTabs";
import type { VideoPlayback } from "@/hooks/studio/useVideoPlayback";
import { fmtAgo } from "@/lib/format/format";
import { PRESET_LABEL } from "@/lib/studio/exports";
import type { VideoDetail } from "@/lib/studio/videos";
import { useRenderStore } from "@/stores/studio/renderStore";

/**
 * Under the player: which render it shows (pills when there are several) and
 * when that was made. A render in flight, of any preset (the header's Render
 * draft and the Export menu start them), takes its place with its bar and
 * Cancel; a failure stays until the next one starts. Nothing here starts a
 * render.
 */
export function RenderBar({ detail, playback }: { detail: VideoDetail; playback: VideoPlayback }) {
  const { dir } = detail.row;
  const active = useRenderStore((s) => s.active[dir]);
  const error = useRenderStore((s) => s.errors[dir]);
  const cancel = useRenderStore((s) => s.cancel);
  const { shown, playable } = playback;
  const presets = playable.map((r) => r.preset);

  if (active) {
    const pct = active.fraction == null ? null : Math.round(active.fraction * 100);
    return (
      <div className="flex items-center gap-3">
        <div className="min-w-0 flex-1">
          <div className="mb-1 flex items-center justify-between text-[11.5px] text-muted-foreground">
            <span>
              {active.phase === "muxing"
                ? "Adding the narration"
                : `Rendering ${PRESET_LABEL[active.preset].toLowerCase()}`}
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

  if (!shown && !error) return null;

  return (
    <div className="space-y-1.5">
      {shown && (
        <div className="flex min-h-8 items-center gap-2 text-[11.5px] text-muted-foreground">
          {presets.length > 1 && (
            <PillTabs
              tabs={presets.map((p) => ({ value: p, label: PRESET_LABEL[p] }))}
              value={shown.preset}
              onChange={playback.pick}
            />
          )}
          <span className="truncate">
            {presets.length > 1 ? "" : `${PRESET_LABEL[shown.preset]} · `}
            {fmtAgo(shown.modified * 1000)}
          </span>
        </div>
      )}
      {error && <p className="text-[12px] leading-snug text-destructive">{error}</p>}
    </div>
  );
}
