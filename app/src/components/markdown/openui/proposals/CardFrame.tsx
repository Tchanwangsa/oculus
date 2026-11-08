import type { ReactNode } from "react";
import { useIsStreaming } from "@openuidev/react-lang";
import { ArrowBendDownRight, ArrowCounterClockwise, Check, Clock, SpinnerGap, WarningCircle, type Icon } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import type { CardState } from "./cardState";

/** Whether the fence holding this card is still arriving. Outside an OpenUI
 * `Renderer` (tests, a static render) there is no stream. */
export function useCardStreaming(): boolean {
  try { return useIsStreaming(); } catch { return false; }
}

/** The one-line status at the left of the footer; `text` overrides the phase's default words. */
export function CardStatus({ state, text }: { state: CardState; text?: ReactNode }) {
  const { phase, failure } = state;
  if (phase === "pending" || (phase === "failed" && failure === "apply")) return null;
  const spin = phase === "loading" || phase === "applying";
  const Glyph = spin ? SpinnerGap : phase === "uncommitted" ? Clock : phase === "created" ? Check : phase === "undone" ? ArrowCounterClockwise : phase === "failed" ? WarningCircle : null;
  const words = text ?? (failure === "load" ? "Couldn’t load this card" : undefined) ?? { streaming: "Still writing…", uncommitted: "Available when the reply finishes", loading: "Loading…", applying: "Working…", created: "Done", failed: "Failed", undone: "Undone" }[phase as string];
  return <span role="status" className={cn("mr-auto inline-flex min-w-0 items-center gap-1.5 text-xs", phase === "created" ? "text-success" : phase === "failed" ? "text-destructive" : "text-muted-foreground")}>
    {Glyph && <Glyph size={14} className={cn("shrink-0", spin && "animate-spin")} />}
    <span className="min-w-0 break-words">{words}</span>
  </span>;
}

/**
 * The frame every proposal and job card shares: header (icon, title,
 * subtitle, meta), a body that locks while the card is not ready or busy, an
 * error strip, and a footer with the phase's status and the card's actions.
 * Discarded and superseded keep only the header; `compact` puts the actions
 * in the header row and drops the footer. `note` is a muted line under the
 * body that every phase keeps: what the agent's follow-up did (`followUp.ts`).
 */
export function CardFrame({ state, icon: Glyph, title, subtitle, meta, actions, status, error, note, compact, label = "Proposal", id, children }: {
  state: CardState; icon: Icon; title: ReactNode; subtitle?: ReactNode; meta?: ReactNode; actions?: ReactNode; status?: ReactNode;
  error?: string; note?: ReactNode; compact?: boolean; label?: string; id?: string; children?: ReactNode;
}) {
  const { phase, collapsed, locked } = state;
  const quiet = collapsed || phase === "undone";
  const heading = <div role="heading" aria-level={4} className={cn("font-display text-[13px] leading-relaxed font-semibold break-words", phase === "streaming" && "text-shimmer", quiet && "font-medium text-muted-foreground")}>{title}</div>;
  return <section id={id} aria-label={label} data-phase={phase} aria-busy={locked || undefined} className={cn(
    "@container/proposal my-3 min-w-0 overflow-hidden rounded-xl border",
    phase === "failed" ? "border-destructive/40" : "border-border",
    collapsed ? "bg-transparent" : "bg-card",
  )}>
    <header className={cn("grid min-w-0 grid-cols-[auto_minmax(0,1fr)] items-start gap-x-2.5 gap-y-2 px-4 py-3", (meta || compact) && "@sm/proposal:grid-cols-[auto_minmax(0,1fr)_auto]")}>
      <Glyph size={compact || collapsed ? 16 : 18} className={cn("mt-0.5 shrink-0", quiet ? "text-muted-foreground" : "text-brand")} />
      <div className="min-w-0">{heading}{subtitle}</div>
      {(meta || (compact && actions)) && <div className="col-span-2 flex flex-wrap items-center justify-end gap-2 @sm/proposal:col-span-1">
        {meta}
        {compact && phase !== "pending" && <CardStatus state={state} text={status} />}
        {compact && actions}
      </div>}
    </header>
    {!collapsed && children != null && <div inert={locked || undefined} className={cn("transition-opacity", locked && "opacity-60")}>{children}</div>}
    {error && state.failure !== "load" && <p role="alert" className="flex items-start gap-1.5 border-t border-border-subtle px-4 py-2 text-xs leading-relaxed text-destructive"><WarningCircle size={14} className="mt-px shrink-0" /><span className="min-w-0 break-words">{error}</span></p>}
    {note && <p className="flex items-start gap-1.5 border-t border-border-subtle px-4 py-2 text-xs leading-relaxed text-muted-foreground"><ArrowBendDownRight size={14} className="mt-px shrink-0" /><span className="min-w-0 break-words">{note}</span></p>}
    {!collapsed && !compact && <footer className="flex min-h-13 flex-wrap items-center justify-end gap-2 border-t border-border-subtle bg-surface/40 px-4 py-2.5">
      <CardStatus state={state} text={state.failure === "load" ? error ? `Couldn’t load this card: ${error}` : undefined : status} />
      {actions}
    </footer>}
  </section>;
}
