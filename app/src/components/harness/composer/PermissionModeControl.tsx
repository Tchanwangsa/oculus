import { useState } from "react";
import { CaretDown, Check, ShieldCheck, ShieldSlash, ShieldStar } from "@phosphor-icons/react";
import type { Icon } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import {
  harnessSetPermissionMode,
  MODE_LABEL,
  modeChoices,
  threadMode,
  type Capabilities,
  type PermissionMode,
  type Provider,
} from "@/lib/harness";
import { useCapabilities } from "@/hooks/agents/useCapabilities";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { cn } from "@/lib/utils";
import { useRunCommand } from "./useRunCommand";

const MODE_ICON: Record<PermissionMode, Icon> = {
  ask: ShieldCheck,
  auto: ShieldStar,
  quiet: ShieldSlash,
};

/**
 * How this chat asks for what the agent's sandbox blocks: Ask, Auto or
 * Don't ask, saved per chat with `harnessSetPermissionMode` from its next
 * run on. Each mode has its line; an agent that cannot ask, or has no
 * reviewer for Auto, says so. Rust refuses a change while a request waits;
 * its sentence shows at the top of the open menu.
 */
export function PermissionModeControl({ threadId, provider }: { threadId: number | null; provider: Provider }) {
  const caps = useCapabilities(provider);
  const stored = useHarnessStore((s) => (threadId == null ? undefined : s.threads.find((t) => t.id === threadId)?.permission_mode));
  // A chat outside the thread list (Studio's) hears no `permission_mode`
  // event, so a saved pick is kept here too.
  const [saved, setSaved] = useState<{ thread: number; mode: PermissionMode } | null>(null);
  const save = useRunCommand();
  const [open, setOpen] = useState(false);
  if (threadId == null) return null;
  const mode = threadMode(stored ?? (saved?.thread === threadId ? saved.mode : undefined));
  const pick = (next: PermissionMode) => {
    if (next === mode) return setOpen(false);
    void save
      .run(() => harnessSetPermissionMode(threadId, next))
      .then((ok) => {
        if (!ok) return;
        setSaved({ thread: threadId, mode: next });
        setOpen(false);
      });
  };
  return (
    <Popover
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        if (!next) save.clear();
      }}
    >
      <PopoverTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          aria-label="How this chat asks"
          title={`How this chat asks: ${MODE_LABEL[mode]}`}
          className="h-6 shrink-0 gap-1 px-1.5 text-[11px] font-normal text-muted-foreground hover:text-foreground"
        >
          <ModeIcon mode={mode} />
          <span>{MODE_LABEL[mode]}</span>
          <CaretDown className="size-3 shrink-0" />
        </Button>
      </PopoverTrigger>
      <PopoverContent align="start" sideOffset={6} className="w-72 p-0">
        <PermissionModeMenu caps={caps} mode={mode} busy={save.pending} refused={save.error} onPick={pick} />
      </PopoverContent>
    </Popover>
  );
}

function ModeIcon({ mode }: { mode: PermissionMode }) {
  const IconC = MODE_ICON[mode];
  return <IconC className="size-3.5 shrink-0" aria-hidden />;
}

/** The open menu's content, given its state (the WebKit checks draw it alone). */
export function PermissionModeMenu({
  caps,
  mode,
  busy,
  refused,
  onPick,
}: {
  caps: Pick<Capabilities, "provider" | "approvals" | "autoReview">;
  mode: PermissionMode;
  busy: boolean;
  refused: string | null;
  onPick: (mode: PermissionMode) => void;
}) {
  const { note, choices } = modeChoices(caps);
  return (
    <div className="flex flex-col">
      <div className="border-b border-border-subtle px-3 pb-1.5 pt-2.5 text-[11px] font-medium text-muted-foreground">
        When the agent's sandbox blocks something
      </div>
      {refused && (
        <div role="alert" className="border-b border-border-subtle px-3 py-2 text-[11px] leading-relaxed text-destructive">
          {refused}
        </div>
      )}
      {note && (
        <div className="border-b border-border-subtle px-3 py-2 text-[11px] leading-relaxed text-muted-foreground">
          {note}
        </div>
      )}
      <div className="flex flex-col p-1">
        {choices.map((c) => (
          <button
            key={c.mode}
            type="button"
            disabled={busy}
            aria-pressed={c.mode === mode}
            onClick={() => onPick(c.mode)}
            className="flex w-full cursor-default items-start gap-2 rounded-md px-2 py-1.5 text-left outline-none transition-colors hover:bg-accent focus-visible:bg-accent disabled:opacity-60"
          >
            <span className="mt-0.5 text-muted-foreground">
              <ModeIcon mode={c.mode} />
            </span>
            <span className="flex min-w-0 flex-1 flex-col gap-0.5">
              <span className="text-xs text-foreground">{c.label}</span>
              <span className="text-[11px] leading-relaxed text-muted-foreground">{c.line}</span>
            </span>
            <Check className={cn("mt-0.5 size-3.5 shrink-0 text-muted-foreground", c.mode === mode ? "opacity-100" : "opacity-0")} />
          </button>
        ))}
      </div>
    </div>
  );
}
