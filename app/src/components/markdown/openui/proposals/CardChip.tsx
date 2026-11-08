import type { ReactNode } from "react";
import { ArrowCounterClockwise, Check, Clock, SpinnerGap, WarningCircle, type Icon } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import type { CardState } from "./cardState";

type Tone = "neutral" | "active" | "success" | "danger";

const TONES: Record<Tone, string> = {
  neutral: "bg-surface text-muted-foreground",
  active: "bg-brand-muted text-brand",
  success: "bg-success/12 text-success",
  danger: "bg-destructive/10 text-destructive",
};

/** The phase's words, glyph and tone; a pending card has no chip, its
 * question and buttons are the state. */
function chip(state: CardState): { words: string; tone: Tone; glyph?: Icon; spin?: boolean; shimmer?: boolean } | null {
  const { phase, failure } = state;
  if (phase === "pending") return null;
  if (phase === "streaming") return { words: "Writing", tone: "neutral", shimmer: true };
  if (phase === "uncommitted") return { words: "Waiting for the reply", tone: "neutral", glyph: Clock };
  if (phase === "loading") return { words: "Loading", tone: "neutral", glyph: SpinnerGap, spin: true };
  if (phase === "applying") return { words: "Working…", tone: "active", glyph: SpinnerGap, spin: true };
  if (phase === "created") return { words: "Done", tone: "success", glyph: Check };
  if (phase === "failed") return { words: failure === "load" ? "Not loaded" : failure === "apply" ? "Not saved" : "Failed", tone: "danger", glyph: WarningCircle };
  if (phase === "undone") return { words: "Undone", tone: "neutral", glyph: ArrowCounterClockwise };
  return { words: phase === "discarded" ? "Discarded" : "Replaced", tone: "neutral" };
}

/**
 * A card's phase as one small pill at the right of its header, the same for
 * every kind: neutral while it waits, brand while it works, green when done,
 * red when it failed. `text` replaces the default words ("Creating…").
 */
export function CardChip({ state, text }: { state: CardState; text?: ReactNode }) {
  const value = chip(state);
  if (!value) return null;
  const { words, tone, glyph: Glyph, spin, shimmer } = value;
  return <span role="status" className={cn("inline-flex h-5 max-w-full min-w-0 shrink-0 items-center gap-1 rounded-full px-2 text-[11px] leading-none font-medium whitespace-nowrap", TONES[tone])}>
    {Glyph && <Glyph size={12} weight="bold" className={cn("shrink-0", spin && "animate-spin")} />}
    <span className={cn("min-w-0 truncate", shimmer && "text-shimmer")}>{text ?? words}</span>
  </span>;
}
