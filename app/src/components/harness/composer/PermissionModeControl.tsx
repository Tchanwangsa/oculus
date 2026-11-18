import { useState } from "react";
import { Check, ShieldCheck, ShieldSlash, ShieldStar, WarningCircle } from "@phosphor-icons/react";
import type { Icon } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { MODE_LABEL, modeChoices, type Capabilities, type PermissionMode, type Provider } from "@/lib/harness";
import { cn } from "@/lib/utils";
import { usePermissionMode } from "./usePermissionMode";

const MODE_ICON: Record<PermissionMode, Icon> = {
  ask: ShieldCheck,
  auto: ShieldStar,
  quiet: ShieldSlash,
};

/**
 * The chat's permission mode, an icon-only trigger: how it asks for what
 * the agent's sandbox blocks (Ask, Auto or Don't ask), saved per chat with `harnessSetPermissionMode` from its next
 * run on; before a chat exists, the mode it starts with. Each mode has its line; an agent that cannot ask says so in a
 * tooltip on the header's warning icon, and one with no reviewer says so
 * on Auto's line. Rust refuses a change while a request waits;
 * its sentence shows at the top of the open menu.
 */
export function PermissionModeControl({ threadId, provider }: { threadId: number | null; provider: Provider }) {
  const permission = usePermissionMode(threadId, provider);
  const [open, setOpen] = useState(false);
  const { mode } = permission;
  return (
    <Popover
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        if (!next) permission.clear();
      }}
    >
      <PopoverTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          aria-label={`Permission mode: ${MODE_LABEL[mode]}`}
          title={`Permission mode: ${MODE_LABEL[mode]}`}
          className="size-6 shrink-0 p-0 text-muted-foreground hover:text-foreground"
        >
          <ModeIcon mode={mode} />
        </Button>
      </PopoverTrigger>
      <PopoverContent
        align="start"
        sideOffset={6}
        collisionPadding={8}
        className="w-72 max-w-[calc(100vw-16px)] p-0"
        onOpenAutoFocus={focusPickedMode}
      >
        <PermissionModeMenu
          caps={permission.caps}
          mode={mode}
          busy={permission.busy}
          refused={permission.refused}
          onPick={(next) => permission.pick(next, () => setOpen(false))}
        />
      </PopoverContent>
    </Popover>
  );
}

/** A mode menu's `onOpenAutoFocus`: the chosen mode takes focus, since Radix
 *  would focus the header's warning icon first and open its tooltip. */
export function focusPickedMode(e: Event) {
  e.preventDefault();
  (e.currentTarget as HTMLElement | null)?.querySelector<HTMLElement>('[aria-pressed="true"]')?.focus();
}

export function ModeIcon({ mode }: { mode: PermissionMode }) {
  const IconC = MODE_ICON[mode];
  return <IconC className="size-3.5 shrink-0" aria-hidden />;
}

/** The open menu's content, given its state (the WebKit checks draw it alone). */
export function PermissionModeMenu(props: PermissionModeChoicesProps) {
  const { note } = modeChoices(props.caps);
  return (
    <div className="flex flex-col">
      <div className="flex items-center justify-between gap-2 border-b border-border-subtle px-3 pb-1.5 pt-2.5 text-[11px] font-medium text-muted-foreground">
        <span>Permission mode</span>
        <ModeNote note={note} />
      </div>
      <PermissionModeChoices {...props} />
    </div>
  );
}

/** The warning that the agent cannot ask at all, its sentence in a tooltip;
 *  nothing when it can. */
export function ModeNote({ note }: { note: string | null }) {
  if (!note) return null;
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span tabIndex={0} aria-label={note} className="flex cursor-default text-warning outline-none">
          <WarningCircle className="size-3.5" aria-hidden />
        </span>
      </TooltipTrigger>
      <TooltipContent side="top" className="max-w-64">
        {note}
      </TooltipContent>
    </Tooltip>
  );
}

interface PermissionModeChoicesProps {
  caps: Pick<Capabilities, "provider" | "approvals" | "autoReview">;
  mode: PermissionMode;
  busy: boolean;
  refused: string | null;
  onPick: (mode: PermissionMode) => void;
}

/** Rust's refusal, then a row per mode with its line; the chosen one is
 *  `aria-pressed` (`focusPickedMode` finds it). The compact composer's
 *  settings menu draws these under its own page header. */
export function PermissionModeChoices({ caps, mode, busy, refused, onPick }: PermissionModeChoicesProps) {
  const { choices } = modeChoices(caps);
  return (
    <>
      {refused && (
        <div role="alert" className="border-b border-border-subtle px-3 py-2 text-[11px] leading-relaxed text-destructive">
          {refused}
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
    </>
  );
}
