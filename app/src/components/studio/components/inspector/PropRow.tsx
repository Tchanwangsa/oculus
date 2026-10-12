import type { ReactNode } from "react";
import { ArrowCounterClockwise } from "@phosphor-icons/react";

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { NamedProp } from "@/lib/studio/components";
import { describeProp } from "@/lib/studio/props";
import { cn } from "@/lib/utils";

/** A prop whose control is small enough to sit beside its name; the rest get
 *  the whole line under it. */
export const isInline = (prop: NamedProp) =>
  prop.kind === "num" || prop.kind === "bool" || prop.kind === "oneOf" || prop.kind === "token";

/**
 * One prop in the form: its name, a brand dot while it differs from its
 * default, and an icon to put it back, which shows on hover. What the prop
 * takes, and its default, is the name's tooltip rather than text beside it.
 * Small controls sit on the name's line, text and lists under it.
 */
export function PropRow({
  prop,
  set,
  changed,
  count,
  onUnset,
  action,
  children,
}: {
  prop: NamedProp;
  /** The inspector has a value for it (else it takes its default). */
  set: boolean;
  /** The value differs from the default. */
  changed: boolean;
  /** A list's length, quietly after the name. */
  count?: number;
  /** Absent when the prop cannot be unset (it is required). */
  onUnset: (() => void) | null;
  /** On the name's line, ahead of the reset: a list's Add while it is empty,
   *  which then has no body. */
  action?: ReactNode;
  /** The control; none for an empty list. */
  children?: ReactNode;
}) {
  const inline = isInline(prop);
  const label = (
    <Tooltip>
      <TooltipTrigger asChild>
        <span className="flex min-w-0 items-center gap-1.5">
          <span
            role={changed ? "img" : undefined}
            aria-label={changed ? "Differs from the default" : undefined}
            aria-hidden={!changed}
            className={cn("size-1.5 shrink-0 rounded-full", changed && "bg-brand")}
          />
          <span className={cn("min-w-0 truncate text-[12.5px]", set ? "text-foreground" : "text-muted-foreground")}>
            {prop.name}
          </span>
          {count != null && <span className="shrink-0 text-[11px] tabular-nums text-muted-foreground">{count}</span>}
        </span>
      </TooltipTrigger>
      <TooltipContent side="left" align="start">
        {describeProp(prop)}
      </TooltipContent>
    </Tooltip>
  );
  const reset = onUnset ? (
    <button
      type="button"
      aria-label={`${prop.default !== undefined ? "Back to the default for" : "Unset"} ${prop.name}`}
      title={prop.default !== undefined ? "Back to the default" : "Unset"}
      onClick={onUnset}
      className="flex size-5 shrink-0 cursor-pointer items-center justify-center rounded-full text-muted-foreground opacity-0 outline-none hover:bg-accent hover:text-foreground focus-visible:opacity-100 focus-visible:ring-[3px] focus-visible:ring-ring/25 group-hover/prop:opacity-100"
    >
      <ArrowCounterClockwise size={11} />
    </button>
  ) : (
    <span aria-hidden className="size-5 shrink-0" />
  );

  if (inline) {
    return (
      <div className="group/prop flex min-h-7 items-center gap-1.5 py-0.5">
        <div className="flex w-[38%] min-w-0 shrink-0">{label}</div>
        <div className="min-w-0 flex-1">{children}</div>
        {reset}
      </div>
    );
  }
  return (
    <div className="group/prop space-y-1 py-1.5">
      <div className="flex min-h-6 items-center gap-1">
        <div className="flex min-w-0 flex-1">{label}</div>
        {action}
        {onUnset && reset}
      </div>
      {children}
    </div>
  );
}
