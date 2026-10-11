import { useEffect, useRef, useState } from "react";
import { Check, ListPlus } from "@phosphor-icons/react";
import { RowAction } from "@/components/ui/RowAction";
import { music } from "@/lib/music";
import { cn } from "@/lib/utils";
import { useMusicStore } from "@/stores/music/musicStore";

/** How long the tick shows after a track is queued. */
const CONFIRM_MS = 1500;

/**
 * "Add to queue" inside a track row: shown on the row's hover (or while
 * focused), a tick for a moment once Spotify took it. It adds to the queue
 * of the device playing, Oculus or another; a refusal (nothing plays) shows
 * as the player's error, like any command.
 */
export function QueueAction({ uri, disabled = false }: { uri: string; disabled?: boolean }) {
  const run = useMusicStore((s) => s.run);
  const [done, setDone] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  useEffect(() => () => clearTimeout(timer.current), []);

  const add = () => {
    if (disabled) return;
    void run(() => music.addToQueue(uri)).then((ok) => {
      if (!ok) return;
      setDone(true);
      clearTimeout(timer.current);
      timer.current = setTimeout(() => setDone(false), CONFIRM_MS);
    });
  };

  return (
    <RowAction
      label={done ? "Added to queue" : "Add to queue"}
      onTrigger={add}
      className={cn(
        "flex size-6 shrink-0 items-center justify-center hover:text-foreground",
        done
          ? "text-brand"
          : "invisible group-hover/row:visible group-focus-within/row:visible focus-visible:visible",
      )}
    >
      {done ? <Check size={14} /> : <ListPlus size={14} />}
    </RowAction>
  );
}
