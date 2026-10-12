import { useState } from "react";

import { Switch } from "@/components/ui/switch";
import { setCaptions, type VideoDetail } from "@/lib/studio/videos";

/**
 * Whether the video's renders burn word-highlight captions in (video.json's
 * `captions`). A change applies from the next render; with captions off the
 * player offers them as CC instead. A failed write says why underneath.
 */
export function CaptionsSwitch({ detail, onChanged }: { detail: VideoDetail; onChanged: () => void }) {
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const id = `captions-${detail.row.slug}`;

  const toggle = async (on: boolean) => {
    setSaving(true);
    setError(null);
    try {
      await setCaptions(detail.row.slug, on);
      onChanged();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="px-3">
      <div className="flex items-center gap-3">
        <label htmlFor={id} className="min-w-0 flex-1 cursor-pointer">
          <div className="text-[13px]">Burn in captions</div>
          <div className="text-[11.5px] text-muted-foreground">
            The words as they are said, the current one highlighted. Applies from the next render.
          </div>
        </label>
        <Switch
          id={id}
          checked={detail.captions}
          disabled={saving}
          onCheckedChange={(on) => void toggle(on)}
          className="shrink-0"
        />
      </div>
      {error && <p className="mt-1 text-[12px] leading-snug text-destructive">{error}</p>}
    </div>
  );
}
