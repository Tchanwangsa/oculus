import { Play } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";

interface ElsewhereOverlayProps {
  title: string;
  /** The other pane is playing, not paused. */
  playing: boolean;
  onPlayHere: () => void;
}

/** Over the frames, not instead of them: their hosts stay mounted for the
 *  moment the elements come back. */
export function ElsewhereOverlay({ title, playing, onPlayHere }: ElsewhereOverlayProps) {
  return (
    <div className="absolute inset-0 z-40 flex flex-col items-center justify-center gap-4 bg-black p-6 text-white/60">
      <p className="max-w-full truncate text-sm font-medium text-white">{title}</p>
      <p className="text-xs">
        {playing ? "Playing in the other pane" : "Paused in the other pane"}
      </p>
      <Button
        size="sm"
        className="gap-2 bg-white/10 hover:bg-white/20 hover:text-white text-white border-white/20"
        variant="outline"
        onClick={() => onPlayHere()}
      >
        <Play size={14} weight="fill" /> Play here
      </Button>
    </div>
  );
}
