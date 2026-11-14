import { memo } from "react";
import { X } from "@phosphor-icons/react";
import type { Icon } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

export type BannerTone = "neutral" | "brand" | "warning" | "danger";

export interface BannerAction {
  label: string;
  onClick: () => void;
  /** The one action that resolves the notice; drawn filled. */
  primary?: boolean;
  disabled?: boolean;
}

/** One thread-level notice. `id` keys it and names what a dismissal hides. */
export interface BannerItem {
  id: string;
  tone: BannerTone;
  icon: Icon;
  title: string;
  description?: string;
  actions?: BannerAction[];
  /** Shows a close button; the owner forgets the notice. */
  onDismiss?: () => void;
}

/** The glyph's colour per tone; the card itself stays neutral. */
export const BANNER_TONE: Record<BannerTone, string> = {
  neutral: "text-muted-foreground",
  brand: "text-brand",
  warning: "text-warning",
  danger: "text-destructive",
};

/**
 * Notices about the whole thread (a usage limit, later a held queue or an
 * agent that ignores Stop), stacked in the order given just above the
 * composer: each a card with its glyph, a title, a muted description, pill
 * actions and an optional close. Never a toast; nothing here times out.
 */
export const BannerLane = memo(function BannerLane({ items }: { items: BannerItem[] }) {
  if (!items.length) return null;
  return (
    <div className="flex flex-col gap-1.5">
      {items.map((item) => (
        <Banner key={item.id} item={item} />
      ))}
    </div>
  );
});

function Banner({ item }: { item: BannerItem }) {
  const IconC = item.icon;
  return (
    <div
      data-banner={item.id}
      data-banner-tone={item.tone}
      className="flex items-start gap-2.5 rounded-xl border border-border bg-card px-3 py-2.5"
    >
      <IconC size={14} className={cn("mt-0.5 shrink-0", BANNER_TONE[item.tone])} aria-hidden />
      <div className="min-w-0 flex-1">
        <div className="text-xs text-foreground">{item.title}</div>
        {item.description && (
          <p className="mt-0.5 break-words text-[11.5px] leading-relaxed text-muted-foreground">{item.description}</p>
        )}
      </div>
      {item.actions?.map((a) => (
        <Button
          key={a.label}
          size="xs"
          variant={a.primary ? "default" : "outline"}
          disabled={a.disabled}
          onClick={a.onClick}
          className="shrink-0"
        >
          {a.label}
        </Button>
      ))}
      {item.onDismiss && (
        <Button
          size="icon-xs"
          variant="ghost"
          aria-label={`Dismiss: ${item.title}`}
          onClick={item.onDismiss}
          className="-mr-1 shrink-0 text-muted-foreground hover:text-foreground"
        >
          <X />
        </Button>
      )}
    </div>
  );
}
