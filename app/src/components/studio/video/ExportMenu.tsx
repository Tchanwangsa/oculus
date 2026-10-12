import { useCallback, useEffect, useState } from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { CaretDown, CheckCircle, CircleNotch, FolderOpen } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Switch } from "@/components/ui/switch";
import { isWindows } from "@/lib/platform";
import { exportState, PRESET_LABEL, presetHint } from "@/lib/studio/exports";
import { exportStatus, type ExportStatus, type RenderPreset } from "@/lib/studio/render";
import { setCaptions, type VideoDetail } from "@/lib/studio/videos";
import { useRenderStore } from "@/stores/studio/renderStore";

const REVEAL = isWindows ? "Show in Explorer" : "Show in Finder";

/**
 * The video page's Export menu: each preset with whether its file is there
 * and current, Render for one that is missing or out of date (the render bar
 * under the player shows its progress), and Show in Finder for a file there.
 * The statuses are read when the menu opens and again when a render ends.
 * Under the presets, the switch for captions drawn into the picture, which
 * makes the files already there out of date; `onChanged` re-reads the video.
 */
export function ExportMenu({ detail, onChanged }: { detail: VideoDetail; onChanged: () => void }) {
  const { dir, slug } = detail.row;
  const [open, setOpen] = useState(false);
  const [rows, setRows] = useState<ExportStatus[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const active = useRenderStore((s) => s.active[dir]);
  const settled = useRenderStore((s) => s.settled[dir] ?? 0);
  const start = useRenderStore((s) => s.start);

  const load = useCallback(() => {
    let stale = false;
    exportStatus(dir)
      .then((r) => {
        if (stale) return;
        setRows(r);
        setError(null);
      })
      .catch((e) => !stale && setError(e instanceof Error ? e.message : String(e)));
    return () => {
      stale = true;
    };
  }, [dir]);

  useEffect(() => (open ? load() : undefined), [open, load, settled]);

  const [saving, setSaving] = useState(false);
  const [captionsError, setCaptionsError] = useState<string | null>(null);
  const toggleCaptions = async (on: boolean) => {
    setSaving(true);
    setCaptionsError(null);
    try {
      await setCaptions(slug, on);
      onChanged();
      load();
    } catch (e) {
      setCaptionsError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  const render = (preset: RenderPreset) => {
    setOpen(false);
    void start(dir, slug, preset);
  };

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button size="sm" className="gap-1.5">
          Export
          <CaretDown size={11} />
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-80 p-1.5">
        {error && <p className="px-2 py-1.5 text-[12px] leading-snug text-destructive">{error}</p>}
        {!rows && !error && <p className="px-2 py-1.5 text-[12px] text-muted-foreground">Checking the exports…</p>}
        {rows?.map((row) => (
          <ExportRow
            key={row.preset}
            row={row}
            hint={presetHint(row.preset, detail.width, detail.height)}
            rendering={active ? active.preset === row.preset : false}
            busy={!!active}
            onRender={() => render(row.preset)}
          />
        ))}
        <div className="mt-1 border-t border-border-subtle px-2 pb-1 pt-2.5">
          <div className="flex items-center gap-3">
            <label htmlFor={`captions-${slug}`} className="min-w-0 flex-1 cursor-pointer">
              <div className="text-[13px] font-medium">Captions in the picture</div>
              <div className="text-[11.5px] leading-snug text-muted-foreground">
                Drawn into the frames, so every player shows them
              </div>
            </label>
            <Switch
              id={`captions-${slug}`}
              checked={detail.captions}
              disabled={saving}
              onCheckedChange={(on) => void toggleCaptions(on)}
              className="shrink-0"
            />
          </div>
          {captionsError && <p className="mt-1 text-[12px] leading-snug text-destructive">{captionsError}</p>}
        </div>
      </PopoverContent>
    </Popover>
  );
}

function ExportRow({
  row,
  hint,
  rendering,
  busy,
  onRender,
}: {
  row: ExportStatus;
  hint: string;
  rendering: boolean;
  busy: boolean;
  onRender: () => void;
}) {
  const state = exportState(row);
  const path = row.path;
  return (
    <div className="flex items-start gap-2 rounded-lg px-2 py-1.5">
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1.5 text-[13px]">
          <span className="font-medium">{PRESET_LABEL[row.preset]}</span>
          {state === "current" && <CheckCircle size={13} weight="fill" className="shrink-0 text-brand" />}
          {state === "stale" && <span className="text-[11px] text-muted-foreground">out of date</span>}
        </div>
        <div className="truncate text-[11.5px] text-muted-foreground">{hint}</div>
        {state === "blocked" && (
          <div className="mt-0.5 text-[11.5px] leading-snug text-muted-foreground">{row.problem}</div>
        )}
      </div>
      <div className="flex shrink-0 items-center gap-1 pt-0.5">
        {path && (
          <Button
            variant="ghost"
            size="icon-xs"
            title={REVEAL}
            aria-label={REVEAL}
            onClick={() => revealItemInDir(path).catch((e) => console.error("reveal failed", e))}
          >
            <FolderOpen />
          </Button>
        )}
        {rendering ? (
          <span className="flex h-6 items-center px-1 text-muted-foreground">
            <CircleNotch size={13} className="animate-spin" />
          </span>
        ) : (
          state !== "current" &&
          state !== "blocked" && (
            <Button variant="outline" size="xs" disabled={busy} onClick={onRender}>
              {state === "stale" ? "Re-render" : "Render"}
            </Button>
          )
        )}
      </div>
    </div>
  );
}
