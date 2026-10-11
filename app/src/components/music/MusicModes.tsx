import { Repeat, Shuffle } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { music, musicControllable, type MusicSnapshot } from "@/lib/music";
import { cn } from "@/lib/utils";
import { useMusicStore } from "@/stores/music/musicStore";

function ModeToggle({
  on,
  label,
  disabled,
  onToggle,
  children,
}: {
  on: boolean;
  label: string;
  disabled: boolean;
  onToggle: () => void;
  children: React.ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="ghost"
          size="icon"
          aria-label={label}
          aria-pressed={on}
          disabled={disabled}
          onClick={onToggle}
          className={cn(on ? "text-brand hover:text-brand" : "text-muted-foreground")}
        >
          {children}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{`${label}: ${on ? "on" : "off"}`}</TooltipContent>
    </Tooltip>
  );
}

/** Shuffle and repeat, lit in the brand colour while on. */
export function ShuffleToggle({ snapshot }: { snapshot: MusicSnapshot }) {
  const run = useMusicStore((s) => s.run);
  return (
    <ModeToggle
      on={snapshot.shuffle}
      label="Shuffle"
      disabled={!musicControllable(snapshot)}
      onToggle={() => run(() => music.setShuffle(!snapshot.shuffle))}
    >
      <Shuffle size={17} />
    </ModeToggle>
  );
}

export function RepeatToggle({ snapshot }: { snapshot: MusicSnapshot }) {
  const run = useMusicStore((s) => s.run);
  return (
    <ModeToggle
      on={snapshot.repeat}
      label="Repeat"
      disabled={!musicControllable(snapshot)}
      onToggle={() => run(() => music.setRepeat(!snapshot.repeat))}
    >
      <Repeat size={17} />
    </ModeToggle>
  );
}
