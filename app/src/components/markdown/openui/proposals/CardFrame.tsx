import type { ReactNode } from "react";
import { useIsStreaming } from "@openuidev/react-lang";
import { ArrowBendDownRight, ArrowClockwise, WarningCircle, type Icon } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { CardChip } from "./CardChip";
import type { CardState } from "./cardState";

/** Whether the fence holding this card is still arriving. Outside an OpenUI
 * `Renderer` (tests, a static render) there is no stream. */
export function useCardStreaming(): boolean {
  try { return useIsStreaming(); } catch { return false; }
}

/** A hairline inset to the card's text column, drawn above a block. */
export const RULE = "relative before:pointer-events-none before:absolute before:inset-x-4 before:top-0 before:h-px before:bg-border-subtle";
/** One row of a card's body: rows are divided by inset hairlines, and the
 * first sits under the header's own rule. */
export const CARD_ROW = cn(RULE, "group/row px-4 py-2.5 first:before:hidden");
/** A row's own tools (edit, remove) show on hover or focus, and always while editing. */
export const ROW_TOOLS = "opacity-0 transition-opacity group-hover/row:opacity-100 group-focus-within/row:opacity-100 data-[editing=true]:opacity-100 [@media(hover:none)]:opacity-100";

/** The top edge carries work in flight (a sweep) and failure (solid red). A
 * running job draws its own bar in the body instead. */
function CardEdge({ state }: { state: CardState }) {
  const sweep = state.phase === "loading" || (state.phase === "applying" && state.locked);
  if (!sweep && state.phase !== "failed") return null;
  return <span aria-hidden className={cn("absolute inset-x-0 top-0 h-0.5", sweep ? "progress-sweep" : "bg-destructive/70")} />;
}

/** An error inside the card, beside what it is about; a row that never loaded offers Try again here. */
export function CardCallout({ children, onRetry, className }: { children: ReactNode; onRetry?: () => void; className?: string }) {
  return <div role="alert" className={cn("mx-4 my-2.5 flex min-w-0 items-start gap-2 rounded-lg bg-destructive/8 px-3 py-2 text-xs leading-5 text-destructive", className)}>
    <WarningCircle size={14} className="mt-0.75 shrink-0" />
    <span data-selectable className="min-w-0 flex-1 break-words">{children}</span>
    {onRetry && <Button variant="outline" size="xs" className="-my-0.5 shrink-0" onClick={onRetry}><ArrowClockwise />Try again</Button>}
  </div>;
}

/** What the agent's follow-up did, as a muted line (`followUp.ts`). */
function Note({ children, className }: { children: ReactNode; className?: string }) {
  return <div className={cn("flex min-w-0 items-start gap-1.5 text-xs leading-5 text-muted-foreground", className)}>
    <ArrowBendDownRight size={13} className="mt-0.75 shrink-0" />
    <span className="min-w-0 break-words">{children}</span>
  </div>;
}

/**
 * The frame every proposal and job card shares (docs/ui.md). The header
 * holds the kind's glyph, the title and subtitle, and at its right the
 * phase's chip (`CardChip`) beside `meta` and `tools`. The body locks while
 * the card is not ready or is busy. Errors sit in a callout, and the footer
 * appears only when there is a decision (`actions`) or something to say
 * beside it (`aside`, the follow-up `note`). `compact` puts the actions in
 * the header row. Discarded and superseded keep only the header.
 */
export function CardFrame({ state, icon: Glyph, title, subtitle, meta, tools, actions, aside, status, error, onRetry, note, compact, label = "Proposal", id, children }: {
  state: CardState; icon: Icon; title: ReactNode; subtitle?: ReactNode; meta?: ReactNode; tools?: ReactNode; actions?: ReactNode; aside?: ReactNode; status?: ReactNode;
  error?: string; onRetry?: () => void; note?: ReactNode; compact?: boolean; label?: string; id?: string; children?: ReactNode;
}) {
  const { phase, collapsed, locked, failure } = state;
  const quiet = collapsed || phase === "undone";
  const loadFailed = failure === "load";
  const footer = !collapsed && !compact && (actions || aside || note);
  return <section id={id} aria-label={label} data-phase={phase} aria-busy={locked || undefined} className={cn(
    "@container/proposal relative my-3 w-full min-w-0 overflow-hidden rounded-xl border",
    collapsed ? "border-dashed bg-transparent" : "bg-card",
  )}>
    <CardEdge state={state} />
    {/* Glyph, title and the right-hand cluster share the first line; in a narrow card the cluster drops under the title. */}
    <header className={cn("grid min-w-0 grid-cols-[auto_minmax(0,1fr)] items-start gap-x-2.5 gap-y-2 px-4 @sm/proposal:grid-cols-[auto_minmax(0,1fr)_auto]", collapsed ? "py-2.5" : "pt-3 pb-2.5")}>
      <span className="flex h-5 items-center"><Glyph size={16} className="text-muted-foreground" /></span>
      <div className="min-w-0">
        <div role="heading" aria-level={4} className={cn("font-display text-sm leading-5 font-semibold tracking-[-0.01em] break-words", phase === "streaming" && "text-shimmer", quiet && "font-medium text-muted-foreground")}>{title}</div>
        {subtitle}
        {collapsed && note && <Note className="mt-0.5">{note}</Note>}
      </div>
      {(meta || tools || (compact && actions) || phase !== "pending") && <div className="col-start-2 flex min-h-5 min-w-0 flex-wrap items-center justify-end gap-x-2 gap-y-1 @sm/proposal:col-start-3 @sm/proposal:row-start-1">
        {meta}
        <CardChip state={state} text={status} />
        {tools}
        {compact && actions && <div className="-my-1.5 flex items-center gap-1.5">{actions}</div>}
      </div>}
    </header>
    {!collapsed && children != null && <div inert={locked || undefined} className={cn(RULE, "transition-opacity", locked && "opacity-55")}>{children}</div>}
    {error && <CardCallout onRetry={loadFailed ? onRetry : undefined}>{loadFailed ? `Couldn’t load this card: ${error}` : error}</CardCallout>}
    {compact && note && <Note className="px-4 pb-3">{note}</Note>}
    {footer && <footer className="flex min-h-12 flex-wrap items-center justify-end gap-2 px-4 pt-1.5 pb-3">
      {(aside || note) && <div className="mr-auto flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1">{aside}{note && <Note>{note}</Note>}</div>}
      {actions && <div className="flex items-center gap-1.5">{actions}</div>}
    </footer>}
  </section>;
}

/** The card's shape before its props arrive: its glyph, a shimmering line
 * saying what is coming, and two placeholder rows, so the real card replaces
 * it without the reply jumping as much. */
export function CardSkeleton({ icon: Glyph, title }: { icon: Icon; title: string }) {
  return <section aria-busy="true" aria-label="Proposal" className="my-3 w-full min-w-0 overflow-hidden rounded-xl border bg-card">
    <header className="flex items-start gap-2.5 px-4 pt-3 pb-2.5">
      <span className="flex h-5 shrink-0 items-center"><Glyph size={16} className="text-muted-foreground" /></span>
      <span className="text-shimmer font-display text-sm leading-5 font-semibold">{title}</span>
    </header>
    <div className={cn(RULE, "space-y-2.5 px-4 pt-3 pb-4")} aria-hidden>
      <div className="block-shimmer h-3 w-3/5 rounded-full" />
      <div className="block-shimmer h-3 w-2/5 rounded-full" />
    </div>
  </section>;
}
