import { memo } from "react";
import type { Icon } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

export type DividerTone = "neutral" | "danger";

export interface DividerAction {
  label: string;
  onClick: () => void;
  /** Said on hover; the label alone when absent. */
  tooltip?: string;
  disabled?: boolean;
}

/** The tone's classes for the hairlines and for the centred text. */
export function dividerTone(tone: DividerTone): { rule: string; label: string; detail: string } {
  return tone === "danger"
    ? { rule: "bg-destructive/30", label: "text-destructive", detail: "text-destructive/80" }
    : { rule: "bg-border", label: "text-foreground/75", detail: "text-muted-foreground" };
}

/**
 * A lifecycle line in the timeline (a stop, a compaction, a continuation):
 * a hairline either side of a centred glyph, its label, an optional
 * "· detail" and an optional pill action. The text wraps rather than
 * truncates, so a long notice still reads in a narrow dock.
 */
export const SystemDivider = memo(function SystemDivider({
  icon: IconC,
  label,
  detail,
  tone = "neutral",
  action,
}: {
  icon?: Icon;
  label: string;
  detail?: string;
  tone?: DividerTone;
  action?: DividerAction;
}) {
  const t = dividerTone(tone);
  return (
    <div data-divider-tone={tone} className="flex w-full min-w-0 items-center gap-3 py-1.5 text-[11px] leading-4">
      <span aria-hidden className={cn("h-px min-w-4 flex-1", t.rule)} />
      <span className="flex min-w-0 max-w-[85%] flex-wrap items-center justify-center gap-x-1.5 gap-y-1 text-center">
        <span className={cn("inline-flex min-w-0 items-center gap-1.5 font-medium", t.label)}>
          {IconC && <IconC size={12} className="shrink-0" aria-hidden />}
          <span className="min-w-0">{label}</span>
        </span>
        {detail && <span className={t.detail}>· {detail}</span>}
        {action && <DividerButton action={action} />}
      </span>
      <span aria-hidden className={cn("h-px min-w-4 flex-1", t.rule)} />
    </div>
  );
});

function DividerButton({ action }: { action: DividerAction }) {
  const button = (
    <Button size="xs" variant="outline" disabled={action.disabled} onClick={action.onClick} className="h-5 px-2 text-[11px]">
      {action.label}
    </Button>
  );
  if (!action.tooltip) return button;
  return (
    <Tooltip>
      <TooltipTrigger asChild>{button}</TooltipTrigger>
      <TooltipContent>{action.tooltip}</TooltipContent>
    </Tooltip>
  );
}
